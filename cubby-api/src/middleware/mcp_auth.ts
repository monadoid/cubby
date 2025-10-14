/**
 * MCP Authentication Middleware
 *
 * Provides selective OAuth validation for MCP JSON-RPC requests.
 * Protocol methods (initialize, tools/list, etc.) work without auth.
 * Action methods (tools/call, resources/read, etc.) require OAuth tokens.
 */

import type { MiddlewareHandler } from "hono";
import { createMiddleware } from "hono/factory";
import { createRemoteJWKSet, jwtVerify } from "jose";
import type { Bindings, Variables } from "../index";

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

    let body;
    try {
      body = JSON.parse(bodyText);
    } catch {
      // Invalid JSON - let MCP handler deal with protocol errors
      await next();
      return;
    }

    const method = body.method;
    c.set("rpcMethod", method);

    const requiresAuth = AUTH_REQUIRED_METHODS.includes(method);
    const token = c.req.header("Authorization")?.replace(/^Bearer\s+/i, "");

    // 2. Validate token if present OR if method requires auth
    if (token) {
      try {
        const JWKS = createRemoteJWKSet(
          new URL(`${c.env.STYTCH_PROJECT_DOMAIN}/.well-known/jwks.json`),
        );

        const result = await jwtVerify(token, JWKS, {
          issuer: c.env.STYTCH_PROJECT_DOMAIN,
          audience: c.env.STYTCH_PROJECT_ID,
        });

        const userId = (result.payload as any).user_id;
        const scopeString = (result.payload as any).scope as
          | string
          | undefined;
        const scopes = scopeString
          ? scopeString.split(" ").filter(Boolean)
          : [];

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
                message: "Invalid token - missing user_id claim",
              },
            },
            401,
          );
        }
      } catch (error) {
        console.error("JWT verification failed for MCP request:", error);
        if (requiresAuth) {
          // Invalid/expired token and method requires auth - fail
          return c.json(
            {
              jsonrpc: "2.0",
              id: body.id,
              error: {
                code: -32001,
                message: "Invalid or expired token",
              },
            },
            401,
          );
        }
        // Invalid token but method doesn't require auth - continue without authInfo
      }
    } else if (requiresAuth) {
      // No token provided but method requires auth
      return c.json(
        {
          jsonrpc: "2.0",
          id: body.id,
          error: {
            code: -32001,
            message: "Authentication required",
            data: {
              reason: "This method requires OAuth authentication",
              oauth_info:
                "See /.well-known/oauth-protected-resource for OAuth configuration",
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

