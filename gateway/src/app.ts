import { loadGatewayConfig } from "./config.js";
import { buildGatewayHealth, buildRoutePlan } from "./router.js";
import { GatewayConfig, RouteRequest } from "./types.js";

function jsonResponse(payload: unknown, status = 200): Response {
  return new Response(JSON.stringify(payload, null, 2), {
    status,
    headers: {
      "content-type": "application/json",
    },
  });
}

async function parseRouteRequest(request: Request): Promise<RouteRequest> {
  const body = (await request.json()) as Partial<RouteRequest>;
  if (
    typeof body.subnetId !== "number" ||
    typeof body.operation !== "string" ||
    typeof body.latencyBudgetMs !== "number" ||
    typeof body.confidential !== "boolean" ||
    typeof body.preferredGpuClass !== "string"
  ) {
    throw new Error("invalid route request payload");
  }

  return body as RouteRequest;
}

export async function handleRequest(
  request: Request,
  config: GatewayConfig = loadGatewayConfig(),
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
      const routeRequest = await parseRouteRequest(request);
      const plan = buildRoutePlan({ ...routeRequest, operation: "context" }, config);
      return jsonResponse({
        route: plan,
        backend: "surrealdb",
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
