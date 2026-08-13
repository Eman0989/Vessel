use std::sync::{Arc, Mutex};

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use tokio::net::TcpListener;
use vessel_core::{NodeStatus, WorkerHeartbeat, WorkerRegistration};
use vessel_worker::{ClusterClient, WorkerConfig, WorkerService};

#[derive(Debug, Default)]
struct Received {
    registrations: Vec<WorkerRegistration>,
    heartbeats: Vec<WorkerHeartbeat>,
}

type SharedReceived = Arc<Mutex<Received>>;

async fn receive_registration(
    State(state): State<SharedReceived>,
    Json(registration): Json<WorkerRegistration>,
) -> StatusCode {
    state.lock().unwrap().registrations.push(registration);

    StatusCode::OK
}

async fn receive_heartbeat(
    State(state): State<SharedReceived>,
    Json(heartbeat): Json<WorkerHeartbeat>,
) -> StatusCode {
    state.lock().unwrap().heartbeats.push(heartbeat);

    StatusCode::OK
}

async fn reject_heartbeat(Json(_heartbeat): Json<WorkerHeartbeat>) -> StatusCode {
    StatusCode::NOT_FOUND
}

#[tokio::test]
async fn cluster_client_sends_registration_and_heartbeat() {
    let received = Arc::new(Mutex::new(Received::default()));

    let app = Router::new()
        .route("/v1/cluster/register", post(receive_registration))
        .route("/v1/cluster/heartbeat", post(receive_heartbeat))
        .with_state(Arc::clone(&received));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = ClusterClient::new(format!("http://{address}"));

    let worker = WorkerService::new(WorkerConfig::new("cluster-client-01"));

    let registration = worker.registration().unwrap();

    client.register(&registration).await.unwrap();

    worker.drain().unwrap();

    let heartbeat = worker.heartbeat().unwrap();

    client.heartbeat(&heartbeat).await.unwrap();

    {
        let received = received.lock().unwrap();

        assert_eq!(received.registrations.len(), 1,);

        assert_eq!(
            received.registrations[0].node.id.as_str(),
            "cluster-client-01",
        );

        assert_eq!(received.registrations[0].endpoint, "http://127.0.0.1:7001",);

        assert_eq!(received.heartbeats.len(), 1,);

        assert_eq!(received.heartbeats[0].status, NodeStatus::Draining,);
    }

    server.abort();
}

#[tokio::test]
async fn cluster_client_surfaces_rejected_heartbeat() {
    let app = Router::new().route("/v1/cluster/heartbeat", post(reject_heartbeat));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = ClusterClient::new(format!("http://{address}"));

    let worker = WorkerService::new(WorkerConfig::new("cluster-client-02"));

    let heartbeat = worker.heartbeat().unwrap();

    let result = client.heartbeat(&heartbeat).await;

    assert!(result.is_err());

    server.abort();
}
