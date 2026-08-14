use sqlx::{
    PgPool,
    migrate::{MigrateError, Migrator},
    types::Json,
};
use thiserror::Error;
use vessel_core::{Deployment, Instance, Node, NodeId, Workload};

use crate::{ControlError, ControlState};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),

    #[error(transparent)]
    Control(#[from] ControlError),

    #[error(
        "node {node_id} last-seen timestamp {value} cannot be represented as PostgreSQL BIGINT"
    )]
    LastSeenOutOfRange { node_id: NodeId, value: u64 },

    #[error("node {node_id} has invalid negative persisted last-seen timestamp {value}")]
    NegativeLastSeen { node_id: NodeId, value: i64 },
}

#[derive(Debug, Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPool::connect(database_url).await?;

        Ok(Self { pool })
    }

    pub fn connect_lazy(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPool::connect_lazy(database_url)?;

        Ok(Self { pool })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn migrate(&self) -> Result<(), MigrateError> {
        MIGRATOR.run(&self.pool).await
    }

    pub async fn health_check(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;

        Ok(())
    }

    pub async fn save_snapshot(&self, state: &ControlState) -> Result<(), PersistenceError> {
        let nodes = state
            .list_nodes()
            .into_iter()
            .map(|node| {
                let endpoint = state.worker_endpoint(&node.id).map(str::to_owned);

                let last_seen_ms = state
                    .node_last_seen_ms(&node.id)
                    .map(|value| {
                        i64::try_from(value).map_err(|_| PersistenceError::LastSeenOutOfRange {
                            node_id: node.id.clone(),
                            value,
                        })
                    })
                    .transpose()?;

                Ok((node, endpoint, last_seen_ms))
            })
            .collect::<Result<Vec<_>, PersistenceError>>()?;

        let workloads = state.list_workloads();
        let deployments = state.list_deployments();
        let instances = state.list_instances();

        let mut transaction = self.pool.begin().await?;

        // Replace the persisted control-plane snapshot atomically.
        // Delete children before parents because of foreign keys.
        sqlx::query("DELETE FROM vessel_instances")
            .execute(&mut *transaction)
            .await?;

        sqlx::query("DELETE FROM vessel_deployments")
            .execute(&mut *transaction)
            .await?;

        sqlx::query("DELETE FROM vessel_workloads")
            .execute(&mut *transaction)
            .await?;

        sqlx::query("DELETE FROM vessel_nodes")
            .execute(&mut *transaction)
            .await?;

        for (node, endpoint, last_seen_ms) in nodes {
            let id = node.id.as_str().to_owned();

            sqlx::query(
                r#"
                INSERT INTO vessel_nodes (
                    id,
                    payload,
                    endpoint,
                    last_seen_ms
                )
                VALUES ($1, $2, $3, $4)
                "#,
            )
            .bind(id)
            .bind(Json(node))
            .bind(endpoint)
            .bind(last_seen_ms)
            .execute(&mut *transaction)
            .await?;
        }

        for workload in workloads {
            let id = workload.id.as_str().to_owned();

            sqlx::query(
                r#"
                INSERT INTO vessel_workloads (
                    id,
                    payload
                )
                VALUES ($1, $2)
                "#,
            )
            .bind(id)
            .bind(Json(workload))
            .execute(&mut *transaction)
            .await?;
        }

        for deployment in deployments {
            let id = deployment.id.as_str().to_owned();
            let workload_id = deployment.workload_id.as_str().to_owned();

            sqlx::query(
                r#"
                INSERT INTO vessel_deployments (
                    id,
                    workload_id,
                    payload
                )
                VALUES ($1, $2, $3)
                "#,
            )
            .bind(id)
            .bind(workload_id)
            .bind(Json(deployment))
            .execute(&mut *transaction)
            .await?;
        }

        for instance in instances {
            let id = instance.id.as_str().to_owned();
            let deployment_id = instance.deployment_id.as_str().to_owned();
            let workload_id = instance.workload_id.as_str().to_owned();
            let node_id = instance
                .node_id
                .as_ref()
                .map(|node_id| node_id.as_str().to_owned());

            sqlx::query(
                r#"
                INSERT INTO vessel_instances (
                    id,
                    deployment_id,
                    workload_id,
                    node_id,
                    payload
                )
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(id)
            .bind(deployment_id)
            .bind(workload_id)
            .bind(node_id)
            .bind(Json(instance))
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;

        Ok(())
    }

    pub async fn load_snapshot(&self) -> Result<ControlState, PersistenceError> {
        let mut transaction = self.pool.begin().await?;

        // Ensure all SELECT statements observe one consistent PostgreSQL
        // snapshot even if another control-plane process writes concurrently.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(&mut *transaction)
            .await?;

        let node_rows = sqlx::query_as::<_, (Json<Node>, Option<String>, Option<i64>)>(
            r#"
            SELECT payload, endpoint, last_seen_ms
            FROM vessel_nodes
            ORDER BY id
            "#,
        )
        .fetch_all(&mut *transaction)
        .await?;

        let workload_rows = sqlx::query_as::<_, (Json<Workload>,)>(
            r#"
                SELECT payload
                FROM vessel_workloads
                ORDER BY id
                "#,
        )
        .fetch_all(&mut *transaction)
        .await?;

        let deployment_rows = sqlx::query_as::<_, (Json<Deployment>,)>(
            r#"
                SELECT payload
                FROM vessel_deployments
                ORDER BY id
                "#,
        )
        .fetch_all(&mut *transaction)
        .await?;

        let instance_rows = sqlx::query_as::<_, (Json<Instance>,)>(
            r#"
                SELECT payload
                FROM vessel_instances
                ORDER BY id
                "#,
        )
        .fetch_all(&mut *transaction)
        .await?;

        transaction.commit().await?;

        let nodes = node_rows
            .into_iter()
            .map(|(Json(node), endpoint, last_seen_ms)| (node, endpoint, last_seen_ms))
            .collect();

        let workloads = workload_rows
            .into_iter()
            .map(|(Json(workload),)| workload)
            .collect();

        let deployments = deployment_rows
            .into_iter()
            .map(|(Json(deployment),)| deployment)
            .collect();

        let instances = instance_rows
            .into_iter()
            .map(|(Json(instance),)| instance)
            .collect();

        restore_state(nodes, workloads, deployments, instances)
    }
}

fn restore_state(
    nodes: Vec<(Node, Option<String>, Option<i64>)>,
    workloads: Vec<Workload>,
    deployments: Vec<Deployment>,
    instances: Vec<Instance>,
) -> Result<ControlState, PersistenceError> {
    let mut state = ControlState::new();

    for (node, endpoint, last_seen_ms) in nodes {
        let node_id = node.id.clone();

        let last_seen_ms = last_seen_ms
            .map(|value| {
                u64::try_from(value).map_err(|_| PersistenceError::NegativeLastSeen {
                    node_id: node_id.clone(),
                    value,
                })
            })
            .transpose()?;

        state.restore_node_snapshot(node, endpoint, last_seen_ms)?;
    }

    for workload in workloads {
        state.register_workload(workload)?;
    }

    for deployment in deployments {
        state.create_deployment(deployment)?;
    }

    for instance in instances {
        state.restore_instance_snapshot(instance)?;
    }

    Ok(state)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use vessel_core::{
        ArtifactRef, Deployment, DeploymentId, DeploymentStatus, Instance, InstanceId,
        InstanceStatus, Node, NodeId, NodeStatus, ResourceCapacity, ResourceRequest, Workload,
        WorkloadId, WorkloadSpec, WorkloadStatus,
    };

    use super::{PersistenceError, PostgresStore, restore_state};

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

    #[tokio::test]
    async fn lazy_postgres_store_does_not_require_live_database() {
        let store = PostgresStore::connect_lazy("postgres://vessel:vessel@127.0.0.1:5432/vessel");

        assert!(store.is_ok());
    }

    #[test]
    fn snapshot_restoration_preserves_worker_metadata_independently() {
        let state = restore_state(
            vec![
                (
                    node("node-endpoint"),
                    Some("http://node-endpoint:7001".to_string()),
                    None,
                ),
                (node("node-liveness"), None, Some(4_200)),
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();

        assert_eq!(
            state.worker_endpoint(&NodeId::new("node-endpoint")),
            Some("http://node-endpoint:7001"),
        );

        assert_eq!(state.node_last_seen_ms(&NodeId::new("node-endpoint")), None,);

        assert_eq!(state.worker_endpoint(&NodeId::new("node-liveness")), None,);

        assert_eq!(
            state.node_last_seen_ms(&NodeId::new("node-liveness")),
            Some(4_200),
        );
    }

    #[test]
    fn snapshot_restoration_rejects_negative_liveness_timestamp() {
        let error = restore_state(
            vec![(node("node-01"), None, Some(-1))],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            PersistenceError::NegativeLastSeen {
                node_id,
                value: -1,
            } if node_id == NodeId::new("node-01")
        ));
    }

    #[test]
    fn snapshot_restoration_allows_previous_workload_revision() {
        let old_workload = Workload {
            id: WorkloadId::new("workload-v1"),
            spec: WorkloadSpec {
                name: "workload-v1".to_string(),
                artifact: ArtifactRef {
                    digest: "sha256:v1".to_string(),
                },
                resources: ResourceRequest::new(500, 67_108_864),
                timeout_ms: 5_000,
                environment: BTreeMap::new(),
            },
            status: WorkloadStatus::Ready,
        };

        let target_workload = Workload {
            id: WorkloadId::new("workload-v2"),
            spec: WorkloadSpec {
                name: "workload-v2".to_string(),
                artifact: ArtifactRef {
                    digest: "sha256:v2".to_string(),
                },
                resources: ResourceRequest::new(500, 67_108_864),
                timeout_ms: 5_000,
                environment: BTreeMap::new(),
            },
            status: WorkloadStatus::Ready,
        };

        let deployment = Deployment {
            id: DeploymentId::new("deployment-01"),
            workload_id: WorkloadId::new("workload-v2"),
            desired_replicas: 1,
            generation: 2,
            status: DeploymentStatus::Progressing,
            canary: None,
        };

        let old_instance = Instance {
            id: InstanceId::new("deployment-01-replica-1"),
            deployment_id: DeploymentId::new("deployment-01"),
            workload_id: WorkloadId::new("workload-v1"),
            node_id: None,
            status: InstanceStatus::Pending,
            resources: ResourceRequest::new(500, 67_108_864),
            restart_count: 0,
        };

        let state = restore_state(
            Vec::new(),
            vec![old_workload, target_workload],
            vec![deployment],
            vec![old_instance],
        )
        .unwrap();

        let restored_deployment = state
            .deployment(&DeploymentId::new("deployment-01"))
            .unwrap();

        assert_eq!(
            restored_deployment.workload_id,
            WorkloadId::new("workload-v2")
        );

        assert_eq!(restored_deployment.generation, 2);

        let restored_instance = state
            .instance(&InstanceId::new("deployment-01-replica-1"))
            .unwrap();

        assert_eq!(
            restored_instance.workload_id,
            WorkloadId::new("workload-v1")
        );
    }
}
