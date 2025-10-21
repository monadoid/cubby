use cubby_foundationmodels::{
    generate_person, language_model_availability, LanguageModelAvailabilityStatus,
};

#[test]
fn test_model_availability_status() {
    println!("checking foundationmodels language model availability...");

    match language_model_availability() {
        Ok(info) => {
            println!(
                "status: {:?}, reason: {:?}, reason_code: {:?}",
                info.status, info.reason, info.reason_code
            );

            match info.status {
                LanguageModelAvailabilityStatus::Available => {
                    assert!(
                        info.reason.is_none(),
                        "available status should not include a reason"
                    );
                }
                LanguageModelAvailabilityStatus::Unavailable => {
                    assert!(
                        info.reason.is_some(),
                        "unavailable status should include a reason"
                    );
                }
                LanguageModelAvailabilityStatus::Unknown => {
                    // nothing to assert, but ensure parsing works
                }
            }
        }
        Err(e) => {
            let error_msg = e.to_string();

            if error_msg.contains("foundationmodels requires macOS 26.0+") {
                println!("⚠️  SKIPPED: {}", error_msg);
                return;
            }

            panic!(
                "unexpected error when checking model availability: {}",
                error_msg
            );
        }
    }
}

#[tokio::test]
async fn test_generate_person() {
    println!("testing foundationmodels bridge...");

    let result = generate_person("generate info for a software engineer named alice, age 28").await;

    match result {
        Ok(json) => {
            println!(
                "success! generated json:\n{}",
                serde_json::to_string_pretty(&json).unwrap()
            );

            // validate structure
            assert!(json.is_object(), "response should be a json object");
            assert!(
                json.get("name").is_some(),
                "response should have 'name' field"
            );
            assert!(
                json.get("age").is_some(),
                "response should have 'age' field"
            );

            // validate types
            assert!(
                json.get("name").unwrap().is_string(),
                "name should be a string"
            );
            assert!(
                json.get("age").unwrap().is_number(),
                "age should be a number"
            );
        }
        Err(e) => {
            let error_msg = e.to_string();

            // Only pass if model is unavailable - this is expected on systems without models
            if error_msg.contains("Model assets are unavailable") {
                println!("⚠️  SKIPPED: foundationmodels assets not downloaded");
                println!("to download: settings > apple intelligence & siri > enable");
                println!("tests will pass once models are available\n");
                // Mark as skipped but don't fail CI
                return;
            }
            if error_msg.contains("Detected content likely to be unsafe") {
                println!("⚠️  SKIPPED: content moderation blocked the request");
                println!("consider using a different prompt for local verification");
                return;
            }

            // Any other error should fail the test
            panic!("test failed with unexpected error: {}", error_msg);
        }
    }
}
