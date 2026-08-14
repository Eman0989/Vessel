use std::{collections::BTreeMap, env};

use vessel_control::{ControlState, PostgresStore};
use vessel_core::{
    ArtifactRef, Deployment, DeploymentId, DeploymentStatus, Node, NodeId, NodeStatus,
    ResourceCapacity, ResourceRequest, WorkerRegistration, Workload, WorkloadId, WorkloadSpec,
    WorkloadStatus,
};

fn node(id: &str) -> Node {
    Node {
        id: NodeId::new(id),
        name: id.to_string(),
        region: "test".to_string(),
        status: NodeStatus::Ready,
        capacity: ResourceCapacity::new(4_000, 536_870_912, 8),
        allocated: ResourceRequest::default(),
        allocated_instances: 0,
        labels: BTreeMap::new(),
    }
}

fn workload(id: &str) -> Workload {
    Workload {
        id: WorkloadId::new(id),
        spec: WorkloadSpec {
            name: id.to_string(),
            artifact: ArtifactRef {
                digest: "sha256:persistence-test".to_string(),
            },
            resources: ResourceRequest::new(500, 67_108_864),
            timeout_ms: 5_000,
            environment: BTreeMap::from([("VESSEL_TEST".to_string(), "persistent".to_string())]),
        },
        status: WorkloadStatus::Ready,
    }
}

fn deployment(id: &str, workload_id: &str) -> Deployment {
    Deployment {
        id: DeploymentId::new(id),
        workload_id: WorkloadId::new(workload_id),
        desired_replicas: 1,
        generation: 1,
        status: DeploymentStatus::Pending,
        canary: None,
    }
}

#[tokio::test]
async fn postgres_snapshot_round_trip_restores_control_state() {
    let database_url = match env::var("VESSEL_TEST_DATABASE_URL") {
        Ok(value) => value,
        Err(_) => {
            eprintln!(
                "skipping PostgreSQL integration test: \
                     VESSEL_TEST_DATABASE_URL is not set"
            );
            return;
        }
    };

    let store = PostgresStore::connect(&database_url).await.unwrap();

    store.migrate().await.unwrap();

    let mut original = ControlState::new();

    original.register_worker(
        WorkerRegistration::new(node("node-a"), "http://node-a:7001"),
        12_345,
    );

    original.register_node(node("node-b")).unwrap();

    original.register_workload(workload("workload-01")).unwrap();

    original
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    let reconciled = original
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(reconciled.len(), 1);

    store.save_snapshot(&original).await.unwrap();

    let restored = store.load_snapshot().await.unwrap();

    assert_eq!(restored.list_nodes(), original.list_nodes(),);

    assert_eq!(restored.list_workloads(), original.list_workloads(),);

    assert_eq!(restored.list_deployments(), original.list_deployments(),);

    assert_eq!(restored.list_instances(), original.list_instances(),);

    assert_eq!(
        restored.worker_endpoint(&NodeId::new("node-a")),
        Some("http://node-a:7001"),
    );

    assert_eq!(
        restored.node_last_seen_ms(&NodeId::new("node-a")),
        Some(12_345),
    );

    assert_eq!(restored.worker_endpoint(&NodeId::new("node-b")), None,);

    assert_eq!(restored.node_last_seen_ms(&NodeId::new("node-b")), None,);
}
