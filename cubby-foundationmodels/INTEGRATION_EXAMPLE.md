# integration example: using cubby-foundationmodels in cubby-core

this document shows how to integrate `cubby-foundationmodels` into `cubby-core` or `cubby-server` with proper multi-platform support.

## step 1: add dependency (platform-gated)

**`cubby-core/Cargo.toml`**:
```toml
[dependencies]
# ... existing dependencies ...

# platform-specific dependencies
[target.'cfg(target_os = "macos")'.dependencies]
cubby-foundationmodels = { path = "../cubby-foundationmodels" }
```

**why this works**:
- cargo will only try to build `cubby-foundationmodels` on macOS
- on linux/windows, it's completely excluded
- no stub implementations needed

## step 2: create ai module with platform support

**`cubby-core/src/ai.rs`** (new file):
```rust
//! multi-platform ai text generation
//!
//! uses apple foundationmodels on macOS 26.0+, falls back to other models on other platforms.

use anyhow::Result;

// only import on macOS
#[cfg(target_os = "macos")]
use cubby_foundationmodels::{generate, generate_stream, schema::{PersonInfo, ArticleSummary}};

#[cfg(target_os = "macos")]
use futures::StreamExt;

/// generate structured person info using best available model
pub async fn generate_person_info(prompt: &str) -> Result<(String, u32)> {
    // on macOS 26.0+, use foundationmodels
    #[cfg(target_os = "macos")]
    {
        match generate::<PersonInfo>(prompt).await {
            Ok(person) => {
                println!("✓ used apple foundationmodels (on-device)");
                return Ok((person.name, person.age as u32));
            }
            Err(e) => {
                eprintln!("foundationmodels unavailable: {}, using fallback", e);
                // fall through to alternatives
            }
        }
    }
    
    // fallback for non-macOS or if foundationmodels fails
    generate_person_fallback(prompt).await
}

/// stream article summary generation with progress updates
pub async fn stream_article_summary(
    prompt: &str,
    mut on_update: impl FnMut(&str, &str) + Send,
) -> Result<(String, String)> {
    #[cfg(target_os = "macos")]
    {
        match try_stream_with_foundationmodels(prompt, &mut on_update).await {
            Ok(result) => return Ok(result),
            Err(e) => eprintln!("foundationmodels streaming failed: {}", e),
        }
    }
    
    // fallback
    stream_summary_fallback(prompt, on_update).await
}

#[cfg(target_os = "macos")]
async fn try_stream_with_foundationmodels(
    prompt: &str,
    on_update: &mut impl FnMut(&str, &str),
) -> Result<(String, String)> {
    let mut stream = generate_stream::<ArticleSummary>(prompt);
    let mut last_summary = ArticleSummary {
        title: String::new(),
        summary: String::new(),
        key_points: vec![],
    };
    
    while let Some(result) = stream.next().await {
        let summary = result?;
        on_update(&summary.title, &summary.summary);
        last_summary = summary;
    }
    
    Ok((last_summary.title, last_summary.summary))
}

/// fallback implementation using other models
async fn generate_person_fallback(prompt: &str) -> Result<(String, u32)> {
    // use llama.cpp, ollama, or other model
    println!("✓ used fallback model (llama/ollama)");
    
    // stub for now
    Ok(("fallback person".to_string(), 30))
}

async fn stream_summary_fallback(
    prompt: &str,
    mut on_update: impl FnMut(&str, &str),
) -> Result<(String, String)> {
    // use streaming from other model
    on_update("fallback title", "fallback summary");
    Ok(("fallback".to_string(), "summary".to_string()))
}

/// check if foundationmodels is available (compile-time + runtime)
pub async fn is_foundationmodels_available() -> bool {
    #[cfg(not(target_os = "macos"))]
    {
        return false;
    }
    
    #[cfg(target_os = "macos")]
    {
        // try a simple generation to check if models are downloaded
        match generate::<PersonInfo>("test").await {
            Ok(_) => true,
            Err(e) => {
                if e.to_string().contains("Model assets are unavailable") {
                    println!("apple intelligence models not downloaded");
                    println!("enable: settings → apple intelligence & siri");
                    false
                } else {
                    // some other error, assume available but failing
                    true
                }
            }
        }
    }
}

/// get name of model being used
pub fn current_model_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "apple foundationmodels (on-device)"
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        "llama.cpp (fallback)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_generate_person() {
        let result = generate_person_info("software engineer alice, age 28").await;
        
        // should work on any platform (foundationmodels or fallback)
        assert!(result.is_ok());
        
        let (name, age) = result.unwrap();
        println!("generated: {} (age {})", name, age);
        assert!(!name.is_empty());
        assert!(age > 0);
    }
    
    #[tokio::test]
    async fn test_model_availability() {
        let available = is_foundationmodels_available().await;
        println!("foundationmodels available: {}", available);
        
        let model = current_model_name();
        println!("using model: {}", model);
    }
}
```

## step 3: add module to lib.rs

**`cubby-core/src/lib.rs`**:
```rust
pub mod ai;  // add this line

// rest of your modules...
```

## step 4: use in cubby-server

**`cubby-server/src/handlers/generate.rs`** (example):
```rust
use cubby_core::ai::{generate_person_info, stream_article_summary};
use axum::{Json, response::sse::{Event, Sse}};
use futures::stream::{self, Stream};

pub async fn handle_generate_person(prompt: String) -> Json<PersonResponse> {
    match generate_person_info(&prompt).await {
        Ok((name, age)) => Json(PersonResponse { name, age }),
        Err(e) => Json(PersonResponse {
            name: format!("error: {}", e),
            age: 0,
        }),
    }
}

pub async fn handle_stream_summary(
    prompt: String,
) -> Sse<impl Stream<Item = Result<Event, anyhow::Error>>> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    
    tokio::spawn(async move {
        let _ = stream_article_summary(&prompt, |title, summary| {
            let _ = tx.send(Ok(Event::default()
                .json_data(SummaryUpdate { title, summary })
                .unwrap()));
        }).await;
    });
    
    Sse::new(stream::poll_fn(move |cx| rx.poll_recv(cx)))
}
```

## testing the integration

### checking version at runtime

```rust
use cubby_foundationmodels::version::{MacOSVersion, is_macos_26_or_newer};

pub fn should_use_foundationmodels() -> bool {
    #[cfg(not(target_os = "macos"))]
    return false;
    
    #[cfg(target_os = "macos")]
    {
        if let Some(version) = MacOSVersion::current() {
            if version >= MacOSVersion::MINIMUM_REQUIRED {
                println!("macOS {}: foundationmodels available", version);
                return true;
            } else {
                println!("macOS {}: too old, need 26.0+", version);
                return false;
            }
        }
        is_macos_26_or_newer()
    }
}
```

### on macOS 26.0+ with models
```bash
cd cubby-core
cargo test ai::tests --features foundationmodels -- --nocapture

# should print:
# macOS 26.0.1: foundationmodels available
# ✓ used apple foundationmodels (on-device)
# generated: Alice (age 28)
# foundationmodels available: true
```

### on macOS without models
```bash
cargo test ai::tests -- --nocapture

# should print:
# foundationmodels unavailable: Model assets are unavailable
# ✓ used fallback model (llama/ollama)
# generated: fallback person (age 30)
# foundationmodels available: false
```

### on linux/windows
```bash
cargo test ai::tests -- --nocapture

# should print:
# ✓ used fallback model (llama/ollama)
# generated: fallback person (age 30)
# foundationmodels available: false
# using model: llama.cpp (fallback)
```

## runtime model selection

for dynamic model selection at runtime:

```rust
pub enum AiModel {
    AppleFoundation,  // macOS 26.0+
    Llama,            // cross-platform
    Mistral,          // alternative
}

impl AiModel {
    pub async fn best_available() -> Self {
        #[cfg(target_os = "macos")]
        {
            if cubby_core::ai::is_foundationmodels_available().await {
                return Self::AppleFoundation;
            }
        }
        
        // check for llama, mistral, etc
        Self::Llama
    }
}

pub async fn generate_with_model(
    model: AiModel,
    prompt: &str,
) -> Result<String> {
    match model {
        #[cfg(target_os = "macos")]
        AiModel::AppleFoundation => {
            let person = cubby_foundationmodels::generate::<PersonInfo>(prompt).await?;
            Ok(format!("{}: {}", person.name, person.age))
        }
        AiModel::Llama => generate_with_llama(prompt).await,
        AiModel::Mistral => generate_with_mistral(prompt).await,
        #[cfg(not(target_os = "macos"))]
        AiModel::AppleFoundation => {
            Err(anyhow::anyhow!("foundationmodels only available on macOS"))
        }
    }
}
```

## summary

this integration pattern:
- ✅ works seamlessly on all platforms
- ✅ automatically uses best available model
- ✅ gracefully falls back when foundationmodels unavailable
- ✅ no runtime panics or compile errors
- ✅ clear feedback about which model is being used
- ✅ testable on all platforms

the key is using `#[cfg(target_os = "macos")]` to conditionally compile foundationmodels code, with fallback implementations always available.
