export type PreferredGpuClass = "b200" | "h200" | "h100";
export type RouteOperation = "inference" | "context" | "weights";
export type TargetKind = "bare_metal" | "confidential_bare_metal" | "chutes" | "vector";

export interface GatewayConfig {
  port: number;
  gatewayId: string;
  region: string;
  bareMetalBaseUrl: string;
  confidentialBaseUrl: string;
  chutesBaseUrl: string;
  vectorUrl: string;
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
