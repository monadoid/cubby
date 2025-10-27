//! Apple Intelligence bridge embedded directly in cubby-audio.
//!
//! This module hosts the bridging logic that previously lived in the standalone
//! `cubby-foundationmodels` crate. It currently exposes both the Speech Analyzer
//! realtime entry points (used by the audio manager) and the structured
//! Foundation Models client, so downstream crates can reuse the bindings without
//! depending on a separate crate.

pub mod availability;
pub mod generation;
pub mod microphone;
pub mod schema;
pub mod speech;
pub mod streaming;
pub mod version;

pub use availability::{
    language_model_availability, LanguageModelAvailability, LanguageModelAvailabilityStatus,
};
pub use generation::{
    generate, generate_person, generate_person_blocking, generate_person_stream, generate_stream,
    generate_structured, generate_structured_blocking, StreamSnapshot,
};
pub use microphone::run_live_microphone_demo;
pub use schema::{ArticleSummary, GenerableSchema, PersonInfo};
pub use speech::{
    ensure_speech_assets_installed, speech_asset_status, SpeechAssetEnsureResponse,
    SpeechAssetStatus,
};
pub use streaming::{start_streaming_session, SpeechStreamSnapshot, SpeechStreamingSession};
pub use version::{is_macos_26_or_newer, MacOSVersion};
