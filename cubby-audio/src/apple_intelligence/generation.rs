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

    swift!(pub fn fm_generate_structured(prompt: &SRString, schema: &SRString) -> SRString);
}

extern "C" {
    fn fm_generate_person_stream_sync(
        prompt: *const i8,
        callback: StreamCallbackFn,
        context: *mut c_void,
    );
}

/// Generate structured output for the provided schema type.
///
/// The Foundation Models runtime may serialize concurrent sessions and reorder completions; invoke
/// this sequentially if your caller expects predictable turnaround time.
pub async fn generate<T: GenerableSchema>(prompt: &str) -> Result<T> {
    let json = generate_structured_raw(prompt, T::schema_name()).await?;
    T::from_json(json)
}

/// Blocking variant of [`generate`].
///
/// The same serialization caveat applies—avoid overlapping requests if latency predictability
/// matters.
pub fn generate_blocking<T: GenerableSchema>(prompt: &str) -> Result<T> {
    let json = generate_structured_blocking_raw(prompt, T::schema_name())?;
    T::from_json(json)
}

/// Generate structured output with streaming updates for schema types that support it.
///
/// Apple’s Foundation Models API may queue sessions and finish them out of order; keep stream
/// requests serialized if your caller depends on predictable latency.
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

    let raw_stream = person_stream_raw(prompt);

    Box::pin(stream! {
        for await snapshot_result in raw_stream {
            match snapshot_result {
                Ok(json) => match T::from_json(json) {
                    Ok(typed) => yield Ok(typed),
                    Err(err) => {
                        yield Err(err);
                        break;
                    }
                },
                Err(err) => {
                    yield Err(err);
                    break;
                }
            }
        }
    })
}

async fn generate_structured_raw(prompt: &str, schema: &str) -> Result<Value> {
    ensure_supported()?;

    let prompt = prompt.to_string();
    let schema = schema.to_string();

    let json_str = tokio::task::spawn_blocking(move || {
        let prompt_sr = SRString::from(prompt.as_str());
        let schema_sr = SRString::from(schema.as_str());
        unsafe { ffi::fm_generate_structured(&prompt_sr, &schema_sr) }.to_string()
    })
    .await?;

    parse_response(&json_str)
}

fn generate_structured_blocking_raw(prompt: &str, schema: &str) -> Result<Value> {
    ensure_supported()?;

    let prompt_sr = SRString::from(prompt);
    let schema_sr = SRString::from(schema);
    let json_str = unsafe { ffi::fm_generate_structured(&prompt_sr, &schema_sr) }.to_string();
    parse_response(&json_str)
}

fn person_stream_raw(prompt: &str) -> Pin<Box<dyn Stream<Item = Result<Value>> + Send>> {
    if let Err(err) = ensure_supported() {
        return Box::pin(stream! {
            yield Err(err);
        });
    }

    let prompt = prompt.to_string();

    Box::pin(stream! {
        let (tx, mut rx): (
            mpsc::UnboundedSender<Result<(Option<String>, Option<String>)>>,
            _
        ) = mpsc::unbounded_channel();

        extern "C" fn stream_callback(
            ctx: *mut c_void,
            content_json: *const i8,
            raw_content: *const i8,
        ) {
            let tx = unsafe {
                &*(ctx as *const mpsc::UnboundedSender<Result<(Option<String>, Option<String>)>>)
            };

            if content_json.is_null() {
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
                Ok((Some(content_json), _raw_opt)) => {
                    let content_value: Value = match serde_json::from_str(&content_json) {
                        Ok(v) => v,
                        Err(err) => {
                            yield Err(anyhow!("failed to parse content json: {}", err));
                            break;
                        }
                    };

                    if let Some(err_msg) = content_value.get("error").and_then(|v| v.as_str()) {
                        yield Err(anyhow!("foundationmodels error: {}", err_msg));
                        break;
                    }

                    yield Ok(content_value);
                }
                Ok((None, _)) => break,
                Err(err) => {
                    yield Err(err);
                    break;
                }
            }
        }

        let _ = handle.await;
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
