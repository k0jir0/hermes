import assert from "node:assert/strict";
import test from "node:test";

import { handleRequest } from "../src/app.js";
import type { GatewayConfig } from "../src/types.js";

const config: GatewayConfig = {
  port: 3000,
  gatewayId: "gateway-1",
  region: "us-east-1",
  bareMetalBaseUrl: "http://validator.internal",
  confidentialBaseUrl: "http://validator-confidential.internal",
  chutesBaseUrl: "https://api.chutes.ai/v1",
  vectorUrl: "https://api.cloudflare.com/client/v4/accounts/test/vectorize/v2/indexes/hermes",
  vectorBackend: "cloudflare_vectorize",
  vectorApiToken: "test-token",
  redisUrl: "redis://redis.internal:6379",
  postgresUrl: "postgres://hermes:hermes@postgres.internal:5432/hermes",
  requestTimeoutMs: 1200,
  allowedSubnets: [1, 8, 19],
};

test("context query retrieves evidence and assembles a bounded prompt", async () => {
  const fetchStub: typeof fetch = async (input, init) => {
    assert.equal(String(input), "https://api.cloudflare.com/client/v4/accounts/test/vectorize/v2/indexes/hermes/query");
    assert.equal(init?.method, "POST");
    assert.equal((init?.headers as Record<string, string>).Authorization, "Bearer test-token");

    return new Response(
      JSON.stringify({
        result: {
          matches: [
            {
              id: "paper1-section-1",
              score: 0.91,
              metadata: {
                source: "paper1.txt",
                text: "An 800M distributed corpus can outperform a 128k context window when retrieval quality is high and the right evidence is surfaced consistently.",
              },
            },
            {
              id: "paper1-section-2",
              score: 0.52,
              metadata: {
                source: "paper1.txt",
                text: "A native context window remains stronger when all relevant evidence already fits in prompt and exact long-range reasoning matters.",
              },
            },
          ],
        },
      }),
      { status: 200, headers: { "content-type": "application/json" } },
    );
  };

  const response = await handleRequest(
    new Request("http://localhost/v1/context/query", {
      method: "POST",
      headers: {
        "content-type": "application/json",
      },
      body: JSON.stringify({
        subnetId: 8,
        operation: "context",
        query: "When does an 800M corpus outperform a 128k context window?",
        latencyBudgetMs: 300,
        confidential: false,
        preferredGpuClass: "h100",
      }),
    }),
    config,
    fetchStub,
  );

  assert.equal(response.status, 200);
  const payload = (await response.json()) as {
    backend: string;
    harnessPlan: {
      corpus: { addressableCorpusTokens: number };
      throughput: { effectiveWorkingSetTokens: number };
    };
    retrieval: {
      backend: string;
      returnedCandidates: number;
      selectedDocuments: number;
      prompt: { usedTokens: number; tokenBudget: number; assembledPrompt: string };
      documents: Array<{ source: string; rerankScore: number }>;
    };
  };
  assert.equal(payload.backend, "cloudflare_vectorize");
  assert.equal(payload.harnessPlan.corpus.addressableCorpusTokens, 800_000_000);
  assert.equal(payload.harnessPlan.throughput.effectiveWorkingSetTokens, 102_400);
  assert.equal(payload.retrieval.backend, "cloudflare_vectorize");
  assert.equal(payload.retrieval.returnedCandidates, 2);
  assert.equal(payload.retrieval.selectedDocuments, 2);
  assert.ok(payload.retrieval.prompt.usedTokens <= payload.retrieval.prompt.tokenBudget);
  assert.match(payload.retrieval.prompt.assembledPrompt, /800M distributed corpus/i);
  assert.equal(payload.retrieval.documents[0]?.source, "paper1.txt");
  assert.ok((payload.retrieval.documents[0]?.rerankScore ?? 0) >= (payload.retrieval.documents[1]?.rerankScore ?? 0));
});