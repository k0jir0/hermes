# Hermes

Hermes is a Rust-first Bittensor miner and validator operations stack built from the requirements in `index.txt`. Current `main` combines a native Yuma Consensus engine, a Bun API gateway, Kubernetes and Helm deployment assets, Ansible bare-metal provisioning, and Prometheus or Grafana observability for low-latency subnet operations. The gateway now treats large context as searchable external memory: it plans a bounded working set, executes live retrieval against SurrealDB or Cloudflare Vectorize, reranks candidates, and assembles prompt-safe evidence packets for downstream inference.

## What Is Implemented

- A Rust workspace with a configurable YC3-style consensus engine, deployment planner, and explicit repository topology for vector, cache, and history backends.
- A Bun gateway with `/healthz`, `/readyz`, `/v1/route`, `/v1/context/query`, and `/v1/chutes/dispatch`.
- A context harness that converts a distributed corpus into a bounded working-set plan with corpus, budget, throughput, and prompt-slack calculations.
- Live retrieval against SurrealDB or Cloudflare Vectorize, reranking, timeout enforcement, and bounded prompt assembly.
- Fail-fast gateway configuration validation for required service endpoints, allowed subnets, vector backend settings, and request timeout values.
- Helm templates for gateway and validator workloads, plus Vault-based hotkey injection and vector-backend env or secret wiring.
- Ansible roles for bare-metal preparation, NVIDIA setup, Kubernetes bootstrap, and telemetry rollout.
- Prometheus alert rules and a Grafana dashboard for GPU and Subtensor health tracking.
- Security and operations runbooks for coldkey handling, confidential compute, and incident response.
- `paper1.txt`, which captures the design premise that Hermes should treat a very large corpus as searchable external memory rather than direct-attention prompt length.

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
├── index.txt
└── paper1.txt
```

## Local Commands

Rust:

```bash
cargo test --workspace
cargo run -p hermes-node -- control
cargo run -p hermes-node -- validate-config configs/hermes.example.json
cargo run -p hermes-node -- plan configs/hermes.example.json
cargo run -p hermes-node -- score-snapshot fixtures/epoch-snapshot.json --config configs/hermes.example.json --pretty
cargo run -p hermes-node -- serve configs/hermes.example.json --interval-seconds 60 --once
```

The numbered operator control plane is now available via `cargo run -p hermes-node -- control`.

Windows note:

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup override set stable-x86_64-pc-windows-gnu
```

If Visual Studio Build Tools are not installed, Hermes expects a working GNU Windows linker toolchain. A MinGW distribution such as WinLibs must be installed and available on `PATH` for local Rust workspace builds on that setup.

Gateway:

```bash
cd gateway
npm install
npm run typecheck
npm test
bun run dev
```

`npm run typecheck` and `npm test` are enough for static validation and gateway tests. Bun is required when you want to run the live gateway process.

Current `main` has been validated with:

```bash
cargo test --workspace
cd gateway && npm run typecheck && npm test
helm template hermes ./deploy/helm/hermes
```

## Gateway Configuration

`loadGatewayConfig` validates these inputs at startup:

- Required core settings: `PORT`, `HERMES_GATEWAY_ID`, `HERMES_REGION`, `HERMES_ALLOWED_SUBNETS`, `HERMES_REQUEST_TIMEOUT_MS`
- Required service endpoints: `HERMES_BARE_METAL_URL`, `HERMES_CONFIDENTIAL_URL`, `HERMES_CHUTES_URL`, `HERMES_VECTOR_URL`, `HERMES_REDIS_URL`, `HERMES_POSTGRES_URL`
- SurrealDB retrieval: `HERMES_VECTOR_BACKEND=surrealdb` plus `HERMES_VECTOR_NAMESPACE`, `HERMES_VECTOR_DATABASE`, and `HERMES_VECTOR_TABLE`
- Cloudflare Vectorize retrieval: `HERMES_VECTOR_BACKEND=cloudflare_vectorize` plus `HERMES_VECTOR_API_TOKEN`

## Context Query Example

```bash
curl -X POST http://localhost:3000/v1/context/query \
	-H "content-type: application/json" \
	-d '{
		"subnetId": 19,
		"operation": "context",
		"latencyBudgetMs": 1200,
		"confidential": false,
		"preferredGpuClass": "h100",
		"query": "summarize recent validator anomalies"
	}'
```

The response includes the selected route, active vector backend, computed `harnessPlan`, and the final retrieval or prompt assembly result.

## Production Notes

- Dedicated bare-metal GPU nodes are treated as a hard requirement.
- `configs/hermes.example.json` uses HashiCorp Vault hotkey delivery by default.
- Confidential compute is enforced through TDX or SEV-oriented scheduling hints and runbook guidance.
- The gateway context path assumes Redis, PostgreSQL, and either SurrealDB or Cloudflare Vectorize are reachable as external services.
- The Helm gateway deployment will fail template rendering if Cloudflare Vectorize is selected without an auth secret.
