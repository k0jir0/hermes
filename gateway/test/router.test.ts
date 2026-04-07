import assert from "node:assert/strict";
import test from "node:test";

import { buildGatewayHealth, buildRoutePlan } from "../src/router.js";
import type { GatewayConfig } from "../src/types.js";

const config: GatewayConfig = {
  port: 3000,
  gatewayId: "gateway-1",
  region: "us-east-1",
  bareMetalBaseUrl: "http://validator.internal",
  confidentialBaseUrl: "http://validator-confidential.internal",
  chutesBaseUrl: "https://api.chutes.ai/v1",
  vectorUrl: "http://surrealdb.internal",
  redisUrl: "redis://redis.internal:6379",
  postgresUrl: "postgres://hermes:hermes@postgres.internal:5432/hermes",
  requestTimeoutMs: 1200,
  allowedSubnets: [1, 8, 19],
};

test("routes confidential traffic to the confidential bare-metal pool", () => {
  const plan = buildRoutePlan(
    {
      subnetId: 8,
      operation: "inference",
      latencyBudgetMs: 200,
      confidential: true,
      preferredGpuClass: "h100",
    },
    config,
  );

  assert.equal(plan.target.kind, "confidential_bare_metal");
  assert.equal(plan.target.baseUrl, "http://validator-confidential.internal");
  assert.match(plan.target.pool, /^confidential-h100/);
});

test("routes context retrieval to the vector backend", () => {
  const plan = buildRoutePlan(
    {
      subnetId: 1,
      operation: "context",
      latencyBudgetMs: 500,
      confidential: false,
      preferredGpuClass: "h100",
    },
    config,
  );

  assert.equal(plan.target.kind, "vector");
  assert.equal(plan.requiresWarmGpu, false);
  assert.equal(plan.target.baseUrl, "http://surrealdb.internal");
});

test("builds health payload for required services", () => {
  const health = buildGatewayHealth(config);

  assert.equal(health.ok, true);
  assert.equal(health.services.length, 4);
  assert.ok(health.telemetry.includes("grafana"));
});
