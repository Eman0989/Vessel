use std::{collections::BTreeMap, env, time::Duration};

use clap::{Parser, Subcommand};
use reqwest::Method;
use serde_json::{Value, json};
use thiserror::Error;

const DEFAULT_CONTROL_URL: &str = "http://127.0.0.1:7000";
const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 2_000;
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 10_000;

#[derive(Debug, Parser)]
#[command(
    name = "vessel",
    version,
    about = "Command-line client for the VESSEL execution fabric"
)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "URL",
        help = "VESSEL control-plane URL"
    )]
    pub control_url: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum Command {
    /// Check control-plane health.
    Health,

    /// List registered worker nodes.
    Nodes,

    /// List registered workloads.
    Workloads,

    /// List deployments.
    Deployments,

    /// List workload instances.
    Instances,

    /// Manage workloads.
    Workload {
        #[command(subcommand)]
        command: WorkloadCommand,
    },

    /// Manage deployments.
    Deployment {
        #[command(subcommand)]
        command: DeploymentCommand,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum WorkloadCommand {
    /// Register a workload with the control plane.
    Create {
        /// Workload identifier.
        #[arg(long)]
        id: String,

        /// Human-readable workload name.
        #[arg(long)]
        name: String,

        /// Content-addressed artifact digest.
        #[arg(long, value_name = "DIGEST")]
        artifact: String,

        /// Requested CPU in millicores.
        #[arg(long, default_value_t = 100)]
        cpu_millis: u32,

        /// Requested memory in bytes.
        #[arg(long, default_value_t = 67_108_864)]
        memory_bytes: u64,

        /// Execution timeout in milliseconds.
        #[arg(long, default_value_t = 1_000)]
        timeout_ms: u64,

        /// Environment variable in KEY=VALUE form.
        #[arg(
            long = "env",
            value_name = "KEY=VALUE",
            value_parser = parse_key_value
        )]
        environment: Vec<(String, String)>,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum DeploymentCommand {
    /// Create a deployment for a workload.
    Create {
        /// Deployment identifier.
        #[arg(long)]
        id: String,

        /// Workload identifier.
        #[arg(long)]
        workload: String,

        /// Desired replica count.
        #[arg(long, default_value_t = 1)]
        replicas: u32,
    },

    /// Change a deployment's desired replica count.
    Scale {
        /// Deployment identifier.
        id: String,

        /// Desired replica count.
        #[arg(long)]
        replicas: u32,
    },

    /// Reconcile a deployment toward its desired replica count.
    Reconcile {
        /// Deployment identifier.
        id: String,
    },
}

fn parse_key_value(value: &str) -> Result<(String, String), String> {
    let Some((key, value)) = value.split_once('=') else {
        return Err("environment variables must use KEY=VALUE syntax".to_string());
    };

    if key.is_empty() {
        return Err("environment variable name cannot be empty".to_string());
    }

    Ok((key.to_string(), value.to_string()))
}

impl Command {
    fn read_path(&self) -> Option<&'static str> {
        match self {
            Self::Health => Some("/health"),
            Self::Nodes => Some("/v1/nodes"),
            Self::Workloads => Some("/v1/workloads"),
            Self::Deployments => Some("/v1/deployments"),
            Self::Instances => Some("/v1/instances"),
            Self::Workload { .. } | Self::Deployment { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliConfig {
    pub control_url: String,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
}

impl CliConfig {
    pub fn new(
        control_url: impl Into<String>,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Self {
        Self {
            control_url: control_url.into(),
            connect_timeout,
            request_timeout,
        }
    }

    pub fn from_cli(cli: &Cli) -> Self {
        let control_url = cli
            .control_url
            .clone()
            .or_else(|| env::var("VESSEL_CONTROL_URL").ok())
            .unwrap_or_else(|| DEFAULT_CONTROL_URL.to_string());

        let connect_timeout = Duration::from_millis(
            env_u64("VESSEL_CLI_CONNECT_TIMEOUT_MS", DEFAULT_CONNECT_TIMEOUT_MS).max(1),
        );

        let request_timeout = Duration::from_millis(
            env_u64("VESSEL_CLI_REQUEST_TIMEOUT_MS", DEFAULT_REQUEST_TIMEOUT_MS).max(1),
        );

        Self::new(control_url, connect_timeout, request_timeout)
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("failed to build CLI HTTP client: {0}")]
    ClientBuild(reqwest::Error),

    #[error("control-plane request failed: {0}")]
    Http(reqwest::Error),

    #[error("control plane returned HTTP {status}: {message}")]
    Status { status: u16, message: String },

    #[error("control plane returned invalid JSON: {0}")]
    Json(serde_json::Error),

    #[error("command is not a control-plane read command")]
    NotReadCommand,
}

#[derive(Clone)]
pub struct ControlClient {
    client: reqwest::Client,
    control_url: String,
}

impl ControlClient {
    pub fn new(config: CliConfig) -> Result<Self, CliError> {
        let client = reqwest::Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout)
            .build()
            .map_err(CliError::ClientBuild)?;

        Ok(Self {
            client,
            control_url: config.control_url.trim_end_matches('/').to_string(),
        })
    }

    pub async fn get(&self, command: Command) -> Result<Value, CliError> {
        let path = command.read_path().ok_or(CliError::NotReadCommand)?;

        self.request_json(Method::GET, path, None).await
    }

    pub async fn post(&self, path: &str, body: Option<&Value>) -> Result<Value, CliError> {
        self.request_json(Method::POST, path, body).await
    }

    async fn request_json(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, CliError> {
        let mut request = self
            .client
            .request(method, format!("{}{}", self.control_url, path));

        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request.send().await.map_err(CliError::Http)?;

        let status = response.status();

        let bytes = response.bytes().await.map_err(CliError::Http)?;

        if !status.is_success() {
            let message = serde_json::from_slice::<Value>(&bytes)
                .ok()
                .and_then(|json| json.get("error").and_then(Value::as_str).map(str::to_owned))
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| {
                    let body = String::from_utf8_lossy(&bytes).trim().to_string();

                    if body.is_empty() {
                        status
                            .canonical_reason()
                            .unwrap_or("request failed")
                            .to_string()
                    } else {
                        body
                    }
                });

            return Err(CliError::Status {
                status: status.as_u16(),
                message,
            });
        }

        serde_json::from_slice(&bytes).map_err(CliError::Json)
    }
}

pub async fn execute(cli: Cli) -> Result<String, CliError> {
    let config = CliConfig::from_cli(&cli);
    let client = ControlClient::new(config)?;

    let value = match cli.command {
        command @ (Command::Health
        | Command::Nodes
        | Command::Workloads
        | Command::Deployments
        | Command::Instances) => client.get(command).await?,

        Command::Workload {
            command:
                WorkloadCommand::Create {
                    id,
                    name,
                    artifact,
                    cpu_millis,
                    memory_bytes,
                    timeout_ms,
                    environment,
                },
        } => {
            let environment = environment.into_iter().collect::<BTreeMap<_, _>>();

            let body = json!({
                "id": id,
                "spec": {
                    "name": name,
                    "artifact": {
                        "digest": artifact
                    },
                    "resources": {
                        "cpu_millis": cpu_millis,
                        "memory_bytes": memory_bytes
                    },
                    "timeout_ms": timeout_ms,
                    "environment": environment
                },
                "status": "registered"
            });

            client.post("/v1/workloads", Some(&body)).await?
        }

        Command::Deployment {
            command:
                DeploymentCommand::Create {
                    id,
                    workload,
                    replicas,
                },
        } => {
            let body = json!({
                "id": id,
                "workload_id": workload,
                "desired_replicas": replicas,
                "generation": 1,
                "status": "pending"
            });

            client.post("/v1/deployments", Some(&body)).await?
        }

        Command::Deployment {
            command: DeploymentCommand::Scale { id, replicas },
        } => {
            let body = json!({
                "replicas": replicas
            });

            client
                .post(&format!("/v1/deployments/{id}/scale"), Some(&body))
                .await?
        }

        Command::Deployment {
            command: DeploymentCommand::Reconcile { id },
        } => {
            client
                .post(&format!("/v1/deployments/{id}/reconcile"), None)
                .await?
        }
    };

    serde_json::to_string_pretty(&value).map_err(CliError::Json)
}
