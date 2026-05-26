import assert from "node:assert/strict";
import test from "node:test";

import { GatewayConfigError, loadGatewayConfig } from "../src/config.js";

test("loadGatewayConfig validates the default SurrealDB production profile", () => {
  const config = loadGatewayConfig({
    PORT: "3000",
    HERMES_GATEWAY_ID: "gateway-1",
    HERMES_REGION: "us-east-1",
    HERMES_ALLOWED_SUBNETS: "1,8,19",
    HERMES_REQUEST_TIMEOUT_MS: "1200",
    HERMES_BARE_METAL_URL: "http://validator.internal",
    HERMES_CONFIDENTIAL_URL: "http://validator-confidential.internal",
    HERMES_CHUTES_URL: "https://api.chutes.ai/v1",
    HERMES_VECTOR_URL: "http://surrealdb.internal",
    HERMES_VECTOR_BACKEND: "surrealdb",
    HERMES_VECTOR_NAMESPACE: "hermes",
    HERMES_VECTOR_DATABASE: "context",
    HERMES_VECTOR_TABLE: "context_chunks",
    HERMES_REDIS_URL: "redis://redis.internal:6379",
    HERMES_POSTGRES_URL: "postgres://postgres.internal/hermes",
  });

  assert.equal(config.vectorBackend, "surrealdb");
  assert.equal(config.vectorNamespace, "hermes");
  assert.equal(config.vectorTable, "context_chunks");
});

test("loadGatewayConfig fails fast when Cloudflare Vectorize lacks an API token", () => {
  assert.throws(
    () =>
      loadGatewayConfig({
        PORT: "3000",
        HERMES_GATEWAY_ID: "gateway-1",
        HERMES_REGION: "us-east-1",
        HERMES_ALLOWED_SUBNETS: "1,8,19",
        HERMES_REQUEST_TIMEOUT_MS: "1200",
        HERMES_BARE_METAL_URL: "http://validator.internal",
        HERMES_CONFIDENTIAL_URL: "http://validator-confidential.internal",
        HERMES_CHUTES_URL: "https://api.chutes.ai/v1",
        HERMES_VECTOR_URL: "https://api.cloudflare.com/client/v4/accounts/test/vectorize/v2/indexes/hermes",
        HERMES_VECTOR_BACKEND: "cloudflare_vectorize",
        HERMES_REDIS_URL: "redis://redis.internal:6379",
        HERMES_POSTGRES_URL: "postgres://postgres.internal/hermes",
      }),
    (error: unknown) => error instanceof GatewayConfigError && error.message.includes("HERMES_VECTOR_API_TOKEN"),
  );
});

test("loadGatewayConfig fails fast when the gateway would have no allowed subnets", () => {
  assert.throws(
    () =>
      loadGatewayConfig({
        PORT: "3000",
        HERMES_GATEWAY_ID: "gateway-1",
        HERMES_REGION: "us-east-1",
        HERMES_ALLOWED_SUBNETS: "",
        HERMES_REQUEST_TIMEOUT_MS: "1200",
        HERMES_BARE_METAL_URL: "http://validator.internal",
        HERMES_CONFIDENTIAL_URL: "http://validator-confidential.internal",
        HERMES_CHUTES_URL: "https://api.chutes.ai/v1",
        HERMES_VECTOR_URL: "http://surrealdb.internal",
        HERMES_VECTOR_BACKEND: "surrealdb",
        HERMES_REDIS_URL: "redis://redis.internal:6379",
        HERMES_POSTGRES_URL: "postgres://postgres.internal/hermes",
      }),
    (error: unknown) => error instanceof GatewayConfigError && error.message.includes("HERMES_ALLOWED_SUBNETS"),
  );
});