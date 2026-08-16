use std::{env, error::Error, sync::Arc, time::Duration};

use tokio::{net::TcpListener, time::sleep};
use tracing::{error, info, warn};
use vessel_telemetry::init_tracing;
use vessel_worker::{ClusterClient, WorkerConfig, WorkerService, shared_router};

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init_tracing("worker")?;

    let node_id = env::var("VESSEL_NODE_ID").unwrap_or_else(|_| "worker-local".to_string());

    let address = env::var("VESSEL_WORKER_ADDR").unwrap_or_else(|_| "127.0.0.1:7001".to_string());

    let worker_url = env::var("VESSEL_WORKER_URL").unwrap_or_else(|_| format!("http://{address}"));

    let control_url =
        env::var("VESSEL_CONTROL_URL").unwrap_or_else(|_| "http://127.0.0.1:7000".to_string());

    let registry_url =
        env::var("VESSEL_REGISTRY_URL").unwrap_or_else(|_| "http://127.0.0.1:7002".to_string());

    let heartbeat_interval_ms = env_u64("VESSEL_HEARTBEAT_INTERVAL_MS", 5_000);

    let cluster_connect_timeout_ms = env_u64("VESSEL_CLUSTER_CONNECT_TIMEOUT_MS", 2_000).max(1);

    let cluster_request_timeout_ms = env_u64("VESSEL_CLUSTER_REQUEST_TIMEOUT_MS", 5_000).max(1);

    let registry_connect_timeout_ms = env_u64("VESSEL_REGISTRY_CONNECT_TIMEOUT_MS", 2_000).max(1);

    let registry_request_timeout_ms = env_u64("VESSEL_REGISTRY_REQUEST_TIMEOUT_MS", 30_000).max(1);

    let worker = Arc::new(WorkerService::with_registry_and_timeouts(
        WorkerConfig::new(node_id).with_endpoint(worker_url),
        registry_url,
        Duration::from_millis(registry_connect_timeout_ms),
        Duration::from_millis(registry_request_timeout_ms),
    )?);

    let cluster_client = ClusterClient::with_timeouts(
        control_url,
        Duration::from_millis(cluster_connect_timeout_ms),
        Duration::from_millis(cluster_request_timeout_ms),
    )?;

    let listener = TcpListener::bind(&address).await?;

    let worker_node_id = worker.node_id()?;
    let local_addr = listener.local_addr()?;

    info!(
        node_id = %worker_node_id,
        address = %local_addr,
        "worker listening"
    );

    info!(
        cluster_connect_timeout_ms,
        cluster_request_timeout_ms, "cluster networking configured"
    );

    info!(
        registry_connect_timeout_ms,
        registry_request_timeout_ms, "registry networking configured"
    );

    let cluster_worker = Arc::clone(&worker);

    tokio::spawn(async move {
        let client = cluster_client;

        let mut registered = false;

        loop {
            if !registered {
                match cluster_worker.registration() {
                    Ok(registration) => match client.register(&registration).await {
                        Ok(()) => {
                            registered = true;

                            info!(
                                node_id = %worker_node_id,
                                "worker registered with control plane"
                            );
                        }

                        Err(error) => {
                            warn!(
                                node_id = %worker_node_id,
                                error = %error,
                                "worker registration failed"
                            );
                        }
                    },

                    Err(error) => {
                        error!(
                            node_id = %worker_node_id,
                            error = %error,
                            "worker registration snapshot failed"
                        );
                    }
                }
            } else {
                match cluster_worker.heartbeat() {
                    Ok(heartbeat) => {
                        if let Err(error) = client.heartbeat(&heartbeat).await {
                            warn!(
                                node_id = %worker_node_id,
                                error = %error,
                                "worker heartbeat failed"
                            );

                            registered = false;
                        }
                    }

                    Err(error) => {
                        error!(
                            node_id = %worker_node_id,
                            error = %error,
                            "worker heartbeat snapshot failed"
                        );
                    }
                }
            }

            sleep(Duration::from_millis(heartbeat_interval_ms)).await;
        }
    });

    axum::serve(listener, shared_router(worker)).await?;

    Ok(())
}
