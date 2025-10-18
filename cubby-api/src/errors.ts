import { HTTPException } from "hono/http-exception";

export const errors = {
  auth: {
    MISSING_TOKEN: () =>
      new HTTPException(401, {
        res: new Response(JSON.stringify({ error: "Missing bearer token" }), {
          status: 401,
          headers: {
            "Content-Type": "application/json",
            "WWW-Authenticate":
              'Bearer error="invalid_token", error_description="missing"',
          },
        }),
      }),
    INVALID_TOKEN: () =>
      new HTTPException(401, {
        res: new Response(JSON.stringify({ error: "Invalid token" }), {
          status: 401,
          headers: { 
            "Content-Type": "application/json",
            "WWW-Authenticate": 'Bearer error="invalid_token"' 
          },
        }),
      }),
    INSUFFICIENT_SCOPE: (scopes: string[]) =>
      new HTTPException(403, {
        res: new Response(JSON.stringify({ error: "Insufficient scope" }), {
          status: 403,
          headers: {
            "Content-Type": "application/json",
            "WWW-Authenticate": `Bearer error="insufficient_scope", scope="${scopes.join(" ")}"`,
          },
        }),
      }),
    INVALID_AUTH_DATA: () =>
      new HTTPException(401, {
        res: new Response(JSON.stringify({ error: "Invalid authentication data" }), {
          status: 401,
          headers: { 
            "Content-Type": "application/json",
            "WWW-Authenticate": 'Bearer error="invalid_token"' 
          },
        }),
      }),
  },
} as const;
