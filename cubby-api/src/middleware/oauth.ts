import type { MiddlewareHandler } from "hono";
import { createMiddleware } from "hono/factory";
import { z } from "zod";
import type { Bindings, Variables } from "../index";
import { errors } from "../errors";
import { validateToken } from "./token_validator";

/**
 * OAuth Authentication Middleware
 *
 * Supports both OAuth access tokens and Stytch session JWTs.
 * Automatically detects token type based on issuer claim.
 *
 * Token Types:
 * - OAuth Access Token: Issuer is PROJECT_DOMAIN (e.g., https://login.cubby.sh)
 * - Session JWT: Issuer is stytch.com/{PROJECT_ID}
 *
 * Both token types include user_id claim and optional scopes.
 *
 * @param opts.requiredScopes - Optional array of scopes required for this endpoint
 */

export type OAuthOptions = {
  requiredScopes?: string[];
};

export const AuthUserSchema = z.object({
  userId: z.uuid(),
  issuer: z.string(),
  audiences: z.array(z.string()),
  scopes: z.array(z.string()),
  claims: z.any(),
});

export type AuthUser = z.infer<typeof AuthUserSchema>;

export const oauth = (
  opts: OAuthOptions = {},
): MiddlewareHandler<{ Bindings: Bindings; Variables: Variables }> => {
  return createMiddleware(async (c, next) => {
    // Extract token from Authorization header, or query param (for WS/browser)
    let token = c.req.header("Authorization")?.replace(/^Bearer\s+/i, "");
    if (!token) {
      const url = new URL(c.req.url);
      const qp = url.searchParams.get("access_token");
      if (qp) token = qp;
    }
    if (!token) throw errors.auth.MISSING_TOKEN();

    // Trim and remove any quotes
    token = token.trim().replace(/^"|"$/g, "");

    // Validate token using shared validator
    let validationResult;
    try {
      validationResult = await validateToken(token, c.env);
    } catch (error) {
      console.error("jwt verification failed:", error);
      throw errors.auth.INVALID_TOKEN();
    }

    const { userId, scopes, payload } = validationResult;

    // Validate required scopes
    if (
      opts.requiredScopes?.length &&
      !opts.requiredScopes.every((s) => scopes.includes(s))
    ) {
      throw errors.auth.INSUFFICIENT_SCOPE(opts.requiredScopes);
    }

    // Normalize audiences to array
    const aud = payload.aud;
    const audiences = Array.isArray(aud)
      ? aud.map(String)
      : aud
        ? [String(aud)]
        : [];

    // Map to AuthUser schema
    const parseResult = AuthUserSchema.safeParse({
      userId,
      issuer: payload.iss,
      audiences,
      scopes,
      claims: payload,
    });

    if (!parseResult.success) {
      throw errors.auth.INVALID_AUTH_DATA();
    }

    // Set context variables
    c.set("auth", parseResult.data);
    c.set("userId", parseResult.data.userId);
    await next();
  });
};
