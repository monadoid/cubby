/**
 * MCP Server Instance
 *
 * Configures the Model Context Protocol server for Cubby.
 * Provides AI assistants with structured access to Screenpipe data.
 */

import { McpServer } from "mcp-lite";
import { z } from "zod";

/**
 * Main MCP server instance.
 * Uses Zod schema adapter to convert Zod schemas to JSON Schema
 * for MCP protocol compatibility.
 */
export const mcpServer = new McpServer({
  name: "cubby-screenpipe",
  version: "1.0.0",
  schemaAdapter: (schema) => z.toJSONSchema(schema as z.ZodType),
});
