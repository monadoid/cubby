use cubby_playground::{bootstrap, PlaygroundOptions};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ctx = bootstrap(PlaygroundOptions::default()).await?;
    println!("Playground data directory: {}", ctx.data_dir().display());
    Ok(())
}
