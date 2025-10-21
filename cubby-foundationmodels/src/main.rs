use cubby_foundationmodels::{generate, generate_person, generate_person_stream, schema::PersonInfo};
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("cubby-foundationmodels\n");
    
    // Parse command line args
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("non-streaming");
    let prompt = args.get(2)
        .map(|s| s.as_str())
        .unwrap_or("generate info for a software engineer named alice, age 28");
    
    match mode {
        "stream" | "streaming" => {
            println!("mode: streaming");
            println!("prompt: {}\n", prompt);
            
            println!("starting stream...");
            let mut stream = generate_person_stream(prompt);
            let mut count = 0;
            
            while let Some(result) = stream.next().await {
                let snapshot = result?;
                count += 1;
                
                println!("\n--- snapshot {} ---", count);
                println!("raw text ({} chars): {}", 
                    snapshot.raw_content.len(),
                    snapshot.raw_content
                );
                println!("structured content:\n{}", 
                    serde_json::to_string_pretty(&snapshot.content)?
                );
            }
            
            println!("\nstream completed with {} snapshots", count);
        }
        
        "generic" => {
            println!("mode: generic typed");
            println!("prompt: {}\n", prompt);
            
            println!("generating...");
            let person: PersonInfo = generate(prompt).await?;
            
            println!("\ngenerated person:");
            println!("  name: {}", person.name);
            println!("  age: {}", person.age);
        }
        
        "generic-stream" => {
            println!("mode: generic typed streaming");
            println!("prompt: {}\n", prompt);
            
            println!("starting stream...");
            let mut stream = generate_stream::<PersonInfo>(prompt);
            let mut count = 0;
            
            while let Some(result) = stream.next().await {
                let person = result?;
                count += 1;
                
                println!("\n--- update {} ---", count);
                println!("name: {}", person.name);
                println!("age: {}", person.age);
            }
            
            println!("\nstream completed with {} updates", count);
        }
        
        _ => {
            println!("mode: non-streaming (default)");
            println!("prompt: {}\n", prompt);
            
            println!("generating...");
            let result = generate_person(prompt).await?;
            
            println!("\ngenerated json:\n{}", serde_json::to_string_pretty(&result)?);
        }
    }
    
    Ok(())
}

use cubby_foundationmodels::generate_stream;
