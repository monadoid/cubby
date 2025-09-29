fn main() {
    // Fetch from localhost:8787/openapi
    let url = "http://localhost:8787/openapi".to_string();
    
    println!("cargo:rerun-if-env-changed=OPENAPI_URL");
    
    let raw = match ureq::get(&url).call() {
        Ok(response) => response.into_string().unwrap(),
        Err(_) => {
            println!("cargo:warning=Could not fetch OpenAPI spec from {}, skipping client generation", url);
            return;
        }
    };
    
    let spec = serde_json::from_str(&raw).unwrap();

    let mut generator = progenitor::Generator::default();
    let tokens = generator.generate_tokens(&spec).unwrap();
    let ast = syn::parse2(tokens).unwrap();
    let content = prettyplease::unparse(&ast);

    let mut out_file = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).to_path_buf();
    out_file.push("codegen.rs");

    std::fs::write(out_file, content).unwrap();
}