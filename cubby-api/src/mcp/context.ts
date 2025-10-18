import type { Ctx as MCPServerContext } from "mcp-lite";
import type { Bindings } from "../index";

export interface McpAuthContext {
  env: Bindings;
  userId: string;
}

export function getMcpAuthContext(ctx: MCPServerContext): McpAuthContext {
  const extra = ctx.authInfo?.extra as Partial<McpAuthContext> | undefined;
  if (
    !extra?.env ||
    typeof extra.userId !== "string" ||
    extra.userId.length === 0
  ) {
    throw new Error("Missing MCP auth context");
  }

  return {
    env: extra.env,
    userId: extra.userId,
  };
}
