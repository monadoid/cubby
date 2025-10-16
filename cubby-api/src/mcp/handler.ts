/**
 * MCP HTTP Handler
 *
 * Binds the MCP server to HTTP transport for request/response handling.
 */

import { StreamableHttpTransport, InMemorySessionAdapter } from "mcp-lite";
import { mcpServer } from "./server";

/**
 * HTTP transport for MCP server.
 * Uses sessionful mode with in-memory adapter for SSE streaming.
 */
const transport = new StreamableHttpTransport({
  sessionAdapter: new InMemorySessionAdapter({ maxEventBufferSize: 1024 }),
});

/**
 * HTTP handler for MCP requests.
 * Pass Web API Request objects to this handler.
 */
export const mcpHttpHandler = transport.bind(mcpServer);
