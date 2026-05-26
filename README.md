# Hermes

Hermes is a Rust-first Bittensor miner and validator operations stack built from the requirements in `index.txt`. It combines a native Yuma Consensus engine, a Bun API gateway, Kubernetes and Helm deployment assets, Ansible bare-metal provisioning, and Prometheus or Grafana observability for low-latency subnet operations.

## What Is Implemented

- A Rust workspace with a configurable YC3-style consensus engine and deployment planner.
- A Bun gateway for low-latency request routing, live context retrieval against SurrealDB or Cloudflare Vectorize, reranking, bounded prompt assembly, Chutes dispatch preparation, and readiness endpoints.
- Helm templates for gateway and validator workloads, plus Vault-based hotkey injection.
- Ansible roles for bare-metal preparation, NVIDIA setup, Kubernetes bootstrap, and telemetry rollout.
- Prometheus alert rules and a Grafana dashboard for GPU and Subtensor health tracking.
- Security and operations runbooks for coldkey handling, confidential compute, and incident response.

## Repository Layout

```text
Hermes/
├── configs/
├── crates/
│   ├── hermes-core/
│   └── hermes-node/
├── deploy/
│   ├── docker/
│   └── helm/
├── docs/
├── fixtures/
├── gateway/
├── ops/
│   ├── ansible/
│   ├── grafana/
│   └── prometheus/
└── index.txt
```

## Local Commands

Rust:

```bash
cargo test --workspace
cargo run -p hermes-node -- validate-config configs/hermes.example.json
cargo run -p hermes-node -- plan configs/hermes.example.json
cargo run -p hermes-node -- score-snapshot fixtures/epoch-snapshot.json --config configs/hermes.example.json --pretty
cargo run -p hermes-node -- serve configs/hermes.example.json --interval-seconds 60 --once
```

Gateway:

```bash
cd gateway
bun install
bun run typecheck
bun run test
bun run dev
```

## Production Notes

- Dedicated bare-metal GPU nodes are treated as a hard requirement.
- `configs/hermes.example.json` uses HashiCorp Vault hotkey delivery by default.
- Confidential compute is enforced through TDX or SEV-oriented scheduling hints and runbook guidance.
- The gateway context path assumes Redis, PostgreSQL, and either SurrealDB or Cloudflare Vectorize are reachable as external services.
