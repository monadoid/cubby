use cubby_foundationmodels::generate_person;

#[tokio::test]
async fn test_generate_person() {
    println!("testing foundationmodels bridge...");
    
    let result = generate_person(
        "generate info for a software engineer named alice, age 28"
    ).await;
    
    match result {
        Ok(json) => {
            println!("success! generated json:\n{}", serde_json::to_string_pretty(&json).unwrap());
            
            // validate structure
            assert!(json.is_object(), "response should be a json object");
            assert!(json.get("name").is_some(), "response should have 'name' field");
            assert!(json.get("age").is_some(), "response should have 'age' field");
            
            // validate types
            assert!(json.get("name").unwrap().is_string(), "name should be a string");
            assert!(json.get("age").unwrap().is_number(), "age should be a number");
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
            
            // Any other error should fail the test
            panic!("test failed with unexpected error: {}", error_msg);
        }
    }
}

