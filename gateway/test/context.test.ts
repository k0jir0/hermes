import assert from "node:assert/strict";
import test from "node:test";

import { buildContextHarnessPlan } from "../src/context.js";
import type { ContextHarnessRequest } from "../src/types.js";

function sampleRequest(overrides: Partial<ContextHarnessRequest> = {}): ContextHarnessRequest {
  return {
    subnetId: 8,
    operation: "context",
    latencyBudgetMs: 300,
    confidential: false,
    preferredGpuClass: "h100",
    ...overrides,
  };
}

test("builds the default 800M searchable frame into a bounded working-set plan", () => {
  const plan = buildContextHarnessPlan(sampleRequest());

  assert.equal(plan.corpus.addressableCorpusTokens, 800_000_000);
  assert.equal(plan.budget.directContextBudgetTokens, 96_000);
  assert.equal(plan.throughput.retrievedTokensPerRound, 20_480);
  assert.equal(plan.throughput.effectiveWorkingSetTokens, 102_400);
  assert.equal(plan.throughput.finalContextPayloadTokens, 76_800);
  assert.match(plan.notes[0] ?? "", /searchable external memory/i);
});

test("honors custom corpus overlap and retrieval budgets", () => {
  const plan = buildContextHarnessPlan(
    sampleRequest({
      corpus: {
        shardCount: 8,
        uniqueTokensPerShard: 25_000_000,
        overlapRatio: 0.2,
      },
      budget: {
        modelContextWindowTokens: 64_000,
        reservedSystemTokens: 8_000,
        reservedResponseTokens: 8_000,
        retrievalRounds: 3,
        chunksPerRound: 24,
        chunkTokens: 1_024,
        summaryCompressionRatio: 0.5,
      },
    }),
  );

  assert.equal(plan.corpus.addressableCorpusTokens, 160_000_000);
  assert.equal(plan.budget.directContextBudgetTokens, 48_000);
  assert.equal(plan.throughput.effectiveWorkingSetTokens, 73_728);
  assert.equal(plan.throughput.finalContextPayloadTokens, 36_864);
  assert.equal(plan.throughput.finalChunkCount, 36);
});