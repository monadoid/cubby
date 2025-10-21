use cubby_foundationmodels::{generate, generate_stream, schema::PersonInfo};
use futures::StreamExt;

#[tokio::test]
async fn test_generic_generate() {
    println!("testing generic generate with PersonInfo...");
    
    let result: Result<PersonInfo, _> = generate("generate info for alice, age 28").await;
    
    match result {
        Ok(person) => {
            println!("success! generated person: {:?}", person);
            assert!(!person.name.is_empty(), "name should not be empty");
            assert!(person.age > 0, "age should be positive");
        }
        Err(e) => {
            let error_msg = e.to_string();
            
            // Only pass if model is unavailable
            if error_msg.contains("Model assets are unavailable") {
                println!("⚠️  SKIPPED: foundationmodels assets not downloaded");
                return;
            }
            
            // Any other error should fail
            panic!("unexpected error: {}", error_msg);
        }
    }
}

#[tokio::test]
async fn test_generic_stream() {
    println!("testing generic streaming with PersonInfo...");
    
    let mut stream = generate_stream::<PersonInfo>("generate info for bob, age 35");
    let mut people = vec![];
    
    while let Some(result) = stream.next().await {
        match result {
            Ok(person) => {
                println!("received partial person: name={}, age={}", person.name, person.age);
                people.push(person);
            }
            Err(e) => {
                let error_msg = e.to_string();
                
                // Only pass if model is unavailable
                if error_msg.contains("Model assets are unavailable") {
                    println!("⚠️  SKIPPED: foundationmodels assets not downloaded");
                    return;
                }
                
                // Any other error should fail
                panic!("unexpected streaming error: {}", error_msg);
            }
        }
    }
    
    // if we got here, the stream completed successfully
    println!("stream completed successfully with {} updates", people.len());
    
    if !people.is_empty() {
        let final_person = people.last().unwrap();
        println!("final person: {:?}", final_person);
        assert!(!final_person.name.is_empty(), "name should not be empty");
        assert!(final_person.age > 0, "age should be positive");
    }
}

