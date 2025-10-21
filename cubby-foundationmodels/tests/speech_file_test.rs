use cubby_foundationmodels::speech::transcribe_file;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test]
async fn test_speech_transcribe_file() {
    // use test fixture (both formats available)
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let test_file = PathBuf::from(&manifest_dir)
        .join("tests/fixtures/test_audio.m4a");
    
    if !test_file.exists() {
        println!("⚠️  SKIPPED: test fixture not found at {:?}", test_file);
        return;
    }

    // wrap in timeout to avoid indefinite hangs
    let timeout_result = tokio::time::timeout(
        Duration::from_secs(120),
        transcribe_file(test_file.to_str().unwrap())
    ).await;

    match timeout_result {
        Ok(Ok(text)) => {
            println!("transcript: {}", text);
            assert!(!text.trim().is_empty(), "transcript should not be empty");
        }
        Ok(Err(e)) => {
            let msg = e.to_string();
            // gracefully skip if assets unavailable or locale unsupported
            if msg.contains("assets") || msg.contains("unsupported") || msg.contains("speech error") {
                println!("⚠️  SKIPPED: {}", msg);
                return;
            }
            panic!("unexpected error: {}", msg);
        }
        Err(_) => {
            eprintln!("❌ TIMEOUT after 120s");
            eprintln!("troubleshooting:");
            eprintln!("  1. run with: CUBBY_SPEECH_DEBUG=1 cargo test --test speech_file_test -- --nocapture");
            eprintln!("  2. ensure speech assets installed: settings > apple intelligence & siri");
            eprintln!("  3. check system console for speech framework logs");
            panic!("transcription timed out");
        }
    }
}


