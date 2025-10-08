import { Hono } from "hono";
import type { Bindings, Variables } from "../index";
import {
  buildAuthorizationUrl,
  calculatePKCECodeChallenge,
  createOAuthContext,
  exchangeAuthorizationCode,
  generateRandomCodeVerifier,
  generateRandomState,
  validateCallbackParameters,
  type AuthorizationSession,
} from "../lib/oauth";
import {
  clearSessionCookie,
  readSessionCookie,
  writeSessionCookie,
} from "../lib/session";
import { renderCallbackPage } from "../views/callback_page";

const app = new Hono<{ Bindings: Bindings; Variables: Variables }>();

// OAuth configuration helper with optional domain override
function getOAuthConfig(
  env: Env,
  domainOverride?: string,
): {
  authorizationEndpoint: string;
  tokenEndpoint: string;
  clientId: string;
  clientSecret: string;
  redirectUri: string;
  scopes: string[];
  issuer: string;
} {
  // Use domain override if provided, otherwise fall back to env var
  const cubbyApiDomain = domainOverride || env.CUBBY_API_URL;

  return {
    authorizationEndpoint: `${cubbyApiDomain}/oauth/authorize`,
    tokenEndpoint: env.STYTCH_TOKEN_URL,
    clientId: env.STYTCH_CLIENT_ID,
    clientSecret: env.STYTCH_CLIENT_SECRET,
    redirectUri: env.REDIRECT_URI,
    scopes: env.REQUESTED_SCOPES.split(",").map((s) => s.trim()),
    issuer: env.STYTCH_ISSUER,
  };
}

// Error message helper
function getErrorMessage(error: unknown): string {
  if (error instanceof DOMException && error.name === "AbortError") {
    return "Token endpoint request timed out";
  }

  if (error instanceof Error) {
    return error.message;
  }

  return "Unknown error";
}

app.get("/connect", async (c) => {
  // Get domain from query parameter (user selection)
  const domainParam = c.req.query("domain");
  const oauthConfig = getOAuthConfig(c.env, domainParam);

  const codeVerifier = generateRandomCodeVerifier();
  const codeChallenge = await calculatePKCECodeChallenge(codeVerifier);
  const state = generateRandomState();

  const session: AuthorizationSession = {
    state,
    codeVerifier,
    issuedAt: Date.now(),
  };

  const secureCookies = c.env.SECURE_COOKIES === "true";
  await writeSessionCookie(c, session, c.env.SESSION_SECRET, secureCookies);

  const authorizationUrl = buildAuthorizationUrl(
    oauthConfig,
    state,
    codeChallenge,
  );
  return c.redirect(authorizationUrl.toString(), 302);
});

app.get("/callback", async (c) => {
  const oauthConfig = getOAuthConfig(c.env);
  const context = createOAuthContext(oauthConfig);
  const secureCookies = c.env.SECURE_COOKIES === "true";

  const session = await readSessionCookie(c, c.env.SESSION_SECRET);

  if (!session) {
    clearSessionCookie(c, secureCookies);
    return c.text(
      "Invalid or expired OAuth session. Start over from /connect",
      400,
    );
  }

  let callbackParameters: URLSearchParams;
  try {
    callbackParameters = validateCallbackParameters(
      context,
      new URL(c.req.url),
      session.state,
    );
  } catch (error) {
    console.error("Invalid callback parameters", error);
    clearSessionCookie(c, secureCookies);
    return c.text("Invalid callback parameters", 400);
  }

  try {
    const connection = await exchangeAuthorizationCode(
      context,
      callbackParameters,
      oauthConfig.redirectUri,
      session.codeVerifier,
    );

    clearSessionCookie(c, secureCookies);

    return c.html(renderCallbackPage(connection.accessToken));
  } catch (error) {
    console.error("Token exchange failed", error);
    clearSessionCookie(c, secureCookies);
    const message = getErrorMessage(error);
    return c.text(`Token exchange failed: ${message}`, 502);
  }
});

export default app;
