use cubby_playground::{bootstrap, Language, PlaygroundOptions};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut options = PlaygroundOptions::default();
    options.languages = vec![Language::English, Language::Spanish];

    let ctx = bootstrap(options).await?;
    let codes: Vec<&str> = ctx
        .options
        .languages
        .iter()
        .map(|lang| lang.as_lang_code())
        .collect();

    println!("Configured language codes: {}", codes.join(", "));
    println!("Database stored at {}", ctx.data_dir().display());
    Ok(())
}
