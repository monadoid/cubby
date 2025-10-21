//! example: inspect SystemLanguageModel availability status

use cubby_foundationmodels::{language_model_availability, LanguageModelAvailabilityStatus};

fn main() {
    println!("=== system language model availability ===\n");

    match language_model_availability() {
        Ok(info) => {
            println!("status: {:?}", info.status);
            if let Some(reason) = info.reason.as_deref() {
                println!("reason: {}", reason);
            }
            if let Some(code) = info.reason_code.as_deref() {
                println!("reason_code: {}", code);
            }

            match info.status {
                LanguageModelAvailabilityStatus::Available => {
                    println!("\n✓ foundation model ready to use");
                }
                LanguageModelAvailabilityStatus::Unavailable => {
                    println!("\n✗ model unavailable; consider prompting the user to install assets");
                }
                LanguageModelAvailabilityStatus::Unknown => {
                    println!("\n⚠️  availability unknown; check system configuration");
                }
            }
        }
        Err(err) => {
            eprintln!("error querying availability: {err}");
            eprintln!("note: requires macOS 26.0+ with Apple Intelligence enabled.");
        }
    }
}
