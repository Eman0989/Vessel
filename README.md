<div align="center">

# ⚓ VESSEL

### Distributed WebAssembly Execution Fabric

**Portable compute. Capability-constrained execution. Deterministic scheduling. Automatic recovery. Safe rollouts.**

<img
  src="https://readme-typing-svg.demolab.com?font=JetBrains+Mono&weight=600&size=22&duration=2800&pause=900&color=58A6FF&center=true&vCenter=true&width=900&lines=Run+portable+WebAssembly+workloads;Schedule+across+distributed+workers;Enforce+WASI+capability+boundaries;Recover+automatically+from+worker+failure;Roll+out+new+revisions+safely"
  alt="Animated VESSEL capability banner"
/>

<br />

![Rust](https://img.shields.io/badge/Rust-2024-18181b?style=for-the-badge&logo=rust)
![WebAssembly](https://img.shields.io/badge/WebAssembly-Component%20Model-18181b?style=for-the-badge&logo=webassembly)
![Wasmtime](https://img.shields.io/badge/Wasmtime-runtime-18181b?style=for-the-badge)
![Axum](https://img.shields.io/badge/Axum-control%20plane-18181b?style=for-the-badge)
![PostgreSQL](https://img.shields.io/badge/PostgreSQL-persistence-18181b?style=for-the-badge&logo=postgresql)
![License](https://img.shields.io/badge/License-Apache--2.0-18181b?style=for-the-badge)

</div>

---

## 🌐 Project Overview

**VESSEL** is a distributed WebAssembly execution fabric written in Rust.

It is designed to run portable workloads across a cluster of workers while providing the control-plane behavior expected from a real distributed execution system:

- workload isolation
- deterministic scheduling
- resource accounting
- content-addressed artifacts
- worker registration and heartbeats
- failure detection
- automatic recovery
- durable control-plane state
- rolling workload revisions
- bounded service-to-service networking

Rather than treating WebAssembly execution as a single runtime call, VESSEL builds an orchestration layer around it.

```text
WebAssembly Artifact
        ↓
Content-Addressed Registry
        ↓
Workload Definition
        ↓
Deployment Desired State
        ↓
Control Plane
        ↓
Deterministic Scheduler
        ↓
Worker Assignment
        ↓
Verified Artifact Fetch
        ↓
Capability-Constrained Wasmtime Runtime
        ↓
Execution Result
```

VESSEL is built as a systems-engineering project focused on the boundaries between **runtime isolation, scheduling, state reconciliation, networking, persistence, and failure recovery**.

> **Status:** active development. Core runtime, worker, scheduler, registry, control-plane, persistence, recovery, CLI, and rolling-deployment functionality are implemented. Canary releases, autoscaling, observability, and the operations dashboard are the next major milestones.

---

## 🚀 Core Features

| Feature | Description |
| --- | --- |
| ⚙️ **Wasmtime Runtime** | Executes WebAssembly workloads using Wasmtime |
| 🧩 **Component Model Support** | Executes WebAssembly components in addition to core modules |
| 📜 **WIT Workload Contract** | Defines an explicit guest/host workload interface |
| ⛽ **Fuel Limits** | Stops workloads that exceed configured execution fuel |
| 🧠 **Memory Limits** | Enforces memory ceilings for guest execution |
| ⏱️ **Execution Timeouts** | Uses runtime deadlines to stop long-running workloads |
| 🔐 **Capability Policies** | Applies explicit WASI permissions instead of ambient host access |
| 📁 **Filesystem Preopens** | Exposes selected guest paths with controlled permissions |
| 🌐 **Network Capabilities** | Controls WASI DNS and TCP access |
| 📦 **Content-Addressed Registry** | Stores WebAssembly artifacts using SHA-256 identity |
| ✅ **Artifact Verification** | Workers verify artifact content before execution |
| 🖥️ **Worker Service** | Provides execution, health, draining, capacity, and lifecycle behavior |
| 📡 **Worker Registration** | Workers register with the control plane and advertise endpoints |
| 💓 **Heartbeats** | Workers continuously publish node state and capacity |
| 🧭 **Deterministic Scheduler** | Selects eligible nodes with deterministic tie-breaking |
| 🧮 **Resource Accounting** | Reserves and releases CPU, memory, and instance capacity |
| 🧠 **Authoritative Control State** | Tracks nodes, workloads, deployments, and instances |
| 🔁 **Desired-State Reconciliation** | Converges deployments toward requested replica state |
| 🚪 **Invocation Gateway** | Routes workload invocation to the assigned worker |
| 🚨 **Failure Detection** | Detects workers that stop sending heartbeats |
| ♻️ **Automatic Recovery** | Marks affected instances lost and reconciles replacements |
| 🗄️ **PostgreSQL Persistence** | Saves and restores durable control-plane snapshots |
| 🌍 **Bounded Networking** | Applies explicit connect and request timeouts between services |
| 🧰 **Operator CLI** | Exposes cluster, workload, deployment, instance, and artifact commands |
| 🚢 **Rolling Deployments** | Replaces workload revisions one replica at a time |
| 🧪 **Extensive Testing** | Covers runtime, scheduling, recovery, persistence, networking, and rollout behavior |

---

## 🧩 System Architecture

```mermaid
flowchart LR
    USER[Developer / Operator]
    CLI[VESSEL CLI]

    CONTROL[Rust + Axum Control Plane]
    SCHED[Deterministic Scheduler]
    DB[(PostgreSQL)]
    REG[Content-Addressed Registry]

    subgraph CLUSTER[Worker Cluster]
        W1[Worker A]
        W2[Worker B]
        WN[Worker N]
    end

    POLICY[Capability Policy]
    RUNTIME[Wasmtime Runtime]
    WASM[WebAssembly Workload]

    USER --> CLI
    CLI -->|HTTP| CONTROL

    CONTROL --> SCHED
    CONTROL --> DB

    W1 -->|register + heartbeat| CONTROL
    W2 -->|register + heartbeat| CONTROL
    WN -->|register + heartbeat| CONTROL

    CONTROL -->|assignment / invocation| W1
    CONTROL -->|assignment / invocation| W2
    CONTROL -->|assignment / invocation| WN

    W1 --> REG
    W2 --> REG
    WN --> REG

    REG --> WASM
    WASM --> RUNTIME
    POLICY --> RUNTIME

    W1 --> RUNTIME
    W2 --> RUNTIME
    WN --> RUNTIME
```

### Architectural boundaries

- The **control plane** owns authoritative cluster and deployment state.
- The **scheduler** decides where pending workloads should run.
- **Workers** own execution admission and local resource accounting.
- The **artifact registry** provides immutable content-addressed workload bytes.
- **Wasmtime** executes workloads under explicit runtime limits.
- The **policy layer** defines which WASI capabilities a workload receives.
- **PostgreSQL** provides optional durable recovery of control-plane state.
- Service-to-service requests use explicit timeout boundaries.

---

## 🧠 Desired-State Reconciliation

VESSEL models deployments as **desired state**.

A deployment does not simply issue a one-time instruction to create processes. Instead, reconciliation repeatedly compares requested state with actual state.

```text
Deployment
desired replicas = N
target workload  = revision X
        ↓
Reconciliation
        ↓
Inspect active instances
        ↓
Create / cancel / replace replicas
        ↓
Schedule pending instances
        ↓
Update deployment state
```

The reconciler currently handles:

- initial deployment creation
- scale-up
- deterministic scale-down
- capacity shortages
- pending replica retries
- replacement of terminal replicas
- worker-failure recovery
- rolling workload revision replacement

This reconciliation model forms the basis for upcoming **canary releases, rollback, autoscaling, and deployment visualization**.

---

## 🚢 Rolling Deployments

VESSEL implements bounded rolling deployment behavior.

A deployment points at its **current target workload revision**, while every existing instance keeps the workload revision from which it was originally created.

That makes old and new replicas distinguishable during reconciliation.

```text
Initial state

[ v1 ][ v1 ]

Rollout target → v2

Pass 1

[ v1 ][ v2 ]

Pass 2

[ v2 ][ v2 ]

Deployment → Healthy
```

### Rollout safety properties

- only **one old replica** is replaced during a normal reconciliation pass
- old-revision replicas are removed deterministically
- capacity belonging to a cancelled replica is released correctly
- a pending target replica blocks further destructive replacement
- repeated reconciliation cannot drain every old replica if the new revision cannot schedule
- scale-down during rollout prefers previous-revision replicas
- rollout completes only when the desired replicas are on the target revision
- persisted old-revision instances remain restorable after the target revision changes

The rollout algorithm intentionally starts conservative before introducing configurable surge, availability, canary, and rollback policies.

---

## ♻️ Failure Detection & Automatic Recovery

Workers periodically send heartbeats containing their current state and capacity.

Default behavior:

```text
Worker heartbeat interval     5 seconds
Failure timeout              15 seconds
Failure check interval        1 second
```

The recovery flow is:

```text
Worker stops heartbeating
        ↓
Failure timeout reached
        ↓
Worker marked Unreachable
        ↓
Assigned workload instances marked Lost
        ↓
Affected deployments identified
        ↓
Deployment reconciliation runs
        ↓
Replacement replicas scheduled
```

Recovery behavior is designed to be idempotent so repeatedly running the failure detector does not continuously mutate already-recovered state.

---

## 🔐 Capability-Based Runtime Security

WebAssembly workloads do not automatically inherit unrestricted host access.

VESSEL exposes capabilities explicitly.

```text
Workload Execution
│
├── Fuel budget
├── Memory limit
├── Execution timeout
│
└── WASI capability policy
    │
    ├── Environment variables
    │
    ├── Filesystem preopens
    │   ├── Read-only
    │   └── Read-write
    │
    ├── DNS access
    │
    └── TCP access
```

The default model is restrictive: additional access is granted deliberately rather than inherited implicitly from the host process.

> VESSEL is still under active development and should not yet be considered a hardened production sandbox.

---

## 📦 Content-Addressed Artifact Registry

Artifacts are identified using their content rather than upload identity.

```text
WebAssembly bytes
        ↓
SHA-256 digest
        ↓
Artifact Registry
        ↓
sha256:...
        ↓
Workload reference
        ↓
Worker download
        ↓
Digest verification
        ↓
Worker cache
        ↓
Execution
```

This provides deterministic artifact identity and natural deduplication.

Current registry endpoints:

| Method | Endpoint | Purpose |
| --- | --- | --- |
| `GET` | `/health` | Registry health |
| `POST` | `/v1/artifacts` | Upload WebAssembly bytes |
| `GET` | `/v1/artifacts/{digest}` | Download an artifact |

---

## 🧭 Deterministic Scheduling

The scheduler filters out workers that cannot accept a workload and then ranks eligible nodes deterministically.

Scheduling considers:

- node lifecycle status
- CPU capacity
- memory capacity
- maximum instance capacity
- current resource allocation
- current instance load
- deterministic node-ID tie-breaking

```text
Pending Instance
      ↓
Requested Resources
      ↓
Eligible Worker Filter
      ↓
Capacity Ranking
      ↓
Deterministic Tie-Break
      ↓
Selected Worker
      ↓
Resource Reservation
```

Deterministic placement makes scheduling behavior easier to test, reproduce, and reason about.

---

## 🧮 Resource Accounting

Node state tracks both total capacity and currently reserved resources.

VESSEL currently accounts for:

```text
CPU millicores
Memory bytes
Allocated instance count
```

Assignment reserves capacity.

Terminal instance transitions release capacity.

Failed assignment paths roll reservations back instead of leaving leaked capacity behind.

---

## 🗄️ Persistent Control-Plane State

When `DATABASE_URL` is configured, VESSEL persists control-plane snapshots to PostgreSQL.

Persisted state includes:

- nodes
- worker endpoint metadata
- heartbeat timestamps
- workloads
- deployments
- instances
- workload revision identity

On startup:

```text
Control Plane
     ↓
Connect PostgreSQL
     ↓
Run migrations
     ↓
Load persisted snapshot
     ↓
Restore cluster state
     ↓
Start normal control loops
```

Persistence restoration deliberately supports old-revision replicas that still exist during an in-progress rolling deployment.

Without `DATABASE_URL`, VESSEL runs using in-memory control state.

---

## 🌍 Bounded Service Networking

VESSEL currently uses HTTP between services.

Production request paths are bounded with explicit connect and request timeouts.

```text
Worker → Control Plane
Control Plane → Worker
Worker → Registry
```

Each path has independently configurable timeout values.

This avoids allowing a stalled downstream service to block cluster operations indefinitely.

---

## 🧰 CLI

The `vessel` binary provides an operator interface over the current HTTP APIs.

### Cluster inspection

```bash
vessel health
vessel nodes
vessel workloads
vessel deployments
vessel instances
```

### Workloads

```bash
vessel workload create \
  --id workload-01 \
  --name example \
  --artifact sha256:...
```

### Deployments

```bash
vessel deployment create \
  --id deployment-01 \
  --workload workload-01 \
  --replicas 2

vessel deployment scale deployment-01 \
  --replicas 4

vessel deployment reconcile deployment-01
```

### Instances

```bash
vessel instance create ...
vessel instance assign ...
vessel instance schedule ...
vessel instance transition ...
vessel instance invoke ...
```

### Artifacts

```bash
vessel artifact push workload.wasm
```

### Endpoint overrides

```bash
vessel \
  --control-url http://127.0.0.1:7000 \
  nodes
```

```bash
vessel \
  --registry-url http://127.0.0.1:7002 \
  artifact push workload.wasm
```

The rollout API already exists at the control-plane layer. Dedicated CLI rollout ergonomics will be added as deployment tooling expands.

---

## 🛠️ Technology Stack

| Technology | Role |
| --- | --- |
| **Rust 2024** | Core systems implementation |
| **Tokio** | Async runtime, timers, networking, and service loops |
| **Axum** | HTTP APIs for control plane, worker, and registry |
| **Wasmtime** | WebAssembly and Component Model execution |
| **wasmtime-wasi** | WASI host integration |
| **WIT** | Guest/host workload contract |
| **Serde** | Domain serialization |
| **SQLx** | PostgreSQL persistence and migrations |
| **PostgreSQL** | Durable control-plane state |
| **Reqwest** | Service-to-service HTTP clients |
| **Clap** | Operator CLI |
| **SHA-256** | Artifact addressing and verification |
| **Tracing** | Observability foundation |
| **Cargo** | Workspace management, testing, and linting |

---

## 📁 Project Structure

```text
Vessel/
│
├── crates/
│   │
│   ├── vessel-core/
│   │   └── Domain model, typed IDs, resources, lifecycle state
│   │
│   ├── vessel-runtime/
│   │   └── Wasmtime execution, Component Model, WIT, runtime limits
│   │
│   ├── vessel-policy/
│   │   └── Capability policy model
│   │
│   ├── vessel-scheduler/
│   │   └── Deterministic resource-aware scheduling
│   │
│   ├── vessel-registry/
│   │   └── Content-addressed artifact storage and HTTP API
│   │
│   ├── vessel-worker/
│   │   └── Worker execution service, artifact cache, cluster client
│   │
│   ├── vessel-control/
│   │   └── Control state, reconciliation, persistence, recovery, gateway
│   │
│   ├── vessel-cli/
│   │   └── Operator command-line client
│   │
│   └── vessel-telemetry/
│       └── Observability foundation
│
├── wit/
│   └── vessel-workload.wit
│
├── Cargo.toml
├── Cargo.lock
└── README.md
```

The workspace deliberately separates runtime, policy, scheduling, registry, worker, control-plane, and operator concerns rather than combining them into a single service.

---

## ⚙️ Getting Started

### Prerequisites

Install:

- Git
- Rust
- Cargo
- `rustfmt`
- `clippy`
- PostgreSQL if persistent state is required

### Clone

```bash
git clone https://github.com/Eman0989/Vessel.git
cd Vessel
```

### Build

```bash
cargo build --workspace
```

### Start the control plane

```bash
cargo run -p vessel-control
```

Default:

```text
http://127.0.0.1:7000
```

### Start the registry

Open another terminal:

```bash
cargo run -p vessel-registry
```

Default:

```text
http://127.0.0.1:7002
```

### Start a worker

Open another terminal:

```bash
cargo run -p vessel-worker
```

Default:

```text
http://127.0.0.1:7001
```

The worker automatically registers with the default control-plane endpoint and uses the default registry endpoint.

### Inspect the cluster

```bash
cargo run -p vessel-cli --bin vessel -- health
cargo run -p vessel-cli --bin vessel -- nodes
```

---

## 🗄️ PostgreSQL Configuration

Persistence is optional.

To enable it:

```bash
export DATABASE_URL='postgres://USER:PASSWORD@127.0.0.1:5432/vessel'
cargo run -p vessel-control
```

The control plane will:

1. connect to PostgreSQL
2. run migrations
3. restore the saved control-state snapshot
4. periodically persist updated state

---

## 🔧 Environment Configuration

### Control plane

| Variable | Default | Purpose |
| --- | --- | --- |
| `VESSEL_CONTROL_ADDR` | `127.0.0.1:7000` | Bind address |
| `DATABASE_URL` | unset | Enables PostgreSQL persistence |
| `VESSEL_FAILURE_TIMEOUT_MS` | `15000` | Worker failure timeout |
| `VESSEL_FAILURE_CHECK_INTERVAL_MS` | `1000` | Failure detector interval |
| `VESSEL_PERSIST_INTERVAL_MS` | `1000` | Persistence interval |
| `VESSEL_GATEWAY_CONNECT_TIMEOUT_MS` | `2000` | Control → worker connection timeout |
| `VESSEL_GATEWAY_REQUEST_TIMEOUT_MS` | `30000` | Control → worker request timeout |

### Worker

| Variable | Default | Purpose |
| --- | --- | --- |
| `VESSEL_NODE_ID` | `worker-local` | Worker identifier |
| `VESSEL_WORKER_ADDR` | `127.0.0.1:7001` | Worker bind address |
| `VESSEL_WORKER_URL` | worker address | Advertised worker endpoint |
| `VESSEL_CONTROL_URL` | `http://127.0.0.1:7000` | Control-plane endpoint |
| `VESSEL_REGISTRY_URL` | `http://127.0.0.1:7002` | Registry endpoint |
| `VESSEL_HEARTBEAT_INTERVAL_MS` | `5000` | Heartbeat interval |
| `VESSEL_CLUSTER_CONNECT_TIMEOUT_MS` | `2000` | Worker → control connection timeout |
| `VESSEL_CLUSTER_REQUEST_TIMEOUT_MS` | `5000` | Worker → control request timeout |
| `VESSEL_REGISTRY_CONNECT_TIMEOUT_MS` | `2000` | Worker → registry connection timeout |
| `VESSEL_REGISTRY_REQUEST_TIMEOUT_MS` | `30000` | Worker → registry request timeout |

### Registry

| Variable | Default | Purpose |
| --- | --- | --- |
| `VESSEL_REGISTRY_ADDR` | `127.0.0.1:7002` | Registry bind address |

---

## 🧪 Quality Gates

VESSEL is developed against a strict workspace validation gate:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

Testing currently covers:

- core WebAssembly execution
- invalid module rejection
- Component Model execution
- WIT compatibility
- fuel exhaustion
- memory limits
- execution deadlines
- WASI environment boundaries
- filesystem permissions
- DNS and TCP capability enforcement
- artifact hashing
- artifact deduplication
- artifact verification
- worker execution admission
- resource accounting
- worker draining
- worker registration
- heartbeats
- network timeouts
- deterministic scheduler behavior
- capacity exhaustion
- deployment scale-up
- deployment scale-down
- deployment reconciliation
- invocation forwarding
- worker failure detection
- lost-instance recovery
- PostgreSQL persistence
- rollout persistence restoration
- rolling deployment progression
- stalled rollout safety
- rollout-aware scale-down
- end-to-end rolling deployment through HTTP

---

## 🧭 Development Roadmap

```text
✅ Workspace foundation
✅ Core domain model
✅ WebAssembly runtime
✅ Component Model
✅ WIT workload contract
✅ Fuel + memory + timeout limits
✅ Capability security
✅ Worker execution service
✅ Control plane
✅ Worker registration + heartbeats
✅ Content-addressed artifact registry
✅ Artifact verification + cache
✅ Deterministic scheduler
✅ Desired-state reconciliation
✅ Invocation gateway
✅ Failure detection
✅ Automatic recovery
✅ PostgreSQL persistence
✅ Bounded service networking
✅ Operator CLI
✅ Rolling deployments

→ Canary releases + rollback
→ Autoscaling
→ Observability
→ React operations dashboard
→ Real-time cluster visualization
→ Failure animation
→ Deployment UI
→ Multi-node Docker Compose laboratory
→ Load testing
→ Security hardening
→ Final deterministic demo
→ Documentation
→ v0.1.0 release
```

---

## 🎯 Engineering Focus

VESSEL is deliberately structured to make systems-engineering decisions visible.

### Deterministic behavior

Scheduling and reconciliation use deterministic ordering wherever possible, making distributed state transitions easier to reproduce and test.

### Explicit resource ownership

Worker capacity is reserved during assignment and released through lifecycle transitions rather than being inferred indirectly.

### Immutable revision identity

Instances retain the workload revision from which they were created, which allows safe reconciliation across deployment changes.

### Failure as a first-class state

Unreachable workers and lost workload instances are represented explicitly instead of disappearing from control state.

### Bounded networking

Internal service calls use explicit timeout behavior rather than depending on unbounded network waits.

### Idempotent reconciliation

Repeated reconciliation is expected and must remain safe.

### Persistence across partial operations

The control plane can restore state even when a rolling deployment was in progress when the process stopped.

---

## 💬 Interesting Interview Discussion Areas

If you are reviewing VESSEL as a systems-engineering project, the most interesting implementation areas are:

- WebAssembly Component Model integration
- WIT guest/host contracts
- capability-based WASI exposure
- deterministic scheduler design
- resource reservation and rollback
- desired-state reconciliation
- rollout safety invariants
- worker heartbeat protocol
- failure detector behavior
- automatic workload recovery
- persistence boundaries
- service-to-service timeout design
- separation between control-plane state and worker execution

---

## 📜 License

VESSEL is licensed under **Apache-2.0**.

---

<div align="center">

### ⚓ VESSEL

**Portable execution. Distributed control.**

Built with Rust, WebAssembly, and a focus on predictable distributed-system behavior.

</div>