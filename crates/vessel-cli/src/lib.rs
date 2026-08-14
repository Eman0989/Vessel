use std::{env, time::Duration};

use clap::{Parser, Subcommand};
use serde_json::Value;
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

#[derive(Debug, Clone, Copy, Subcommand, PartialEq, Eq)]
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
}

impl Command {
    fn path(self) -> &'static str {
        match self {
            Self::Health => "/health",
            Self::Nodes => "/v1/nodes",
            Self::Workloads => "/v1/workloads",
            Self::Deployments => "/v1/deployments",
            Self::Instances => "/v1/instances",
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
        let response = self
            .client
            .get(format!("{}{}", self.control_url, command.path(),))
            .send()
            .await
            .map_err(CliError::Http)?;

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

    let value = client.get(cli.command).await?;

    serde_json::to_string_pretty(&value).map_err(CliError::Json)
}
