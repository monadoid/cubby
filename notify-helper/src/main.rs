use anyhow::{Context, Result};
use clap::Parser;
use mac_notification_sys::Notification;

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

    Notification::new()
        .title(&args.title)
        .message(&args.body)
        .content_image("./cubby_logo_black.png")
        .send()
        .context("send mac notification")?;
    
    Ok(())
}
