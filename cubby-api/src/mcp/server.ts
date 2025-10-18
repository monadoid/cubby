/**
 * MCP Server Instance
 *
 * Configures the Model Context Protocol server for Cubby.
 * Provides AI assistants with structured access to cubby data.
 */

import { McpServer } from "mcp-lite";
import { z } from "zod";

/**
 * Main MCP server instance.
 * Uses Zod schema adapter to convert Zod schemas to JSON Schema
 * for MCP protocol compatibility.
 * 
 * OAuth Configuration:
 * - The server requires OAuth 2.0 authentication for tool access
 * - The /mcp endpoint accepts unauthenticated initialize requests
 * - Tools validate authentication via getMcpAuthContext() and fail with helpful errors
 * - OAuth metadata is discovered via /.well-known/oauth-protected-resource
 */
export const mcpServer = new McpServer({
  name: "cubby-mcp",
  version: "1.0.0",
  schemaAdapter: (schema) => z.toJSONSchema(schema as z.ZodType),
});
