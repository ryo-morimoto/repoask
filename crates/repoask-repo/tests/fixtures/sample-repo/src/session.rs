/// Stores refresh-token state for long-lived sessions.
pub struct SessionStore {
    token: String,
}

impl SessionStore {
    pub fn refresh(&self) -> &str {
        &self.token
    }
}
