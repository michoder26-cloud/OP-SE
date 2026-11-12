use std::{env, error::Error, fs, net::SocketAddr, path::PathBuf};

use op_se_control_plane::{app, AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let token = env::var("OP_SE_CONTROL_ADMIN_TOKEN")?;
    if token.len() < 32 {
        return Err("OP_SE_CONTROL_ADMIN_TOKEN must contain at least 32 characters".into());
    }
    let bind: SocketAddr = env::var("OP_SE_CONTROL_BIND")
        .unwrap_or_else(|_| "127.0.0.1:43117".to_owned())
        .parse()?;
    if !bind.ip().is_loopback() {
        return Err(
            "Bind to loopback only; put a TLS reverse proxy in front for remote access".into(),
        );
    }
    let db_path = PathBuf::from(
        env::var("OP_SE_CONTROL_DB").unwrap_or_else(|_| "./data/op-se-control.db".to_owned()),
    );
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let state = AppState::open(&db_path, token)?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    println!(
        "OP-SE control plane listening on {}",
        listener.local_addr()?
    );
    axum::serve(listener, app(state)).await?;
    Ok(())
}
