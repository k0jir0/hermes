import { buildContextHarnessPlan } from "./context.js";
import { loadGatewayConfig } from "./config.js";
import { executeContextRetrieval } from "./retrieval.js";
import { buildGatewayHealth, buildRoutePlan } from "./router.js";
import {
  ContextBudgetProfile,
  ContextHarnessRequest,
  DistributedCorpusProfile,
  GatewayConfig,
  RouteRequest,
} from "./types.js";

function jsonResponse(payload: unknown, status = 200): Response {
  return new Response(JSON.stringify(payload, null, 2), {
    status,
    headers: {
      "content-type": "application/json",
    },
  });
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function parsePositiveInteger(value: unknown, fieldName: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value <= 0) {
    throw new Error(`invalid ${fieldName} payload`);
  }

  return value;
}

function parseUnitRatio(value: unknown, fieldName: string): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) {
    throw new Error(`invalid ${fieldName} payload`);
  }

  return value;
}

function parseEmbedding(value: unknown): number[] {
  if (!Array.isArray(value) || value.some((entry) => typeof entry !== "number" || !Number.isFinite(entry))) {
    throw new Error("invalid queryEmbedding payload");
  }

  return value;
}

function parseBaseRouteRequest(body: Record<string, unknown>): RouteRequest {
  if (
    typeof body.subnetId !== "number" ||
    typeof body.operation !== "string" ||
    typeof body.latencyBudgetMs !== "number" ||
    typeof body.confidential !== "boolean" ||
    typeof body.preferredGpuClass !== "string"
  ) {
    throw new Error("invalid route request payload");
  }

  return body as unknown as RouteRequest;
}

function parseNumericGroup<T extends object>(
  value: unknown,
  fieldName: string,
): Partial<T> {
  if (!isRecord(value)) {
    throw new Error(`invalid ${fieldName} payload`);
  }

  const parsed: Record<string, number> = {};
  for (const [key, nestedValue] of Object.entries(value)) {
    if (typeof nestedValue !== "number" || !Number.isFinite(nestedValue)) {
      throw new Error(`invalid ${fieldName}.${key} payload`);
    }
    parsed[key] = nestedValue;
  }

  return parsed as Partial<T>;
}

async function parseRouteRequest(request: Request): Promise<RouteRequest> {
  const body = (await request.json()) as unknown;
  if (!isRecord(body)) {
    throw new Error("invalid route request payload");
  }

  return parseBaseRouteRequest(body);
}

async function parseContextHarnessRequest(request: Request): Promise<ContextHarnessRequest> {
  const body = (await request.json()) as unknown;
  if (!isRecord(body)) {
    throw new Error("invalid route request payload");
  }

  const parsed: ContextHarnessRequest = {
    ...parseBaseRouteRequest(body),
  };

  if (body.query !== undefined) {
    if (typeof body.query !== "string" || body.query.trim().length === 0) {
      throw new Error("invalid query payload");
    }
    parsed.query = body.query.trim();
  }

  if (body.queryEmbedding !== undefined) {
    parsed.queryEmbedding = parseEmbedding(body.queryEmbedding);
  }

  if (body.candidateCount !== undefined) {
    parsed.candidateCount = parsePositiveInteger(body.candidateCount, "candidateCount");
  }

  if (body.maxDocuments !== undefined) {
    parsed.maxDocuments = parsePositiveInteger(body.maxDocuments, "maxDocuments");
  }

  if (body.minRerankScore !== undefined) {
    parsed.minRerankScore = parseUnitRatio(body.minRerankScore, "minRerankScore");
  }

  if (body.corpus !== undefined) {
    parsed.corpus = parseNumericGroup<DistributedCorpusProfile>(body.corpus, "corpus");
  }

  if (body.budget !== undefined) {
    parsed.budget = parseNumericGroup<ContextBudgetProfile>(body.budget, "budget");
  }

  return parsed;
}

export async function handleRequest(
  request: Request,
  config: GatewayConfig = loadGatewayConfig(),
  fetchImpl: typeof fetch = fetch,
): Promise<Response> {
  const url = new URL(request.url);

  try {
    if (request.method === "GET" && url.pathname === "/healthz") {
      return jsonResponse(buildGatewayHealth(config));
    }

    if (request.method === "GET" && url.pathname === "/readyz") {
      return jsonResponse({
        ok: true,
        gatewayId: config.gatewayId,
        region: config.region,
        allowedSubnets: config.allowedSubnets,
      });
    }

    if (request.method === "POST" && url.pathname === "/v1/route") {
      const routeRequest = await parseRouteRequest(request);
      return jsonResponse(buildRoutePlan(routeRequest, config));
    }

    if (request.method === "POST" && url.pathname === "/v1/context/query") {
      const routeRequest = await parseContextHarnessRequest(request);
      const routePlan = buildRoutePlan({ ...routeRequest, operation: "context" }, config);
      const harnessPlan = buildContextHarnessPlan(routeRequest);
      const retrieval = await executeContextRetrieval(routeRequest, config, harnessPlan, fetchImpl);
      return jsonResponse({
        route: routePlan,
        backend: config.vectorBackend,
        harnessPlan,
        retrieval,
      });
    }

    if (request.method === "POST" && url.pathname === "/v1/chutes/dispatch") {
      const routeRequest = await parseRouteRequest(request);
      const plan = buildRoutePlan(routeRequest, config);
      return jsonResponse({
        dispatch: {
          method: "POST",
          url: `${plan.target.baseUrl}/dispatch`,
          headers: plan.target.headers,
          pool: plan.target.pool,
        },
        routedVia: plan.target.kind,
      });
    }
  } catch (error) {
    return jsonResponse(
      {
        error: error instanceof Error ? error.message : String(error),
      },
      400,
    );
  }

  return jsonResponse({ error: "not found" }, 404);
}
