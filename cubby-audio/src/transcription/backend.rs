use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use tracing::trace;
use whisper_rs::WhisperContext;

#[derive(Clone)]
pub enum TranscriptionBackend {
    Whisper {
        context: Arc<WhisperContext>,
    },
    SpeechAnalyzer {
        transcript_store: Arc<SpeechAnalyzerTranscriptStore>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechAnalyzerTranscript {
    pub timestamp: DateTime<Utc>,
    pub text: String,
}

#[derive(Default)]
pub struct SpeechAnalyzerTranscriptStore {
    inner: Arc<Mutex<HashMap<String, Vec<SpeechAnalyzerTranscript>>>>,
}

impl SpeechAnalyzerTranscriptStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn push_final(
        &self,
        device: &str,
        text: impl Into<String>,
        timestamp: Option<DateTime<Utc>>,
    ) {
        let text = text.into();
        if text.trim().is_empty() {
            trace!("speech analyzer transcript ignored (empty) for {}", device);
            return;
        }

        let ts = timestamp.unwrap_or_else(Utc::now);
        let mut guard = self.inner.lock().unwrap();
        let entry = guard.entry(device.to_string()).or_default();
        entry.push(SpeechAnalyzerTranscript {
            timestamp: ts,
            text: text.to_string(),
        });
        trace!(
            "speech analyzer transcript stored for {} ({} total)",
            device,
            entry.len()
        );
    }

    pub fn drain(&self, device: &str) -> Vec<SpeechAnalyzerTranscript> {
        let mut guard = self.inner.lock().unwrap();
        let drained = guard.remove(device).unwrap_or_default();
        if drained.is_empty() {
            trace!("speech analyzer transcript drain empty for {}", device);
        } else {
            trace!(
                "speech analyzer transcript drain returned {} entries for {}",
                drained.len(),
                device
            );
        }
        drained
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_accumulates_and_drains_per_device() {
        let store = SpeechAnalyzerTranscriptStore::new();
        let now = Utc::now();
        store.push_final("mic", "Hello", Some(now));
        store.push_final("mic", "   ", None);
        store.push_final("mic", "world", None);
        store.push_final("other", "skip", None);

        let drained = store.drain("mic");
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].text, "Hello");
        assert_eq!(drained[0].timestamp, now);
        assert_eq!(drained[1].text, "world");

        let remaining = store.drain("mic");
        assert!(remaining.is_empty());

        let other = store.drain("other");
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].text, "skip");
    }
}
