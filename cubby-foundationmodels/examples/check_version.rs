//! example: check macOS version compatibility

use cubby_foundationmodels::version::{is_foundationmodels_supported, MacOSVersion};

fn main() {
    println!("=== macos version check ===\n");

    // get current version
    match MacOSVersion::current() {
        Some(version) => {
            println!("detected macOS version: {}", version);
            println!("  major: {}", version.major);
            println!("  minor: {}", version.minor);
            println!("  patch: {}\n", version.patch);

            // check if foundationmodels is supported
            let supported = version.supports_foundationmodels();
            println!("foundationmodels supported: {}", supported);

            if supported {
                println!("✓ you can use cubby-foundationmodels apis!");
            } else {
                println!("✗ please upgrade to macOS 26.0+ to use foundationmodels");
                println!("  minimum required: {}", MacOSVersion::MINIMUM_REQUIRED);
            }
        }
        None => {
            println!("could not detect macOS version");
            println!("this might happen if:");
            println!("  - sw_vers command is not available");
            println!("  - running on non-macOS system");
        }
    }

    println!("\n=== global check ===");
    println!(
        "is_foundationmodels_supported(): {}",
        is_foundationmodels_supported()
    );
}
