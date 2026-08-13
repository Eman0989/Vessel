CREATE TABLE vessel_nodes (
    id TEXT PRIMARY KEY,
    payload JSONB NOT NULL,
    endpoint TEXT,
    last_seen_ms BIGINT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE vessel_workloads (
    id TEXT PRIMARY KEY,
    payload JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE vessel_deployments (
    id TEXT PRIMARY KEY,
    workload_id TEXT NOT NULL
        REFERENCES vessel_workloads(id),
    payload JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX vessel_deployments_workload_id_idx
    ON vessel_deployments(workload_id);

CREATE TABLE vessel_instances (
    id TEXT PRIMARY KEY,
    deployment_id TEXT NOT NULL
        REFERENCES vessel_deployments(id),
    workload_id TEXT NOT NULL
        REFERENCES vessel_workloads(id),
    node_id TEXT
        REFERENCES vessel_nodes(id),
    payload JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX vessel_instances_deployment_id_idx
    ON vessel_instances(deployment_id);

CREATE INDEX vessel_instances_workload_id_idx
    ON vessel_instances(workload_id);

CREATE INDEX vessel_instances_node_id_idx
    ON vessel_instances(node_id);
