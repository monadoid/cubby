use std::{fs, path::PathBuf, process::Command};

fn main() {
    // This crate only works on macOS 26.0+ (which provides FoundationModels framework)
    // We check at build time to provide a clear error message
    #[cfg(not(target_os = "macos"))]
    {
        compile_error!("cubby-foundationmodels is macOS-only (requires macOS 26.0+ / SDK 26.0+)");
    }

    #[cfg(target_os = "macos")]
    {
        // 1. Use `swift-bridge-build` to generate Swift/C FFI glue.
        // Include additional Rust bridge files that declare Swift externs.
        let bridge_files = vec!["src/lib.rs", "src/speech.rs", "src/streaming.rs"];
        swift_bridge_build::parse_bridges(bridge_files)
            .write_all_concatenated(swift_bridge_out_dir(), "foundationmodels-bridge");

        // 2. Compile Swift library
        compile_swift();

        // 3. Link to Swift library
        println!("cargo:rustc-link-lib=static=FoundationModelsBridge");
        println!(
            "cargo:rustc-link-search={}",
            swift_library_static_lib_dir().to_str().unwrap()
        );

        // Link Swift runtime libraries
        let xcode_path = if let Ok(output) = std::process::Command::new("xcode-select")
            .arg("--print-path")
            .output()
        {
            String::from_utf8(output.stdout.as_slice().into())
                .unwrap()
                .trim()
                .to_string()
        } else {
            "/Applications/Xcode.app/Contents/Developer".to_string()
        };

        let swift_lib_path = format!(
            "{}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx/",
            &xcode_path
        );
        println!("cargo:rustc-link-search={}", swift_lib_path);
        println!("cargo:rustc-link-search={}", "/usr/lib/swift");

        // Set rpath for Swift dynamic libraries
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", swift_lib_path);
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

        // Link frameworks (requires macOS 26.0+)
        println!("cargo:rustc-link-lib=framework=FoundationModels");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=Speech");
        println!("cargo:rustc-link-lib=framework=AVFoundation");
    }
}

#[cfg(target_os = "macos")]
fn compile_swift() {
    let swift_package_dir = manifest_dir().join("swift");
    let module_cache_dir = swift_package_dir.join(".module-cache");
    let _ = fs::create_dir_all(&module_cache_dir);

    let mut cmd = Command::new("swift");

    cmd.env("CLANG_MODULE_CACHE_PATH", &module_cache_dir);
    cmd.env("SWIFT_MODULE_CACHE_PATH", &module_cache_dir);
    cmd.env("SWIFT_PM_MODULECACHE_OVERRIDE", &module_cache_dir);
    cmd.env("SWIFTPM_DISABLE_SANDBOX", "1");

    cmd.current_dir(&swift_package_dir)
        .arg("build")
        .arg("--disable-sandbox")
        .args(&[
            "-Xswiftc",
            "-import-objc-header",
            "-Xswiftc",
            swift_source_dir()
                .join("bridging-header.h")
                .to_str()
                .unwrap(),
            "-Xswiftc",
            "-module-cache-path",
            "-Xswiftc",
            module_cache_dir.to_str().unwrap(),
        ]);

    if is_release_build() {
        cmd.args(&["-c", "release"]);
    }

    let exit_status = cmd.spawn().unwrap().wait_with_output().unwrap();

    if !exit_status.status.success() {
        eprintln!("swift build failed");
        eprintln!("stderr: {}", String::from_utf8(exit_status.stderr).unwrap());
        eprintln!("stdout: {}", String::from_utf8(exit_status.stdout).unwrap());
        panic!("swift build failed");
    }
}

#[cfg(target_os = "macos")]
fn swift_bridge_out_dir() -> PathBuf {
    generated_code_dir()
}

#[cfg(target_os = "macos")]
fn manifest_dir() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    PathBuf::from(manifest_dir)
}

#[cfg(target_os = "macos")]
fn is_release_build() -> bool {
    std::env::var("PROFILE").unwrap() == "release"
}

#[cfg(target_os = "macos")]
fn swift_source_dir() -> PathBuf {
    manifest_dir().join("swift/Sources/FoundationModelsBridge")
}

#[cfg(target_os = "macos")]
fn generated_code_dir() -> PathBuf {
    swift_source_dir().join("generated")
}

#[cfg(target_os = "macos")]
fn swift_library_static_lib_dir() -> PathBuf {
    let debug_or_release = if is_release_build() {
        "release"
    } else {
        "debug"
    };

    manifest_dir().join(format!("swift/.build/{}", debug_or_release))
}
