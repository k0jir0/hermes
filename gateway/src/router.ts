import { GatewayConfig, GatewayHealth, RoutePlan, RouteRequest } from "./types.js";

function describeVectorBackend(config: GatewayConfig): string {
  return config.vectorBackend === "cloudflare_vectorize" ? "Cloudflare Vectorize" : "SurrealDB";
}

function assertAllowedSubnet(request: RouteRequest, config: GatewayConfig): void {
  if (!config.allowedSubnets.includes(request.subnetId)) {
    throw new Error(`subnet ${request.subnetId} is not allowed by this gateway`);
  }
}

function buildPoolName(request: RouteRequest, prefix: string): string {
  const region = request.region ?? "local";
  return `${prefix}-${request.preferredGpuClass}-${region}`;
}

export function buildRoutePlan(request: RouteRequest, config: GatewayConfig): RoutePlan {
  assertAllowedSubnet(request, config);

  const reasons: string[] = [];
  const timeoutMs = Math.max(50, Math.min(config.requestTimeoutMs, request.latencyBudgetMs || config.requestTimeoutMs));

  if (request.operation === "context") {
    reasons.push(`Context retrieval is routed to the ${describeVectorBackend(config)} backend.`);
    return {
      target: {
        kind: "vector",
        baseUrl: config.vectorUrl,
        pool: "vector-search",
        headers: {
          "x-hermes-region": config.region,
          "x-hermes-subnet": String(request.subnetId),
        },
      },
      cacheTtlSeconds: 10,
      requiresWarmGpu: false,
      timeoutMs,
      reasons,
    };
  }

  if (request.confidential) {
    reasons.push("Confidential compute requested, routing to attested hardware.");
    return {
      target: {
        kind: "confidential_bare_metal",
        baseUrl: config.confidentialBaseUrl,
        pool: buildPoolName(request, "confidential"),
        headers: {
          "x-hermes-region": config.region,
          "x-hermes-attestation": "required",
          "x-hermes-subnet": String(request.subnetId),
        },
      },
      cacheTtlSeconds: 2,
      requiresWarmGpu: true,
      timeoutMs,
      reasons,
    };
  }

  if (request.operation === "weights" || request.latencyBudgetMs <= 150) {
    reasons.push("Low-latency scoring traffic remains on the bare-metal validator pool.");
    return {
      target: {
        kind: "bare_metal",
        baseUrl: config.bareMetalBaseUrl,
        pool: buildPoolName(request, "validator"),
        headers: {
          "x-hermes-region": config.region,
          "x-hermes-subnet": String(request.subnetId),
        },
      },
      cacheTtlSeconds: 1,
      requiresWarmGpu: true,
      timeoutMs,
      reasons,
    };
  }

  reasons.push("Burst inference request is eligible for Chutes dispatch.");
  return {
    target: {
      kind: "chutes",
      baseUrl: config.chutesBaseUrl,
      pool: buildPoolName(request, "burst"),
      headers: {
        "x-hermes-region": config.region,
        "x-hermes-subnet": String(request.subnetId),
      },
    },
    cacheTtlSeconds: 3,
    requiresWarmGpu: true,
    timeoutMs,
    reasons,
  };
}

export function buildGatewayHealth(config: GatewayConfig): GatewayHealth {
  return {
    ok: true,
    gatewayId: config.gatewayId,
    services: [
      { name: "redis", endpoint: config.redisUrl, mode: "required" },
      { name: "postgres", endpoint: config.postgresUrl, mode: "required" },
      { name: `vector:${config.vectorBackend}`, endpoint: config.vectorUrl, mode: "required" },
      { name: "chutes", endpoint: config.chutesBaseUrl, mode: "optional" },
    ],
    telemetry: ["prometheus", "grafana", "dcgm-exporter"],
  };
}
