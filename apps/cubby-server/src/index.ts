import { Hono } from "hono";
import { HTTPException } from "hono/http-exception";
import { cors } from "hono/cors";
import { proxy } from "hono/proxy";
import { setCookie } from "hono/cookie";
import { describeRoute, resolver, openAPIRouteHandler } from "hono-openapi";
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
import oauthRoutes from "./routes/oauth_routes";
import authViews from "./routes/auth_views";
import { strictJSONResponse } from "./helpers";
import { isPathAllowed } from "./proxy_config";

type Bindings = CloudflareBindings;

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

// CORS middleware - allow any origin for token-protected API
// Security is enforced by Bearer token validation, not origin restrictions
app.use(
  "/*",
  cors({
    origin: "*",
    allowMethods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
    allowHeaders: ["Content-Type", "Authorization"],
  }),
);

// Global error handler
app.onError((err, c) => {
  if (err instanceof HTTPException) {
    console.error("HTTP Exception:", err.status, err.message);
    return err.getResponse();
  }
  console.error("Unhandled error:", err);
  return c.text("Internal Server Error", 500);
});

// Routes
app.route("/oauth", oauthRoutes);
app.route("/", authViews);

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
      const client = new stytch.Client({
        project_id: c.env.STYTCH_PROJECT_ID,
        secret: c.env.STYTCH_PROJECT_SECRET,
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

      console.log(
        `Proxying ${method} request to device ${deviceId}${path}${url.search} (request ID: ${requestId})`,
      );

      return proxy(targetUrl, {
        headers: {
          "CF-Access-Client-Id": c.env.ACCESS_CLIENT_ID,
          "CF-Access-Client-Secret": c.env.ACCESS_CLIENT_SECRET,
          "X-Cubby-Request-Id": requestId,
        },
      });
    } catch (error) {
      console.error("Device proxy error:", error);
      return c.json({ error: "Failed to proxy request to device" }, 502);
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

app.get(
  "/openapi",
  openAPIRouteHandler(app, {
    documentation: {
      openapi: "3.0.0",
      info: {
        title: "Cubby API",
        version: "1.0.0",
        description: "Authentication API for Cubby",
      },
      servers: [
        {
          url: "http://localhost:8787",
          description: "Local Development Server",
        },
      ],
    },
  }),
);

export default app;
