/**
 * MCP (Model Context Protocol) Client
 *
 * Implements JSON-RPC 2.0 client for calling MCP tools
 */

export type McpToolResult = {
  content: Array<{ type: string; text: string }>;
  structuredContent?: any;
};

export type McpResponse = {
  jsonrpc: "2.0";
  id: number | string;
  result?: McpToolResult;
  error?: {
    code: number;
    message: string;
    data?: any;
  };
};

/**
 * Call an MCP tool via JSON-RPC 2.0
 *
 * @param mcpEndpoint - The MCP server endpoint URL
 * @param accessToken - OAuth access token for authentication
 * @param toolName - Name of the tool to call
 * @param args - Tool arguments
 * @param timeoutMs - Request timeout in milliseconds
 * @returns Tool execution result
 */
export async function callMcpTool(
  mcpEndpoint: string,
  accessToken: string,
  toolName: string,
  args: Record<string, any>,
  timeoutMs = 30_000,
): Promise<McpToolResult> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);

  try {
    // Construct JSON-RPC 2.0 request
    const request = {
      jsonrpc: "2.0" as const,
      id: Date.now(),
      method: "tools/call",
      params: {
        name: toolName,
        arguments: args,
      },
    };

    const response = await fetch(mcpEndpoint, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${accessToken}`,
      },
      body: JSON.stringify(request),
      signal: controller.signal,
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(
        `MCP request failed: ${response.status} ${response.statusText} - ${errorText}`,
      );
    }

    const data: McpResponse = await response.json();

    // Check for JSON-RPC error
    if (data.error) {
      throw new Error(`MCP tool error: ${data.error.message}`);
    }

    if (!data.result) {
      throw new Error("MCP response missing result");
    }

    return data.result;
  } finally {
    clearTimeout(timeout);
  }
}
