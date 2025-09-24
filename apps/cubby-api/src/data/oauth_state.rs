use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StateEntry {
    pub user_id: Uuid,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub nonce: Option<String>,
    pub created_at: Instant,
}

impl StateEntry {
    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() >= ttl
    }
}

#[derive(Clone)]
pub struct OAuthStateStore {
    states: Arc<RwLock<HashMap<String, StateEntry>>>,
    ttl: Duration,
}

impl OAuthStateStore {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
            ttl: Duration::from_secs(ttl_seconds),
        }
    }

    pub async fn store_state(
        &self,
        state: String,
        user_id: Uuid,
        client_id: String,
        redirect_uri: String,
        scope: String,
        code_challenge: Option<String>,
        code_challenge_method: Option<String>,
        nonce: Option<String>,
    ) {
        let entry = StateEntry {
            user_id,
            client_id,
            redirect_uri,
            scope,
            code_challenge,
            code_challenge_method,
            nonce,
            created_at: Instant::now(),
        };

        let mut states = self.states.write().await;
        states.insert(state, entry);

        // Clean up expired states (simple cleanup)
        states.retain(|_, entry| !entry.is_expired(self.ttl));
    }

    pub async fn verify_and_consume_state(
        &self,
        state: &str,
        user_id: Uuid,
        client_id: &str,
        redirect_uri: &str,
        scope: &str,
    ) -> Option<StateEntry> {
        let mut states = self.states.write().await;

        if let Some(entry) = states.remove(state) {
            if entry.is_expired(self.ttl) {
                return None;
            }

            // Verify the parameters match
            if entry.user_id == user_id
                && entry.client_id == client_id
                && entry.redirect_uri == redirect_uri
                && entry.scope == scope
            {
                return Some(entry);
            }
        }

        None
    }

    pub async fn cleanup_expired(&self) {
        let mut states = self.states.write().await;
        states.retain(|_, entry| !entry.is_expired(self.ttl));
    }
}
