/**
 * MCP Authentication Middleware
 *
 * Provides selective OAuth validation for MCP JSON-RPC requests.
 * Protocol methods (initialize, tools/list, etc.) work without auth.
 * Action methods (tools/call, resources/read, etc.) require OAuth tokens.
 */

import type { MiddlewareHandler } from "hono";
import { createMiddleware } from "hono/factory";
import type { Bindings, Variables } from "../index";
import { validateToken } from "./token_validator";

/**
 * MCP JSON-RPC methods that require OAuth authentication.
 * These methods access or modify user data and must have a valid token.
 */
const AUTH_REQUIRED_METHODS = [
  "tools/call", // Execute a tool (accesses user devices/data)
  "resources/read", // Read a resource (accesses user data)
  "resources/subscribe", // Subscribe to resource updates
  "prompts/get", // Get a prompt (may contain user data)
];

/**
 * Extended variables available in context after middleware runs
 */
export type McpAuthVariables = Variables & {
  bodyText?: string; // Original request body text (for reconstruction)
  rpcMethod?: string; // JSON-RPC method name
  authInfo?: {
    // OAuth authentication info (if validated)
    token: string;
    scopes: string[];
    extra: {
      env: Bindings;
      userId: string;
    };
  };
};

/**
 * MCP Auth Middleware
 *
 * Validates OAuth tokens conditionally based on the MCP method being called:
 * - Protocol methods (initialize, list, etc.): No auth required
 * - Action methods (tools/call, etc.): Auth required
 * - If token is present but invalid: Fail for auth-required methods only
 *
 * Sets context variables:
 * - bodyText: Original request body (needed for MCP handler)
 * - rpcMethod: The JSON-RPC method name
 * - authInfo: Validated authentication info (if token present and valid)
 */
export const mcpAuth = (): MiddlewareHandler<{
  Bindings: Bindings;
  Variables: McpAuthVariables;
}> => {
  return createMiddleware(async (c, next) => {
    // 1. Read and parse JSON-RPC request body
    const bodyText = await c.req.text();
    c.set("bodyText", bodyText);

    let body: any = undefined;
    try {
      body = JSON.parse(bodyText);
    } catch {
      // Let upstream MCP handler return protocol parse errors
      await next();
      return;
    }

    const method = body.method;
    c.set("rpcMethod", method);

    const requiresAuth = AUTH_REQUIRED_METHODS.includes(method);
    const authHeader = c.req.header("Authorization");
    let token = authHeader?.replace(/^Bearer\s+/i, "");
    if (token) token = token.trim().replace(/^"|"$/g, "");

    // Normalize JWT to base64url (header, payload, signature): replace +/ -> -_, strip '=' padding
    if (token) {
      const parts = token.split(".");
      if (parts.length === 3) {
        const normalize = (s: string) => s.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
        const [h, p, s] = parts;
        token = `${normalize(h)}.${normalize(p)}.${normalize(s)}`;
      }
    }

    // No session fallback: require header on auth-required methods (keep initialize/tools/list public)

    // 2. Validate token only when required (avoid failing initialize/tools/list)
    if (token && requiresAuth) {
      try {
        // Use shared token validator
        const validationResult = await validateToken(token, c.env);
        const { userId, scopes } = validationResult;

        if (userId && typeof userId === "string") {
          // Token is valid - set auth info
          c.set("authInfo", {
            token,
            scopes,
            extra: { env: c.env, userId },
          });
        } else if (requiresAuth) {
          // Token is missing user_id and method requires auth
          return c.json(
            {
              jsonrpc: "2.0",
              id: body.id,
              error: {
                code: -32001,
                message: "invalid token - missing user_id claim",
              },
            },
            401,
          );
        }
      } catch (error) {
        // Log exact verification error for diagnostics
        console.error("jwt verification error:", error);
        if (requiresAuth) {
          // Invalid/expired token and method requires auth - fail
          return c.json(
            {
              jsonrpc: "2.0",
              id: body.id,
              error: {
                code: -32001,
                message: "invalid or expired token",
              },
            },
            401,
          );
        }
        // Invalid token but method doesn't require auth - continue without authInfo
      }
    } else if (requiresAuth) {
      // No token provided and method requires auth
      const origin = new URL(c.req.url).origin;
      c.header("WWW-Authenticate", `Bearer resource_metadata="${origin}/.well-known/oauth-protected-resource/mcp"`);
      return c.json(
        {
          jsonrpc: "2.0",
          id: body.id,
          error: {
            code: -32001,
            message: "authentication required",
            data: {
              reason: "this method requires oauth authentication",
              oauth_info: "see /.well-known/oauth-protected-resource for oauth configuration",
              hint: "pass header: authorization: bearer <token>",
            },
          },
        },
        401,
      );
    }

    // 3. Continue to handler
    await next();
  });
};

