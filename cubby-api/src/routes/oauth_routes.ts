import { Hono, type MiddlewareHandler } from "hono";
import { HTTPException } from "hono/http-exception";
import { getCookie } from "hono/cookie";
import { z } from "zod/v4";
import stytch from "stytch";
import { Consumer } from "@hono/stytch-auth";
import type { Bindings, Variables } from "../index";
import type {
  IDPOAuthAuthorizeRequest,
  IDPOAuthAuthorizeStartRequest,
  IDPOAuthAuthorizeStartResponse,
} from "stytch";
import { renderOAuthConsentPage } from "../views/oauth_consent_page";

const rawOAuthSchema = z.object({
  client_id: z.string().min(1, "client_id is required"),
  redirect_uri: z.string().url("redirect_uri must be a valid URL"),
  response_type: z.literal("code").default("code"),
  scope: z.string().optional().default("openid"),
  state: z.string().optional(),
  nonce: z.string().optional(),
  code_challenge: z.string().optional(),
  prompt: z.string().optional(),
});

const rawSubmitSchema = rawOAuthSchema.extend({
  consent_granted: z
    .union([z.literal("true"), z.literal("false")])
    .optional()
    .default("true")
    .transform((value) => value !== "false"),
});

type RawOAuthParams = z.infer<typeof rawOAuthSchema>;
type RawSubmitParams = z.infer<typeof rawSubmitSchema>;

export type BaseOAuthParams = RawOAuthParams & { scopes: string[] };
type SubmitOAuthParams = RawSubmitParams & { scopes: string[] };

const app = new Hono<{ Bindings: Bindings; Variables: Variables }>();

function scopeStringToArray(scope: string): string[] {
  return scope
    .split(" ")
    .map((value) => value.trim())
    .filter((value) => value.length > 0);
}

function withScopes<T extends { scope: string }>(
  data: T,
): T & { scopes: string[] } {
  return {
    ...data,
    scopes: scopeStringToArray(data.scope),
  };
}

function firstString(value: unknown): string | undefined {
  if (typeof value === "string") {
    return value;
  }

  if (Array.isArray(value)) {
    for (const entry of value) {
      if (typeof entry === "string") {
        return entry;
      }
    }
  }

  return undefined;
}

function stringsFromValue(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.filter((entry): entry is string => typeof entry === "string");
  }

  if (typeof value === "string") {
    return [value];
  }

  return [];
}

function coerceScopeValue(scopeValue: unknown, scopesValue: unknown): string {
  const scope = firstString(scopeValue)?.trim();
  if (scope) {
    return scope;
  }

  const scopes = stringsFromValue(scopesValue)
    .map((value) => value.trim())
    .filter((value) => value.length > 0);

  return scopes.length > 0 ? scopes.join(" ") : "openid";
}

// Middleware to require authentication with redirect to sign-up
function requireAuthWithRedirect(): MiddlewareHandler {
  return async (c, next) => {
    const authMiddleware = Consumer.authenticateSessionLocal();

    try {
      await authMiddleware(c, async () => {
        await next();
      });
    } catch (error) {
      const currentUrl = new URL(c.req.url);
      const redirectTo = `${currentUrl.pathname}${currentUrl.search}`;
      const loginUrl = `/login?redirect_to=${encodeURIComponent(redirectTo)}`;
      return c.redirect(loginUrl, 302);
    }
  };
}

app.get("/authorize", requireAuthWithRedirect(), async (c) => {
  const scopeFromScopeParam = c.req.query("scope");
  const scopeFromScopesParam = c.req.queries("scopes");
  const scope = coerceScopeValue(scopeFromScopeParam, scopeFromScopesParam);

  const params = {
    client_id: c.req.query("client_id"),
    redirect_uri: c.req.query("redirect_uri"),
    response_type: c.req.query("response_type"),
    scope: scope || "",
    state: c.req.query("state"),
    nonce: c.req.query("nonce"),
    code_challenge: c.req.query("code_challenge"),
    prompt: c.req.query("prompt"),
  };

  const parsed = rawOAuthSchema.safeParse(params);
  if (!parsed.success) {
    throw new HTTPException(400, { message: z.prettifyError(parsed.error) });
  }

  const normalized = withScopes(parsed.data);

  const sessionJWT = getCookie(c, "stytch_session_jwt");
  if (!sessionJWT) {
    throw new HTTPException(401, { message: "Session JWT not found" });
  }

  const client = new stytch.Client({
    project_id: c.env.STYTCH_PROJECT_ID,
    secret: c.env.STYTCH_PROJECT_SECRET,
    custom_base_url: "https://login.cubby.sh",
  });

  const startReq: IDPOAuthAuthorizeStartRequest = {
    client_id: normalized.client_id,
    redirect_uri: normalized.redirect_uri,
    response_type: "code",
    scopes: normalized.scopes,
    session_jwt: sessionJWT,
  };

  let startResp: IDPOAuthAuthorizeStartResponse;
  try {
    startResp = await client.idp.oauth.authorizeStart(startReq);
  } catch (error) {
    console.error("authorizeStart failed", error);
    throw new HTTPException(502, {
      message: "Authorization service error (start)",
    });
  }

  if (!startResp.consent_required) {
    const { scope: _scope, ...stytchData } = normalized;
    const authReq: IDPOAuthAuthorizeRequest = {
      ...stytchData,
      consent_granted: true,
      session_jwt: sessionJWT,
    };

    try {
      const authResp = await client.idp.oauth.authorize(authReq);
      return c.redirect(authResp.redirect_uri, 302);
    } catch (error) {
      console.error("authorize failed", error);
      throw new HTTPException(502, {
        message: "Authorization service error (authorize)",
      });
    }
  }

  const html = renderOAuthConsentPage(startResp, normalized);
  return c.html(html);
});

app.post("/authorize/submit", requireAuthWithRedirect(), async (c) => {
  const body = await c.req.parseBody();

  const submitParams = {
    client_id: firstString(body.client_id),
    redirect_uri: firstString(body.redirect_uri),
    response_type: firstString(body.response_type),
    scope: coerceScopeValue(body.scope, body.scopes) || "",
    state: firstString(body.state),
    nonce: firstString(body.nonce),
    code_challenge: firstString(body.code_challenge),
    prompt: firstString(body.prompt),
    consent_granted: firstString(body.consent_granted),
  };

  const parsed = rawSubmitSchema.safeParse(submitParams);
  if (!parsed.success) {
    throw new HTTPException(400, { message: z.prettifyError(parsed.error) });
  }

  const normalized: SubmitOAuthParams = withScopes(parsed.data);

  const sessionJWT = getCookie(c, "stytch_session_jwt");
  if (!sessionJWT) {
    throw new HTTPException(401, { message: "Session JWT not found" });
  }

  const client = new stytch.Client({
    project_id: c.env.STYTCH_PROJECT_ID,
    secret: c.env.STYTCH_PROJECT_SECRET,
    custom_base_url: "https://login.cubby.sh",
  });

  const { scope: _scope, consent_granted, ...stytchData } = normalized;
  const authReq: IDPOAuthAuthorizeRequest = {
    ...stytchData,
    consent_granted,
    session_jwt: sessionJWT,
  };

  try {
    const authResp = await client.idp.oauth.authorize(authReq);
    return c.redirect(authResp.redirect_uri, 302);
  } catch (error) {
    console.error("authorize (submit) failed", error);
    throw new HTTPException(502, {
      message: "Authorization service error (submit)",
    });
  }
});

export default app;
