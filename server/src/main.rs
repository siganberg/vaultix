//! `vaultix-server`: the server as a plain binary (Docker image, CLI user admin).
use vaultix_server::{cli, open, serve, shutdown_signal, Options};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let opts = Options::from_env();
    if std::env::args().nth(1).as_deref() == Some("health") {
        // Used by the Docker HEALTHCHECK (the image ships no curl).
        use std::io::{Read, Write};
        let port = opts.bind.rsplit(':').next().unwrap_or("8080").to_string();
        let mut s = std::net::TcpStream::connect(format!("127.0.0.1:{port}"))?;
        s.set_read_timeout(Some(std::time::Duration::from_secs(3)))?;
        s.write_all(b"GET /icon.svg HTTP/1.0\r\n\r\n")?;
        let mut buf = [0u8; 12];
        s.read_exact(&mut buf)?;
        anyhow::ensure!(buf.ends_with(b"200"), "unhealthy");
        return Ok(());
    }
    let state = open(&opts)?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        return cli(&state, &args);
    }
    serve(state, opts, shutdown_signal()).await
}
