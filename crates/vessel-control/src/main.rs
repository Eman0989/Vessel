use std::{env, error::Error};

use tokio::net::TcpListener;
use vessel_control::{ControlState, router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let address = env::var("VESSEL_CONTROL_ADDR").unwrap_or_else(|_| "127.0.0.1:7000".to_string());

    let listener = TcpListener::bind(&address).await?;

    println!(
        "VESSEL control plane listening on {}",
        listener.local_addr()?,
    );

    axum::serve(listener, router(ControlState::new())).await?;

    Ok(())
}
