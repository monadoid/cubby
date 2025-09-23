use std::{collections::HashMap, sync::Arc, time::{Duration, Instant}};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct LoginStashEntry {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub scope: String,
    pub state: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub nonce: Option<String>,
    pub prompt: Option<String>,
    pub created_at: Instant,
}

impl LoginStashEntry {
    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() >= ttl
    }
}

#[derive(Clone)]
pub struct LoginStash {
    entries: Arc<RwLock<HashMap<String, LoginStashEntry>>>,
    ttl: Duration,
}

impl LoginStash {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            ttl: Duration::from_secs(ttl_seconds),
        }
    }

    pub async fn store_oauth_params(
        &self,
        key: String,
        client_id: String,
        redirect_uri: String,
        response_type: String,
        scope: String,
        state: Option<String>,
        code_challenge: String,
        code_challenge_method: String,
        nonce: Option<String>,
        prompt: Option<String>,
    ) {
        let entry = LoginStashEntry {
            client_id,
            redirect_uri,
            response_type,
            scope,
            state,
            code_challenge,
            code_challenge_method,
            nonce,
            prompt,
            created_at: Instant::now(),
        };

        let mut entries = self.entries.write().await;
        entries.insert(key, entry);
        
        // Clean up expired entries (simple cleanup)
        entries.retain(|_, entry| !entry.is_expired(self.ttl));
    }

    pub async fn retrieve_and_consume_oauth_params(&self, key: &str) -> Option<LoginStashEntry> {
        let mut entries = self.entries.write().await;
        
        if let Some(entry) = entries.remove(key) {
            if entry.is_expired(self.ttl) {
                return None;
            }
            return Some(entry);
        }
        
        None
    }

    pub async fn cleanup_expired(&self) {
        let mut entries = self.entries.write().await;
        entries.retain(|_, entry| !entry.is_expired(self.ttl));
    }
}