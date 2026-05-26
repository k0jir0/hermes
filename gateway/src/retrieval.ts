import {
  ContextCandidateDocument,
  ContextHarnessPlan,
  ContextHarnessRequest,
  ContextMetadataValue,
  ContextRetrievalExecution,
  GatewayConfig,
  PromptAssembly,
  PromptContextDocument,
} from "./types.js";

type FetchLike = typeof fetch;

type RawCandidate = {
  id: string;
  source: string;
  content: string;
  metadata: Record<string, ContextMetadataValue>;
  score: number;
};

async function withTimeout<T>(
  timeoutMs: number,
  operation: (signal: AbortSignal) => Promise<T>,
): Promise<T> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  try {
    return await operation(controller.signal);
  } catch (error) {
    if (controller.signal.aborted) {
      throw new Error(`context retrieval timed out after ${timeoutMs}ms`);
    }

    throw error;
  } finally {
    clearTimeout(timer);
  }
}

function estimateTokens(text: string): number {
  return Math.max(1, Math.ceil(text.trim().length / 4));
}

function clipTextToTokenBudget(text: string, tokenBudget: number): string {
  if (tokenBudget <= 0) {
    return "";
  }

  const maxChars = tokenBudget * 4;
  if (text.length <= maxChars) {
    return text;
  }

  const clipped = text.slice(0, Math.max(0, maxChars - 3));
  const lastWhitespace = clipped.lastIndexOf(" ");
  const safeClip = lastWhitespace > maxChars / 2 ? clipped.slice(0, lastWhitespace) : clipped;
  return `${safeClip.trimEnd()}...`;
}

function normalizeMetadata(value: unknown): Record<string, ContextMetadataValue> {
  if (!value || typeof value !== "object") {
    return {};
  }

  return Object.fromEntries(
    Object.entries(value)
      .filter(([, entry]) => entry === null || ["string", "number", "boolean"].includes(typeof entry))
      .map(([key, entry]) => [key, entry as ContextMetadataValue]),
  );
}

function asString(value: unknown): string | undefined {
  if (typeof value === "string" && value.trim()) {
    return value;
  }

  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }

  return undefined;
}

function asFiniteNumber(value: unknown, fallback = 0): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function normalizeCandidate(entry: Record<string, unknown>): RawCandidate {
  const metadata = normalizeMetadata(entry.metadata);
  const id = asString(entry.id) ?? asString(metadata.id) ?? crypto.randomUUID();
  const source =
    asString(entry.source) ??
    asString(metadata.source) ??
    asString(metadata.uri) ??
    asString(metadata.url) ??
    id;
  const content =
    asString(entry.content) ??
    asString(entry.text) ??
    asString(entry.document) ??
    asString(metadata.content) ??
    asString(metadata.text) ??
    JSON.stringify(metadata);

  return {
    id,
    source,
    content,
    metadata,
    score: asFiniteNumber(entry.score),
  };
}

function uniqueTerms(query: string): string[] {
  return [...new Set(query.toLowerCase().match(/[a-z0-9]{3,}/g) ?? [])];
}

function lexicalScore(query: string, content: string): number {
  const terms = uniqueTerms(query);
  if (terms.length === 0) {
    return 0;
  }

  const lowerContent = content.toLowerCase();
  const overlap = terms.filter((term) => lowerContent.includes(term)).length;
  const phraseBonus = lowerContent.includes(query.toLowerCase()) ? 0.15 : 0;
  return Math.min(1, overlap / terms.length + phraseBonus);
}

function normalizeBackendScore(score: number, maxScore: number, minScore: number): number {
  if (maxScore === minScore) {
    if (score <= 0) {
      return 0;
    }
    return Math.min(1, score);
  }

  return Math.max(0, Math.min(1, (score - minScore) / (maxScore - minScore)));
}

function rerankCandidates(query: string, candidates: RawCandidate[]): ContextCandidateDocument[] {
  const maxScore = Math.max(...candidates.map((candidate) => candidate.score), 0);
  const minScore = Math.min(...candidates.map((candidate) => candidate.score), 0);

  return candidates
    .map((candidate) => {
      const backendScore = normalizeBackendScore(candidate.score, maxScore, minScore);
      const lexical = lexicalScore(query, candidate.content);
      const rerankScore = backendScore * 0.65 + lexical * 0.35;
      return {
        id: candidate.id,
        source: candidate.source,
        content: candidate.content,
        metadata: candidate.metadata,
        backendScore,
        lexicalScore: lexical,
        rerankScore,
        estimatedTokens: estimateTokens(candidate.content),
      };
    })
    .sort((left, right) => right.rerankScore - left.rerankScore);
}

function buildPromptAssembly(
  query: string,
  ranked: ContextCandidateDocument[],
  tokenBudget: number,
  maxDocuments: number,
): PromptAssembly {
  const intro = `Query: ${query}\nUse the following bounded context excerpts when answering.\n`;
  const introTokens = estimateTokens(intro);
  let remaining = Math.max(0, tokenBudget - introTokens);
  const sections: string[] = [intro.trimEnd()];
  const documents: PromptContextDocument[] = [];

  for (const candidate of ranked) {
    if (documents.length >= maxDocuments || remaining <= 24) {
      break;
    }

    const remainingSlots = Math.max(1, maxDocuments - documents.length);
    const header = `Document ${documents.length + 1} | source=${candidate.source} | rerank=${candidate.rerankScore.toFixed(3)}`;
    const headerTokens = estimateTokens(header);
    const contentBudget = Math.max(24, Math.floor((remaining - headerTokens) / remainingSlots));
    if (contentBudget <= 0) {
      break;
    }

    const excerpt = clipTextToTokenBudget(candidate.content, contentBudget);
    const excerptTokens = estimateTokens(excerpt);
    const packedTokens = headerTokens + excerptTokens;

    if (packedTokens > remaining) {
      const reducedExcerpt = clipTextToTokenBudget(candidate.content, Math.max(8, remaining - headerTokens));
      const reducedTokens = headerTokens + estimateTokens(reducedExcerpt);
      if (reducedTokens > remaining || !reducedExcerpt) {
        continue;
      }

      sections.push(`${header}\n${reducedExcerpt}`);
      documents.push({
        id: candidate.id,
        source: candidate.source,
        backendScore: candidate.backendScore,
        rerankScore: candidate.rerankScore,
        originalTokens: candidate.estimatedTokens,
        packedTokens: reducedTokens,
        truncated: reducedExcerpt.length < candidate.content.length,
        excerpt: reducedExcerpt,
      });
      remaining -= reducedTokens;
      continue;
    }

    sections.push(`${header}\n${excerpt}`);
    documents.push({
      id: candidate.id,
      source: candidate.source,
      backendScore: candidate.backendScore,
      rerankScore: candidate.rerankScore,
      originalTokens: candidate.estimatedTokens,
      packedTokens,
      truncated: excerpt.length < candidate.content.length,
      excerpt,
    });
    remaining -= packedTokens;
  }

  return {
    tokenBudget,
    usedTokens: tokenBudget - remaining,
    droppedCandidates: Math.max(0, ranked.length - documents.length),
    compressionApplied:
      documents.some((document) => document.truncated) || documents.length < ranked.length,
    assembledPrompt: sections.join("\n\n"),
    documents,
  };
}

async function parseCloudflareResponse(response: Response): Promise<RawCandidate[]> {
  const payload = (await response.json()) as { result?: { matches?: unknown[] }; matches?: unknown[] };
  const matches = payload.result?.matches ?? payload.matches ?? [];
  return matches
    .filter((entry): entry is Record<string, unknown> => typeof entry === "object" && entry !== null)
    .map(normalizeCandidate);
}

async function queryCloudflareVectorize(
  request: ContextHarnessRequest,
  config: GatewayConfig,
  limit: number,
  fetchImpl: FetchLike,
  signal: AbortSignal,
): Promise<RawCandidate[]> {
  const url = config.vectorUrl.endsWith("/query") ? config.vectorUrl : `${config.vectorUrl.replace(/\/$/, "")}/query`;
  const headers: Record<string, string> = {
    "content-type": "application/json",
  };

  if (config.vectorApiToken) {
    headers.Authorization = `Bearer ${config.vectorApiToken}`;
  }

  const body = request.queryEmbedding?.length
    ? {
        vector: request.queryEmbedding,
        topK: limit,
        returnMetadata: true,
      }
    : {
        query: request.query,
        topK: limit,
        returnMetadata: true,
      };

  const response = await fetchImpl(url, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
    signal,
  });

  if (!response.ok) {
    throw new Error(`Cloudflare Vectorize query failed with status ${response.status}`);
  }

  return parseCloudflareResponse(response);
}

function buildSurrealSql(request: ContextHarnessRequest, config: GatewayConfig, limit: number): string {
  const table = config.vectorTable ?? "context_chunks";
  const escapedQuery = (request.query ?? "").replaceAll("\\", "\\\\").replaceAll('"', '\\"');

  if (request.queryEmbedding?.length) {
    const embedding = `[${request.queryEmbedding.map((value) => Number(value).toFixed(8)).join(", ")}]`;
    return `SELECT id, content, metadata, vector::similarity::cosine(embedding, ${embedding}) AS score FROM ${table} ORDER BY score DESC LIMIT ${limit};`;
  }

  return `SELECT id, content, metadata, search::score(1) AS score FROM ${table} WHERE content @1@ "${escapedQuery}" ORDER BY score DESC LIMIT ${limit};`;
}

async function parseSurrealResponse(response: Response): Promise<RawCandidate[]> {
  const payload = (await response.json()) as Array<{ result?: unknown[] }> | { result?: unknown[] };
  const result = Array.isArray(payload) ? payload[0]?.result ?? [] : payload.result ?? [];
  return result
    .filter((entry): entry is Record<string, unknown> => typeof entry === "object" && entry !== null)
    .map(normalizeCandidate);
}

async function querySurrealDb(
  request: ContextHarnessRequest,
  config: GatewayConfig,
  limit: number,
  fetchImpl: FetchLike,
  signal: AbortSignal,
): Promise<RawCandidate[]> {
  const url = config.vectorUrl.endsWith("/sql") ? config.vectorUrl : `${config.vectorUrl.replace(/\/$/, "")}/sql`;
  const headers: Record<string, string> = {
    "content-type": "text/plain",
    accept: "application/json",
  };

  if (config.vectorNamespace) {
    headers.NS = config.vectorNamespace;
  }

  if (config.vectorDatabase) {
    headers.DB = config.vectorDatabase;
  }

  if (config.vectorApiToken) {
    headers.Authorization = `Bearer ${config.vectorApiToken}`;
  }

  const response = await fetchImpl(url, {
    method: "POST",
    headers,
    body: buildSurrealSql(request, config, limit),
    signal,
  });

  if (!response.ok) {
    throw new Error(`SurrealDB query failed with status ${response.status}`);
  }

  return parseSurrealResponse(response);
}

async function fetchCandidates(
  request: ContextHarnessRequest,
  config: GatewayConfig,
  limit: number,
  fetchImpl: FetchLike,
  signal: AbortSignal,
): Promise<RawCandidate[]> {
  if ((!request.query || !request.query.trim()) && !(request.queryEmbedding?.length)) {
    throw new Error("context queries require a non-empty query or queryEmbedding");
  }

  if (config.vectorBackend === "cloudflare_vectorize") {
    return queryCloudflareVectorize(request, config, limit, fetchImpl, signal);
  }

  return querySurrealDb(request, config, limit, fetchImpl, signal);
}

export async function executeContextRetrieval(
  request: ContextHarnessRequest,
  config: GatewayConfig,
  harnessPlan: ContextHarnessPlan,
  fetchImpl: FetchLike,
): Promise<ContextRetrievalExecution> {
  const requestedCandidates = Math.max(
    1,
    Math.min(
      request.candidateCount ?? harnessPlan.budget.chunksPerRound,
      harnessPlan.throughput.totalRetrievedChunks,
    ),
  );
  const maxDocuments = Math.max(
    1,
    Math.min(
      request.maxDocuments ?? harnessPlan.throughput.finalChunkCount,
      harnessPlan.throughput.finalChunkCount,
    ),
  );
  const minRerankScore = Math.max(0, Math.min(1, request.minRerankScore ?? 0));
  const rawCandidates = await withTimeout(config.requestTimeoutMs, (signal) =>
    fetchCandidates(request, config, requestedCandidates, fetchImpl, signal),
  );
  const ranked = rerankCandidates(request.query ?? "", rawCandidates).filter(
    (candidate) => candidate.rerankScore >= minRerankScore,
  );
  const prompt = buildPromptAssembly(
    request.query ?? "",
    ranked,
    harnessPlan.throughput.finalContextPayloadTokens,
    maxDocuments,
  );

  const notes = [
    `Hermes retrieved ${rawCandidates.length} raw candidates from ${config.vectorBackend}.`,
    `Hermes packed ${prompt.documents.length} evidence documents into a ${prompt.tokenBudget}-token bounded prompt budget.`,
  ];

  if (prompt.compressionApplied) {
    notes.push("Prompt compression or truncation was applied to stay within the bounded context budget.");
  }

  return {
    backend: config.vectorBackend,
    requestedCandidates,
    returnedCandidates: rawCandidates.length,
    selectedDocuments: prompt.documents.length,
    documents: ranked,
    prompt,
    notes,
  };
}