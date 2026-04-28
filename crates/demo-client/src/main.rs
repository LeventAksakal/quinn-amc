use anyhow::Result;
use clap::Parser;
use demo_client::Args;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt().with_target(false).try_init();
    demo_client::run(Args::parse()).await?;
    Ok(())
}
