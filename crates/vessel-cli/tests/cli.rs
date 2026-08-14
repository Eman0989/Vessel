use std::time::Duration;

use axum::{
    Json, Router,
    extract::Path,
    http::StatusCode,
    routing::{get, post},
};
use clap::Parser;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use vessel_cli::{Cli, CliConfig, Command, ControlClient, execute};

#[test]
fn cli_parses_control_url_and_nodes_command() {
    let cli = Cli::try_parse_from([
        "vessel",
        "--control-url",
        "http://control.example:7000",
        "nodes",
    ])
    .unwrap();

    assert_eq!(
        cli.control_url.as_deref(),
        Some("http://control.example:7000"),
    );

    assert_eq!(cli.command, Command::Nodes);
}

#[tokio::test]
async fn control_client_fetches_nodes() {
    let app = Router::new().route(
        "/v1/nodes",
        get(|| async {
            Json(json!([
                {
                    "id": "node-01",
                    "status": "ready"
                }
            ]))
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = ControlClient::new(CliConfig::new(
        format!("http://{address}"),
        Duration::from_secs(1),
        Duration::from_secs(1),
    ))
    .unwrap();

    let value = client.get(Command::Nodes).await.unwrap();

    assert_eq!(value[0]["id"], "node-01");
    assert_eq!(value[0]["status"], "ready");

    server.abort();
}

#[tokio::test]
async fn control_client_surfaces_api_error_message() {
    let app = Router::new().route(
        "/v1/workloads",
        get(|| async {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": "control plane temporarily unavailable"
                })),
            )
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = ControlClient::new(CliConfig::new(
        format!("http://{address}"),
        Duration::from_secs(1),
        Duration::from_secs(1),
    ))
    .unwrap();

    let error = client.get(Command::Workloads).await.unwrap_err();

    assert!(
        error
            .to_string()
            .contains("control plane temporarily unavailable")
    );

    server.abort();
}

async fn echo_created_json(Json(payload): Json<Value>) -> (StatusCode, Json<Value>) {
    (StatusCode::CREATED, Json(payload))
}

#[tokio::test]
async fn workload_create_posts_control_contract() {
    let app = Router::new().route("/v1/workloads", post(echo_created_json));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let cli = Cli::try_parse_from([
        "vessel",
        "--control-url",
        &format!("http://{address}"),
        "workload",
        "create",
        "--id",
        "workload-01",
        "--name",
        "calculator",
        "--artifact",
        "sha256:abc123",
        "--cpu-millis",
        "250",
        "--memory-bytes",
        "1048576",
        "--timeout-ms",
        "2000",
        "--env",
        "MODE=test",
    ])
    .unwrap();

    let output = execute(cli).await.unwrap();

    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["id"], "workload-01");
    assert_eq!(value["status"], "registered");
    assert_eq!(value["spec"]["name"], "calculator");
    assert_eq!(value["spec"]["artifact"]["digest"], "sha256:abc123");
    assert_eq!(value["spec"]["resources"]["cpu_millis"], 250);
    assert_eq!(value["spec"]["resources"]["memory_bytes"], 1_048_576);
    assert_eq!(value["spec"]["timeout_ms"], 2_000);
    assert_eq!(value["spec"]["environment"]["MODE"], "test");

    server.abort();
}

#[tokio::test]
async fn deployment_scale_posts_replica_contract() {
    let app = Router::new().route(
        "/v1/deployments/{id}/scale",
        post(
            |Path(id): Path<String>, Json(payload): Json<Value>| async move {
                Json(json!({
                    "id": id,
                    "desired_replicas": payload["replicas"],
                    "generation": 2,
                    "status": "progressing"
                }))
            },
        ),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let cli = Cli::try_parse_from([
        "vessel",
        "--control-url",
        &format!("http://{address}"),
        "deployment",
        "scale",
        "deployment-01",
        "--replicas",
        "5",
    ])
    .unwrap();

    let output = execute(cli).await.unwrap();

    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["id"], "deployment-01");
    assert_eq!(value["desired_replicas"], 5);
    assert_eq!(value["generation"], 2);
    assert_eq!(value["status"], "progressing");

    server.abort();
}
