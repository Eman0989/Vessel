use std::{env, error::Error, sync::Arc, time::Duration};

use tokio::{net::TcpListener, time::sleep};
use vessel_worker::{ClusterClient, WorkerConfig, WorkerService, shared_router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let node_id = env::var("VESSEL_NODE_ID").unwrap_or_else(|_| "worker-local".to_string());

    let address = env::var("VESSEL_WORKER_ADDR").unwrap_or_else(|_| "127.0.0.1:7001".to_string());

    let worker_url = env::var("VESSEL_WORKER_URL").unwrap_or_else(|_| format!("http://{address}"));

    let control_url =
        env::var("VESSEL_CONTROL_URL").unwrap_or_else(|_| "http://127.0.0.1:7000".to_string());

    let registry_url =
        env::var("VESSEL_REGISTRY_URL").unwrap_or_else(|_| "http://127.0.0.1:7002".to_string());

    let heartbeat_interval_ms = env::var("VESSEL_HEARTBEAT_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(5_000);

    let worker = Arc::new(WorkerService::with_registry(
        WorkerConfig::new(node_id).with_endpoint(worker_url),
        registry_url,
    ));

    let listener = TcpListener::bind(&address).await?;

    println!(
        "VESSEL worker {} listening on {}",
        worker.node_id()?,
        listener.local_addr()?,
    );

    let cluster_worker = Arc::clone(&worker);

    tokio::spawn(async move {
        let client = ClusterClient::new(control_url);

        let mut registered = false;

        loop {
            if !registered {
                match cluster_worker.registration() {
                    Ok(registration) => match client.register(&registration).await {
                        Ok(()) => {
                            registered = true;

                            println!("VESSEL worker registered with control plane");
                        }

                        Err(error) => {
                            eprintln!("VESSEL worker registration failed: {error}");
                        }
                    },

                    Err(error) => {
                        eprintln!("VESSEL worker registration snapshot failed: {error}");
                    }
                }
            } else {
                match cluster_worker.heartbeat() {
                    Ok(heartbeat) => {
                        if let Err(error) = client.heartbeat(&heartbeat).await {
                            eprintln!("VESSEL worker heartbeat failed: {error}");

                            registered = false;
                        }
                    }

                    Err(error) => {
                        eprintln!("VESSEL worker heartbeat snapshot failed: {error}");
                    }
                }
            }

            sleep(Duration::from_millis(heartbeat_interval_ms)).await;
        }
    });

    axum::serve(listener, shared_router(worker)).await?;

    Ok(())
}
