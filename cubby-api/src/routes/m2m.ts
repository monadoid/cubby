import { Hono } from "hono";
import { describeRoute, resolver } from "hono-openapi";
import { zValidator } from "@hono/zod-validator";
import { z } from "zod/v4";
import stytch from "stytch";
import type { Bindings, Variables } from "../index";
import { session } from "../middleware/session";

const app = new Hono<{ Bindings: Bindings; Variables: Variables }>();

// schemas
const createM2MClientRequestSchema = z.object({
  name: z.string().optional(),
});

const createM2MClientResponseSchema = z.object({
  client_id: z.string(),
  client_secret: z.string(),
  name: z.string().optional(),
  created_at: z.string(),
});

const listM2MClientsResponseSchema = z.object({
  clients: z.array(
    z.object({
      client_id: z.string(),
      name: z.string().optional(),
      created_at: z.string(),
    })
  ),
});

function isDevEnvironment(env: Bindings): boolean {
  return env.STYTCH_PROJECT_ID?.startsWith("project-test-") ?? false;
}

// POST /m2m/clients - create a new m2m client for the authenticated user
app.post(
  "/clients",
  session(),
  describeRoute({
    description: "create a new m2m client (api key) for the authenticated user",
    responses: {
      200: {
        description: "client created successfully",
        content: {
          "application/json": { schema: resolver(createM2MClientResponseSchema) },
        },
      },
      401: { description: "unauthorized" },
      500: { description: "internal server error" },
    },
    tags: ["m2m"],
  }),
  zValidator("json", createM2MClientRequestSchema),
  async (c) => {
    const userId = c.get("userId");
    const { name } = c.req.valid("json");

    const client = new stytch.Client({
      project_id: c.env.STYTCH_PROJECT_ID,
      secret: c.env.STYTCH_PROJECT_SECRET,
      ...(isDevEnvironment(c.env) ? {} : { custom_base_url: "https://login.cubby.sh" }),
    });

    try {
      // create m2m client with user_id in trusted_metadata
      const response = await client.m2m.clients.create({
        trusted_metadata: { user_id: userId },
        scopes: ["read:cubby"],
        ...(name ? { client_name: name } : {}),
      });

      return c.json({
        client_id: response.m2m_client.client_id,
        client_secret: response.m2m_client.client_secret,
        name: response.m2m_client.client_name,
        created_at: new Date().toISOString(),
      });
    } catch (error: any) {
      console.error("failed to create m2m client:", error);
      return c.json(
        { error: error?.error_message || "failed to create m2m client" },
        500
      );
    }
  }
);

// GET /m2m/clients - not implemented yet (searching by trusted_metadata not supported)
app.get(
  "/clients",
  session(),
  describeRoute({
    description: "list all m2m clients for the authenticated user (not implemented)",
    responses: {
      501: { description: "not implemented" },
    },
    tags: ["m2m"],
  }),
  async (c) => {
    return c.json({ error: "listing clients not implemented yet" }, 501);
  }
);

// DELETE /m2m/clients/:clientId - revoke an m2m client
app.delete(
  "/clients/:clientId",
  session(),
  describeRoute({
    description: "revoke an m2m client (api key)",
    responses: {
      200: {
        description: "client revoked successfully",
        content: {
          "application/json": { schema: resolver(z.object({ success: z.boolean() })) },
        },
      },
      401: { description: "unauthorized" },
      403: { description: "forbidden - client does not belong to user" },
      404: { description: "client not found" },
      500: { description: "internal server error" },
    },
    tags: ["m2m"],
  }),
  async (c) => {
    const userId = c.get("userId");
    const clientId = c.req.param("clientId");

    const client = new stytch.Client({
      project_id: c.env.STYTCH_PROJECT_ID,
      secret: c.env.STYTCH_PROJECT_SECRET,
      ...(isDevEnvironment(c.env) ? {} : { custom_base_url: "https://login.cubby.sh" }),
    });

    try {
      // verify client belongs to user
      const getResponse = await client.m2m.clients.get({
        client_id: clientId,
      });

      const clientUserId = (getResponse.m2m_client.trusted_metadata as any)?.user_id;

      if (clientUserId !== userId) {
        return c.json({ error: "client does not belong to user" }, 403);
      }

      // delete the client
      await client.m2m.clients.delete({
        client_id: clientId,
      });

      return c.json({ success: true });
    } catch (error: any) {
      console.error("failed to revoke m2m client:", error);
      const statusCode = error?.status_code || 500;
      return c.json(
        { error: error?.error_message || "failed to revoke m2m client" },
        statusCode
      );
    }
  }
);

export default app;

