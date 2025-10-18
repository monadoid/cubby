// cubby-api - main API server for cubby
import { Hono } from "hono";
import { HTTPException } from "hono/http-exception";
import { cors } from "hono/cors";
import { setCookie } from "hono/cookie";
import { describeRoute, resolver } from "hono-openapi";
import { zValidator } from "@hono/zod-validator";
import { z } from "zod/v4";
import stytch from "stytch";
import {
  buildCnameForTunnel,
  buildIngressForHost,
  CloudflareClient,
} from "./clients/cloudflare";
import { createDbClient } from "./db/client";
import {
  createDevice,
  getDeviceForUser,
  getDevicesByUserId,
} from "./db/devices_repo";
import { createUser, createUserSchema } from "./db/users_repo";
import { oauth, type AuthUser } from "./middleware/oauth";
import { session } from "./middleware/session";
import { mcpAuth } from "./middleware/mcp_auth";
import oauthRoutes from "./routes/oauth_routes";
import authViews from "./routes/auth_views";
import m2mRoutes from "./routes/m2m";
import { strictJSONResponse } from "./helpers";
import { isPathAllowed } from "./proxy_config";
import { mcpHttpHandler } from "./mcp/handler";
import { generateMcpOpenAPISpec } from "./mcp/openapi";
import { generateGatewayOpenAPISpec } from "./gateway_openapi";
import {
  DcrRegisterRequestSchema,
  DcrRegisterResponseSchema,
} from "./schemas/dcr";
import {
  getSession,
  getOrCreateSession,
  getGwSessionId,
  setSessionAuth,
} from "./mcp/session_store";
import { postMcp, getMcpSse } from "./mcp/device_client";
import {
  GATEWAY_TOOLS,
  handleDevicesList,
  handleDevicesSet,
} from "./mcp/gateway_tools";

// Explicit bindings type for lints; wrangler will still provide runtime types
type Bindings = {
  STYTCH_PROJECT_ID: string;
  STYTCH_PROJECT_SECRET: string;
  STYTCH_PROJECT_DOMAIN: string;
  CF_API_TOKEN: string;
  CF_ACCOUNT_ID: string;
  CF_ZONE_ID: string;
  ACCESS_CLIENT_ID: string;
  ACCESS_CLIENT_SECRET: string;
  TUNNEL_DOMAIN: string;
  DATABASE_URL: string;
  STYTCH_BASE_URL?: string;
  TEST_EMAIL: string;
  TEST_PASSWORD: string;
};

type Variables = {
  auth: AuthUser;
  userId: string;
  session?: any;
};

export type { Bindings, Variables };

const signUpRequestSchema = createUserSchema.pick({ email: true }).extend({
  password: z.string(),
});

const signUpResponseSchema = z.object({
  user_id: z.string(),
  session_token: z.string(),
  session_jwt: z.string(),
});

const stytchSuccessResponseSchema = z
  .object({
    user_id: z.string(),
    session_token: z.string(),
    session_jwt: z.string(),
  })
  .transform((data) => ({
    auth_id: data.user_id,
    ...data,
  }));

const stytchErrorResponseSchema = z.object({
  error_message: z.string(),
  error_type: z.string(),
});

// Device enrollment schemas
const deviceEnrollRequestSchema = z.object({});

const deviceEnrollResponseSchema = z.object({
  device_id: z.string(),
  hostname: z.string(),
  tunnel_token: z.string(),
});

const whoamiResponseSchema = z.object({
  ok: z.boolean(),
  sub: z.string(),
  iss: z.string(),
  aud: z.array(z.string()),
  scopes: z.array(z.string()),
  claims: z.any(),
});

const app = new Hono<{ Bindings: Bindings; Variables: Variables }>();

function isDevEnvironment(env: Bindings): boolean {
  return Boolean(env.STYTCH_BASE_URL?.includes("test.stytch.com"));
}

// CORS middleware - allow any origin for token-protected API
// Security is enforced by Bearer token validation, not origin restrictions
app.use(
  "/*",
  cors({
    origin: "*",
    allowMethods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
    allowHeaders: [
      "Content-Type",
      "Authorization",
      "mcp-protocol-version", // MCP protocol header
    ],
    exposeHeaders: ["mcp-protocol-version"],
    credentials: true,
  }),
);

// Global error handler
app.onError((err, c) => {
  if (err instanceof HTTPException) {
    console.error("HTTP Exception:", err.status, err.message);
    return err.getResponse();
  }
  console.error("Unhandled error:", err);
  return c.json({ error: "Internal Server Error" }, 500);
});

// OAuth token endpoint - must be defined BEFORE mounting oauth routes
// Simple passthrough proxy to avoid CORS issues with browser-based OAuth clients
// Public clients use PKCE (code_verifier), confidential clients include client_secret
app.post("/oauth/token", async (c) => {
  try {
    const body = await c.req.text();

    // Simple passthrough proxy - no modification
    const response = await fetch(
      `${c.env.STYTCH_PROJECT_DOMAIN}/v1/oauth2/token`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/x-www-form-urlencoded",
        },
        body: body,
      },
    );

    const data = await response.json();

    if (!response.ok) {
      console.error("Token exchange failed:", data);
    } else {
      console.log("Token exchange successful");
    }

    return c.json(data, response.status as any);
  } catch (error) {
    console.error("Token exchange error:", error);
    return c.json({ error: "Token exchange failed" }, 500);
  }
});

// OAuth Dynamic Client Registration endpoint
// Passthrough proxy to Stytch's DCR endpoint for third-party clients
// No authentication required - this is a public endpoint per OAuth 2.0 DCR spec
app.post("/oauth/register", async (c) => {
  try {
    const body = await c.req.json();

    // Validate request body
    const validated = DcrRegisterRequestSchema.safeParse(body);
    if (!validated.success) {
      console.error("DCR validation failed:", validated.error);
      return c.json(
        {
          error: "invalid_client_metadata",
          error_description: "Invalid client registration request",
        },
        400,
      );
    }

    // Forward to Stytch DCR endpoint
    const response = await fetch(
      `${c.env.STYTCH_PROJECT_DOMAIN}/v1/oauth2/register`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(validated.data),
      },
    );

    const data = await response.json();

    if (!response.ok) {
      console.error("Client registration failed:", data);
    } else {
      const successData = DcrRegisterResponseSchema.safeParse(data);
      if (successData.success) {
        console.log(
          "Client registration successful:",
          successData.data.client_id,
        );
      } else {
        console.log("Client registration successful");
      }
    }

    return c.json(data, response.status as any);
  } catch (error) {
    console.error("Client registration error:", error);
    return c.json(
      {
        error: "server_error",
        error_description: "Client registration failed",
      },
      500,
    );
  }
});

// Routes
app.route("/oauth", oauthRoutes);
app.route("/", authViews);
app.route("/m2m", m2mRoutes);

app.post(
  "/sign-up",
  describeRoute({
    description: "Create a new user account",
    responses: {
      201: {
        description: "User created successfully",
        content: {
          "application/json": { schema: resolver(signUpResponseSchema) },
        },
      },
      400: {
        description: "Bad request - invalid email or duplicate user",
      },
      500: {
        description: "Internal server error",
      },
    },
    tags: ["Authentication"],
  }),
  async (c) => {
    // Handle both form data (htmx) and JSON (API)
    const contentType = c.req.header("content-type") || "";
    let email: string;
    let password: string;
    let redirectTo: string | undefined;

    if (contentType.includes("application/x-www-form-urlencoded")) {
      const formData = await c.req.parseBody();
      email = formData.email as string;
      password = formData.password as string;
      redirectTo = formData.redirect_to as string | undefined;
    } else {
      const validated = signUpRequestSchema.safeParse(await c.req.json());
      if (!validated.success) {
        return c.json({ error: "Invalid request data" }, 400);
      }
      email = validated.data.email;
      password = validated.data.password;
    }

    if (!email || !password) {
      const errorMsg = "Email and password are required";
      if (contentType.includes("application/x-www-form-urlencoded")) {
        return c.html(`<div class="error">${errorMsg}</div>`);
      }
      return c.json({ error: errorMsg }, 400);
    }

    const newUserId = crypto.randomUUID();

    try {
      const client = new stytch.Client({
        project_id: c.env.STYTCH_PROJECT_ID,
        secret: c.env.STYTCH_PROJECT_SECRET,
        ...(isDevEnvironment(c.env) ? {} : { custom_base_url: "https://login.cubby.sh" }),
      });

      const response = await client.passwords.create({
        email,
        password,
        session_duration_minutes: 60,
        trusted_metadata: { user_id: newUserId },
        session_custom_claims: { user_id: newUserId },
      });

      const db = createDbClient(c.env.DATABASE_URL);
      await createUser(db, {
        id: newUserId,
        authId: response.user_id,
        email,
      });

      // Set session cookie
      setCookie(c, "stytch_session_jwt", response.session_jwt, {
        path: "/",
        secure: true,
        httpOnly: true,
        sameSite: "Lax",
        maxAge: 60 * 60, // 1 hour
      });

      // Return appropriate response based on request type
      if (contentType.includes("application/x-www-form-urlencoded")) {
        // htmx request - trigger redirect
        const redirect = redirectTo || "/";
        c.header("HX-Redirect", redirect);
        return c.html("");
      } else {
        // JSON API request
        return strictJSONResponse(
          c,
          signUpResponseSchema,
          {
            user_id: newUserId,
            session_token: response.session_token,
            session_jwt: response.session_jwt,
          },
          201,
        );
      }
    } catch (error: any) {
      console.error("Sign-up error:", error);
      console.error("Error keys:", Object.keys(error || {}));
      console.error("Error message:", error?.error_message);
      console.error("Error type:", error?.error_type);
      console.error("Full error object:", JSON.stringify(error, null, 2));

      // Extract Stytch error details
      const statusCode = error?.status_code || 500;
      const errorMsg =
        error?.error_message || error?.message || "Failed to create account";

      if (contentType.includes("application/x-www-form-urlencoded")) {
        return c.html(`<div class="error">${errorMsg}</div>`);
      }
      return c.json({ error: errorMsg }, statusCode);
    }
  },
);

app.post(
  "/login",
  describeRoute({
    description: "Authenticate a user",
    responses: {
      200: {
        description: "User authenticated successfully",
        content: {
          "application/json": { schema: resolver(signUpResponseSchema) }, // Same schema as sign-up
        },
      },
      400: {
        description: "Bad request - invalid credentials",
      },
      401: {
        description: "Unauthorized - incorrect email or password",
      },
      500: {
        description: "Internal server error",
      },
    },
    tags: ["Authentication"],
  }),
  async (c) => {
    // Handle both form data (htmx) and JSON (API)
    const contentType = c.req.header("content-type") || "";
    let email: string;
    let password: string;
    let redirectTo: string | undefined;

    if (contentType.includes("application/x-www-form-urlencoded")) {
      const formData = await c.req.parseBody();
      email = formData.email as string;
      password = formData.password as string;
      redirectTo = formData.redirect_to as string | undefined;
    } else {
      const validated = signUpRequestSchema.safeParse(await c.req.json());
      if (!validated.success) {
        return c.json({ error: "Invalid request data" }, 400);
      }
      email = validated.data.email;
      password = validated.data.password;
    }

    if (!email || !password) {
      const errorMsg = "Email and password are required";
      if (contentType.includes("application/x-www-form-urlencoded")) {
        return c.html(`<div class="error">${errorMsg}</div>`);
      }
      return c.json({ error: errorMsg }, 400);
    }

    try {
      // Diagnostic logging for production debugging
      console.log("login - env keys available:", Object.keys(c.env || {}));
      console.log("login - STYTCH_PROJECT_ID exists:", !!c.env.STYTCH_PROJECT_ID);
      
      const client = new stytch.Client({
        project_id: c.env.STYTCH_PROJECT_ID,
        secret: c.env.STYTCH_PROJECT_SECRET,
        ...(isDevEnvironment(c.env) ? {} : { custom_base_url: "https://login.cubby.sh" }),
      });

      const response = await client.passwords.authenticate({
        email,
        password,
        session_duration_minutes: 60,
      });

      // Set session cookie
      setCookie(c, "stytch_session_jwt", response.session_jwt, {
        path: "/",
        secure: true,
        httpOnly: true,
        sameSite: "Lax",
        maxAge: 60 * 60, // 1 hour
      });

      // Return appropriate response based on request type
      if (contentType.includes("application/x-www-form-urlencoded")) {
        // htmx request - trigger redirect
        const redirect = redirectTo || "/";
        c.header("HX-Redirect", redirect);
        return c.html("");
      } else {
        // JSON API request
        return c.json(
          {
            user_id: response.user_id,
            session_token: response.session_token,
            session_jwt: response.session_jwt,
          },
          200,
        );
      }
    } catch (error: any) {
      console.error("Login error:", error);

      // Extract Stytch error details
      const statusCode = error?.status_code || 401;
      const errorMsg = error?.error_message || "Invalid email or password";

      if (contentType.includes("application/x-www-form-urlencoded")) {
        return c.html(`<div class="error">${errorMsg}</div>`);
      }
      return c.json({ error: errorMsg }, statusCode);
    }
  },
);

app.post(
  "/devices/enroll",
  describeRoute({
    description: "Enroll a new device and create Cloudflare tunnel",
    responses: {
      200: {
        description: "Device enrolled successfully",
        content: {
          "application/json": { schema: resolver(deviceEnrollResponseSchema) },
        },
      },
      400: {
        description: "Bad request - invalid device information",
      },
      401: {
        description: "Unauthorized - invalid or missing session",
      },
      500: {
        description: "Internal server error",
      },
    },
    tags: ["Devices"],
  }),
  session(),
  zValidator("json", deviceEnrollRequestSchema),
  async (c) => {
    const _ = c.req.valid("json");

    try {
      // Get user_id from context (set by sessionAuth middleware)
      const userId = c.get("userId");
      console.log('The "userId" from the session is:', userId);

      const db = createDbClient(c.env.DATABASE_URL);

      const device = await createDevice(db, {
        userId,
      });

      const device_id = device.id;

      // Initialize Cloudflare client
      const cf = new CloudflareClient({
        apiToken: c.env.CF_API_TOKEN,
        accountId: c.env.CF_ACCOUNT_ID,
        zoneId: c.env.CF_ZONE_ID,
      });

      const name = `cubby-${device_id}`;
      const hostname = `${device_id}.cubby.sh`;

      // 1) Create or reuse tunnel (idempotent)
      const createdOrExisting = await cf.createOrGetTunnel(name);
      const tunnel_id = createdOrExisting.id;

      // 2) Ensure config is correct (PUT is idempotent)
      await cf.putTunnelConfig(
        tunnel_id,
        buildIngressForHost(hostname, "http://localhost:3030"),
      );

      // 3) Ensure DNS points to the tunnel (idempotent)
      await cf.upsertCnameRecord(buildCnameForTunnel(hostname, tunnel_id));

      // 4) Ensure we have a token (create may not return it)
      const tunnel_token =
        createdOrExisting.token ?? (await cf.getTunnelToken(tunnel_id));

      return strictJSONResponse(
        c,
        deviceEnrollResponseSchema,
        {
          device_id,
          hostname,
          tunnel_token,
        },
        200,
      );
    } catch (error) {
      console.error("Device enrollment error:", error);
      return c.json({ error: "Failed to enroll device" }, 500);
    }
  },
);

app.get(
  "/devices",
  describeRoute({
    description: "List all devices for the authenticated user",
    responses: {
      200: {
        description: "List of user devices",
        content: {
          "application/json": {
            schema: resolver(
              z.object({
                devices: z.array(
                  z.object({
                    id: z.string(),
                    userId: z.string().uuid(),
                    createdAt: z.string(),
                    updatedAt: z.string(),
                  }),
                ),
              }),
            ),
          },
        },
      },
      500: {
        description: "Internal server error",
      },
    },
    tags: ["Devices"],
  }),
  oauth({
    // TODO: Add proper scope when designing scope system (e.g., 'read:devices' or 'read:user')
    // requiredScopes: ['read:user']
  }),
  async (c) => {
    try {
      const userId = c.get("userId");
      const db = createDbClient(c.env.DATABASE_URL);

      const userDevices = await getDevicesByUserId(db, userId);

      return c.json({
        devices: userDevices,
      });
    } catch (error) {
      console.error("Error fetching devices:", error);
      return c.json({ error: "Failed to fetch devices" }, 500);
    }
  },
);

// Generic proxy to device endpoints with allowlist-based security
app.all(
  "/devices/:deviceId/*",
  oauth({
    // TODO: Add proper scope when designing scope system (e.g., 'read:devices' or 'access:device')
    // requiredScopes: ['read:user']
  }),
  async (c) => {
    const { deviceId } = c.req.param();
    const path = c.req.path.replace(`/devices/${deviceId}`, "");
    const method = c.req.method;

    // Validate device ID format (alphanumeric and hyphens only)
    if (!/^[a-zA-Z0-9-]+$/.test(deviceId)) {
      return c.json({ error: "Invalid device ID format" }, 400);
    }

    // Check if the requested path and method are in the allowlist
    if (!isPathAllowed(path, method)) {
      console.warn(`Blocked request to disallowed endpoint: ${method} ${path}`);
      return c.json({ error: "Endpoint not allowed" }, 403);
    }

    const userId = c.get("userId");
    const db = createDbClient(c.env.DATABASE_URL);
    const device = await getDeviceForUser(db, deviceId, userId);

    if (!device) {
      console.warn(`Denied access to device ${deviceId} for user ${userId}`);
      return c.json({ error: "Device not found" }, 404);
    }

    try {
      // Build target URL with query parameters preserved
      const url = new URL(c.req.url);
      const targetUrl = `https://${deviceId}.${c.env.TUNNEL_DOMAIN}${path}${url.search}`;
      const requestId = crypto.randomUUID();
      const isWebSocketUpgrade =
        method === "GET" &&
        path === "/ws/events" &&
        (c.req.header("upgrade") || "").toLowerCase() === "websocket";

      if (isWebSocketUpgrade) {
        console.log(
          `Proxying WebSocket request to device ${deviceId}${path}${url.search} (request ID: ${requestId})`,
        );
        try {
          const forwardedRequest = new Request(targetUrl, c.req.raw);
          const headers = forwardedRequest.headers;

          headers.delete("authorization");
          headers.delete("host");
          headers.delete("origin");
          headers.delete("referer");
          headers.delete("cf-connecting-ip");
          headers.delete("x-forwarded-for");
          headers.delete("x-real-ip");

          headers.set("CF-Access-Client-Id", c.env.ACCESS_CLIENT_ID);
          headers.set("CF-Access-Client-Secret", c.env.ACCESS_CLIENT_SECRET);
          headers.set("X-Cubby-Request-Id", requestId);

          const upstreamResponse = await fetch(forwardedRequest);

          if (upstreamResponse.status !== 101) {
            const snapshot = upstreamResponse.clone();
            let bodyPreview = "";
            try {
              bodyPreview = (await snapshot.text()).slice(0, 500);
            } catch {
              // ignore preview issues
            }
            console.warn(
              `WebSocket upgrade to device ${deviceId} failed`,
              {
                requestId,
                status: upstreamResponse.status,
                statusText: upstreamResponse.statusText,
                upstreamHeaders: Array.from(snapshot.headers.entries()),
                bodyPreview,
              },
            );
          }

          return upstreamResponse;
        } catch (error) {
          console.error("Device WebSocket proxy error:", error);
          return c.json(
            { error: "Failed to establish WebSocket to device" },
            502,
          );
        }
      }

      console.log(
        `Proxying ${method} request to device ${deviceId}${path}${url.search} (request ID: ${requestId})`,
      );

      // Manual proxy to ensure body is forwarded for POST requests
      const proxyHeaders = new Headers();
      proxyHeaders.set("CF-Access-Client-Id", c.env.ACCESS_CLIENT_ID);
      proxyHeaders.set("CF-Access-Client-Secret", c.env.ACCESS_CLIENT_SECRET);
      proxyHeaders.set("X-Cubby-Request-Id", requestId);
      
      // Copy content-type if present
      const contentType = c.req.header("content-type");
      if (contentType) {
        proxyHeaders.set("Content-Type", contentType);
      }

      const proxyInit: RequestInit = {
        method,
        headers: proxyHeaders,
      };

      // Forward body for POST/PUT/PATCH
      if (method === "POST" || method === "PUT" || method === "PATCH") {
        proxyInit.body = await c.req.raw.clone().arrayBuffer();
      }

      const upstreamResponse = await fetch(targetUrl, proxyInit);
      
      // Return response with original headers and body
      return new Response(upstreamResponse.body, {
        status: upstreamResponse.status,
        statusText: upstreamResponse.statusText,
        headers: upstreamResponse.headers,
      });
    } catch (error) {
      console.error("Device proxy error:", error);
      return c.json({ error: "Failed to proxy request to device" }, 502);
    }
  },
);

app.all(
  "/mcp",
  describeRoute({
    description:
      "Model Context Protocol (MCP) endpoint for AI assistants to access cubby data. Protocol methods (initialize, tools/list) work without auth. Action methods (tools/call) require OAuth.",
    responses: {
      200: {
        description: "MCP protocol response",
        content: {
          "application/json": {
            schema: {
              type: "object",
              description: "MCP JSON-RPC 2.0 response",
            },
          },
        },
      },
      401: {
        description:
          "Unauthorized - missing or invalid OAuth token for auth-required methods (e.g., tools/call)",
      },
      500: {
        description: "Internal server error",
      },
    },
    tags: ["MCP"],
  }),
  mcpAuth(), // Middleware handles OAuth validation similar to the clerk example
  async (c) => {
    const httpMethod = c.req.method;

    // GET requests are for SSE streaming - proxy to device if selected
    if (httpMethod === "GET") {
      const gwSessionId = c.req.header("mcp-session-id");
      const authInfo = c.get("authInfo");

      if (gwSessionId && authInfo) {
        const session = getSession(gwSessionId);

        if (session?.deviceId && session?.deviceSessionId) {
          // Proxy SSE to device
          const url = new URL(c.req.url);
          const searchParams = url.searchParams;

          console.log(
            `proxying sse to device ${session.deviceId} (device session: ${session.deviceSessionId})`,
          );

          const deviceResponse = await getMcpSse(
            c.env,
            session.deviceId,
            searchParams,
            {
              sessionId: session.deviceSessionId,
              userId: session.userId,
              gwSessionId,
            },
          );

          return deviceResponse;
        }
      }

      // No device selected or not authenticated - use local handler
      // Note: GET has no body. Call mcpHttpHandler directly like the example does
      const localRequest = new Request(c.req.raw.url, {
        method: "GET",
        headers: c.req.raw.headers,
      });
      return mcpHttpHandler(localRequest, { authInfo });
    }

    // POST requests - handle JSON-RPC
    const bodyText = c.get("bodyText");
    const authInfo = c.get("authInfo");
 
    let body: any;
    try {
      body = JSON.parse(bodyText || "{}");
    } catch {
      // Let the upstream MCP handler return the JSON-RPC parse error to match mcp-lite behavior closely
      return mcpHttpHandler(new Request(c.req.raw.url, { method: c.req.raw.method, headers: c.req.raw.headers, body: bodyText }), { authInfo });
    }

    const rpcMethod = body.method as string | undefined;
    const gwSessionId = c.req.header("mcp-session-id");

    // Helper to reconstruct request with body (since mcpAuth consumed it)
    const reconstructRequest = () =>
      new Request(c.req.raw.url, {
        method: c.req.raw.method,
      headers: c.req.raw.headers,
      body: bodyText,
    });

    // Helper: persist auth into session if authenticated and session exists
    // Keep but simplify logs; this is a UX improvement over the example
    const persistAuthIfNeeded = () => {
      if (authInfo && gwSessionId) {
        const session = getSession(gwSessionId);
        if (session && !session.accessToken) {
          if (session.userId === "anonymous") {
            session.userId = authInfo.extra.userId;
          }
          setSessionAuth(gwSessionId, authInfo.token, authInfo.scopes);
        }
      }
    };

    // Handle different methods
    switch (rpcMethod) {
      case "initialize": {
        // Forward to local handler and create session mapping if authenticated
        const upstream = await mcpHttpHandler(reconstructRequest(), { authInfo });

        // Capture upstream response
        const status = upstream.status as any;
        const headers = new Headers(upstream.headers);

        let bodyJson: any;
        try {
          bodyJson = await upstream.json();
        } catch {
          // If body isn't JSON, just return upstream
          return upstream;
        }

        // Ensure capabilities advertise tool support so inspectors know tools are available
        if (bodyJson?.result) {
          bodyJson.result.capabilities = {
            ...(bodyJson.result.capabilities || {}),
            tools: { ...(bodyJson.result.capabilities?.tools || {}) },
            resources: {
              ...(bodyJson.result.capabilities?.resources || {}),
            },
          };
        }

        // Always create session on initialize
        const responseSessionId = headers.get("mcp-session-id");
        if (responseSessionId) {
          if (authInfo) {
            // Authenticated initialize: create session with userId and store token
            const userId = authInfo.extra.userId;
            getOrCreateSession(responseSessionId, userId);
            setSessionAuth(responseSessionId, authInfo.token, authInfo.scopes);
            console.log(
              `created gateway session ${responseSessionId} for user ${userId} with stored auth`,
            );
          } else {
            // Unauthenticated initialize: create session with placeholder userId
            // Auth can be added later via tools/call with Bearer token
            getOrCreateSession(responseSessionId, "anonymous");
            console.log(
              `created anonymous gateway session ${responseSessionId}`,
            );
          }
        }

        // Return modified response with same headers (including Mcp-Session-Id)
        return new Response(JSON.stringify(bodyJson), {
          status,
          headers,
        });
      }

      case "tools/list": {
        // Persist auth if provided (for future requests)
        persistAuthIfNeeded();

        // Build union of gateway tools + device tools (if device selected)
        let tools = [...GATEWAY_TOOLS];

        console.log(`tools/list called with gwSessionId: ${gwSessionId}`);

        if (gwSessionId) {
          const session = getSession(gwSessionId);
          console.log(`session found:`, session);

          if (session?.deviceId && session?.deviceSessionId) {
            console.log(
              `fetching tools from device ${session.deviceId} for union`,
            );

            try {
              // Fetch device tools
              const deviceRequest = {
                jsonrpc: "2.0",
                id: body.id || crypto.randomUUID(),
                method: "tools/list",
              };

              // Open SSE first to ensure we don't miss early streamable events
              const sseResponsePromise = getMcpSse(
                c.env,
                session.deviceId,
                // pass device session id also via query for rmcp stream binding
                new URLSearchParams(),
                {
                  sessionId: session.deviceSessionId,
                  userId: session.userId,
                  gwSessionId,
                  timeoutMs: 30000,
                },
              );

              const deviceResponse = await postMcp(
                c.env,
                session.deviceId,
                JSON.stringify(deviceRequest),
                {
                  sessionId: session.deviceSessionId,
                  userId: session.userId,
                  gwSessionId,
                  // prefer JSON, but advertise SSE as a fallback
                  accept: "application/json; q=1.0, text/event-stream; q=0.1",
                },
              );

              if (deviceResponse.ok) {
                const status = deviceResponse.status;
                const contentType = deviceResponse.headers.get("content-type") || "";
                let deviceData: any | undefined;

                const parseSseStream = async (response: Response): Promise<any | undefined> => {
                  const reader = response.body?.getReader();
                  if (!reader) return undefined;
                  const decoder = new TextDecoder();
                  let buffer = "";
                  const deadline = Date.now() + 15000; // 15s budget
                  let loggedEvents = 0;
                  while (Date.now() < deadline) {
                    const { value, done } = await reader.read();
                    if (done) break;
                    buffer += decoder.decode(value, { stream: true });
                    let sepIdx;
                    // support both \n\n and \r\n\r\n
                    let findDoubleNewline = () => {
                      const nn = buffer.indexOf("\n\n");
                      const crnn = buffer.indexOf("\r\n\r\n");
                      if (nn === -1) return crnn;
                      if (crnn === -1) return nn;
                      return Math.min(nn, crnn);
                    };
                    while ((sepIdx = findDoubleNewline()) !== -1) {
                      const eventChunk = buffer.slice(0, sepIdx);
                      // remove either \n\n or \r\n\r\n
                      const advance = buffer.startsWith("\r\n", sepIdx - 1) ? 4 : 2;
                      buffer = buffer.slice(sepIdx + advance);
                      const dataLines = eventChunk
                        .split("\n")
                        .filter((l) => l.startsWith("data:"))
                        .map((l) => l.slice(5).trim());
                      if (dataLines.length > 0) {
                        const dataPayload = dataLines.join("\n");
                        if (loggedEvents < 3) {
                          console.log("[device sse] event payload:", dataPayload.slice(0, 200));
                          loggedEvents++;
                        }
                        try {
                          const evtObj = JSON.parse(dataPayload);
                          if (evtObj?.result?.tools) {
                            return evtObj;
                          }
                        } catch {}
                      }
                    }
                  }
                  return undefined;
                };

                if (status === 202) {
                  // Streamable HTTP pattern: result over SSE; use the pre-opened stream
                  const sseResp = await sseResponsePromise;
                  if (sseResp.ok) {
                    deviceData = await parseSseStream(sseResp);
                    if (!deviceData) {
                      console.warn("device sse did not yield tools result within budget");
                    }
                  } else {
                    console.warn("device sse get failed:", sseResp.status, sseResp.statusText);
                  }
                } else if (contentType.includes("application/json")) {
                  const text = await deviceResponse.text();
                  try {
                    deviceData = JSON.parse(text);
                  } catch (e) {
                    console.error(
                      "error parsing device tools/list json:",
                      e,
                      "payload:",
                      text.slice(0, 200),
                    );
                    throw e;
                  }
                } else if (contentType.includes("text/event-stream")) {
                  deviceData = await parseSseStream(deviceResponse);
                  if (!deviceData) {
                    console.warn("device returned sse without tools result within budget");
                  }
                } else {
                  const preview = (await deviceResponse.text()).slice(0, 200);
                  console.warn("unexpected content-type from device:", contentType, "body:", preview);
                }
                if (deviceData?.result?.tools) {
                  const deviceTools = deviceData.result.tools;

                  // Check for name collisions
                  const gatewayNames = new Set(tools.map((t) => t.name));
                  for (const dt of deviceTools) {
                    if (gatewayNames.has(dt.name)) {
                      return c.json(
                        {
                          jsonrpc: "2.0",
                          id: body.id,
                          error: {
                            code: -32000,
                            message: `tool name collision: ${dt.name}`,
                          },
                        },
                        500,
                      );
                    }
                  }

                  tools = [...tools, ...deviceTools];
                  console.log(
                    `unioned ${deviceTools.length} device tools with ${GATEWAY_TOOLS.length} gateway tools`,
                  );
                }
              } else {
                const headersObj: Record<string, string> = {};
                deviceResponse.headers.forEach((v, k) => (headersObj[k] = v));
                let bodySnippet = "";
                try {
                  bodySnippet = (await deviceResponse.text()).slice(0, 500);
                } catch {}
                console.warn(
                  `failed to fetch device tools: ${deviceResponse.status} ${deviceResponse.statusText}`,
                  { headers: headersObj, body: bodySnippet },
                );
              }
            } catch (error) {
              console.error(`error fetching device tools:`, error);
              // Continue with just gateway tools if device fetch fails
            }
          } else {
            console.log(`no device selected for session ${gwSessionId}`);
          }
        } else {
          console.log(`no gwSessionId provided`);
        }

        console.log(`returning ${tools.length} tools`);

        return c.json({
          jsonrpc: "2.0",
          id: body.id,
          result: { tools },
        });
      }

      case "tools/call": {
        // Persist auth if provided (for future requests)
        persistAuthIfNeeded();

        const toolName = body.params?.name;

        // Handle gateway tools
        if (toolName?.startsWith("devices/")) {
          if (!authInfo) {
            return c.json(
              {
                jsonrpc: "2.0",
                id: body.id,
                error: {
                  code: -32001,
                  message: "authentication required for gateway tools",
                },
              },
              401,
            );
          }

          const userId = authInfo.extra.userId;

          try {
            let result;
            if (toolName === "devices/list") {
              result = await handleDevicesList(c.env, userId);
            } else if (toolName === "devices/set") {
              if (!gwSessionId) {
                return c.json(
                  {
                    jsonrpc: "2.0",
                    id: body.id,
                    error: {
                      code: -32000,
                      message: "no gateway session found - call initialize first",
                    },
                  },
                  400,
                );
              }
              result = await handleDevicesSet(
                c.env,
                userId,
                gwSessionId,
                body.params?.arguments,
              );
            } else {
              return c.json(
                {
                  jsonrpc: "2.0",
                  id: body.id,
                  error: {
                    code: -32601,
                    message: `unknown gateway tool: ${toolName}`,
                  },
                },
                404,
              );
            }

            return c.json({
              jsonrpc: "2.0",
              id: body.id,
              result,
            });
          } catch (error) {
            console.error(`gateway tool ${toolName} error:`, error);
            return c.json(
              {
                jsonrpc: "2.0",
                id: body.id,
                error: {
                  code: -32000,
                  message:
                    error instanceof Error ? error.message : "tool call failed",
                },
              },
              500,
            );
          }
        }

        // Handle device tools - proxy to device
        if (!gwSessionId) {
          return c.json(
            {
              jsonrpc: "2.0",
              id: body.id,
              error: {
                code: -32000,
                message: "no gateway session - call initialize first",
              },
            },
            400,
          );
        }

        const session = getSession(gwSessionId);
        if (!session?.deviceId || !session?.deviceSessionId) {
          return c.json(
            {
              jsonrpc: "2.0",
              id: body.id,
              error: {
                code: -32000,
                message:
                  "no device selected - call devices/set to select a device first",
              },
            },
            400,
          );
        }

        console.log(
          `proxying tool call ${toolName} to device ${session.deviceId}`,
        );

        // Open SSE first to avoid missing streamable events
        const ssePromise = getMcpSse(
          c.env,
          session.deviceId,
          new URLSearchParams(),
          {
            sessionId: session.deviceSessionId,
            userId: session.userId,
            gwSessionId,
            timeoutMs: 60000,
          },
        );

        const deviceResponse = await postMcp(
          c.env,
          session.deviceId,
          bodyText || "{}",
          {
            sessionId: session.deviceSessionId,
            userId: session.userId,
            gwSessionId,
            accept: "application/json; q=1.0, text/event-stream; q=0.5",
          },
        );

        const parseSseStream = async (response: Response): Promise<any | undefined> => {
          const reader = response.body?.getReader();
          if (!reader) return undefined;
          const decoder = new TextDecoder();
          let buffer = "";
          const deadline = Date.now() + 60000; // 60s budget for tool call
          while (Date.now() < deadline) {
            const { value, done } = await reader.read();
            if (done) break;
            buffer += decoder.decode(value, { stream: true });
            let sepIdx;
            const findDoubleNewline = () => {
              const nn = buffer.indexOf("\n\n");
              const crnn = buffer.indexOf("\r\n\r\n");
              if (nn === -1) return crnn;
              if (crnn === -1) return nn;
              return Math.min(nn, crnn);
            };
            while ((sepIdx = findDoubleNewline()) !== -1) {
              const eventChunk = buffer.slice(0, sepIdx);
              const advance = buffer.startsWith("\r\n", sepIdx - 1) ? 4 : 2;
              buffer = buffer.slice(sepIdx + advance);
              const dataLines = eventChunk
                .split("\n")
                .filter((l) => l.startsWith("data:"))
                .map((l) => l.slice(5).trim());
              if (dataLines.length > 0) {
                const dataPayload = dataLines.join("\n");
                try {
                  const evtObj = JSON.parse(dataPayload);
                  // For tool calls, a complete JSON-RPC response includes result or error
                  if (evtObj?.result || evtObj?.error) {
                    return evtObj;
                  }
                } catch {
                  // ignore non-JSON fragments
                }
              }
            }
          }
          return undefined;
        };

        if (!deviceResponse.ok) {
          const errorText = await deviceResponse.text();
          console.error(`device proxy error: ${deviceResponse.status} - ${errorText}`);
          return c.json(
            {
              jsonrpc: "2.0",
              id: body.id,
              error: {
                code: -32000,
                message: `device request failed: ${deviceResponse.status}`,
              },
            },
            502,
          );
        }

        const status = deviceResponse.status;
        const contentType = deviceResponse.headers.get("content-type") || "";

        if (status === 202) {
          const sseResp = await ssePromise;
          if (sseResp.ok) {
            const evt = await parseSseStream(sseResp);
            if (evt) return c.json(evt);
            console.warn("device tool sse did not yield a final result within budget");
            return c.json(
              {
                jsonrpc: "2.0",
                id: body.id,
                error: { code: -32000, message: "device did not produce a result in time" },
              },
              504,
            );
          }
          return c.json(
            {
              jsonrpc: "2.0",
              id: body.id,
              error: { code: -32000, message: `device sse failed: ${sseResp.status}` },
            },
            502,
          );
        }

        if (contentType.includes("application/json")) {
          const text = await deviceResponse.text();
          try {
            const json = JSON.parse(text);
            return c.json(json);
          } catch (e) {
            console.error("device returned invalid json:", e, "payload:", text.slice(0, 200));
            return c.json(
              {
                jsonrpc: "2.0",
                id: body.id,
                error: { code: -32000, message: "invalid json from device" },
              },
              502,
            );
          }
        }

        if (contentType.includes("text/event-stream")) {
          const evt = await parseSseStream(deviceResponse);
          if (evt) return c.json(evt);
          return c.json(
            {
              jsonrpc: "2.0",
              id: body.id,
              error: { code: -32000, message: "device sse did not produce a result" },
            },
            502,
          );
        }

        const preview = (await deviceResponse.text()).slice(0, 200);
        console.warn("unexpected device response content-type:", contentType, "body:", preview);
        return c.json(
          {
            jsonrpc: "2.0",
            id: body.id,
            error: { code: -32000, message: "unexpected device response" },
          },
          502,
        );
      }

      case "resources/list":
      case "resources/read":
      case "resources/templates/list": {
        // Persist auth if provided (for future requests)
        persistAuthIfNeeded();

        // If device selected, proxy to device; otherwise return empty
        if (gwSessionId) {
          const session = getSession(gwSessionId);
          if (session?.deviceId && session?.deviceSessionId) {
            console.log(
              `proxying ${rpcMethod} to device ${session.deviceId}`,
            );

            const deviceResponse = await postMcp(
              c.env,
              session.deviceId,
              bodyText || "{}",
              {
                sessionId: session.deviceSessionId,
                userId: session.userId,
                gwSessionId,
              },
            );

            if (deviceResponse.ok) {
              const deviceData = await deviceResponse.json();
              return c.json(deviceData);
            }
          }
        }

        // Default: return empty resources
        const emptyResult =
          rpcMethod === "resources/list"
            ? { resources: [] }
            : rpcMethod === "resources/templates/list"
              ? { resourceTemplates: [] }
              : null;

        return c.json({
          jsonrpc: "2.0",
          id: body.id,
          result: emptyResult,
        });
      }

      default: {
        // Forward all other methods to local handler
        return mcpHttpHandler(reconstructRequest(), { authInfo });
      }
    }
  },
);

app.get(
  "/whoami",
  oauth({
    // requiredScopes: undefined  // add later in Step 4
  }),
  (c) => {
    const auth = c.get("auth");
    return strictJSONResponse(c, whoamiResponseSchema, {
      ok: true,
      sub: auth.userId,
      iss: auth.issuer,
      aud: auth.audiences,
      scopes: auth.scopes,
      // for debugging only; remove later:
      claims: auth.claims,
    });
  },
);

app.get("/mcp/openapi", (c) => {
  const baseUrl = new URL(c.req.url).origin;
  const spec = generateMcpOpenAPISpec(baseUrl);
  return c.json(spec);
});

// OAuth 2.0 Protected Resource Metadata
// Tells clients that Stytch is the authorization server for this resource
app.get("/.well-known/oauth-protected-resource", (c) => {
  const isDev = isDevEnvironment(c.env);
  const baseUrl = isDev ? "http://localhost:8787" : "https://api.cubby.sh";

  return c.json({
    resource: `${baseUrl}/mcp`,
    // Stytch is the authorization server
    authorization_servers: [isDev ? c.env.STYTCH_PROJECT_DOMAIN : "https://login.cubby.sh"],
    bearer_methods_supported: ["header"],
    scopes_supported: ["openid", "read:cubby"],
    resource_documentation: `${baseUrl}/mcp/openapi`,
  });
});

app.get("/.well-known/oauth-protected-resource/mcp", (c) => {
  const isDev = isDevEnvironment(c.env);
  const baseUrl = isDev ? "http://localhost:8787" : "https://api.cubby.sh";

  return c.json({
    resource: `${baseUrl}/mcp`,
    // Stytch is the authorization server
    authorization_servers: [isDev ? c.env.STYTCH_PROJECT_DOMAIN : "https://login.cubby.sh"],
    bearer_methods_supported: ["header"],
    scopes_supported: ["openid", "read:cubby"],
    resource_documentation: `${baseUrl}/mcp/openapi`,
  });
});

// Development helper: Get a token for testing MCP
// In production, remove this or add proper authentication
app.post(
  "/dev/get-token",
  describeRoute({
    description: "Development endpoint to get an OAuth token for testing",
    responses: {
      200: {
        description: "Token response",
        content: {
          "application/json": {
            schema: {
              type: "object",
              properties: {
                access_token: { type: "string" },
                token_type: { type: "string" },
              },
            },
          },
        },
      },
    },
    tags: ["Development"],
  }),
  async (c) => {
    const sessionJwt = c.req
      .header("authorization")
      ?.replace(/^Bearer\s+/i, "");

    if (!sessionJwt) {
      return c.json({ error: "Provide session JWT as Bearer token" }, 401);
    }

    return c.json({
      access_token: sessionJwt,
      token_type: "Bearer",
      note: "This is your session JWT which works as an OAuth token for testing",
    });
  },
);

// Development helper: Login and get a Bearer token for testing
// Creates or authenticates a user and returns a token you can use for API testing
app.post(
  "/dev/token",
  describeRoute({
    description:
      "development endpoint to get a bearer token for testing. creates/logs in a user and returns the token.",
    responses: {
      200: {
        description: "bearer token for testing",
        content: {
          "application/json": {
            schema: {
              type: "object",
              properties: {
                access_token: { type: "string" },
                token_type: { type: "string" },
                user_id: { type: "string" },
                instructions: { type: "string" },
              },
            },
          },
        },
      },
    },
    tags: ["Development"],
  }),
  zValidator("json", z.object({
    email: z.string().email(),
    password: z.string().min(1),
  })),
  async (c) => {
    // Only allow in dev environment
    if (!isDevEnvironment(c.env)) {
      return c.json({ error: "This endpoint is only available in dev" }, 403);
    }

    const { email, password } = c.req.valid("json");

    try {
      const client = new stytch.Client({
        project_id: c.env.STYTCH_PROJECT_ID,
        secret: c.env.STYTCH_PROJECT_SECRET,
      });

      let response;
      const db = createDbClient(c.env.DATABASE_URL);

      // Try to authenticate first
      try {
        response = await client.passwords.authenticate({
          email: email,
          password: password,
          session_duration_minutes: 60,
        });
        console.log(`authenticated existing user: ${email}`);
      } catch (error: any) {
        // If auth fails, try to create the user
        if (error?.error_type === "invalid_credentials") {
          console.log(`user not found, creating: ${email}`);
          
          const newUserId = crypto.randomUUID();
          
          response = await client.passwords.create({
            email: email,
            password: password,
            session_duration_minutes: 60,
            trusted_metadata: { user_id: newUserId },
            session_custom_claims: { user_id: newUserId },
          });

          await createUser(db, {
            id: newUserId,
            authId: response.user_id,
            email: email,
          });

          console.log(`created new user: ${email}`);
        } else {
          throw error;
        }
      }

      return c.json({
        access_token: response.session_jwt,
        token_type: "Bearer",
        user_id: (response as any).user?.user_id || "unknown",
        instructions:
          "copy the access_token value and paste it into authorization header as: Bearer <token>",
      });
    } catch (error) {
      console.error("dev token generation error:", error);
      return c.json(
        {
          error: "failed to generate token",
          details: error instanceof Error ? error.message : "unknown error",
        },
        500,
      );
    }
  },
);

// Development endpoint to check environment variables (remove in production)
app.get("/debug/env", (c) => {
  const envStatus = {
    STYTCH_PROJECT_ID: !!c.env.STYTCH_PROJECT_ID,
    STYTCH_PROJECT_SECRET: !!c.env.STYTCH_PROJECT_SECRET,
    STYTCH_PROJECT_DOMAIN: !!c.env.STYTCH_PROJECT_DOMAIN,
    STYTCH_BASE_URL: !!c.env.STYTCH_BASE_URL,
    CF_API_TOKEN: !!c.env.CF_API_TOKEN,
    CF_ACCOUNT_ID: !!c.env.CF_ACCOUNT_ID,
    CF_ZONE_ID: !!c.env.CF_ZONE_ID,
    ACCESS_CLIENT_ID: !!c.env.ACCESS_CLIENT_ID,
    ACCESS_CLIENT_SECRET: !!c.env.ACCESS_CLIENT_SECRET,
    TUNNEL_DOMAIN: !!c.env.TUNNEL_DOMAIN,
    DATABASE_URL: !!c.env.DATABASE_URL,
  };
  
  return c.json({
    message: "environment variables status (true = exists, false = missing)",
    env: envStatus,
  });
});

// Main API root endpoint
app.get("/", (c) => {
  return c.text(
    "cubby api - visit /openapi.json for rest api documentation or /mcp/openapi for mcp tools documentation",
  );
});

// OpenAPI documentation endpoint - comprehensive gateway + device proxy docs
app.get("/openapi.json", (c) => {
  const url = new URL(c.req.url);
  const baseUrl = `${url.protocol}//${url.host}`;
  const spec = generateGatewayOpenAPISpec(baseUrl);
  return c.json(spec);
});

// Legacy endpoint - redirect to openapi.json
app.get("/openapi", (c) => {
  return c.redirect("/openapi.json", 301);
});

export default app;
