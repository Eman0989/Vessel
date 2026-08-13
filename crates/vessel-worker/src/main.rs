use std::{env, error::Error};

use tokio::net::TcpListener;
use vessel_worker::{WorkerConfig, WorkerService, router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let node_id = env::var("VESSEL_NODE_ID").unwrap_or_else(|_| "worker-local".to_string());

    let address = env::var("VESSEL_WORKER_ADDR").unwrap_or_else(|_| "127.0.0.1:7001".to_string());

    let worker = WorkerService::new(WorkerConfig::new(node_id));

    let listener = TcpListener::bind(&address).await?;

    println!(
        "VESSEL worker {} listening on {}",
        worker.node_id()?,
        listener.local_addr()?,
    );

    axum::serve(listener, router(worker)).await?;

    Ok(())
}
