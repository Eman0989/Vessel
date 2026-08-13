use std::{
    env,
    error::Error,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{net::TcpListener, time::sleep};
use vessel_control::{ControlState, SharedState, shared_router};

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

        let changed = match state.lock() {
            Ok(mut state) => state.detect_stale_workers(observed_at_ms, timeout_ms),

            Err(_) => {
                eprintln!("VESSEL failure detector: control state lock was poisoned");
                continue;
            }
        };

        for node in changed {
            eprintln!(
                "VESSEL worker {} marked unreachable after heartbeat timeout",
                node.id,
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

    let listener = TcpListener::bind(&address).await?;

    println!(
        "VESSEL control plane listening on {}",
        listener.local_addr()?,
    );

    println!(
        "VESSEL failure detector timeout={}ms check_interval={}ms",
        failure_timeout_ms, failure_check_interval_ms,
    );

    let state: SharedState = Arc::new(Mutex::new(ControlState::new()));

    let detector_state = Arc::clone(&state);

    tokio::spawn(run_failure_detector(
        detector_state,
        failure_timeout_ms,
        failure_check_interval_ms,
    ));

    axum::serve(listener, shared_router(state)).await?;

    Ok(())
}
