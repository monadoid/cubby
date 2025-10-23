# Speech Analyzer Integration Plan
In `cubby-server`, when the setup script runs we will detect whether the host is macOS 26.0 or newer so that we can surface the Speech Analyzer option, reusing the capability helpers already exposed by the Apple Speech bridge (now located at `cubby_audio::apple_intelligence`).

## Objectives
- Expose Apple’s Speech Analyzer as a first-class transcription backend in the CLI/setup flow with sane defaults and fallbacks.
- Swap the realtime transcription path from Deepgram to the Speech Analyzer when it is selected, while keeping existing Whisper/Deepgram behaviour intact for other cases.
- Preserve data-model compatibility (stored chunk metadata, events) and ensure we can exercise the new path through automated tests.

## Planned Changes

### 0. Platform Detection Utilities (`cubby-foundationmodels/src/version.rs`, call sites)
- Rename `is_foundationmodels_supported` to a platform-oriented helper such as `is_macos_26_or_newer`.
- Update all call sites within `cubby-foundationmodels` (speech/streaming modules, examples, tests) and downstream crates to use the new helper when checking OS eligibility.

### 1. Setup Flow & Persistent State (`cubby-server/src/bin/cubby-server.rs`, `cubby-server/src/setup_state.rs`)
- Add a `preferred_transcription_backend` (enum or string) to `SetupState`, defaulting to `None` so existing installs remain unchanged.
- During `run_setup_flow`:
- After `ensure_audio_preference`, probe `cubby_audio::apple_intelligence::is_macos_26_or_newer()` (new helper) and `language_model_availability()` (both gated with `#[cfg(target_os = "macos")]`) to determine eligibility.
  - Prompt the user (cliclack confirm/select) only when audio is enabled and Speech Analyzer is supported, storing the choice in `SetupState`. Skip and clear the preference on unsupported platforms.
  - Persist the preference and reuse it on subsequent runs without re-prompting.
- Update `build_service_args` to append the correct CLI flag/value (see Section 2) based on stored preference when the user has not supplied an explicit flag.

### 2. CLI Surface & Flag Propagation (`cubby-server/src/cli.rs`, `cubby-server/src/bin/cubby-server.rs`)
- Introduce a new `CliAudioTranscriptionEngine::SpeechAnalyzer` variant behind `#[cfg(target_os = "macos")]`, mapping to the new core enum variant.
- Extend clap metadata so `--audio-transcription-engine speech-analyzer` is discoverable in `--help`, and document that it automatically enables realtime streaming when supported.
- When `SpeechAnalyzer` is selected:
  - Force `enable_realtime_audio_transcription = true` unless the user explicitly disables it, logging a warning if there is a conflict.
  - Emit the new flag from `build_service_args` so the background service mirrors the preference.
- Ensure non-mac builds either hide the new variant or gracefully error if a user passes it (clap already handles hidden variants when gated).
- Keep Deepgram as the default when the user does not opt in.

### 3. Core Audio Model Updates (`cubby-audio/src/core/engine.rs`, builder/manager)
- Add `AudioTranscriptionEngine::SpeechAnalyzer` (macOS only) with a `Display` string such as `"SpeechAnalyzer"` to preserve DB compatibility.
- Extend `AudioManagerOptions` and `AudioManagerBuilder`:
  - Allow specifying the speech analyzer engine without requiring a Deepgram key in `validate_options`.
  - Track the selected realtime backend separately if needed (e.g., `realtime_backend: Option<RealtimeBackend>`), defaulting to Deepgram when realtime is enabled and no explicit choice is made.
- Update `AudioManager::record_device` to dispatch realtime streaming based on the selected backend:
  - Deepgram path remains unchanged.
  - Speech Analyzer path calls a new helper under `transcription::speech_analyzer` (Section 4).
  - If realtime is enabled but the backend is `None`, emit a warning and skip instead of panicking.
- Ensure cleanup logic cancels the Speech Analyzer session when stopping a device.

### 4. Speech Analyzer Streaming Integration (`cubby-audio/src/transcription`)
- Add a new module (e.g., `transcription/speech_analyzer/mod.rs`) compiled only on macOS that wraps:
  - Session creation via `cubby_audio::apple_intelligence::start_streaming_session()`.
  - Asset installation preflight: before starting a session, call into the new bridge helper (see Section 5) that invokes `AssetInventory.assetInstallationRequest(supporting:)` and blocks until assets are available or fails gracefully.
  - Bridging of `Vec<f32>` chunks (mono) to `push_samples_f32`, handling sample-rate mismatches via existing resample utilities if necessary.
  - Forwarding partial/final results as `RealtimeTranscriptionEvent` through `cubby-events`, mirroring the Deepgram payload fields (device name, timestamps, `is_final`, etc.).
  - Graceful shutdown (`finish`/`cancel`) tied to the audio manager’s `is_running` flag.
- Provide a stub implementation (no-op that returns `anyhow::bail!`) for non-mac targets so the crate still compiles even if the enum leaks via shared code.
- Centralise realtime backend selection in a small enum (`RealtimeBackend::{Deepgram, SpeechAnalyzer}`) to keep `record_device` readable.

### 5. FoundationModels Speech Bridge Enhancements (`cubby-foundationmodels/src/speech.rs`, Swift bridge)
- Introduce Swift-bridged helpers to:
  - Detect OS support (`fm_isMacOS26OrNewer`) aligning with the renamed Rust API.
  - Ensure required assets are installed by creating the relevant modules, calling `AssetInventory.assetInstallationRequest`, and awaiting `downloadAndInstall()`, returning structured status (already installed vs. newly installed vs. failure).
- Expose a Rust async wrapper (`ensure_speech_assets_installed(locale, preset)`) that the CLI/setup and runtime can call to guarantee readiness while surfacing progress updates/logging.

### 6. Database & Event Compatibility (`cubby-audio/src/transcription/transcription_result.rs`, `cubby-events`)
- Confirm that injecting `"SpeechAnalyzer"` as the transcription engine string is accepted by `cubby-db`. Add a migration note if downstream analytics expect a fixed set.
- Reuse existing event schemas for realtime transcripts so consumers do not need changes; document the new backend identifier if relevant.

### 7. Testing & Tooling
- **Rust unit/integration tests**
  - Add clap parsing tests in `cubby-server/tests` covering the new CLI value and ensuring unsupported platforms reject it.
  - Extend `SetupState` serialization tests to cover the new preference field.
  - Under `cubby-audio/tests`, add a macOS-only async test (ignored by default) that:
    - Checks `language_model_availability().is_available()`.
    - Invokes the new asset installer helper to ensure assets exist (skip gracefully when the download cannot proceed, e.g., in CI).
    - Streams a short synthetic sine wave through the new Speech Analyzer helper and asserts we receive at least one non-empty snapshot.
    - Skips gracefully (with informative reason) when the model is unavailable.
- **Manual verification checklist (to be run after implementation)**
  1. Run the setup flow on macOS 26+, confirm the new prompt appears and storing “Speech Analyzer” auto-enables realtime mode.
  2. Launch the service with `--audio-transcription-engine speech-analyzer`; observe CLI messaging while assets install (if needed), then confirm real-time transcripts arriving via `cubby-events`.
  3. Re-run on macOS <26 (or via forced mock) to ensure the prompt is skipped and Deepgram path still works.

### 8. Documentation & Developer Experience
- Update `cubby-server/README.md` (and any relevant onboarding docs) to describe the new option, platform requirements, and how to toggle it via CLI or setup.
- Add troubleshooting notes (e.g., what happens if Apple Intelligence assets are not downloaded) pointing to the availability check we surface.
- Include follow-up todos in `todos.md` if we defer features such as batch transcription via Speech Analyzer.

## Open Questions / Assumptions
- Assume the Speech Analyzer requires macOS 26.0+ and that pushing raw f32 PCM (mono) at the device sample rate is acceptable, matching the existing example. If resampling to 16 kHz is required, we will incorporate the existing `utils::audio::resample`.
- Need confirmation on whether we should entirely replace the offline STT pipeline when Speech Analyzer is selected; current plan keeps Whisper for chunk processing to avoid delaying this integration.
- Determine acceptable behaviour when availability is “Unknown”: plan is to allow opt-in but fall back to Deepgram (or disable realtime) with clear logging if session creation fails.

## Dependencies
- `cubby-foundationmodels` Swift bridge already exposes the necessary APIs; no additional Xcode work is expected.
- Requires access to Apple Intelligence assets during testing; CI may need to skip the macOS-only integration test by default.
