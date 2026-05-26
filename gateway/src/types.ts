export type PreferredGpuClass = "b200" | "h200" | "h100";
export type RouteOperation = "inference" | "context" | "weights";
export type TargetKind = "bare_metal" | "confidential_bare_metal" | "chutes" | "vector";
export type VectorBackend = "surrealdb" | "cloudflare_vectorize";

export interface GatewayConfig {
  port: number;
  gatewayId: string;
  region: string;
  bareMetalBaseUrl: string;
  confidentialBaseUrl: string;
  chutesBaseUrl: string;
  vectorUrl: string;
  vectorBackend: VectorBackend;
  vectorApiToken?: string;
  vectorNamespace?: string;
  vectorDatabase?: string;
  vectorTable?: string;
  redisUrl: string;
  postgresUrl: string;
  requestTimeoutMs: number;
  allowedSubnets: number[];
}

export interface RouteRequest {
  subnetId: number;
  operation: RouteOperation;
  latencyBudgetMs: number;
  confidential: boolean;
  preferredGpuClass: PreferredGpuClass;
  payloadSizeBytes?: number;
  region?: string;
}

export interface RouteTarget {
  kind: TargetKind;
  baseUrl: string;
  pool: string;
  headers: Record<string, string>;
}

export interface RoutePlan {
  target: RouteTarget;
  cacheTtlSeconds: number;
  requiresWarmGpu: boolean;
  timeoutMs: number;
  reasons: string[];
}

export interface DistributedCorpusProfile {
  shardCount: number;
  uniqueTokensPerShard: number;
  overlapRatio?: number;
}

export interface ContextBudgetProfile {
  modelContextWindowTokens: number;
  reservedSystemTokens?: number;
  reservedResponseTokens?: number;
  retrievalRounds: number;
  chunksPerRound: number;
  chunkTokens: number;
  summaryCompressionRatio?: number;
}

export interface ContextHarnessRequest extends RouteRequest {
  query?: string;
  queryEmbedding?: number[];
  candidateCount?: number;
  maxDocuments?: number;
  minRerankScore?: number;
  corpus?: Partial<DistributedCorpusProfile>;
  budget?: Partial<ContextBudgetProfile>;
}

export type ContextMetadataValue = string | number | boolean | null;

export interface ContextCandidateDocument {
  id: string;
  source: string;
  content: string;
  metadata: Record<string, ContextMetadataValue>;
  backendScore: number;
  lexicalScore: number;
  rerankScore: number;
  estimatedTokens: number;
}

export interface PromptContextDocument {
  id: string;
  source: string;
  backendScore: number;
  rerankScore: number;
  originalTokens: number;
  packedTokens: number;
  truncated: boolean;
  excerpt: string;
}

export interface PromptAssembly {
  tokenBudget: number;
  usedTokens: number;
  droppedCandidates: number;
  compressionApplied: boolean;
  assembledPrompt: string;
  documents: PromptContextDocument[];
}

export interface ContextRetrievalExecution {
  backend: VectorBackend;
  requestedCandidates: number;
  returnedCandidates: number;
  selectedDocuments: number;
  documents: ContextCandidateDocument[];
  prompt: PromptAssembly;
  notes: string[];
}

export interface ContextHarnessPlan {
  corpus: {
    shardCount: number;
    uniqueTokensPerShard: number;
    overlapRatio: number;
    addressableCorpusTokens: number;
  };
  budget: {
    modelContextWindowTokens: number;
    reservedSystemTokens: number;
    reservedResponseTokens: number;
    retrievalRounds: number;
    chunksPerRound: number;
    chunkTokens: number;
    summaryCompressionRatio: number;
    directContextBudgetTokens: number;
  };
  throughput: {
    retrievedTokensPerRound: number;
    totalRetrievedChunks: number;
    effectiveWorkingSetTokens: number;
    finalContextPayloadTokens: number;
    finalChunkCount: number;
    coverageRatio: number;
    coveragePercent: number;
    promptSlackTokens: number;
    corpusToPromptCompression: number;
  };
  notes: string[];
}

export interface ServiceHealth {
  name: string;
  endpoint: string;
  mode: "required" | "optional";
}

export interface GatewayHealth {
  ok: boolean;
  gatewayId: string;
  services: ServiceHealth[];
  telemetry: string[];
}
