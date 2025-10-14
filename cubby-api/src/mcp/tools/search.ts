import { mcpServer } from "../server";
import {
  SearchToolInputSchema,
  SearchResponseSchema,
  normalizeSearchResponse,
  type SearchToolInput,
  type SearchResponse,
} from "../../schemas/cubby";
import type { MCPServerContext } from "mcp-lite";
import { getMcpAuthContext } from "../context";
import { createDbClient } from "../../db/client";
import { getDeviceForUser } from "../../db/devices_repo";
import { isPathAllowed } from "../../proxy_config";

/**
 * MCP Search Tool
 *
 * Provides AI assistants with the ability to search cubby content
 * on authenticated user devices.
 */

/**
 * Register the search tool with the MCP server
 */
mcpServer.tool("search", {
  description:
    "Search screen and audio content on a user's cubby device. Supports filtering by text query, content type (ocr/audio/ui), time range, application, and more.",
  inputSchema: SearchToolInputSchema,
  outputSchema: SearchResponseSchema,
  handler: async (args: SearchToolInput, ctx: MCPServerContext) => {
    const { env, userId } = getMcpAuthContext(ctx);

    // Extract deviceId from args and prepare query params
    const { deviceId, ...queryParams } = args;

    // Validate device ID format
    if (!/^[a-zA-Z0-9-]+$/.test(deviceId)) {
      throw new Error("Invalid device ID format");
    }

    // Verify the path is allowed in our proxy config
    const path = "/search";
    const method = "GET";
    if (!isPathAllowed(path, method)) {
      throw new Error("Search endpoint not allowed");
    }

    // Verify user owns this device
    const db = createDbClient(env.DATABASE_URL);
    const device = await getDeviceForUser(db, deviceId, userId);

    if (!device) {
      throw new Error(`Device not found or access denied: ${deviceId}`);
    }

    // Build query string from parameters
    const searchParams = new URLSearchParams();
    for (const [key, value] of Object.entries(queryParams)) {
      if (value !== undefined && value !== null) {
        if (Array.isArray(value)) {
          // Handle array parameters (e.g., speaker_ids)
          value.forEach((v) => searchParams.append(key, String(v)));
        } else {
          searchParams.append(key, String(value));
        }
      }
    }

    // Construct target URL
    const queryString = searchParams.toString();
    const targetUrl = `https://${deviceId}.${env.TUNNEL_DOMAIN}${path}${queryString ? "?" + queryString : ""}`;

    // Make authenticated request to device
    const requestId = crypto.randomUUID();
    console.log(
      `MCP search tool proxying to device ${deviceId}${path} (request ID: ${requestId})`,
    );

    try {
      const response = await fetch(targetUrl, {
        method: "GET",
        headers: {
          "CF-Access-Client-Id": env.ACCESS_CLIENT_ID,
          "CF-Access-Client-Secret": env.ACCESS_CLIENT_SECRET,
          "X-Cubby-Request-Id": requestId,
        },
      });

      if (!response.ok) {
        const errorText = await response.text();
        console.error(
          `Device proxy error: ${response.status} ${response.statusText} - ${errorText}`,
        );
        throw new Error(
          `Device request failed: ${response.status} ${response.statusText}`,
        );
      }

      const data = await response.json();

      // Normalise and validate response payload
      const normalized: SearchResponse = normalizeSearchResponse(data);

      // Return MCP tool response with structured content
      return {
        content: [
          {
            type: "text" as const,
            text: `Found ${normalized.total} results from device ${deviceId}. Showing ${normalized.results.length} results.`,
          },
        ],
        structuredContent: normalized,
      };
    } catch (error) {
      console.error("MCP search tool error:", error);
      throw new Error(
        `Failed to search device: ${error instanceof Error ? error.message : "Unknown error"}`,
      );
    }
  },
});
