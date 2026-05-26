# Hermes Architecture

Hermes is organized as a low-latency Bittensor operations monorepo with two execution planes.

## Native Validator Plane

The Rust workspace is the control center for validator and miner operations:

- `hermes-core` models the deployment contract, security posture, storage interfaces, and YC3 consensus math.
- `hermes-node` provides a CLI for validating cluster configuration, producing a deployment plan, and scoring snapshot inputs against the YC3 engine.
- The consensus implementation follows the public Yuma formulas for kappa-based clipping, penalized bonds, EMA bonds, miner incentives, and validator rewards.

## Gateway Plane

The Bun gateway is the lightweight ingress layer that keeps hot-path routing out of the Rust validator process.

- Low-latency or weight-sensitive traffic stays on the bare-metal validator pool.
- Confidential workloads are pinned to TDX or SEV-backed nodes.
- Context queries execute a bounded retrieval harness: route to the configured vector backend, fetch candidates, rerank them, and assemble a prompt-safe evidence packet from the larger distributed corpus.
- Overflow or burst inference can be handed to Chutes while preserving subnet and region headers.

## Data Plane

Hermes assumes three distinct data services:

- Redis for short-lived rate limits, routing caches, and fast inter-process state.
- PostgreSQL for epochs, pruning history, and validator reward analytics.
- SurrealDB or Cloudflare Vectorize for embedding-backed context retrieval.

## Operations Plane

- Kubernetes schedules gateway and validator workloads on dedicated GPU hosts.
- Helm captures runtime configuration, external dependencies, and Vault-based hotkey delivery.
- Ansible prepares the hardware, network, NVIDIA stack, and telemetry prerequisites.
- Prometheus, Grafana, and DCGM exporter provide cluster health, GPU thermals, and network visibility.
