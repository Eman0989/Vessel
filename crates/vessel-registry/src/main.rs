use std::{env, error::Error};

use tokio::net::TcpListener;
use vessel_registry::{ArtifactStore, router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let address = env::var("VESSEL_REGISTRY_ADDR").unwrap_or_else(|_| "127.0.0.1:7002".to_string());

    let listener = TcpListener::bind(&address).await?;

    println!(
        "VESSEL artifact registry listening on {}",
        listener.local_addr()?,
    );

    axum::serve(listener, router(ArtifactStore::new())).await?;

    Ok(())
}
