/**
 * gateway openapi spec - manual, curated documentation
 * 
 * this spec combines:
 * 1. gateway-specific endpoints (devices, auth, oauth)
 * 2. proxied device endpoints (manually documented subset from proxy_config.ts)
 */

export interface OpenAPISpec {
  openapi: string;
  info: {
    title: string;
    version: string;
    description: string;
  };
  servers: Array<{
    url: string;
    description: string;
  }>;
  paths: Record<string, any>;
  components?: {
    schemas?: Record<string, any>;
    securitySchemes?: Record<string, any>;
    parameters?: Record<string, any>;
  };
  security?: Array<Record<string, string[]>>;
}

/**
 * generate complete gateway openapi spec
 */
export function generateGatewayOpenAPISpec(baseUrl: string): OpenAPISpec {
  return {
    openapi: "3.0.0",
    info: {
      title: "cubby gateway api",
      version: "1.0.0",
      description: 
        "gateway api for cubby - manage devices, authenticate users, and access device data. " +
        "the gateway proxies a curated subset of device endpoints at /devices/{deviceId}/*",
    },
    servers: [
      {
        url: baseUrl,
        description: "cubby gateway",
      },
    ],
    security: [
      { M2MClientCredentials: [] },
      { OAuth2: ["openid", "read:cubby"] },
      { BearerAuth: [] }
    ],
    paths: {
      // ========================================
      // Authentication Endpoints
      // ========================================
      "/sign-up": {
        post: {
          summary: "create user account",
          description: "register a new user with email and password",
          tags: ["authentication"],
          operationId: "signUp",
          security: [], // public endpoint
          requestBody: {
            required: true,
            content: {
              "application/json": {
                schema: {
                  type: "object",
                  required: ["email", "password"],
                  properties: {
                    email: { type: "string", format: "email" },
                    password: { type: "string", minLength: 8 },
                  },
                },
              },
            },
          },
          responses: {
            201: {
              description: "user created successfully",
              content: {
                "application/json": {
                  schema: { $ref: "#/components/schemas/AuthResponse" },
                },
              },
            },
            400: { description: "invalid email or duplicate user" },
            500: { description: "internal server error" },
          },
        },
      },
      "/login": {
        post: {
          summary: "authenticate user",
          description: "login with email and password",
          tags: ["authentication"],
          operationId: "login",
          security: [], // public endpoint
          requestBody: {
            required: true,
            content: {
              "application/json": {
                schema: {
                  type: "object",
                  required: ["email", "password"],
                  properties: {
                    email: { type: "string", format: "email" },
                    password: { type: "string" },
                  },
                },
              },
            },
          },
          responses: {
            200: {
              description: "user authenticated successfully",
              content: {
                "application/json": {
                  schema: { $ref: "#/components/schemas/AuthResponse" },
                },
              },
            },
            401: { description: "invalid credentials" },
            500: { description: "internal server error" },
          },
        },
      },
      "/whoami": {
        get: {
          summary: "get current user info",
          description: "return information about the authenticated user from their token",
          tags: ["authentication"],
          operationId: "whoami",
          responses: {
            200: {
              description: "user information",
              content: {
                "application/json": {
                  schema: { $ref: "#/components/schemas/WhoAmIResponse" },
                },
              },
            },
            401: { description: "unauthorized" },
          },
        },
      },

      // ========================================
      // OAuth 2.0 Endpoints
      // ========================================
      "/oauth/token": {
        post: {
          summary: "oauth token exchange",
          description: "exchange authorization code, refresh token, or client credentials for access token. supports PKCE for public clients and M2M authentication via client_credentials grant.",
          tags: ["oauth"],
          operationId: "oauthToken",
          security: [], // public endpoint
          requestBody: {
            required: true,
            content: {
              "application/x-www-form-urlencoded": {
                schema: {
                  type: "object",
                  required: ["grant_type"],
                  properties: {
                    grant_type: { 
                      type: "string",
                      enum: ["authorization_code", "refresh_token", "client_credentials"],
                      description: "oauth grant type. use 'client_credentials' for M2M authentication with api keys, 'authorization_code' for user oauth flow, 'refresh_token' to refresh an existing token",
                    },
                    code: { 
                      type: "string",
                      description: "authorization code (required for authorization_code grant)",
                    },
                    redirect_uri: { 
                      type: "string",
                      description: "must match the redirect_uri from /authorize (required for authorization_code grant)",
                    },
                    client_id: { 
                      type: "string",
                      description: "client id from M2M credentials or oauth app (required for client_credentials grant)",
                    },
                    client_secret: { 
                      type: "string",
                      description: "client secret from M2M credentials (required for client_credentials grant). for oauth apps: required for confidential clients, omit for public clients using PKCE",
                    },
                    code_verifier: { 
                      type: "string",
                      description: "PKCE code verifier (required for authorization_code grant with public clients)",
                    },
                    refresh_token: { 
                      type: "string",
                      description: "refresh token (required for refresh_token grant)",
                    },
                    scope: {
                      type: "string",
                      description: "space-separated list of scopes. for M2M: use 'read:cubby' for api access",
                      example: "read:cubby",
                    },
                  },
                },
                examples: {
                  m2m: {
                    summary: "M2M client credentials",
                    description: "exchange api credentials for access token (recommended for scripts, automation, mcp servers)",
                    value: {
                      grant_type: "client_credentials",
                      client_id: "project-test-...-m2m-...",
                      client_secret: "secret_...",
                      scope: "read:cubby",
                    },
                  },
                  oauth: {
                    summary: "oauth authorization code",
                    description: "exchange authorization code for access token (for user-facing oauth flows)",
                    value: {
                      grant_type: "authorization_code",
                      code: "...",
                      redirect_uri: "https://yourapp.com/callback",
                      client_id: "...",
                      code_verifier: "...",
                    },
                  },
                  refresh: {
                    summary: "refresh access token",
                    description: "exchange refresh token for new access token",
                    value: {
                      grant_type: "refresh_token",
                      refresh_token: "...",
                    },
                  },
                },
              },
            },
          },
          responses: {
            200: {
              description: "token issued successfully",
              content: {
                "application/json": {
                  schema: { $ref: "#/components/schemas/TokenResponse" },
                },
              },
            },
            400: { description: "invalid request" },
            401: { description: "invalid client or credentials" },
          },
        },
      },
      "/oauth/register": {
        post: {
          summary: "register oauth client",
          description: "dynamic client registration (RFC 7591) for third-party oauth clients",
          tags: ["oauth"],
          operationId: "oauthRegister",
          security: [], // public endpoint per OAuth DCR spec
          requestBody: {
            required: true,
            content: {
              "application/json": {
                schema: { $ref: "#/components/schemas/ClientRegistrationRequest" },
              },
            },
          },
          responses: {
            200: {
              description: "client registered successfully",
              content: {
                "application/json": {
                  schema: { $ref: "#/components/schemas/ClientRegistrationResponse" },
                },
              },
            },
            400: { description: "invalid client metadata" },
            500: { description: "internal server error" },
          },
        },
      },

      // ========================================
      // Device Management Endpoints
      // ========================================
      "/devices": {
        get: {
          summary: "list user devices",
          description: "get all devices enrolled by the authenticated user",
          tags: ["devices"],
          operationId: "listDevices",
          responses: {
            200: {
              description: "list of devices",
              content: {
                "application/json": {
                  schema: { $ref: "#/components/schemas/DeviceListResponse" },
                },
              },
            },
            401: { description: "unauthorized" },
            500: { description: "internal server error" },
          },
        },
      },
      "/devices/enroll": {
        post: {
          summary: "enroll a new device",
          description: "create a new device and cloudflare tunnel. returns credentials for the device to connect.",
          tags: ["devices"],
          operationId: "enrollDevice",
          requestBody: {
            required: true,
            content: {
              "application/json": {
                schema: { type: "object", properties: {} },
              },
            },
          },
          responses: {
            200: {
              description: "device enrolled successfully",
              content: {
                "application/json": {
                  schema: { $ref: "#/components/schemas/DeviceEnrollResponse" },
                },
              },
            },
            401: { description: "unauthorized" },
            500: { description: "internal server error" },
          },
        },
      },

      // ========================================
      // Proxied Device Endpoints - Core
      // ========================================
      "/devices/{deviceId}/health": {
        get: {
          summary: "[device] health check",
          description: "check device health and recording status",
          tags: ["device - monitoring"],
          operationId: "deviceHealth",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
          ],
          responses: {
            200: {
              description: "health status",
              content: {
                "application/json": {
                  schema: { $ref: "#/components/schemas/HealthCheckResponse" },
                },
              },
            },
            404: { description: "device not found" },
            502: { description: "device unreachable" },
          },
        },
      },
      "/devices/{deviceId}/search": {
        get: {
          summary: "[device] search content",
          description: "search across screen captures, audio transcriptions, and ui elements",
          tags: ["device - search"],
          operationId: "deviceSearch",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
            { name: "q", in: "query", schema: { type: "string" }, description: "search query text" },
            { name: "limit", in: "query", schema: { type: "integer", default: 50 } },
            { name: "offset", in: "query", schema: { type: "integer", default: 0 } },
            { name: "content_type", in: "query", schema: { $ref: "#/components/schemas/ContentType" } },
            { name: "start_time", in: "query", schema: { type: "string", format: "date-time" } },
            { name: "end_time", in: "query", schema: { type: "string", format: "date-time" } },
            { name: "app_name", in: "query", schema: { type: "string" }, description: "filter by application" },
            { name: "window_name", in: "query", schema: { type: "string" } },
            { name: "include_frames", in: "query", schema: { type: "boolean", default: false } },
            { name: "speaker_ids", in: "query", schema: { type: "array", items: { type: "integer" } }, style: "form", explode: true },
          ],
          responses: {
            200: {
              description: "search results",
              content: {
                "application/json": {
                  schema: { $ref: "#/components/schemas/SearchResponse" },
                },
              },
            },
          },
        },
      },
      "/devices/{deviceId}/search/keyword": {
        get: {
          summary: "[device] keyword search",
          description: "fast keyword-based search with fuzzy matching support",
          tags: ["device - search"],
          operationId: "deviceKeywordSearch",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
            { name: "query", in: "query", required: true, schema: { type: "string" } },
            { name: "limit", in: "query", schema: { type: "integer", default: 50 } },
            { name: "offset", in: "query", schema: { type: "integer", default: 0 } },
            { name: "fuzzy_match", in: "query", schema: { type: "boolean", default: false } },
            { name: "start_time", in: "query", schema: { type: "string", format: "date-time" } },
            { name: "end_time", in: "query", schema: { type: "string", format: "date-time" } },
          ],
          responses: {
            200: {
              description: "keyword matches",
              content: {
                "application/json": {
                  schema: {
                    type: "array",
                    items: { $ref: "#/components/schemas/SearchMatch" },
                  },
                },
              },
            },
          },
        },
      },
      "/devices/{deviceId}/semantic-search": {
        get: {
          summary: "[device] semantic search",
          description: "vector similarity search using embeddings",
          tags: ["device - search"],
          operationId: "deviceSemanticSearch",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
            { name: "text", in: "query", required: true, schema: { type: "string" } },
            { name: "limit", in: "query", schema: { type: "integer", default: 10 } },
            { name: "threshold", in: "query", schema: { type: "number", format: "float" } },
          ],
          responses: {
            200: {
              description: "semantically similar content",
              content: {
                "application/json": {
                  schema: {
                    type: "array",
                    items: { $ref: "#/components/schemas/OCRResult" },
                  },
                },
              },
            },
          },
        },
      },

      // ========================================
      // Proxied Device Endpoints - Media
      // ========================================
      "/devices/{deviceId}/audio/list": {
        get: {
          summary: "[device] list audio devices",
          description: "list available audio input/output devices",
          tags: ["device - media"],
          operationId: "deviceListAudio",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
          ],
          responses: {
            200: {
              description: "audio devices",
              content: {
                "application/json": {
                  schema: {
                    type: "array",
                    items: { $ref: "#/components/schemas/AudioDevice" },
                  },
                },
              },
            },
          },
        },
      },
      "/devices/{deviceId}/vision/list": {
        get: {
          summary: "[device] list monitors",
          description: "list available display monitors/screens",
          tags: ["device - media"],
          operationId: "deviceListMonitors",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
          ],
          responses: {
            200: {
              description: "monitors",
              content: {
                "application/json": {
                  schema: {
                    type: "array",
                    items: { $ref: "#/components/schemas/MonitorInfo" },
                  },
                },
              },
            },
          },
        },
      },
      "/devices/{deviceId}/frames/{frameId}": {
        get: {
          summary: "[device] get frame data",
          description: "retrieve a specific screen capture frame by id",
          tags: ["device - media"],
          operationId: "deviceGetFrame",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
            { name: "frameId", in: "path", required: true, schema: { type: "integer" } },
          ],
          responses: {
            200: {
              description: "frame data",
              content: {
                "application/json": {
                  schema: { type: "object" },
                },
              },
            },
            404: { description: "frame not found" },
          },
        },
      },

      // ========================================
      // Proxied Device Endpoints - Speakers
      // ========================================
      "/devices/{deviceId}/speakers/search": {
        get: {
          summary: "[device] search speakers",
          description: "search for speakers by name",
          tags: ["device - speakers"],
          operationId: "deviceSearchSpeakers",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
            { name: "name", in: "query", schema: { type: "string" } },
          ],
          responses: {
            200: {
              description: "matching speakers",
              content: {
                "application/json": {
                  schema: {
                    type: "array",
                    items: { $ref: "#/components/schemas/Speaker" },
                  },
                },
              },
            },
          },
        },
      },
      "/devices/{deviceId}/speakers/similar": {
        get: {
          summary: "[device] find similar speakers",
          description: "find speakers similar to a given speaker (based on voice characteristics)",
          tags: ["device - speakers"],
          operationId: "deviceSimilarSpeakers",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
            { name: "speaker_id", in: "query", required: true, schema: { type: "integer" } },
            { name: "limit", in: "query", schema: { type: "integer", default: 5 } },
          ],
          responses: {
            200: {
              description: "similar speakers",
              content: {
                "application/json": {
                  schema: {
                    type: "array",
                    items: { $ref: "#/components/schemas/Speaker" },
                  },
                },
              },
            },
          },
        },
      },
      "/devices/{deviceId}/speakers/unnamed": {
        get: {
          summary: "[device] get unnamed speakers",
          description: "list speakers that haven't been assigned a name yet",
          tags: ["device - speakers"],
          operationId: "deviceUnnamedSpeakers",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
            { name: "limit", in: "query", schema: { type: "integer", default: 50 } },
            { name: "offset", in: "query", schema: { type: "integer", default: 0 } },
          ],
          responses: {
            200: {
              description: "unnamed speakers",
              content: {
                "application/json": {
                  schema: {
                    type: "array",
                    items: { $ref: "#/components/schemas/Speaker" },
                  },
                },
              },
            },
          },
        },
      },

      // ========================================
      // Proxied Device Endpoints - Tags
      // ========================================
      "/devices/{deviceId}/tags/{contentType}/{id}": {
        get: {
          summary: "[device] get tags",
          description: "retrieve tags for a specific content item",
          tags: ["device - tags"],
          operationId: "deviceGetTags",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
            { name: "contentType", in: "path", required: true, schema: { type: "string", enum: ["vision", "audio"] } },
            { name: "id", in: "path", required: true, schema: { type: "integer" } },
          ],
          responses: {
            200: {
              description: "tags",
              content: {
                "application/json": {
                  schema: {
                    type: "array",
                    items: { type: "string" },
                  },
                },
              },
            },
          },
        },
      },

      // ========================================
      // Proxied Device Endpoints - AI/ML
      // ========================================
      "/devices/{deviceId}/v1/embeddings": {
        post: {
          summary: "[device] generate embeddings",
          description: "generate vector embeddings for text (openai-compatible endpoint)",
          tags: ["device - ai"],
          operationId: "deviceEmbeddings",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
          ],
          requestBody: {
            required: true,
            content: {
              "application/json": {
                schema: { $ref: "#/components/schemas/EmbeddingRequest" },
              },
            },
          },
          responses: {
            200: {
              description: "embeddings generated",
              content: {
                "application/json": {
                  schema: { $ref: "#/components/schemas/EmbeddingResponse" },
                },
              },
            },
          },
        },
      },
      "/devices/{deviceId}/add": {
        post: {
          summary: "[device] add content",
          description: "add custom content (frames, audio) to the device's database",
          tags: ["device - data"],
          operationId: "deviceAddContent",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
          ],
          requestBody: {
            required: true,
            content: {
              "application/json": {
                schema: { $ref: "#/components/schemas/AddContentRequest" },
              },
            },
          },
          responses: {
            200: {
              description: "content added",
              content: {
                "application/json": {
                  schema: { $ref: "#/components/schemas/AddContentResponse" },
                },
              },
            },
          },
        },
      },

      // ========================================
      // Proxied Device Endpoints - Automation
      // ========================================
      "/devices/{deviceId}/open-application": {
        post: {
          summary: "[device] open application",
          description: "launch an application on the device",
          tags: ["device - automation"],
          operationId: "deviceOpenApplication",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
          ],
          requestBody: {
            required: true,
            content: {
              "application/json": {
                schema: {
                  type: "object",
                  required: ["app_name"],
                  properties: {
                    app_name: { type: "string", description: "name of the application to open" },
                  },
                },
              },
            },
          },
          responses: {
            200: {
              description: "application opened",
              content: {
                "application/json": {
                  schema: {
                    type: "object",
                    properties: {
                      success: { type: "boolean" },
                      message: { type: "string" },
                    },
                  },
                },
              },
            },
          },
        },
      },
      "/devices/{deviceId}/open-url": {
        post: {
          summary: "[device] open url",
          description: "open a url in the device's default browser",
          tags: ["device - automation"],
          operationId: "deviceOpenUrl",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
          ],
          requestBody: {
            required: true,
            content: {
              "application/json": {
                schema: {
                  type: "object",
                  required: ["url"],
                  properties: {
                    url: { type: "string", format: "uri" },
                    browser: { type: "string", description: "optional specific browser to use" },
                  },
                },
              },
            },
          },
          responses: {
            200: {
              description: "url opened",
              content: {
                "application/json": {
                  schema: {
                    type: "object",
                    properties: {
                      success: { type: "boolean" },
                      message: { type: "string" },
                    },
                  },
                },
              },
            },
          },
        },
      },
      "/devices/{deviceId}/notify": {
        post: {
          summary: "[device] send notification",
          description: "show a desktop notification on the device",
          tags: ["device - automation"],
          operationId: "deviceNotify",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
          ],
          requestBody: {
            required: true,
            content: {
              "application/json": {
                schema: {
                  type: "object",
                  required: ["title", "body"],
                  properties: {
                    title: { type: "string" },
                    body: { type: "string" },
                  },
                },
              },
            },
          },
          responses: {
            200: {
              description: "notification sent",
              content: {
                "application/json": {
                  schema: {
                    type: "object",
                    properties: {
                      success: { type: "boolean" },
                      message: { type: "string" },
                    },
                  },
                },
              },
            },
          },
        },
      },

      // ========================================
      // Proxied Device Endpoints - Streaming
      // ========================================
      "/devices/{deviceId}/ws/events": {
        get: {
          summary: "[device] websocket events",
          description: "websocket connection for real-time device events",
          tags: ["device - streaming"],
          operationId: "deviceWebSocket",
          parameters: [
            { $ref: "#/components/parameters/DeviceId" },
          ],
          responses: {
            101: {
              description: "websocket upgrade successful",
            },
            400: { description: "websocket upgrade failed" },
          },
        },
      },
    },

    components: {
      parameters: {
        DeviceId: {
          name: "deviceId",
          in: "path",
          required: true,
          schema: { type: "string" },
          description: "unique device identifier",
        },
      },

      securitySchemes: {
        M2MClientCredentials: {
          type: "oauth2",
          description: "M2M authentication using client credentials (recommended for scripts, automation, mcp servers). get credentials at https://cubby.sh/dashboard",
          flows: {
            clientCredentials: {
              tokenUrl: `${baseUrl}/oauth/token`,
              scopes: {
                "read:cubby": "read and write access to cubby data",
              },
            },
          },
        },
        OAuth2: {
          type: "oauth2",
          description: "user oauth authentication for third-party apps",
          flows: {
            authorizationCode: {
              authorizationUrl: `${baseUrl}/oauth/authorize`,
              tokenUrl: `${baseUrl}/oauth/token`,
              refreshUrl: `${baseUrl}/oauth/token`,
              scopes: {
                "openid": "openid connect authentication",
                "read:cubby": "read access to cubby data",
              },
            },
          },
          "x-registrationUrl": `${baseUrl}/oauth/register`,
        },
        BearerAuth: {
          type: "http",
          scheme: "bearer",
          bearerFormat: "JWT",
          description: "bearer token authentication using access token from M2M or oauth flow, or session jwt from direct login",
        },
      },

      schemas: {
        // ========================================
        // Authentication Schemas
        // ========================================
        AuthResponse: {
          type: "object",
          required: ["user_id", "session_token", "session_jwt"],
          properties: {
            user_id: { type: "string" },
            session_token: { type: "string" },
            session_jwt: { type: "string" },
          },
        },
        WhoAmIResponse: {
          type: "object",
          required: ["ok", "sub", "iss", "aud", "scopes"],
          properties: {
            ok: { type: "boolean" },
            sub: { type: "string", description: "user id" },
            iss: { type: "string", description: "token issuer" },
            aud: { type: "array", items: { type: "string" } },
            scopes: { type: "array", items: { type: "string" } },
            claims: { type: "object" },
          },
        },

        // ========================================
        // OAuth Schemas
        // ========================================
        TokenResponse: {
          type: "object",
          required: ["access_token", "token_type"],
          properties: {
            access_token: { type: "string" },
            token_type: { type: "string", example: "Bearer" },
            expires_in: { type: "integer", description: "seconds until expiration" },
            refresh_token: { type: "string" },
            scope: { type: "string" },
          },
        },
        ClientRegistrationRequest: {
          type: "object",
          required: ["redirect_uris"],
          properties: {
            redirect_uris: {
              type: "array",
              items: { type: "string", format: "uri" },
              description: "list of allowed redirect uris",
            },
            token_endpoint_auth_method: {
              type: "string",
              enum: ["none", "client_secret_post", "client_secret_basic"],
              default: "client_secret_basic",
            },
            grant_types: {
              type: "array",
              items: { type: "string" },
              default: ["authorization_code"],
            },
            response_types: {
              type: "array",
              items: { type: "string" },
              default: ["code"],
            },
            client_name: { type: "string" },
            client_uri: { type: "string", format: "uri" },
            logo_uri: { type: "string", format: "uri" },
            scope: { type: "string", default: "openid read:cubby" },
          },
        },
        ClientRegistrationResponse: {
          type: "object",
          required: ["client_id"],
          properties: {
            client_id: { type: "string" },
            client_secret: { type: "string", description: "only for confidential clients" },
            client_id_issued_at: { type: "integer" },
            client_secret_expires_at: { type: "integer" },
          },
        },

        // ========================================
        // Device Management Schemas
        // ========================================
        DeviceListResponse: {
          type: "object",
          required: ["devices"],
          properties: {
            devices: {
              type: "array",
              items: {
                type: "object",
                required: ["id", "userId", "createdAt", "updatedAt"],
                properties: {
                  id: { type: "string" },
                  userId: { type: "string", format: "uuid" },
                  createdAt: { type: "string", format: "date-time" },
                  updatedAt: { type: "string", format: "date-time" },
                },
              },
            },
          },
        },
        DeviceEnrollResponse: {
          type: "object",
          required: ["device_id", "hostname", "tunnel_token"],
          properties: {
            device_id: { type: "string" },
            hostname: { type: "string", description: "cloudflare tunnel hostname" },
            tunnel_token: { type: "string", description: "tunnel credentials" },
          },
        },

        // ========================================
        // Device Data Schemas
        // ========================================
        ContentType: {
          type: "string",
          enum: ["all", "ocr", "audio", "ui", "audio+ui", "ocr+ui", "audio+ocr"],
          description: "type of content to query",
        },
        HealthCheckResponse: {
          type: "object",
          required: ["status", "status_code", "message"],
          properties: {
            status: { type: "string" },
            status_code: { type: "integer" },
            last_frame_timestamp: { type: "string", format: "date-time", nullable: true },
            last_audio_timestamp: { type: "string", format: "date-time", nullable: true },
            last_ui_timestamp: { type: "string", format: "date-time", nullable: true },
            frame_status: { type: "string" },
            audio_status: { type: "string" },
            ui_status: { type: "string" },
            message: { type: "string" },
          },
        },
        SearchResponse: {
          type: "object",
          required: ["data", "pagination"],
          properties: {
            data: {
              type: "array",
              items: { $ref: "#/components/schemas/ContentItem" },
            },
            pagination: { $ref: "#/components/schemas/PaginationInfo" },
          },
        },
        ContentItem: {
          type: "object",
          required: ["type", "content"],
          properties: {
            type: { type: "string", enum: ["OCR", "Audio", "UI"] },
            content: { 
              type: "object",
              description: "content varies by type",
            },
          },
          discriminator: {
            propertyName: "type",
          },
        },
        PaginationInfo: {
          type: "object",
          required: ["limit", "offset", "total"],
          properties: {
            limit: { type: "integer" },
            offset: { type: "integer" },
            total: { type: "integer" },
          },
        },
        SearchMatch: {
          type: "object",
          required: ["frame_id", "timestamp", "text", "app_name", "window_name"],
          properties: {
            frame_id: { type: "integer" },
            timestamp: { type: "string", format: "date-time" },
            text: { type: "string" },
            app_name: { type: "string" },
            window_name: { type: "string" },
            confidence: { type: "number", format: "float" },
            url: { type: "string" },
          },
        },
        OCRResult: {
          type: "object",
          required: ["frame_id", "ocr_text", "timestamp", "file_path", "app_name"],
          properties: {
            frame_id: { type: "integer" },
            frame_name: { type: "string" },
            ocr_text: { type: "string" },
            timestamp: { type: "string", format: "date-time" },
            file_path: { type: "string" },
            app_name: { type: "string" },
            window_name: { type: "string" },
            tags: { type: "array", items: { type: "string" } },
          },
        },
        AudioDevice: {
          type: "object",
          required: ["name", "is_default"],
          properties: {
            name: { type: "string" },
            is_default: { type: "boolean" },
          },
        },
        MonitorInfo: {
          type: "object",
          required: ["id", "name", "width", "height", "is_default"],
          properties: {
            id: { type: "integer" },
            name: { type: "string" },
            width: { type: "integer" },
            height: { type: "integer" },
            is_default: { type: "boolean" },
          },
        },
        Speaker: {
          type: "object",
          required: ["id", "name", "metadata"],
          properties: {
            id: { type: "integer" },
            name: { type: "string" },
            metadata: { type: "string", description: "json metadata about the speaker" },
          },
        },
        EmbeddingRequest: {
          type: "object",
          required: ["model", "input"],
          properties: {
            model: { type: "string", example: "text-embedding-3-small" },
            input: { 
              oneOf: [
                { type: "string" },
                { type: "array", items: { type: "string" } },
              ],
            },
            encoding_format: { type: "string", default: "float" },
          },
        },
        EmbeddingResponse: {
          type: "object",
          required: ["object", "data", "model", "usage"],
          properties: {
            object: { type: "string", example: "list" },
            data: {
              type: "array",
              items: {
                type: "object",
                required: ["object", "embedding", "index"],
                properties: {
                  object: { type: "string", example: "embedding" },
                  embedding: { type: "array", items: { type: "number" } },
                  index: { type: "integer" },
                },
              },
            },
            model: { type: "string" },
            usage: {
              type: "object",
              required: ["prompt_tokens", "total_tokens"],
              properties: {
                prompt_tokens: { type: "integer" },
                total_tokens: { type: "integer" },
              },
            },
          },
        },
        AddContentRequest: {
          type: "object",
          required: ["device_name", "content"],
          properties: {
            device_name: { type: "string" },
            content: {
              type: "object",
              required: ["content_type", "data"],
              properties: {
                content_type: { type: "string", enum: ["vision", "audio"] },
                data: { type: "object" },
              },
            },
          },
        },
        AddContentResponse: {
          type: "object",
          required: ["success"],
          properties: {
            success: { type: "boolean" },
            message: { type: "string", nullable: true },
          },
        },
      },
    },
  };
}
