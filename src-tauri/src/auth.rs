// Token storage. Currently file-backed via Config (config.json with mode 0600).
// macOS keychain ACLs cause read failures for background-thread access in
// unsigned dev builds, so we use the config file for now.

use crate::config::Config;

pub fn store_token(token: &str) -> Result<(), String> {
    let mut cfg = Config::load();
    cfg.access_token = Some(token.to_string());
    cfg.save()
}

pub fn get_token() -> Option<String> {
    Config::load().access_token
}

pub fn delete_token() -> Result<(), String> {
    let mut cfg = Config::load();
    cfg.access_token = None;
    cfg.save()
}

pub fn is_authenticated() -> bool {
    get_token().is_some()
}
