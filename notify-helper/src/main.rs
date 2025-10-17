use anyhow::{Context, Result};
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    title: String,
    #[arg(long)]
    body: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    mac_notification_sys::set_application("com.tabsandtabs.cubby")
        .context("set application bundle id")?;

    mac_notification_sys::send_notification(&args.title, None, &args.body, None)
        .context("send mac notification")?;
    Ok(())
}
