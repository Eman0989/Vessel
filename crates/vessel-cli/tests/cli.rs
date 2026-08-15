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

#[tokio::test]
async fn deployment_autoscaling_enable_posts_policy_contract() {
    let app = Router::new().route(
        "/v1/deployments/{id}/autoscaling",
        post(
            |Path(id): Path<String>, Json(payload): Json<Value>| async move {
                Json(json!({
                    "id": id,
                    "desired_replicas": 2,
                    "generation": 2,
                    "status": "healthy",
                    "autoscaling": payload
                }))
            },
        ),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let control_url = format!("http://{address}");

    let cli = Cli::try_parse_from([
        "vessel",
        "--control-url",
        &control_url,
        "deployment",
        "autoscaling",
        "enable",
        "deployment-01",
        "--min-replicas",
        "1",
        "--max-replicas",
        "6",
        "--target-cpu",
        "70",
    ])
    .unwrap();

    let output = execute(cli).await.unwrap();
    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["id"], "deployment-01");

    assert_eq!(value["autoscaling"]["min_replicas"], 1,);

    assert_eq!(value["autoscaling"]["max_replicas"], 6,);

    assert_eq!(value["autoscaling"]["target_cpu_utilization_percent"], 70,);

    server.abort();
}

#[tokio::test]
async fn deployment_autoscaling_disable_posts_without_payload() {
    let app = Router::new().route(
        "/v1/deployments/{id}/autoscaling/disable",
        post(|Path(id): Path<String>| async move {
            Json(json!({
                "id": id,
                "desired_replicas": 4,
                "generation": 4,
                "status": "healthy",
                "autoscaling": null
            }))
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let control_url = format!("http://{address}");

    let cli = Cli::try_parse_from([
        "vessel",
        "--control-url",
        &control_url,
        "deployment",
        "autoscaling",
        "disable",
        "deployment-01",
    ])
    .unwrap();

    let output = execute(cli).await.unwrap();
    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["id"], "deployment-01");
    assert!(value["autoscaling"].is_null());
    assert_eq!(value["desired_replicas"], 4);

    server.abort();
}

#[tokio::test]
async fn deployment_autoscaling_evaluate_posts_cpu_sample() {
    let app = Router::new().route(
        "/v1/deployments/{id}/autoscaling/evaluate",
        post(
            |Path(_id): Path<String>, Json(payload): Json<Value>| async move {
                Json(json!({
                    "current_replicas": 2,
                    "desired_replicas": 3,
                    "observed_cpu_utilization_percent":
                        payload[
                            "observed_cpu_utilization_percent"
                        ],
                    "target_cpu_utilization_percent": 70,
                    "direction": "scale_up"
                }))
            },
        ),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let control_url = format!("http://{address}");

    let cli = Cli::try_parse_from([
        "vessel",
        "--control-url",
        &control_url,
        "deployment",
        "autoscaling",
        "evaluate",
        "deployment-01",
        "--observed-cpu",
        "85",
    ])
    .unwrap();

    let output = execute(cli).await.unwrap();
    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["current_replicas"], 2);
    assert_eq!(value["desired_replicas"], 3);

    assert_eq!(value["observed_cpu_utilization_percent"], 85,);

    assert_eq!(value["target_cpu_utilization_percent"], 70,);

    assert_eq!(value["direction"], "scale_up");

    server.abort();
}

#[tokio::test]
async fn deployment_canary_posts_candidate_contract() {
    let app = Router::new().route(
        "/v1/deployments/{id}/canary",
        post(
            |Path(id): Path<String>, Json(payload): Json<Value>| async move {
                Json(json!({
                    "id": id,
                    "workload_id": "workload-v1",
                    "generation": 2,
                    "status": "progressing",
                    "canary": {
                        "stable_workload_id": "workload-v1",
                        "candidate_workload_id":
                            payload["workload_id"],
                        "candidate_replicas":
                            payload["replicas"]
                    }
                }))
            },
        ),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let control_url = format!("http://{address}");

    let cli = Cli::try_parse_from([
        "vessel",
        "--control-url",
        &control_url,
        "deployment",
        "canary",
        "deployment-01",
        "--workload",
        "workload-v2",
        "--replicas",
        "1",
    ])
    .unwrap();

    let output = execute(cli).await.unwrap();

    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["id"], "deployment-01");
    assert_eq!(value["workload_id"], "workload-v1");
    assert_eq!(value["generation"], 2);
    assert_eq!(value["status"], "progressing");

    assert_eq!(value["canary"]["stable_workload_id"], "workload-v1",);

    assert_eq!(value["canary"]["candidate_workload_id"], "workload-v2",);

    assert_eq!(value["canary"]["candidate_replicas"], 1,);

    server.abort();
}

#[tokio::test]
async fn deployment_promote_posts_without_payload() {
    let app = Router::new().route(
        "/v1/deployments/{id}/promote",
        post(|Path(id): Path<String>| async move {
            Json(json!({
                "id": id,
                "workload_id": "workload-v2",
                "previous_workload_id": "workload-v1",
                "generation": 3,
                "status": "progressing"
            }))
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let control_url = format!("http://{address}");

    let cli = Cli::try_parse_from([
        "vessel",
        "--control-url",
        &control_url,
        "deployment",
        "promote",
        "deployment-01",
    ])
    .unwrap();

    let output = execute(cli).await.unwrap();

    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["id"], "deployment-01");
    assert_eq!(value["workload_id"], "workload-v2");

    assert_eq!(value["previous_workload_id"], "workload-v1",);

    assert_eq!(value["generation"], 3);
    assert_eq!(value["status"], "progressing");

    server.abort();
}

#[tokio::test]
async fn deployment_rollback_posts_without_payload() {
    let app = Router::new().route(
        "/v1/deployments/{id}/rollback",
        post(|Path(id): Path<String>| async move {
            Json(json!({
                "id": id,
                "workload_id": "workload-v1",
                "previous_workload_id": "workload-v2",
                "generation": 4,
                "status": "progressing"
            }))
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let control_url = format!("http://{address}");

    let cli = Cli::try_parse_from([
        "vessel",
        "--control-url",
        &control_url,
        "deployment",
        "rollback",
        "deployment-01",
    ])
    .unwrap();

    let output = execute(cli).await.unwrap();

    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["id"], "deployment-01");
    assert_eq!(value["workload_id"], "workload-v1");

    assert_eq!(value["previous_workload_id"], "workload-v2",);

    assert_eq!(value["generation"], 4);
    assert_eq!(value["status"], "progressing");

    server.abort();
}

#[tokio::test]
async fn instance_create_posts_pending_contract() {
    let app = Router::new().route("/v1/instances", post(echo_created_json));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let control_url = format!("http://{address}");

    let cli = Cli::try_parse_from([
        "vessel",
        "--control-url",
        &control_url,
        "instance",
        "create",
        "--id",
        "instance-01",
        "--deployment",
        "deployment-01",
        "--workload",
        "workload-01",
        "--cpu-millis",
        "500",
        "--memory-bytes",
        "67108864",
    ])
    .unwrap();

    let output = execute(cli).await.unwrap();
    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["id"], "instance-01");
    assert_eq!(value["deployment_id"], "deployment-01");
    assert_eq!(value["workload_id"], "workload-01");
    assert!(value["node_id"].is_null());
    assert_eq!(value["status"], "pending");
    assert_eq!(value["resources"]["cpu_millis"], 500);
    assert_eq!(value["resources"]["memory_bytes"], 67_108_864);
    assert_eq!(value["restart_count"], 0);

    server.abort();
}

#[tokio::test]
async fn instance_assign_posts_node_contract() {
    let app = Router::new().route(
        "/v1/instances/{id}/assign",
        post(
            |Path(id): Path<String>, Json(payload): Json<Value>| async move {
                Json(json!({
                    "id": id,
                    "node_id": payload["node_id"],
                    "status": "assigned"
                }))
            },
        ),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let control_url = format!("http://{address}");

    let cli = Cli::try_parse_from([
        "vessel",
        "--control-url",
        &control_url,
        "instance",
        "assign",
        "instance-01",
        "--node",
        "node-01",
    ])
    .unwrap();

    let output = execute(cli).await.unwrap();
    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["id"], "instance-01");
    assert_eq!(value["node_id"], "node-01");
    assert_eq!(value["status"], "assigned");

    server.abort();
}

#[tokio::test]
async fn instance_schedule_posts_without_payload() {
    let app = Router::new().route(
        "/v1/instances/{id}/schedule",
        post(|Path(id): Path<String>| async move {
            Json(json!({
                "id": id,
                "node_id": "scheduled-node",
                "status": "assigned"
            }))
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let control_url = format!("http://{address}");

    let cli = Cli::try_parse_from([
        "vessel",
        "--control-url",
        &control_url,
        "instance",
        "schedule",
        "instance-01",
    ])
    .unwrap();

    let output = execute(cli).await.unwrap();
    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["id"], "instance-01");
    assert_eq!(value["node_id"], "scheduled-node");
    assert_eq!(value["status"], "assigned");

    server.abort();
}

#[tokio::test]
async fn instance_transition_posts_status_contract() {
    let app = Router::new().route(
        "/v1/instances/{id}/transition",
        post(
            |Path(id): Path<String>, Json(payload): Json<Value>| async move {
                Json(json!({
                    "id": id,
                    "status": payload["status"]
                }))
            },
        ),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let control_url = format!("http://{address}");

    let cli = Cli::try_parse_from([
        "vessel",
        "--control-url",
        &control_url,
        "instance",
        "transition",
        "instance-01",
        "--status",
        "starting",
    ])
    .unwrap();

    let output = execute(cli).await.unwrap();
    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["id"], "instance-01");
    assert_eq!(value["status"], "starting");

    server.abort();
}

#[tokio::test]
async fn instance_invoke_reads_module_and_posts_contract() {
    let app = Router::new().route(
        "/v1/instances/{id}/invoke",
        post(
            |Path(id): Path<String>, Json(payload): Json<Value>| async move {
                Json(json!({
                    "id": id,
                    "node_id": "node-01",
                    "value": 42,
                    "received_module_bytes":
                        payload["module_bytes"],
                    "received_export":
                        payload["export"],
                    "received_lhs":
                        payload["lhs"],
                    "received_rhs":
                        payload["rhs"],
                    "received_resources":
                        payload["resources"]
                }))
            },
        ),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let module_path = std::env::temp_dir().join(format!(
        "vessel-cli-instance-invoke-{}.wasm",
        std::process::id()
    ));

    std::fs::write(&module_path, [0_u8, 97, 115, 109]).unwrap();

    let module_arg = module_path.to_string_lossy().into_owned();

    let control_url = format!("http://{address}");

    let cli = Cli::try_parse_from([
        "vessel",
        "--control-url",
        &control_url,
        "instance",
        "invoke",
        "instance-01",
        "--module",
        &module_arg,
        "--export",
        "add",
        "--lhs",
        "20",
        "--rhs",
        "22",
    ])
    .unwrap();

    let output = execute(cli).await.unwrap();

    std::fs::remove_file(&module_path).unwrap();

    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["id"], "instance-01");

    assert_eq!(value["received_module_bytes"], json!([0, 97, 115, 109]));

    assert_eq!(value["received_export"], "add");
    assert_eq!(value["received_lhs"], 20);
    assert_eq!(value["received_rhs"], 22);

    assert_eq!(value["received_resources"]["cpu_millis"], 0);

    assert_eq!(value["received_resources"]["memory_bytes"], 0);

    server.abort();
}

async fn receive_artifact(
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> (StatusCode, Json<Value>) {
    assert_eq!(
        headers
            .get(axum::http::header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        "application/wasm"
    );

    assert_eq!(body.as_ref(), &[0_u8, 97, 115, 109]);

    (
        StatusCode::CREATED,
        Json(json!({
            "artifact": {
                "digest": "sha256:test-upload"
            },
            "size_bytes": 4
        })),
    )
}

#[tokio::test]
async fn artifact_push_uploads_raw_wasm() {
    let app = Router::new().route("/v1/artifacts", post(receive_artifact));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let artifact_path = std::env::temp_dir().join(format!(
        "vessel-cli-artifact-push-{}.wasm",
        std::process::id()
    ));

    std::fs::write(&artifact_path, [0_u8, 97, 115, 109]).unwrap();

    let artifact_arg = artifact_path.to_string_lossy().into_owned();

    let registry_url = format!("http://{address}");

    let cli = Cli::try_parse_from([
        "vessel",
        "--registry-url",
        &registry_url,
        "artifact",
        "push",
        &artifact_arg,
    ])
    .unwrap();

    let output = execute(cli).await.unwrap();

    std::fs::remove_file(&artifact_path).unwrap();

    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["artifact"]["digest"], "sha256:test-upload");

    assert_eq!(value["size_bytes"], 4);

    server.abort();
}
