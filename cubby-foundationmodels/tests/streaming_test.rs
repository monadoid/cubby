use cubby_foundationmodels::generate_person_stream;
use futures::StreamExt;

#[tokio::test]
async fn test_streaming() {
    println!("testing streaming foundationmodels bridge...");

    let mut stream =
        generate_person_stream("generate info for a software engineer named alice, age 28");
    let mut snapshots = vec![];

    while let Some(result) = stream.next().await {
        match result {
            Ok(snapshot) => {
                println!(
                    "received snapshot - raw length: {}, content fields: {:?}",
                    snapshot.raw_content.len(),
                    snapshot
                        .content
                        .as_object()
                        .map(|o| o.keys().collect::<Vec<_>>())
                );
                snapshots.push(snapshot);
            }
            Err(e) => {
                let error_msg = e.to_string();

                // Only pass if model is unavailable
                if error_msg.contains("Model assets are unavailable") {
                    println!("⚠️  SKIPPED: foundationmodels assets not downloaded");
                    println!("to download: settings > apple intelligence & siri > enable");
                    return;
                }

                // Any other error should fail
                panic!("unexpected streaming error: {}", error_msg);
            }
        }
    }

    // if we got here, the stream completed successfully
    println!(
        "stream completed successfully with {} snapshots",
        snapshots.len()
    );

    if !snapshots.is_empty() {
        // validate the final snapshot
        let final_snapshot = snapshots.last().unwrap();
        assert!(
            final_snapshot.content.is_object(),
            "final content should be a json object"
        );
        assert!(
            final_snapshot.content.get("name").is_some(),
            "should have 'name' field"
        );
        assert!(
            final_snapshot.content.get("age").is_some(),
            "should have 'age' field"
        );

        println!(
            "final snapshot: {}",
            serde_json::to_string_pretty(&final_snapshot.content).unwrap()
        );
    }
}
