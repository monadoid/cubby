/**
 * OpenAPI Generator for MCP Server
 *
 * Generates OpenAPI documentation from MCP server tools.
 * This allows developers to see available tools and their schemas
 * in a familiar OpenAPI format.
 */

import { z } from "zod";
import { mcpServer } from "./server";
import {
  SearchToolInputSchema,
  SearchResponseSchema,
} from "../schemas/cubby";
import {
  DcrRegisterRequestSchema,
  DcrRegisterResponseSchema,
  DcrErrorResponseSchema,
} from "../schemas/dcr";

export interface OpenAPISpec {
  openapi: string;
  info: {
    title: string;
    version: string;
    description: string;
  };
  servers: Array<{
    url: string;
    description: string;
  }>;
  paths: Record<string, any>;
  components?: {
    schemas?: Record<string, any>;
    securitySchemes?: Record<string, any>;
  };
  security?: Array<Record<string, string[]>>;
}

/**
 * Generate OpenAPI specification from MCP server tools
 */
export function generateMcpOpenAPISpec(baseUrl: string): OpenAPISpec {
  const spec: OpenAPISpec = {
    openapi: "3.0.0",
    info: {
      title: "Cubby MCP Tools",
      version: "1.0.0",
      description:
        "Model Context Protocol (MCP) tools for accessing cubby data. These tools are accessed via JSON-RPC 2.0 at the /mcp endpoint, but are documented here in OpenAPI format for convenience.",
    },
    servers: [
      {
        url: baseUrl,
        description: "MCP Server",
      },
    ],
    paths: {},
    components: {
      schemas: {},
      securitySchemes: {
        OAuth2: {
          type: "oauth2",
          flows: {
            authorizationCode: {
              authorizationUrl: `${baseUrl}/oauth/authorize`,
              tokenUrl: `${baseUrl}/oauth/token`,
              refreshUrl: `${baseUrl}/oauth/token`,
              scopes: {
                "read:cubby": "Read access to cubby data",
              },
            },
          },
          "x-registrationUrl": `${baseUrl}/oauth/register`,
        },
        BearerAuth: {
          type: "http",
          scheme: "bearer",
          bearerFormat: "JWT",
        },
      },
    },
    security: [{ OAuth2: ["read:cubby"] }, { BearerAuth: [] }],
  };

  // Add OAuth DCR endpoint documentation
  spec.paths["/oauth/register"] = {
    post: {
      summary: "Register OAuth 2.0 Client",
      description:
        "Dynamic Client Registration endpoint. Allows third-party applications to register themselves as OAuth clients without manual configuration. Implements RFC 7591 and OpenID Connect Dynamic Client Registration.",
      tags: ["OAuth"],
      operationId: "registerOAuthClient",
      security: [], // Public endpoint - no authentication required
      requestBody: {
        required: true,
        content: {
          "application/json": {
            schema: z.toJSONSchema(DcrRegisterRequestSchema),
          },
        },
      },
      responses: {
        200: {
          description: "Client registered successfully",
          content: {
            "application/json": {
              schema: z.toJSONSchema(DcrRegisterResponseSchema),
            },
          },
        },
        400: {
          description: "Invalid client metadata",
          content: {
            "application/json": {
              schema: {
                type: "object",
                properties: {
                  error: { type: "string" },
                  error_description: { type: "string" },
                },
              },
            },
          },
        },
        429: {
          description: "Too many requests",
          content: {
            "application/json": {
              schema: z.toJSONSchema(DcrErrorResponseSchema),
            },
          },
        },
        500: {
          description: "Internal server error",
          content: {
            "application/json": {
              schema: z.toJSONSchema(DcrErrorResponseSchema),
            },
          },
        },
      },
    },
  };

  // Get tools from MCP server
  // Note: mcp-lite doesn't expose a public API to list tools,
  // so we'll need to manually track them or access internal state
  // For now, we'll document the tools we know about

  const tools = getMcpTools();

  for (const tool of tools) {
    // Create a pseudo-REST endpoint for each tool
    // Format: /mcp/tools/{toolName}
    const path = `/mcp/tools/${tool.name}`;

    spec.paths[path] = {
      post: {
        summary: tool.description || `Execute ${tool.name} tool`,
        description: tool.description,
        tags: ["MCP Tools"],
        operationId: `mcp_tool_${tool.name}`,
        requestBody: {
          required: true,
          content: {
            "application/json": {
              schema: tool.inputSchema || { type: "object" },
            },
          },
        },
        responses: {
          200: {
            description: "Tool execution result",
            content: {
              "application/json": {
                schema: tool.outputSchema || { type: "object" },
              },
            },
          },
          401: {
            description: "Unauthorized - missing or invalid OAuth token",
          },
          500: {
            description: "Tool execution error",
          },
        },
      },
    };
  }

  return spec;
}

/**
 * Get registered MCP tools with their schemas
 * This is a workaround since mcp-lite doesn't expose tool registry
 */
function getMcpTools() {
  // Use the actual schemas from our cubby definitions
  // Add descriptions to the input schema for better OpenAPI docs
  const DocumentedSearchInputSchema = SearchToolInputSchema.extend({
    deviceId: z.string().min(1).describe("Device ID to search on"),
    q: z.string().optional().describe("Search query text"),
    limit: z
      .number()
      .optional()
      .describe("Maximum number of results to return"),
    offset: z.number().optional().describe("Offset for pagination"),
    content_type: z
      .enum(["all", "ocr", "audio", "ui", "audio+ui", "ocr+ui", "audio+ocr"])
      .optional()
      .describe("Type of content to search for"),
    start_time: z
      .string()
      .datetime()
      .optional()
      .describe("Start time for search range (ISO 8601)"),
    end_time: z
      .string()
      .datetime()
      .optional()
      .describe("End time for search range (ISO 8601)"),
    app_name: z.string().optional().describe("Filter by application name"),
    window_name: z.string().optional().describe("Filter by window name"),
    frame_name: z.string().optional().describe("Filter by frame name"),
    include_frames: z
      .boolean()
      .optional()
      .describe("Include frame data in results"),
    min_length: z.number().optional().describe("Minimum content length"),
    max_length: z.number().optional().describe("Maximum content length"),
    speaker_ids: z
      .array(z.number())
      .optional()
      .describe("Filter by speaker IDs (for audio content)"),
    focused: z.boolean().optional().describe("Filter by window focus state"),
    browser_url: z.string().optional().describe("Filter by browser URL"),
  });

  return [
    {
      name: "search",
      description:
        "Search screen and audio content on a user's cubby device. Supports filtering by text query, content type (ocr/audio/ui), time range, application, and more. Returns structured results with OCR text, audio transcriptions, and UI element captures.",
      inputSchema: z.toJSONSchema(DocumentedSearchInputSchema),
      outputSchema: z.toJSONSchema(SearchResponseSchema),
    },
  ];
}
