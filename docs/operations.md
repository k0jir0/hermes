# Hermes Operations

## Provisioning Flow

1. Use the Ansible roles under `ops/ansible` to tune the bare-metal nodes, install GPU drivers, and bootstrap Kubernetes.
2. Create or sync the Vault secret at `kv/data/hermes/hotkey` before deploying validator pods.
3. Apply or package the Helm chart under `deploy/helm/hermes` with environment-specific values.
4. Confirm Prometheus is scraping both Hermes workloads and the NVIDIA DCGM exporter.

## Suggested Rollout Sequence

1. Bring up Redis, PostgreSQL, and SurrealDB on the low-latency storage fabric.
2. Roll out the gateway deployment and validate `GET /healthz`.
3. Roll out the validator stateful set and score a fixture epoch using `hermes-node score-snapshot`.
4. Enable autoscaling only after stable GPU temperature and queue latency metrics are observed.

## Runtime Checks

- Verify 10 Gbps or better throughput and zero packet loss between validator nodes.
- Watch GPU memory pressure and thermals on the Grafana dashboard.
- Alert on Subtensor disconnects, routing latency spikes, or Vault mount failures.
