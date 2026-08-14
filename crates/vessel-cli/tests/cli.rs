use std::time::Duration;

use axum::{Json, Router, http::StatusCode, routing::get};
use clap::Parser;
use serde_json::json;
use tokio::net::TcpListener;
use vessel_cli::{Cli, CliConfig, Command, ControlClient};

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
