import { GatewayConfig } from "./types.js";

function parseNumber(value: string | undefined, fallback: number): number {
  if (!value) {
    return fallback;
  }

  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

export function loadGatewayConfig(env: NodeJS.ProcessEnv = process.env): GatewayConfig {
  const allowedSubnets = (env.HERMES_ALLOWED_SUBNETS ?? "1,8,19")
    .split(",")
    .map((value) => Number(value.trim()))
    .filter((value) => Number.isInteger(value));

  return {
    port: parseNumber(env.PORT, 3000),
    gatewayId: env.HERMES_GATEWAY_ID ?? "gateway-1",
    region: env.HERMES_REGION ?? "us-east-1",
    bareMetalBaseUrl: env.HERMES_BARE_METAL_URL ?? "http://validator-pool.hermes.svc.cluster.local:8080",
    confidentialBaseUrl:
      env.HERMES_CONFIDENTIAL_URL ?? "http://validator-confidential.hermes.svc.cluster.local:8080",
    chutesBaseUrl: env.HERMES_CHUTES_URL ?? "https://api.chutes.ai/v1",
    vectorUrl: env.HERMES_VECTOR_URL ?? "http://surrealdb.hermes.svc.cluster.local:8000",
    redisUrl: env.HERMES_REDIS_URL ?? "redis://redis.hermes.svc.cluster.local:6379",
    postgresUrl:
      env.HERMES_POSTGRES_URL ?? "postgres://hermes:hermes@postgres.hermes.svc.cluster.local:5432/hermes",
    requestTimeoutMs: parseNumber(env.HERMES_REQUEST_TIMEOUT_MS, 1200),
    allowedSubnets,
  };
}
