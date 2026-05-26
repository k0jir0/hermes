import {
  ContextBudgetProfile,
  ContextHarnessPlan,
  ContextHarnessRequest,
  DistributedCorpusProfile,
} from "./types.js";

const DEFAULT_CORPUS: Required<DistributedCorpusProfile> = {
  shardCount: 16,
  uniqueTokensPerShard: 50_000_000,
  overlapRatio: 0,
};

const DEFAULT_BUDGET: Required<ContextBudgetProfile> = {
  modelContextWindowTokens: 128_000,
  reservedSystemTokens: 16_000,
  reservedResponseTokens: 16_000,
  retrievalRounds: 5,
  chunksPerRound: 40,
  chunkTokens: 512,
  summaryCompressionRatio: 0.75,
};

function assertPositiveInteger(name: string, value: number): number {
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${name} must be a positive integer`);
  }

  return value;
}

function assertUnitRatio(name: string, value: number): number {
  if (!Number.isFinite(value) || value < 0 || value > 1) {
    throw new Error(`${name} must be between 0 and 1`);
  }

  return value;
}

function resolveCorpusProfile(
  profile: ContextHarnessRequest["corpus"],
): Required<DistributedCorpusProfile> {
  return {
    shardCount: assertPositiveInteger("corpus.shardCount", profile?.shardCount ?? DEFAULT_CORPUS.shardCount),
    uniqueTokensPerShard: assertPositiveInteger(
      "corpus.uniqueTokensPerShard",
      profile?.uniqueTokensPerShard ?? DEFAULT_CORPUS.uniqueTokensPerShard,
    ),
    overlapRatio: assertUnitRatio("corpus.overlapRatio", profile?.overlapRatio ?? DEFAULT_CORPUS.overlapRatio),
  };
}

function resolveBudgetProfile(
  profile: ContextHarnessRequest["budget"],
): Required<ContextBudgetProfile> {
  const resolved = {
    modelContextWindowTokens: assertPositiveInteger(
      "budget.modelContextWindowTokens",
      profile?.modelContextWindowTokens ?? DEFAULT_BUDGET.modelContextWindowTokens,
    ),
    reservedSystemTokens: assertPositiveInteger(
      "budget.reservedSystemTokens",
      profile?.reservedSystemTokens ?? DEFAULT_BUDGET.reservedSystemTokens,
    ),
    reservedResponseTokens: assertPositiveInteger(
      "budget.reservedResponseTokens",
      profile?.reservedResponseTokens ?? DEFAULT_BUDGET.reservedResponseTokens,
    ),
    retrievalRounds: assertPositiveInteger(
      "budget.retrievalRounds",
      profile?.retrievalRounds ?? DEFAULT_BUDGET.retrievalRounds,
    ),
    chunksPerRound: assertPositiveInteger(
      "budget.chunksPerRound",
      profile?.chunksPerRound ?? DEFAULT_BUDGET.chunksPerRound,
    ),
    chunkTokens: assertPositiveInteger(
      "budget.chunkTokens",
      profile?.chunkTokens ?? DEFAULT_BUDGET.chunkTokens,
    ),
    summaryCompressionRatio: assertUnitRatio(
      "budget.summaryCompressionRatio",
      profile?.summaryCompressionRatio ?? DEFAULT_BUDGET.summaryCompressionRatio,
    ),
  };

  if (
    resolved.reservedSystemTokens + resolved.reservedResponseTokens >= resolved.modelContextWindowTokens
  ) {
    throw new Error(
      "budget.modelContextWindowTokens must be greater than reservedSystemTokens plus reservedResponseTokens",
    );
  }

  return resolved;
}

export function buildContextHarnessPlan(request: ContextHarnessRequest): ContextHarnessPlan {
  const corpus = resolveCorpusProfile(request.corpus);
  const budget = resolveBudgetProfile(request.budget);

  const addressableCorpusTokens = Math.round(
    corpus.shardCount * corpus.uniqueTokensPerShard * (1 - corpus.overlapRatio),
  );
  const retrievedTokensPerRound = budget.chunksPerRound * budget.chunkTokens;
  const totalRetrievedChunks = budget.retrievalRounds * budget.chunksPerRound;
  const effectiveWorkingSetTokens = budget.retrievalRounds * retrievedTokensPerRound;
  const directContextBudgetTokens =
    budget.modelContextWindowTokens - budget.reservedSystemTokens - budget.reservedResponseTokens;
  const finalContextPayloadTokens = Math.min(
    directContextBudgetTokens,
    Math.round(effectiveWorkingSetTokens * budget.summaryCompressionRatio),
  );
  const finalChunkCount = Math.min(
    totalRetrievedChunks,
    Math.ceil(finalContextPayloadTokens / budget.chunkTokens),
  );
  const coverageRatio = effectiveWorkingSetTokens / Math.max(addressableCorpusTokens, 1);
  const corpusToPromptCompression = addressableCorpusTokens / Math.max(finalContextPayloadTokens, 1);
  const promptSlackTokens = directContextBudgetTokens - finalContextPayloadTokens;

  const notes = [
    "Addressable corpus tokens describe searchable external memory, not one direct-attention prompt.",
    `Hermes plans ${budget.retrievalRounds} retrieval rounds over ${corpus.shardCount} shards to convert a large corpus into a bounded effective working set.`,
  ];

  if (effectiveWorkingSetTokens > directContextBudgetTokens) {
    notes.push(
      "The retrieved working set exceeds the direct prompt budget, so Hermes must compress, rerank, or stage evidence before final prompt assembly.",
    );
  }

  if (corpus.overlapRatio > 0) {
    notes.push("Corpus overlap reduces the unique addressable frame relative to raw shard capacity.");
  }

  return {
    corpus: {
      ...corpus,
      addressableCorpusTokens,
    },
    budget: {
      ...budget,
      directContextBudgetTokens,
    },
    throughput: {
      retrievedTokensPerRound,
      totalRetrievedChunks,
      effectiveWorkingSetTokens,
      finalContextPayloadTokens,
      finalChunkCount,
      coverageRatio,
      coveragePercent: coverageRatio * 100,
      promptSlackTokens,
      corpusToPromptCompression,
    },
    notes,
  };
}