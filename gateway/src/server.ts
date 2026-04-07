declare const Bun: {
  serve(init: {
    port: number;
    fetch: (request: Request) => Response | Promise<Response>;
  }): { port: number };
};

import { handleRequest } from "./app.js";
import { loadGatewayConfig } from "./config.js";

const config = loadGatewayConfig();
const server = Bun.serve({
  port: config.port,
  fetch(request: Request) {
    return handleRequest(request, config);
  },
});

console.log(`Hermes gateway listening on ${server.port}`);
