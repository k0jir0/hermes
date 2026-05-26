import { GatewayConfig, VectorBackend } from "./types.js";

export class GatewayConfigError extends Error {
  readonly issues: string[];

  constructor(issues: string[]) {
    super(`invalid Hermes gateway configuration: ${issues.join("; ")}`);
    this.name = "GatewayConfigError";
    this.issues = issues;
  }
}

function parseNumber(value: string | undefined, fallback: number): number {
  if (!value) {
    return fallback;
  }

  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function parseVectorBackend(value: string | undefined): VectorBackend {
  if (value?.trim().toLowerCase() === "cloudflare_vectorize") {
    return "cloudflare_vectorize";
  }

  return "surrealdb";
}

function isNonEmpty(value: string | undefined): boolean {
  return typeof value === "string" && value.trim().length > 0;
}

export function validateGatewayConfig(config: GatewayConfig): GatewayConfig {
  const issues: string[] = [];

  if (!Number.isInteger(config.port) || config.port <= 0) {
    issues.push("PORT must be a positive integer");
  }

  if (!isNonEmpty(config.gatewayId)) {
    issues.push("HERMES_GATEWAY_ID must not be empty");
  }

  if (!isNonEmpty(config.region)) {
    issues.push("HERMES_REGION must not be empty");
  }

  if (!config.allowedSubnets.length) {
    issues.push("HERMES_ALLOWED_SUBNETS must contain at least one subnet");
  }

  if (!Number.isFinite(config.requestTimeoutMs) || config.requestTimeoutMs <= 0) {
    issues.push("HERMES_REQUEST_TIMEOUT_MS must be greater than zero");
  }

  const requiredEndpoints: Array<[string, string]> = [
    ["HERMES_BARE_METAL_URL", config.bareMetalBaseUrl],
    ["HERMES_CONFIDENTIAL_URL", config.confidentialBaseUrl],
    ["HERMES_CHUTES_URL", config.chutesBaseUrl],
    ["HERMES_VECTOR_URL", config.vectorUrl],
    ["HERMES_REDIS_URL", config.redisUrl],
    ["HERMES_POSTGRES_URL", config.postgresUrl],
  ];

  for (const [name, value] of requiredEndpoints) {
    if (!isNonEmpty(value)) {
      issues.push(`${name} must not be empty`);
    }
  }

  if (config.vectorBackend === "cloudflare_vectorize" && !isNonEmpty(config.vectorApiToken)) {
    issues.push("HERMES_VECTOR_API_TOKEN is required when HERMES_VECTOR_BACKEND=cloudflare_vectorize");
  }

  if (config.vectorBackend === "surrealdb") {
    if (!isNonEmpty(config.vectorNamespace)) {
      issues.push("HERMES_VECTOR_NAMESPACE must not be empty for SurrealDB retrieval");
    }

    if (!isNonEmpty(config.vectorDatabase)) {
      issues.push("HERMES_VECTOR_DATABASE must not be empty for SurrealDB retrieval");
    }

    if (!isNonEmpty(config.vectorTable)) {
      issues.push("HERMES_VECTOR_TABLE must not be empty for SurrealDB retrieval");
    }
  }

  if (issues.length) {
    throw new GatewayConfigError(issues);
  }

  return config;
}

export function loadGatewayConfig(env: NodeJS.ProcessEnv = process.env): GatewayConfig {
  const allowedSubnets = (env.HERMES_ALLOWED_SUBNETS ?? "1,8,19")
    .split(",")
    .map((value) => value.trim())
    .filter((value) => value.length > 0)
    .map((value) => Number(value.trim()))
    .filter((value) => Number.isInteger(value));

  return validateGatewayConfig({
    port: parseNumber(env.PORT, 3000),
    gatewayId: env.HERMES_GATEWAY_ID ?? "gateway-1",
    region: env.HERMES_REGION ?? "us-east-1",
    bareMetalBaseUrl: env.HERMES_BARE_METAL_URL ?? "http://validator-pool.hermes.svc.cluster.local:8080",
    confidentialBaseUrl:
      env.HERMES_CONFIDENTIAL_URL ?? "http://validator-confidential.hermes.svc.cluster.local:8080",
    chutesBaseUrl: env.HERMES_CHUTES_URL ?? "https://api.chutes.ai/v1",
    vectorUrl: env.HERMES_VECTOR_URL ?? "http://surrealdb.hermes.svc.cluster.local:8000",
    vectorBackend: parseVectorBackend(env.HERMES_VECTOR_BACKEND),
    vectorApiToken: env.HERMES_VECTOR_API_TOKEN,
    vectorNamespace: env.HERMES_VECTOR_NAMESPACE ?? "hermes",
    vectorDatabase: env.HERMES_VECTOR_DATABASE ?? "context",
    vectorTable: env.HERMES_VECTOR_TABLE ?? "context_chunks",
    redisUrl: env.HERMES_REDIS_URL ?? "redis://redis.hermes.svc.cluster.local:6379",
    postgresUrl:
      env.HERMES_POSTGRES_URL ?? "postgres://hermes:hermes@postgres.hermes.svc.cluster.local:5432/hermes",
    requestTimeoutMs: parseNumber(env.HERMES_REQUEST_TIMEOUT_MS, 1200),
    allowedSubnets,
  });
}
