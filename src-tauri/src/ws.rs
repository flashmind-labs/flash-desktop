use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::auth;
use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IncomingMessage {
    #[serde(rename = "tool_call")]
    ToolCall {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        tool: String,
        input: serde_json::Value,
    },
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutgoingMessage {
    #[serde(rename = "tool_result")]
    ToolResult {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        result: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    #[serde(rename = "pong")]
    Pong,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Reconnecting,
    Disconnected,
    AuthError,
}

pub type StatusCallback = Arc<dyn Fn(ConnectionStatus) + Send + Sync>;
pub type ToolHandler = Arc<dyn Fn(String, serde_json::Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>> + Send + Sync>;

pub struct WsClient {
    status: Arc<Mutex<ConnectionStatus>>,
    sender: Arc<Mutex<Option<mpsc::Sender<OutgoingMessage>>>>,
}

impl WsClient {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(ConnectionStatus::Disconnected)),
            sender: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn connect(
        &self,
        on_status: StatusCallback,
        tool_handler: ToolHandler,
    ) {
        let status = self.status.clone();
        let sender_holder = self.sender.clone();
        let mut retries = 0u32;

        loop {
            let config = Config::load();
            // Wait for credentials instead of returning — the user may
            // register the device after the app has already started, in which
            // case we need to pick up the newly stored token + server_id.
            let (token, server_id) = match (auth::get_token(), config.server_id.clone()) {
                (Some(t), Some(s)) => (t, s),
                _ => {
                    let prev = status.lock().await.clone();
                    if prev != ConnectionStatus::AuthError {
                        *status.lock().await = ConnectionStatus::AuthError;
                        on_status(ConnectionStatus::AuthError);
                    }
                    // Slow poll — once registered, we'll see credentials on the next tick
                    sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };

            let ws_url = format!(
                "{}/ws/daemon/{}?token={}&serverId={}",
                config.server_url.replace("https://", "wss://").replace("http://", "ws://"),
                server_id,
                token,
                server_id,
            );

            match connect_async(&ws_url).await {
                Ok((ws_stream, _)) => {
                    retries = 0;
                    *status.lock().await = ConnectionStatus::Connected;
                    on_status(ConnectionStatus::Connected);

                    let (mut write, mut read) = ws_stream.split();
                    let (tx, mut rx) = mpsc::channel::<OutgoingMessage>(32);
                    *sender_holder.lock().await = Some(tx);

                    // Outgoing message forwarder
                    let write_task = tokio::spawn(async move {
                        while let Some(msg) = rx.recv().await {
                            let json = serde_json::to_string(&msg).unwrap();
                            if write.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                    });

                    // Incoming message handler
                    let tool_handler = tool_handler.clone();
                    let sender_for_read = sender_holder.clone();

                    while let Some(Ok(msg)) = read.next().await {
                        if let Message::Text(text) = msg {
                            if let Ok(incoming) = serde_json::from_str::<IncomingMessage>(&text) {
                                match incoming {
                                    IncomingMessage::ToolCall { tool_call_id, tool, input } => {
                                        let handler = tool_handler.clone();
                                        let sender = sender_for_read.clone();
                                        tokio::spawn(async move {
                                            let result = handler(tool.clone(), input).await;
                                            let response = match result {
                                                Ok(r) => OutgoingMessage::ToolResult {
                                                    tool_call_id,
                                                    result: r,
                                                    error: None,
                                                },
                                                Err(e) => OutgoingMessage::ToolResult {
                                                    tool_call_id,
                                                    result: String::new(),
                                                    error: Some(e),
                                                },
                                            };
                                            if let Some(tx) = sender.lock().await.as_ref() {
                                                tx.send(response).await.ok();
                                            }
                                        });
                                    }
                                    IncomingMessage::Ping => {
                                        if let Some(tx) = sender_for_read.lock().await.as_ref() {
                                            tx.send(OutgoingMessage::Pong).await.ok();
                                        }
                                    }
                                }
                            }
                        }
                    }

                    write_task.abort();
                    *sender_holder.lock().await = None;
                }
                Err(e) => {
                    eprintln!("WebSocket connection failed: {}", e);
                }
            }

            // Reconnect with exponential backoff
            *status.lock().await = ConnectionStatus::Reconnecting;
            on_status(ConnectionStatus::Reconnecting);

            let delay = std::cmp::min(1 << retries, 60);
            retries += 1;
            sleep(Duration::from_secs(delay)).await;
        }
    }

    pub async fn status(&self) -> ConnectionStatus {
        self.status.lock().await.clone()
    }
}
