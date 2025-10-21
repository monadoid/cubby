use cubby_foundationmodels::speech::transcribe_file;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .expect("usage: transcribe_file <path>");
    match transcribe_file(&path).await {
        Ok(text) => {
            println!("transcript: {}", text);
        }
        Err(e) => {
            // matching repo style: lower-case logs
            eprintln!("error: {}", e);
            eprintln!("note: ensure assets installed in settings > apple intelligence & siri");
        }
    }
    Ok(())
}
