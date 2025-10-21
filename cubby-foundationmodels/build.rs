fn main() {
    // This crate only works on macOS 26.0+ (which provides FoundationModels framework)
    // We check at build time to provide a clear error message
    #[cfg(not(target_os = "macos"))]
    {
        compile_error!("cubby-foundationmodels is macOS-only (requires macOS 26.0+ / SDK 26.0+)");
    }

    #[cfg(target_os = "macos")]
    macos::link_swift();
}

#[cfg(target_os = "macos")]
mod macos {
    use swift_rs::SwiftLinker;

    pub fn link_swift() {
        SwiftLinker::new("26.0")
            .with_package("FoundationModelsBridge", "swift")
            .link();

    }
}
