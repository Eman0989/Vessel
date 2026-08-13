use vessel_worker::{WorkerConfig, WorkerService};

fn main() {
    let worker = WorkerService::new(WorkerConfig::new("worker-local"));

    println!("VESSEL worker {} ready", worker.node_id());
}
