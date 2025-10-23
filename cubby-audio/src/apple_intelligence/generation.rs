use std::{
    ffi::{c_void, CStr, CString},
    pin::Pin,
};

use anyhow::{anyhow, Result};
use async_stream::stream;
use futures::Stream;
use serde_json::Value;
use tokio::sync::mpsc;

use super::{
    schema::{GenerableSchema, PersonInfo},
    version::{is_macos_26_or_newer, MacOSVersion},
};

use swift_rs::SRString;

type StreamCallbackFn = extern "C" fn(*mut c_void, *const i8, *const i8);

mod ffi {
    use swift_rs::{swift, SRString};

    swift!(pub fn fm_generate_person(prompt: &SRString) -> SRString);
}

extern "C" {
    fn fm_generate_person_stream_sync(
        prompt: *const i8,
        callback: StreamCallbackFn,
        context: *mut c_void,
    );
}

/// Snapshot from a streaming response.
#[derive(Debug, Clone)]
pub struct StreamSnapshot {
    /// Partial structured content as JSON.
    pub content: Value,
    /// Raw text generated so far.
    pub raw_content: String,
}

/// Generate structured JSON output using Apple's FoundationModels SDK (async).
pub async fn generate_person(prompt: &str) -> Result<Value> {
    ensure_supported()?;

    let prompt = prompt.to_string();
    let json_str = tokio::task::spawn_blocking(move || {
        let prompt_sr = SRString::from(prompt.as_str());
        unsafe { ffi::fm_generate_person(&prompt_sr) }.to_string()
    })
    .await?;

    parse_response(&json_str)
}

/// Generate structured JSON output synchronously (blocking).
pub fn generate_person_blocking(prompt: &str) -> Result<Value> {
    ensure_supported()?;

    let prompt_sr = SRString::from(prompt);
    let json_str = unsafe { ffi::fm_generate_person(&prompt_sr) }.to_string();
    parse_response(&json_str)
}

/// Generate structured output with streaming updates.
pub fn generate_person_stream(
    prompt: &str,
) -> Pin<Box<dyn Stream<Item = Result<StreamSnapshot>> + Send>> {
    if let Err(e) = ensure_supported() {
        return Box::pin(stream! {
            yield Err(e);
        });
    }

    let prompt = prompt.to_string();

    Box::pin(stream! {
        let (tx, mut rx): (mpsc::UnboundedSender<Result<(Option<String>, Option<String>)>>, _) =
            mpsc::unbounded_channel();

        extern "C" fn stream_callback(
            ctx: *mut c_void,
            content_json: *const i8,
            raw_content: *const i8,
        ) {
            let tx = unsafe {
                &*(ctx as *const mpsc::UnboundedSender<Result<(Option<String>, Option<String>)>>)
            };

            if content_json.is_null() {
                // Completion (nil, raw) or error (error JSON, nil). We signal by sending None.
                let _ = tx.send(Ok((None, None)));
                return;
            }

            let content_str =
                unsafe { CStr::from_ptr(content_json).to_string_lossy().into_owned() };
            let raw_str = if raw_content.is_null() {
                None
            } else {
                Some(
                    unsafe { CStr::from_ptr(raw_content).to_string_lossy().into_owned() },
                )
            };

            let _ = tx.send(Ok((Some(content_str), raw_str)));
        }

        let tx_clone = tx.clone();
        let prompt_clone = prompt.clone();
        let handle = tokio::task::spawn_blocking(move || {
            let c_prompt = match CString::new(prompt_clone) {
                Ok(c) => c,
                Err(_) => {
                    let _ = tx_clone.send(Err(anyhow!("prompt contained null byte")));
                    return;
                }
            };
            let tx_ptr = &tx_clone as *const _ as *mut c_void;
            unsafe {
                fm_generate_person_stream_sync(c_prompt.as_ptr(), stream_callback, tx_ptr);
            }
        });

        drop(tx);

        while let Some(result) = rx.recv().await {
            match result {
                Ok((Some(content_json), raw_opt)) => {
                    let content_value: Value = match serde_json::from_str(&content_json) {
                        Ok(v) => v,
                        Err(e) => {
                            yield Err(anyhow!("failed to parse content json: {}", e));
                            break;
                        }
                    };

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
                    break;
                }
                Err(e) => {
                    yield Err(e);
                    break;
                }
            }
        }

        let _ = handle.await;
    })
}

/// Generate structured output with a generic schema type.
pub async fn generate<T: GenerableSchema>(prompt: &str) -> Result<T> {
    let schema_name = T::schema_name();

    let json_value = match schema_name {
        "PersonInfo" => generate_person(prompt).await?,
        "ArticleSummary" => {
            return Err(anyhow!(
                "ArticleSummary schema not yet implemented in Swift bridge"
            ));
        }
        _ => return Err(anyhow!("unknown schema: {}", schema_name)),
    };

    T::from_json(json_value)
}

/// Generate structured output with a generic schema type (streaming).
pub fn generate_stream<T: GenerableSchema>(
    prompt: &str,
) -> Pin<Box<dyn Stream<Item = Result<T>> + Send>> {
    if T::schema_name() != PersonInfo::schema_name() {
        return Box::pin(stream! {
            yield Err(anyhow!(
                "schema {} not yet implemented for streaming",
                T::schema_name()
            ));
        });
    }

    let raw_stream = generate_person_stream(prompt);

    Box::pin(stream! {
        for await snapshot_result in raw_stream {
            match snapshot_result {
                Ok(snapshot) => match T::from_json(snapshot.content) {
                    Ok(typed) => yield Ok(typed),
                    Err(e) => {
                        yield Err(e);
                        break;
                    }
                },
                Err(e) => {
                    yield Err(e);
                    break;
                }
            }
        }
    })
}

fn ensure_supported() -> Result<()> {
    if !is_macos_26_or_newer() {
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

fn parse_response(json_str: &str) -> Result<Value> {
    let value: Value = serde_json::from_str(json_str)?;

    if let Some(error_msg) = value.get("error").and_then(|v| v.as_str()) {
        return Err(anyhow!("foundationmodels error: {}", error_msg));
    }

    Ok(value)
}
