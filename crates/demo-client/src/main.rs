use std::{net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use quinn::{ClientConfig, Endpoint};
use tokio::fs;
use tracing::info;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "0.0.0.0:0")]
    bind: SocketAddr,

    #[arg(long, default_value = "127.0.0.1:5000")]
    server: SocketAddr,

    #[arg(long, default_value = "localhost")]
    server_name: String,

    #[arg(long, default_value = "demo-cert.der")]
    cert: PathBuf,

    #[arg(long, default_value = "hello from demo-client")]
    message: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    let args = Args::parse();
    let client_config = build_client_config(&args.cert).await?;

    let mut endpoint = Endpoint::client(args.bind)
        .with_context(|| format!("failed to bind client endpoint on {}", args.bind))?;
    endpoint.set_default_client_config(client_config);

    let connection = endpoint
        .connect(args.server, &args.server_name)
        .with_context(|| {
            format!(
                "failed to start connection to {} using server name {}",
                args.server, args.server_name
            )
        })?
        .await
        .context("client connection failed")?;

    info!(server = %args.server, message = %args.message, "connected to server");

    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .context("failed to open stream")?;
    send.write_all(args.message.as_bytes())
        .await
        .context("failed to send request")?;
    send.finish().context("failed to finish request stream")?;

    let response = recv
        .read_to_end(64 * 1024)
        .await
        .context("failed to read response")?;
    let response = String::from_utf8(response).context("response was not valid UTF-8")?;

    info!(response = %response, "received response");
    endpoint.wait_idle().await;
    Ok(())
}

async fn build_client_config(cert_path: &PathBuf) -> Result<ClientConfig> {
    let cert_der = fs::read(cert_path)
        .await
        .with_context(|| format!("failed to read {}", cert_path.display()))?;

    let mut roots = quinn::rustls::RootCertStore::empty();
    roots
        .add(quinn::rustls::pki_types::CertificateDer::from(cert_der))
        .context("failed to add server certificate to root store")?;

    Ok(ClientConfig::with_root_certificates(std::sync::Arc::new(
        roots,
    ))?)
}
