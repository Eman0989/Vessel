use std::{
    collections::BTreeSet,
    env,
    error::Error,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{net::TcpListener, time::sleep};
use vessel_control::{
    ControlNetworkConfig, ControlState, PostgresStore, SharedState,
    shared_router_with_network_config,
};

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn observed_at_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

async fn run_failure_detector(state: SharedState, timeout_ms: u64, check_interval_ms: u64) {
    loop {
        sleep(Duration::from_millis(check_interval_ms)).await;

        let observed_at_ms = observed_at_ms();

        let (recoveries, reconciled_instances) = match state.lock() {
            Ok(mut state) => {
                let changed = state.detect_stale_workers(observed_at_ms, timeout_ms);

                let mut recoveries = Vec::with_capacity(changed.len());
                let mut affected_deployments = BTreeSet::new();

                for node in changed {
                    let lost_instances = match state.mark_instances_lost_on_node(&node.id) {
                        Ok(instances) => instances,

                        Err(error) => {
                            eprintln!(
                                "VESSEL failure detector: failed to recover instances on worker {}: {}",
                                node.id, error,
                            );

                            Vec::new()
                        }
                    };

                    for instance in &lost_instances {
                        affected_deployments.insert(instance.deployment_id.clone());
                    }

                    recoveries.push((node, lost_instances));
                }

                let mut reconciled_instances = Vec::new();

                for deployment_id in affected_deployments {
                    match state.reconcile_deployment(&deployment_id) {
                        Ok(instances) => {
                            reconciled_instances.extend(instances);
                        }

                        Err(error) => {
                            eprintln!(
                                "VESSEL recovery: failed to reconcile deployment {}: {}",
                                deployment_id, error,
                            );
                        }
                    }
                }

                (recoveries, reconciled_instances)
            }

            Err(_) => {
                eprintln!("VESSEL failure detector: control state lock was poisoned");
                continue;
            }
        };

        for (node, lost_instances) in recoveries {
            eprintln!(
                "VESSEL worker {} marked unreachable after heartbeat timeout",
                node.id,
            );

            for instance in lost_instances {
                eprintln!(
                    "VESSEL instance {} marked lost after worker {} became unreachable",
                    instance.id, node.id,
                );
            }
        }

        for instance in reconciled_instances {
            eprintln!(
                "VESSEL recovery reconciled instance {} with status {:?}",
                instance.id, instance.status,
            );
        }
    }
}

async fn run_persistence_loop(state: SharedState, store: PostgresStore, interval_ms: u64) {
    loop {
        sleep(Duration::from_millis(interval_ms)).await;

        // Copy the control-plane state while holding the mutex only
        // briefly. PostgreSQL I/O happens after the lock is released.
        let snapshot = match state.lock() {
            Ok(state) => state.clone(),

            Err(_) => {
                eprintln!("VESSEL persistence: control state lock was poisoned");
                continue;
            }
        };

        if let Err(error) = store.save_snapshot(&snapshot).await {
            eprintln!(
                "VESSEL persistence: failed to save control state: {}",
                error,
            );
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let address = env::var("VESSEL_CONTROL_ADDR").unwrap_or_else(|_| "127.0.0.1:7000".to_string());

    // Workers currently heartbeat every 5 seconds by default. Three missed
    // heartbeat periods therefore mark a worker unreachable after 15 seconds.
    let failure_timeout_ms = env_u64("VESSEL_FAILURE_TIMEOUT_MS", 15_000);

    let failure_check_interval_ms = env_u64("VESSEL_FAILURE_CHECK_INTERVAL_MS", 1_000);

    let persistence_interval_ms = env_u64("VESSEL_PERSIST_INTERVAL_MS", 1_000).max(100);

    let gateway_connect_timeout_ms = env_u64("VESSEL_GATEWAY_CONNECT_TIMEOUT_MS", 2_000).max(1);

    let gateway_request_timeout_ms = env_u64("VESSEL_GATEWAY_REQUEST_TIMEOUT_MS", 30_000).max(1);

    let network_config = ControlNetworkConfig::new(
        Duration::from_millis(gateway_connect_timeout_ms),
        Duration::from_millis(gateway_request_timeout_ms),
    );

    let (initial_state, postgres_store) = match env::var("DATABASE_URL") {
        Ok(database_url) => {
            let store = PostgresStore::connect(&database_url).await?;

            store.migrate().await?;

            let restored = store.load_snapshot().await?;

            println!(
                "VESSEL restored persistent control state: nodes={} workloads={} deployments={} instances={}",
                restored.node_count(),
                restored.workload_count(),
                restored.deployment_count(),
                restored.instance_count(),
            );

            (restored, Some(store))
        }

        Err(_) => {
            println!("VESSEL persistence disabled: DATABASE_URL is not set");

            (ControlState::new(), None)
        }
    };

    let listener = TcpListener::bind(&address).await?;

    println!(
        "VESSEL control plane listening on {}",
        listener.local_addr()?,
    );

    println!(
        "VESSEL failure detector timeout={}ms check_interval={}ms",
        failure_timeout_ms, failure_check_interval_ms,
    );

    println!(
        "VESSEL gateway networking connect_timeout={}ms request_timeout={}ms",
        gateway_connect_timeout_ms, gateway_request_timeout_ms,
    );

    let state: SharedState = Arc::new(Mutex::new(initial_state));

    let detector_state = Arc::clone(&state);

    tokio::spawn(run_failure_detector(
        detector_state,
        failure_timeout_ms,
        failure_check_interval_ms,
    ));

    if let Some(store) = postgres_store {
        println!(
            "VESSEL persistence enabled interval={}ms",
            persistence_interval_ms,
        );

        let persistence_state = Arc::clone(&state);

        tokio::spawn(run_persistence_loop(
            persistence_state,
            store,
            persistence_interval_ms,
        ));
    }

    axum::serve(
        listener,
        shared_router_with_network_config(state, network_config),
    )
    .await?;

    Ok(())
}
