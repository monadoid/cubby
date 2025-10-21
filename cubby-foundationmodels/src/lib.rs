//! # cubby-foundationmodels
//!
//! **Platform Requirement**: macOS 26.0+ only
//!
//! This crate provides Rust bindings to Apple's FoundationModels framework,
//! which is only available on macOS 26.0 and later (macOS Sequoia with Apple Intelligence).

#![cfg(target_os = "macos")]

pub mod schema;
pub mod version;

use anyhow::{anyhow, Result};
use async_stream::stream;
use futures::Stream;
use schema::GenerableSchema;
use serde_json::Value;
use std::ffi::{c_void, CStr};
use std::pin::Pin;
use tokio::sync::mpsc;

// Callback type for streaming (defined outside bridge module to avoid parser issues)
type StreamCallbackFn = extern "C" fn(*mut c_void, *const i8, *const i8);

#[swift_bridge::bridge]
mod ffi {
    extern "Swift" {
        #[swift_bridge(swift_name = "fm_generatePerson")]
        fn fm_generate_person(prompt: &str) -> String;
    }
}

// Manual FFI declaration for streaming (swift-bridge doesn't handle complex callback types well)
extern "C" {
    fn fm_generatePersonStream_sync(
        prompt: *const i8,
        callback: StreamCallbackFn,
        context: *mut c_void,
    );
}

// MARK: - Non-streaming APIs (original)

/// Generate structured JSON output using Apple's FoundationModels SDK
/// 
/// This is an async function that spawns the blocking FFI call on a dedicated thread pool,
/// ensuring the tokio runtime remains responsive.
/// 
/// # Example
/// ```no_run
/// # use cubby_foundationmodels::generate_person;
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let result = generate_person("generate info for a software engineer named alice, age 28").await?;
/// println!("{}", serde_json::to_string_pretty(&result)?);
/// # Ok(())
/// # }
/// ```
pub async fn generate_person(prompt: &str) -> Result<Value> {
    check_version_support()?;
    
    let prompt = prompt.to_string();
    
    // spawn_blocking ensures the blocking Swift call doesn't block the tokio runtime
    let json_str = tokio::task::spawn_blocking(move || ffi::fm_generate_person(&prompt)).await?;
    
    parse_response(&json_str)
}

/// Synchronous version - generates structured JSON output (blocking)
/// 
/// **WARNING**: This blocks the calling thread until the Swift SDK completes.
/// Prefer using the async `generate_person` function in async contexts.
/// 
/// # Example
/// ```no_run
/// # use cubby_foundationmodels::generate_person_blocking;
/// # fn main() -> anyhow::Result<()> {
/// let result = generate_person_blocking("generate info for a software engineer named bob, age 35")?;
/// # Ok(())
/// # }
/// ```
pub fn generate_person_blocking(prompt: &str) -> Result<Value> {
    check_version_support()?;
    
    let json_str = ffi::fm_generate_person(prompt);
    parse_response(&json_str)
}

// MARK: - Streaming APIs

/// Snapshot from a streaming response
#[derive(Debug, Clone)]
pub struct StreamSnapshot {
    /// Partial structured content as JSON
    pub content: Value,
    /// Raw text generated so far
    pub raw_content: String,
}

/// Generate structured output with streaming updates
///
/// Returns a `Stream` that yields partial results as they arrive from the model.
/// Each snapshot contains both the structured content and raw text.
///
/// # Example
/// ```no_run
/// # use cubby_foundationmodels::generate_person_stream;
/// # use futures::StreamExt;
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let mut stream = generate_person_stream("generate alice, age 28");
/// 
/// while let Some(result) = stream.next().await {
///     let snapshot = result?;
///     println!("partial: {}", snapshot.raw_content);
/// }
/// # Ok(())
/// # }
/// ```
pub fn generate_person_stream(prompt: &str) -> Pin<Box<dyn Stream<Item = Result<StreamSnapshot>> + Send>> {
    // check version once before starting stream
    if let Err(e) = check_version_support() {
        return Box::pin(stream! {
            yield Err(e);
        });
    }
    
    let prompt = prompt.to_string();
    
    Box::pin(stream! {
        let (tx, mut rx): (mpsc::UnboundedSender<Result<(Option<String>, Option<String>)>>, _) = 
            mpsc::unbounded_channel();
        
        // Callback that Swift will invoke for each snapshot
        extern "C" fn stream_callback(
            ctx: *mut c_void,
            content_json: *const i8,
            raw_content: *const i8,
        ) {
            let tx = unsafe { &*(ctx as *const mpsc::UnboundedSender<Result<(Option<String>, Option<String>)>>) };
            
            // nil content_json signals completion or error
            if content_json.is_null() {
                if raw_content.is_null() {
                    // Error case (errorJson, nil)
                    return;
                } else {
                    // Completion case (nil, "")
                    return;
                }
            }
            
            let content_str = unsafe {
                CStr::from_ptr(content_json).to_string_lossy().into_owned()
            };
            
            let raw_str = if raw_content.is_null() {
                None
            } else {
                Some(unsafe {
                    CStr::from_ptr(raw_content).to_string_lossy().into_owned()
                })
            };
            
            let _ = tx.send(Ok((Some(content_str), raw_str)));
        }
        
        // Spawn blocking task for FFI call
        let tx_clone = tx.clone();
        let handle = tokio::task::spawn_blocking(move || {
            let tx_ptr = &tx_clone as *const _ as *mut c_void;
            let prompt_cstr = std::ffi::CString::new(prompt).expect("CString::new failed");
            unsafe {
                fm_generatePersonStream_sync(prompt_cstr.as_ptr(), stream_callback, tx_ptr);
            }
        });
        
        drop(tx); // close sender so stream ends naturally
        
        // Yield snapshots as they arrive
        while let Some(result) = rx.recv().await {
            match result {
                Ok((Some(content_json), raw_opt)) => {
                    // Parse content
                    let content_value: Value = match serde_json::from_str(&content_json) {
                        Ok(v) => v,
                        Err(e) => {
                            yield Err(anyhow!("failed to parse content json: {}", e));
                            break;
                        }
                    };
                    
                    // Check for error
                    if let Some(err_msg) = content_value.get("error").and_then(|v| v.as_str()) {
                        yield Err(anyhow!("foundationmodels error: {}", err_msg));
                        break;
                    }
                    
                    yield Ok(StreamSnapshot {
                        content: content_value,
                        raw_content: raw_opt.unwrap_or_default(),
                    });
                }
                Ok((None, _)) => {
                    // Completion signal
                    break;
                }
                Err(e) => {
                    yield Err(e);
                    break;
                }
            }
        }
        
        // Wait for FFI task to finish
        let _ = handle.await;
    })
}

// MARK: - Generic APIs

/// Generate structured output with a generic schema type
///
/// This function works with any type that implements `GenerableSchema`.
/// Currently only supports PersonInfo schema.
///
/// # Example
/// ```no_run
/// # use cubby_foundationmodels::{generate, schema::PersonInfo};
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let person: PersonInfo = generate("generate alice, age 28").await?;
/// println!("name: {}, age: {}", person.name, person.age);
/// # Ok(())
/// # }
/// ```
pub async fn generate<T: GenerableSchema>(prompt: &str) -> Result<T> {
    let schema_name = T::schema_name();
    
    // Route to appropriate Swift wrapper based on schema name
    let json_value = match schema_name {
        "PersonInfo" => generate_person(prompt).await?,
        "ArticleSummary" => {
            // TODO: implement ArticleSummary Swift wrapper
            return Err(anyhow!("ArticleSummary schema not yet implemented in Swift bridge"));
        }
        _ => return Err(anyhow!("unknown schema: {}", schema_name)),
    };
    
    T::from_json(json_value)
}

/// Generate structured output with a generic schema type (streaming)
///
/// Returns a stream of partial results typed to the schema.
///
/// # Example
/// ```no_run
/// # use cubby_foundationmodels::{generate_stream, schema::PersonInfo};
/// # use futures::StreamExt;
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let mut stream = generate_stream::<PersonInfo>("generate alice");
/// 
/// while let Some(result) = stream.next().await {
///     let person = result?;
///     println!("partial: {}", person.name);
/// }
/// # Ok(())
/// # }
/// ```
pub fn generate_stream<T: GenerableSchema>(
    prompt: &str,
) -> Pin<Box<dyn Stream<Item = Result<T>> + Send>> {
    let schema_name = T::schema_name();
    
    // Only PersonInfo is implemented currently
    if schema_name != "PersonInfo" {
        return Box::pin(stream! {
            yield Err(anyhow!("schema {} not yet implemented for streaming", schema_name));
        });
    }
    
    let raw_stream = generate_person_stream(prompt);
    
    Box::pin(stream! {
        for await snapshot_result in raw_stream {
            match snapshot_result {
                Ok(snapshot) => {
                    match T::from_json(snapshot.content) {
                        Ok(typed) => yield Ok(typed),
                        Err(e) => {
                            yield Err(e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    yield Err(e);
                    break;
                }
            }
        }
    })
}

// MARK: - Helpers

/// Check if the current macOS version supports FoundationModels
fn check_version_support() -> Result<()> {
    use version::{MacOSVersion, is_foundationmodels_supported};
    
    if !is_foundationmodels_supported() {
        let current = MacOSVersion::current()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        return Err(anyhow!(
            "foundationmodels requires macOS 26.0+, but detected version {}. \
             please upgrade to macOS sequoia 26.0 or later (with apple intelligence).",
            current
        ));
    }
    
    Ok(())
}

/// Parse the JSON response and check for errors
fn parse_response(json_str: &str) -> Result<Value> {
    let value: Value = serde_json::from_str(json_str)?;
    
    // check if the response contains an error
    if let Some(error_msg) = value.get("error").and_then(|v| v.as_str()) {
        return Err(anyhow!("foundationmodels error: {}", error_msg));
    }
    
    Ok(value)
}
