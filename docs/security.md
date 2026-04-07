# Hermes Security Model

Hermes treats keys and host trust as first-class operational risks.

## Coldkeys

- Keep coldkeys offline at all times.
- Use a hardware wallet or air-gapped signing machine for coldkey actions.
- Never mount coldkeys into Kubernetes, containers, or CI.

## Hotkeys

- Hotkeys are injected at runtime from Kubernetes Secrets or HashiCorp Vault.
- The default example uses Vault so rotations can happen without rebuilding images.
- Containers read hotkeys from mounted files rather than baked environment defaults.

## Confidential Compute

- Sensitive validator workloads are scheduled onto Intel TDX or AMD SEV capable nodes.
- The Helm chart exposes node selector hooks so confidential compute remains enforceable at scheduling time.
- The gateway distinguishes confidential traffic from standard low-latency routing.

## Platform Hardening

- Dedicated bare-metal nodes only.
- Local NVMe only for hot validator state.
- Strict port mapping and static IP management for predictable peering.
