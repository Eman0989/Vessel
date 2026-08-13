use reqwest::Client;
use thiserror::Error;
use vessel_core::{WorkerHeartbeat, WorkerRegistration};

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
        let control_url = control_url.into();

        Self {
            client: Client::new(),
            control_url: control_url.trim_end_matches('/').to_string(),
        }
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
