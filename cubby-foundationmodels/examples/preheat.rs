use cubby_foundationmodels::speech::preheat_speech;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match preheat_speech().await {
        Ok(()) => println!("preheat ok"),
        Err(e) => eprintln!("error: {}", e),
    }
    Ok(())
}
