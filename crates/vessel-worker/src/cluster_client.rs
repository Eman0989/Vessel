use std::time::Duration;

use reqwest::Client;
use thiserror::Error;
use vessel_core::{WorkerHeartbeat, WorkerRegistration};

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Error)]
pub enum ClusterClientError {
    #[error("cluster HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
}

#[derive(Debug, Clone)]
pub struct ClusterClient {
    client: Client,
    control_url: String,
}

impl ClusterClient {
    pub fn new(control_url: impl Into<String>) -> Self {
        Self::with_timeouts(
            control_url,
            DEFAULT_CONNECT_TIMEOUT,
            DEFAULT_REQUEST_TIMEOUT,
        )
        .expect("default cluster HTTP client configuration must be valid")
    }

    pub fn with_timeouts(
        control_url: impl Into<String>,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, ClusterClientError> {
        let control_url = control_url.into();

        let client = Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .build()?;

        Ok(Self {
            client,
            control_url: control_url.trim_end_matches('/').to_string(),
        })
    }

    pub async fn register(
        &self,
        registration: &WorkerRegistration,
    ) -> Result<(), ClusterClientError> {
        self.client
            .post(format!("{}/v1/cluster/register", self.control_url,))
            .json(registration)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    pub async fn heartbeat(&self, heartbeat: &WorkerHeartbeat) -> Result<(), ClusterClientError> {
        self.client
            .post(format!("{}/v1/cluster/heartbeat", self.control_url,))
            .json(heartbeat)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}
