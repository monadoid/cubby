#[cfg(target_os = "windows")]
use std::{env, fs};
use std::{
    io::Result,
    process::{Command, Output},
};

#[cfg(target_os = "macos")]
mod apple_intelligence_link {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        process::Command,
    };

    use swift_rs::SwiftLinker;


    // TODO: We should move the apple foundation model sdk calls and the speech sdk calls from this code into their own crate(s). I moved it into cubby-audio
    // while debugging a linker issue, and then I think my only problem was that I needed to have this linking code in *both* crates' build.rs files.
    pub fn link() {
        println!("cargo:rerun-if-changed=swift/Package.swift");
        println!("cargo:rerun-if-changed=swift/Sources/FoundationModelsBridge/FoundationModelsBridge.swift");
        println!("cargo:rerun-if-changed=swift/Sources/FoundationModelsBridge/SpeechBridge.swift");

        configure_swift_module_cache();

        SwiftLinker::new("26.0")
            .with_package("FoundationModelsBridge", "swift")
            .link();

        for path in runtime_rpaths() {
            println!("cargo:rustc-link-search=native={path}");
            println!("cargo:rustc-link-arg=-Wl,-rpath,{path}");
        }

        println!("cargo:rustc-link-lib=framework=FoundationModels");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=Speech");
        println!("cargo:rustc-link-lib=framework=AVFoundation");
    }

    fn configure_swift_module_cache() {
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
        let module_cache = out_dir.join("swift-module-cache");
        if let Err(err) = fs::create_dir_all(&module_cache) {
            panic!(
                "failed to create Swift module cache directory {}: {err}",
                module_cache.display()
            );
        }

        let module_cache_str = module_cache.display().to_string();
        env::set_var("CLANG_MODULE_CACHE_PATH", &module_cache_str);
        env::set_var("SWIFT_MODULE_CACHE_PATH", &module_cache_str);
        env::set_var("SWIFT_PM_MODULECACHE_OVERRIDE", &module_cache_str);
        env::set_var("SWIFTPM_DISABLE_SANDBOX", "1");
    }

    fn runtime_rpaths() -> Vec<String> {
        let mut paths = Vec::new();
        if let Some(toolchain_path) = toolchain_runtime_path() {
            paths.push(toolchain_path);
        }
        paths.push("/usr/lib/swift".into());
        paths
    }

    fn toolchain_runtime_path() -> Option<String> {
        let output = Command::new("xcode-select")
            .arg("--print-path")
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let base = String::from_utf8_lossy(&output.stdout);
        let trimmed = base.trim();
        if trimmed.is_empty() {
            return None;
        }

        let path = format!("{trimmed}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx");
        if Path::new(&path).exists() {
            Some(path)
        } else {
            None
        }
    }
}

fn main() {
    // Windows ONNX runtime support disabled - previously installed to cubby-app-tauri
    // #[cfg(target_os = "windows")]
    // {
    //     install_onnxruntime();
    // }

    #[cfg(target_os = "macos")]
    apple_intelligence_link::link();

    if !is_bun_installed() {
        install_bun();
    }
}

fn is_bun_installed() -> bool {
    let output = Command::new("bun").arg("--version").output();

    match output {
        Err(_) => false,
        Ok(output) => output.status.success(),
    }
}

fn run_bun_install_command(command: Result<Output>) {
    match command {
        Err(error) => {
            println!("failed to install bun: {}", error);
            println!("please install bun manually.");
        }
        Ok(output) => {
            if output.status.success() {
                println!("bun installed successfully.");
            } else {
                println!(
                    "failed to install bun: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                println!("please install bun manually.");
            }
        }
    }
}

fn install_bun() {
    println!("installing bun...");

    #[cfg(target_os = "windows")]
    {
        println!("attempting to install bun using npm...");

        run_bun_install_command(Command::new("npm").args(["install", "-g", "bun"]).output());
    }

    #[cfg(not(target_os = "windows"))]
    {
        run_bun_install_command(
            Command::new("sh")
                .args(["-c", "curl -fsSL https://bun.sh/install | bash"])
                .output(),
        );
    }
}

#[cfg(target_os = "windows")]
fn find_unzip() -> Option<std::path::PathBuf> {
    let paths = [
        // check PATH first
        which::which("unzip").ok(),
        // fallback to common GnuWin32 location
        Some(std::path::PathBuf::from(
            r"C:\Program Files (x86)\GnuWin32\bin\unzip.exe",
        )),
    ];

    paths.into_iter().flatten().find(|p| p.exists())
}

// Windows ONNX runtime support disabled - previously installed to cubby-app-tauri
// #[cfg(target_os = "windows")]
// fn install_onnxruntime() {
//     use std::time::Duration;
//     use reqwest::blocking::Client;
//     use std::{process::Command, path::Path};
//     // Set static CRT for Windows MSVC target
//     if env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default() == "msvc" {
//         println!("cargo:rustc-env=KNF_STATIC_CRT=1");
//         println!("cargo:rustc-flag=-C target-feature=+crt-static");
//     }
//
//     let url = "https://github.com/microsoft/onnxruntime/releases/download/v1.19.2/onnxruntime-win-x64-gpu-1.19.2.zip";
//     let client = Client::builder()
//         .timeout(Duration::from_secs(300))
//         .build()
//         .expect("failed to build client");
//     let resp = client.get(url).send().expect("request failed");
//     let body = resp.bytes().expect("body invalid");
//     fs::write("./onnxruntime-win-x64-gpu-1.19.2.zip", &body).expect("failed to write");
//     let unzip_path = find_unzip().expect("could not find unzip executable - please install it via GnuWin32 or add it to PATH");
//
//     let status = Command::new(unzip_path)
//         .args(["-o", "onnxruntime-win-x64-gpu-1.19.2.zip"])
//         .status()
//         .expect("failed to execute unzip");
//
//     if !status.success() {
//         panic!("failed to install onnx binary");
//     }
//     let target_dir = Path::new("../cubby-app-tauri/src-tauri/onnxruntime-win-x64-gpu-1.19.2");
//     if target_dir.exists() {
//         fs::remove_dir_all(target_dir).expect("failed to remove existing directory");
//     }
//     fs::rename(
//         "onnxruntime-win-x64-gpu-1.19.2",
//         target_dir,
//     ).expect("failed to rename");
//     println!("cargo:rustc-link-search=native=../cubby-app-tauri/src-tauri/onnxruntime-win-x64-gpu-1.19.2/lib");
// }
