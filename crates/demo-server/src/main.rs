use anyhow::Result;
use clap::Parser;
use demo_server::Args;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt().with_target(false).try_init();
    demo_server::run(Args::parse()).await?;
    Ok(())
}
