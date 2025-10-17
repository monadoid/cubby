/**
 * Shared Token Validation Logic
 * 
 * Handles both Stytch session JWTs and OAuth access tokens with proper issuer detection.
 */

import { createRemoteJWKSet, jwtVerify } from "jose";
import type { Bindings } from "../index";

export type TokenValidationResult = {
  userId: string;
  scopes: string[];
  payload: any;
};

/**
 * Validates a JWT token, automatically detecting whether it's a session JWT or OAuth token
 * based on the issuer claim in the token payload.
 * 
 * @param token - The JWT token string (without "Bearer " prefix)
 * @param env - Environment bindings containing Stytch configuration
 * @returns Validation result with userId, scopes, and full payload
 * @throws Error if token is invalid or expired
 */
export async function validateToken(
  token: string,
  env: Bindings,
): Promise<TokenValidationResult> {
  // Normalize JWT to base64url (header, payload, signature): replace +/ -> -_, strip '=' padding
  const parts = token.split(".");
  if (parts.length === 3) {
    const normalize = (s: string) =>
      s.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
    const [h, p, s] = parts;
    token = `${normalize(h)}.${normalize(p)}.${normalize(s)}`;
  }

  // Inspect issuer from payload to choose correct JWKS and issuer expectation
  let claimsIss: string | undefined;
  try {
    const payloadPart = token.split(".")[1];
    const payload = JSON.parse(
      atob(payloadPart.replace(/-/g, "+").replace(/_/g, "/")),
    );
    claimsIss = payload?.iss as string | undefined;
  } catch {
    // ignore decode errors, let jwtVerify handle them
  }

  let result;
  if (
    claimsIss &&
    (claimsIss.startsWith("stytch.com/") ||
      claimsIss.startsWith("https://stytch.com/"))
  ) {
    // Session JWT path (Stytch issues iss like "stytch.com/<project-id>")
    const projectId = env.STYTCH_PROJECT_ID || "";
    const isTest = projectId.startsWith("project-test-");
    const jwksUrl =
      (isTest ? "https://test.stytch.com" : "https://stytch.com") +
      `/v1/sessions/jwks/${projectId}`;

    const JWKS = createRemoteJWKSet(new URL(jwksUrl));
    result = await jwtVerify(token, JWKS, {
      issuer: claimsIss,
      audience: projectId,
    });
  } else {
    // OAuth access token path (custom domain issuer)
    const jwksUrl = `${env.STYTCH_PROJECT_DOMAIN}/.well-known/jwks.json`;

    // Prime Cloudflare edge cache for the JWKS URL
    try {
      await fetch(jwksUrl, {
        cf: { cacheEverything: true, cacheTtl: 3600 },
      });
    } catch (error) {
      console.error(
        `failed to prime cloudflare edge cache for ${jwksUrl}:`,
        error,
      );
    }

    const JWKS = createRemoteJWKSet(new URL(jwksUrl));
    result = await jwtVerify(token, JWKS, {
      issuer: env.STYTCH_PROJECT_DOMAIN,
      audience: env.STYTCH_PROJECT_ID,
    });
  }

  const userId = (result.payload as any).user_id;
  if (!userId || typeof userId !== "string") {
    throw new Error("token missing user_id claim");
  }

  const scopeString = (result.payload as any).scope as string | undefined;
  const scopes = scopeString ? scopeString.split(" ").filter(Boolean) : [];

  return {
    userId,
    scopes,
    payload: result.payload,
  };
}

