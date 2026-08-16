use std::{env, error::Error};

use tokio::net::TcpListener;
use tracing::info;
use vessel_registry::{ArtifactStore, router};
use vessel_telemetry::init_tracing;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init_tracing("registry")?;

    let address = env::var("VESSEL_REGISTRY_ADDR").unwrap_or_else(|_| "127.0.0.1:7002".to_string());

    let listener = TcpListener::bind(&address).await?;

    let local_addr = listener.local_addr()?;

    info!(
        address = %local_addr,
        "artifact registry listening"
    );

    axum::serve(listener, router(ArtifactStore::new())).await?;

    Ok(())
}
