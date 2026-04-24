use std::{net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use quinn::{Endpoint, ServerConfig};
use rcgen::generate_simple_self_signed;
use tokio::fs;
use tracing::info;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:5000")]
    bind: SocketAddr,

    #[arg(long, default_value = "demo-cert.der")]
    cert_out: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    let args = Args::parse();
    let (server_config, cert_der) = build_server_config()?;
    write_cert(&args.cert_out, &cert_der).await?;

    let endpoint = Endpoint::server(server_config, args.bind)
        .with_context(|| format!("failed to bind server endpoint on {}", args.bind))?;

    info!(bind = %args.bind, cert = %args.cert_out.display(), "server ready");

    let incoming = endpoint
        .accept()
        .await
        .ok_or_else(|| anyhow!("endpoint closed before receiving a connection"))?;
    let connection = incoming
        .await
        .context("failed to establish incoming connection")?;
    let remote = connection.remote_address();

    info!(remote = %remote, "connection established");

    let (mut send, mut recv) = connection
        .accept_bi()
        .await
        .context("failed to accept stream")?;
    let request = recv
        .read_to_end(64 * 1024)
        .await
        .context("failed to read request")?;
    let request = String::from_utf8(request).context("request was not valid UTF-8")?;

    info!(remote = %remote, request = %request, "received request");

    let response = format!("echo:{request}");
    send.write_all(response.as_bytes())
        .await
        .context("failed to write response")?;
    send.finish().context("failed to finish response stream")?;

    info!(remote = %remote, response = %response, "response sent");
    endpoint.wait_idle().await;
    Ok(())
}

fn build_server_config() -> Result<(ServerConfig, Vec<u8>)> {
    let certified_key = generate_simple_self_signed(vec!["localhost".to_string()])
        .context("failed to generate self-signed certificate")?;
    let cert_der = certified_key.cert.der().to_vec();
    let key_der = certified_key.signing_key.serialize_der();

    let server_config = ServerConfig::with_single_cert(
        vec![certified_key.cert.der().clone()],
        quinn::rustls::pki_types::PrivatePkcs8KeyDer::from(key_der).into(),
    )
    .context("failed to build server TLS configuration")?;

    Ok((server_config, cert_der))
}

async fn write_cert(path: &PathBuf, cert_der: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
    }

    fs::write(path, cert_der)
        .await
        .with_context(|| format!("failed to write certificate to {}", path.display()))
}
