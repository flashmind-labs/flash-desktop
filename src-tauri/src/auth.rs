use keyring::Entry;

const SERVICE: &str = "flash-desktop";
const USERNAME: &str = "oauth-token";

pub fn store_token(token: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE, USERNAME).map_err(|e| e.to_string())?;
    entry.set_password(token).map_err(|e| e.to_string())
}

pub fn get_token() -> Option<String> {
    let entry = Entry::new(SERVICE, USERNAME).ok()?;
    entry.get_password().ok()
}

pub fn delete_token() -> Result<(), String> {
    let entry = Entry::new(SERVICE, USERNAME).map_err(|e| e.to_string())?;
    entry.delete_credential().map_err(|e| e.to_string())
}

pub fn is_authenticated() -> bool {
    get_token().is_some()
}
