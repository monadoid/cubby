use cubby_foundationmodels::speech::{install_speech_assets, supported_locale};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match supported_locale().await {
        Ok(loc) => println!("supported locale: {}", loc),
        Err(e) => eprintln!("error: {}", e),
    }

    match install_speech_assets().await {
        Ok(()) => println!("assets ok"),
        Err(e) => eprintln!("error: {}", e),
    }
    Ok(())
}
