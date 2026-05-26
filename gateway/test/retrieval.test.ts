import assert from "node:assert/strict";
import test from "node:test";

import { buildContextHarnessPlan } from "../src/context.js";
import { executeContextRetrieval } from "../src/retrieval.js";
import type { ContextHarnessRequest, GatewayConfig } from "../src/types.js";

function baseRequest(overrides: Partial<ContextHarnessRequest> = {}): ContextHarnessRequest {
  return {
    subnetId: 8,
    operation: "context",
    query: "bounded context working set",
    latencyBudgetMs: 300,
    confidential: false,
    preferredGpuClass: "h100",
    ...overrides,
  };
}

test("SurrealDB retrieval reranks candidates and keeps prompt bounded", async () => {
  const config: GatewayConfig = {
    port: 3000,
    gatewayId: "gateway-1",
    region: "us-east-1",
    bareMetalBaseUrl: "http://validator.internal",
    confidentialBaseUrl: "http://validator-confidential.internal",
    chutesBaseUrl: "https://api.chutes.ai/v1",
    vectorUrl: "http://surrealdb.internal",
    vectorBackend: "surrealdb",
    vectorNamespace: "hermes",
    vectorDatabase: "context",
    vectorTable: "context_chunks",
    redisUrl: "redis://redis.internal:6379",
    postgresUrl: "postgres://hermes:hermes@postgres.internal:5432/hermes",
    requestTimeoutMs: 1200,
    allowedSubnets: [1, 8, 19],
  };

  const request = baseRequest({
    candidateCount: 3,
    maxDocuments: 2,
    budget: {
      modelContextWindowTokens: 8_000,
      reservedSystemTokens: 1_000,
      reservedResponseTokens: 1_000,
      retrievalRounds: 2,
      chunksPerRound: 3,
      chunkTokens: 256,
      summaryCompressionRatio: 0.5,
    },
  });
  const plan = buildContextHarnessPlan(request);
  let capturedBody = "";

  const fetchStub: typeof fetch = async (_input, init) => {
    capturedBody = String(init?.body ?? "");
    return new Response(
      JSON.stringify([
        {
          result: [
            {
              id: "doc-b",
              content: "A bounded context working set requires retrieval, reranking, and prompt assembly over a much larger searchable corpus.",
              metadata: { source: "paper1.txt" },
              score: 0.62,
            },
            {
              id: "doc-a",
              content: "A bounded context working set is stronger when the prompt explicitly reserves response budget and compresses evidence.",
              metadata: { source: "design.txt" },
              score: 0.64,
            },
            {
              id: "doc-c",
              content: "GPU temperatures should be monitored with Prometheus and Grafana.",
              metadata: { source: "ops.txt" },
              score: 0.7,
            },
          ],
        },
      ]),
      { status: 200, headers: { "content-type": "application/json" } },
    );
  };

  const retrieval = await executeContextRetrieval(request, config, plan, fetchStub);

  assert.match(capturedBody, /context_chunks/);
  assert.equal(retrieval.backend, "surrealdb");
  assert.equal(retrieval.returnedCandidates, 3);
  assert.equal(retrieval.selectedDocuments, 2);
  assert.ok(retrieval.prompt.usedTokens <= retrieval.prompt.tokenBudget);
  assert.match(retrieval.prompt.assembledPrompt, /bounded context working set/i);
  assert.equal(retrieval.documents[0]?.id, "doc-a");
  assert.ok((retrieval.documents[0]?.rerankScore ?? 0) > (retrieval.documents[2]?.rerankScore ?? 0));
});

test("Cloudflare Vectorize retrieval uses query embeddings when provided", async () => {
  const config: GatewayConfig = {
    port: 3000,
    gatewayId: "gateway-1",
    region: "us-east-1",
    bareMetalBaseUrl: "http://validator.internal",
    confidentialBaseUrl: "http://validator-confidential.internal",
    chutesBaseUrl: "https://api.chutes.ai/v1",
    vectorUrl: "https://api.cloudflare.com/client/v4/accounts/test/vectorize/v2/indexes/hermes",
    vectorBackend: "cloudflare_vectorize",
    vectorApiToken: "token-123",
    redisUrl: "redis://redis.internal:6379",
    postgresUrl: "postgres://hermes:hermes@postgres.internal:5432/hermes",
    requestTimeoutMs: 1200,
    allowedSubnets: [1, 8, 19],
  };

  const request = baseRequest({
    queryEmbedding: [0.1, 0.2, 0.3],
    candidateCount: 1,
    maxDocuments: 1,
  });
  const plan = buildContextHarnessPlan(request);
  let capturedBody = "";

  const fetchStub: typeof fetch = async (input, init) => {
    assert.equal(String(input), "https://api.cloudflare.com/client/v4/accounts/test/vectorize/v2/indexes/hermes/query");
    assert.equal((init?.headers as Record<string, string>).Authorization, "Bearer token-123");
    capturedBody = String(init?.body ?? "");
    return new Response(
      JSON.stringify({
        result: {
          matches: [
            {
              id: "doc-z",
              score: 0.88,
              metadata: {
                source: "paper1.txt",
                text: "Distributed retrieval gives a system a much larger searchable universe than a single model window can hold.",
              },
            },
          ],
        },
      }),
      { status: 200, headers: { "content-type": "application/json" } },
    );
  };

  const retrieval = await executeContextRetrieval(request, config, plan, fetchStub);

  assert.match(capturedBody, /"vector":\[0.1,0.2,0.3\]/);
  assert.equal(retrieval.returnedCandidates, 1);
  assert.equal(retrieval.prompt.documents.length, 1);
  assert.match(retrieval.prompt.assembledPrompt, /searchable universe/i);
});

test("retrieval fails fast when the backend exceeds the configured timeout", async () => {
  const config: GatewayConfig = {
    port: 3000,
    gatewayId: "gateway-1",
    region: "us-east-1",
    bareMetalBaseUrl: "http://validator.internal",
    confidentialBaseUrl: "http://validator-confidential.internal",
    chutesBaseUrl: "https://api.chutes.ai/v1",
    vectorUrl: "http://surrealdb.internal",
    vectorBackend: "surrealdb",
    vectorNamespace: "hermes",
    vectorDatabase: "context",
    vectorTable: "context_chunks",
    redisUrl: "redis://redis.internal:6379",
    postgresUrl: "postgres://hermes:hermes@postgres.internal:5432/hermes",
    requestTimeoutMs: 5,
    allowedSubnets: [1, 8, 19],
  };

  const request = baseRequest();
  const plan = buildContextHarnessPlan(request);
  const fetchStub: typeof fetch = async (_input, init) =>
    new Promise<Response>((_resolve, reject) => {
      const abortSignal = init?.signal as AbortSignal | undefined;
      abortSignal?.addEventListener("abort", () => reject(new DOMException("The operation was aborted.", "AbortError")));
    });

  await assert.rejects(
    () => executeContextRetrieval(request, config, plan, fetchStub),
    /timed out after 5ms/,
  );
});