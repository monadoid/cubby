use cubby_foundationmodels::speech::{install_speech_assets, preheat_speech, supported_locale};

#[tokio::test]
async fn test_speech_supported_locale() {
    match supported_locale().await {
        Ok(locale) => {
            println!("supported locale: {}", locale);
            assert!(!locale.is_empty(), "locale should not be empty");
            // locale format should be like "en_US", "en_CA", etc.
            assert!(
                locale.contains('_') || locale.len() >= 2,
                "locale should be valid identifier"
            );
        }
        Err(e) => {
            let msg = e.to_string();
            // skip if platform unsupported or no locale available
            if msg.contains("macos 26.0+") || msg.contains("unsupported") {
                println!("⚠️  SKIPPED: {}", msg);
                return;
            }
            panic!("unexpected error: {}", msg);
        }
    }
}

#[tokio::test]
async fn test_speech_install_assets() {
    match install_speech_assets().await {
        Ok(()) => {
            println!("assets installed successfully");
        }
        Err(e) => {
            let msg = e.to_string();
            // skip if platform unsupported or locale unsupported
            if msg.contains("macos 26.0+") || msg.contains("unsupported") {
                println!("⚠️  SKIPPED: {}", msg);
                return;
            }
            panic!("unexpected error: {}", msg);
        }
    }
}

#[tokio::test]
async fn test_speech_preheat() {
    match preheat_speech().await {
        Ok(()) => {
            println!("preheat completed successfully");
        }
        Err(e) => {
            let msg = e.to_string();
            // skip if platform unsupported, assets unavailable, or locale unsupported
            if msg.contains("macos 26.0+") || msg.contains("unsupported") || msg.contains("assets")
            {
                println!("⚠️  SKIPPED: {}", msg);
                return;
            }
            panic!("unexpected error: {}", msg);
        }
    }
}

#[tokio::test]
async fn test_speech_workflow_preheat_then_transcribe() {
    // test that preheating doesn't break subsequent transcription
    // 1. preheat
    if let Err(e) = preheat_speech().await {
        let msg = e.to_string();
        if msg.contains("macos 26.0+") || msg.contains("unsupported") {
            println!("⚠️  SKIPPED: {}", msg);
            return;
        }
    }

    // 2. transcribe (using existing fixture)
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let test_file = std::path::PathBuf::from(&manifest_dir).join("tests/fixtures/test_audio.m4a");

    if !test_file.exists() {
        println!("⚠️  SKIPPED: test fixture not found");
        return;
    }

    match cubby_foundationmodels::speech::transcribe_file(test_file.to_str().unwrap()).await {
        Ok(text) => {
            println!("transcript after preheat: {} chars", text.len());
            assert!(!text.trim().is_empty(), "transcript should not be empty");
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("assets") || msg.contains("unsupported") {
                println!("⚠️  SKIPPED: {}", msg);
                return;
            }
            panic!("unexpected error: {}", msg);
        }
    }
}
