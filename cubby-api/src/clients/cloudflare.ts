// src/lib/cloudflareClient.ts
// Cloudflare v4 REST client (Workers-compatible) with minimal Zod schemas
// Endpoints covered:
// - POST  /accounts/{account_id}/cfd_tunnel                          (Create Tunnel)
// - PUT   /accounts/{account_id}/cfd_tunnel/{tunnel_id}/configurations (Put Tunnel Configuration)
// - POST  /zones/{zone_id}/dns_records                                 (Create DNS Record)
// - GET   /accounts/{account_id}/cfd_tunnel/{tunnel_id}/token          (Get Tunnel Token)
//
// Docs pages referenced while shaping requests/responses:
// Create Tunnel, Put Config, DNS Create, Get Token
// - https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/get-started/create-remote-tunnel-api/
// - https://developers.cloudflare.com/api/resources/dns/subresources/records/methods/create/
// - https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/configure-tunnels/remote-tunnel-permissions/

import { z } from "zod";

/** Shared “envelope” bits Cloudflare returns */
const ErrorItem = z
  .object({
    code: z.number().optional(),
    message: z.string(),
  })
  .passthrough();

const EnvelopeBase = z.object({
  success: z.boolean(),
  errors: z.array(ErrorItem).default([]),
  messages: z.array(z.any()).optional(),
});

/** -----------------------
 *  Create Tunnel (remote)
 *  POST /accounts/{account_id}/cfd_tunnel
 *  Docs show request { name, config_src:"cloudflare" } and result includes { id, token }.
 *  ----------------------*/
export const CreateTunnelRequestSchema = z.object({
  name: z.string(),
  config_src: z.literal("cloudflare"),
});
export type CreateTunnelRequest = z.infer<typeof CreateTunnelRequestSchema>;

const CreateTunnelResultSchema = z.object({
  id: z.string().uuid(), // tunnel UUID
  token: z.string().optional(), // present for remotely-managed tunnel creation
});
export type CreateTunnelResult = z.infer<typeof CreateTunnelResultSchema>;

export const CreateTunnelResponseSchema = EnvelopeBase.extend({
  success: z.literal(true),
  result: CreateTunnelResultSchema,
}).passthrough();
export type CreateTunnelResponse = z.infer<typeof CreateTunnelResponseSchema>;

/** -----------------------
 *  Put Tunnel Configuration (ingress rules)
 *  PUT /accounts/{account_id}/cfd_tunnel/{tunnel_id}/configurations
 *  Docs example body { config: { ingress: [{ hostname, service }, { service:"http_status:404" }] } }
 *  ----------------------*/
export const IngressRuleSchema = z
  .object({
    // catch-all rule has no hostname
    hostname: z.string().min(1).optional(),
    service: z.string().min(1),
    // We do not need other fields (originRequest, path, etc.) for this flow
  })
  .strict();
export type IngressRule = z.infer<typeof IngressRuleSchema>;

export const PutTunnelConfigRequestSchema = z.object({
  config: z.object({
    ingress: z.array(IngressRuleSchema).min(1),
  }),
});
export type PutTunnelConfigRequest = z.infer<
  typeof PutTunnelConfigRequestSchema
>;

export const PutTunnelConfigResponseSchema = EnvelopeBase.extend({
  // Minimal: we only need to know it succeeded
  success: z.literal(true),
}).passthrough();
export type PutTunnelConfigResponse = z.infer<
  typeof PutTunnelConfigResponseSchema
>;

/** -----------------------
 *  Create DNS Record (CNAME)
 *  POST /zones/{zone_id}/dns_records
 *  Docs example body requires: type, name, content, proxied (ttl optional)
 *  Response “result” includes id & name (and more). We only need id+name.
 *  ----------------------*/
export const CreateDnsRecordRequestSchema = z.object({
  type: z.literal("CNAME"),
  name: z.string().min(1),
  content: z.string().min(1), // e.g. "<tunnel_id>.cfargotunnel.com"
  proxied: z.boolean(),
  ttl: z.number().optional(), // 1 = "automatic"
});
export type CreateDnsRecordRequest = z.infer<
  typeof CreateDnsRecordRequestSchema
>;

const DnsRecordMinimalSchema = z.object({
  id: z.string(),
  name: z.string(),
});
export type DnsRecordMinimal = z.infer<typeof DnsRecordMinimalSchema>;

export const CreateDnsRecordResponseSchema = EnvelopeBase.extend({
  success: z.literal(true),
  result: DnsRecordMinimalSchema,
}).passthrough();
export type CreateDnsRecordResponse = z.infer<
  typeof CreateDnsRecordResponseSchema
>;

/** List & Update DNS records (CNAME upsert helpers) */
const ListDnsRecordsResponseSchema = EnvelopeBase.extend({
  success: z.literal(true),
  result: z.array(
    z
      .object({
        id: z.string(),
        type: z.literal("CNAME"),
        name: z.string(),
        content: z.string(),
        proxied: z.boolean().optional(),
        ttl: z.number().optional(),
      })
      .passthrough(),
  ),
}).passthrough();
type ListDnsRecordsResponse = z.infer<typeof ListDnsRecordsResponseSchema>;

const UpdateDnsRecordRequestSchema = CreateDnsRecordRequestSchema;
type UpdateDnsRecordRequest = z.infer<typeof UpdateDnsRecordRequestSchema>;

const UpdateDnsRecordResponseSchema = EnvelopeBase.extend({
  success: z.literal(true),
  result: DnsRecordMinimalSchema,
}).passthrough();
type UpdateDnsRecordResponse = z.infer<typeof UpdateDnsRecordResponseSchema>;

/** -----------------------
 *  List Tunnels (filter by name)
 *  GET /accounts/{account_id}/cfd_tunnel?name={name}
 *  ----------------------*/
const TunnelItemSchema = z
  .object({
    id: z.string().uuid(),
    name: z.string(),
    deleted_at: z.string().nullable().optional(),
  })
  .passthrough();

const ListTunnelsResponseSchema = EnvelopeBase.extend({
  success: z.literal(true),
  result: z.array(TunnelItemSchema),
}).passthrough();
type ListTunnelsResponse = z.infer<typeof ListTunnelsResponseSchema>;

/** -----------------------
 *  Get Tunnel Token
 *  GET /accounts/{account_id}/cfd_tunnel/{tunnel_id}/token
 *  ----------------------*/
export const GetTunnelTokenResponseSchema = EnvelopeBase.extend({
  success: z.literal(true),
  result: z.string(),
}).passthrough();
export type GetTunnelTokenResponse = z.infer<
  typeof GetTunnelTokenResponseSchema
>;

/** Client */
export type CloudflareClientOptions = {
  accountId: string;
  zoneId: string;
  apiToken: string;
  baseUrl?: string; // default https://api.cloudflare.com/client/v4
};

export class CloudflareClient {
  private accountId: string;
  private zoneId: string;
  private apiToken: string;
  private baseUrl: string;

  constructor(opts: CloudflareClientOptions) {
    this.accountId = opts.accountId;
    this.zoneId = opts.zoneId;
    this.apiToken = opts.apiToken;
    this.baseUrl = opts.baseUrl ?? "https://api.cloudflare.com/client/v4";
  }

  /** Helpers */
  private headers(): HeadersInit {
    return {
      Authorization: `Bearer ${this.apiToken}`,
      "Content-Type": "application/json",
    };
  }

  private async do<T extends z.ZodTypeAny>(
    path: string,
    init: RequestInit,
    schema: T,
  ): Promise<z.infer<T>> {
    const res = await fetch(`${this.baseUrl}${path}`, {
      ...init,
      headers: {
        ...this.headers(),
        ...(init.headers ?? {}),
      },
    });
    const text = await res.text();
    let json: unknown;
    try {
      json = text ? JSON.parse(text) : {};
    } catch {
      throw new Error(
        `Cloudflare API: Non-JSON response (${res.status}) ${text}`,
      );
    }
    if (!res.ok) {
      // Try to surface Cloudflare error details
      try {
        const env = EnvelopeBase.parse(json);
        const msg = env.errors?.map((e) => e.message).join("; ");
        throw new Error(`Cloudflare API error (${res.status}): ${msg || text}`);
      } catch {
        throw new Error(`Cloudflare API error (${res.status}): ${text}`);
      }
    }
    return schema.parse(json);
  }

  /** Create a remotely-managed tunnel; returns tunnel id and (usually) a token */
  async createTunnel(req: CreateTunnelRequest) {
    const body = CreateTunnelRequestSchema.parse(req);
    const out = await this.do(
      `/accounts/${this.accountId}/cfd_tunnel`,
      { method: "POST", body: JSON.stringify(body) },
      CreateTunnelResponseSchema,
    );
    return out.result; // { id, token? }
  }

  /** Find a tunnel by exact name (ignoring deleted ones) */
  async getTunnelByName(name: string) {
    const out: ListTunnelsResponse = await this.do(
      `/accounts/${this.accountId}/cfd_tunnel?name=${encodeURIComponent(name)}`,
      { method: "GET" },
      ListTunnelsResponseSchema,
    );
    // @ts-ignore
    return out.result.find((t: { deleted_at: any }) => !t.deleted_at);
  }

  /**
   * Create or reuse a tunnel by name.
   * Handles "already exists" (409 / code 1013) by fetching the existing tunnel.
   */
  async createOrGetTunnel(name: string) {
    try {
      const created = await this.createTunnel({
        name,
        config_src: "cloudflare",
      });
      console.log(`✅ Created new tunnel: ${name} (${created.id})`);
      return created; // { id, token? }
    } catch (err: any) {
      const status = err?.status;
      const envelope = err?.envelope as
        | z.infer<typeof EnvelopeBase>
        | undefined;
      const code1013 = envelope?.errors?.some((e) => (e as any)?.code === 1013);
      const msg = String(err?.message ?? "");
      const indicatesExists =
        status === 409 ||
        code1013 ||
        msg.includes("already have a tunnel with this name");

      if (!indicatesExists) throw err;

      const existing = await this.getTunnelByName(name);
      if (!existing) throw err; // race or soft-deleted; bubble up

      console.log(`🔄 Reusing existing tunnel: ${name} (${existing.id})`);
      // Token may not be immediately available here; caller can call getTunnelToken
      return { id: existing.id, token: undefined as string | undefined };
    }
  }

  /** Put the tunnel configuration (ingress rules). Include a catch-all as needed. */
  async putTunnelConfig(tunnelId: string, req: PutTunnelConfigRequest) {
    const body = PutTunnelConfigRequestSchema.parse(req);
    await this.do(
      `/accounts/${this.accountId}/cfd_tunnel/${encodeURIComponent(tunnelId)}/configurations`,
      { method: "PUT", body: JSON.stringify(body) },
      PutTunnelConfigResponseSchema,
    );
    return true;
  }

  /** Create a proxied CNAME DNS record that points hostname -> <tunnel_id>.cfargotunnel.com */
  async createCnameRecord(req: CreateDnsRecordRequest) {
    const body = CreateDnsRecordRequestSchema.parse(req);
    const out = await this.do(
      `/zones/${this.zoneId}/dns_records`,
      { method: "POST", body: JSON.stringify(body) },
      CreateDnsRecordResponseSchema,
    );
    return out.result; // { id, name }
  }

  /** List DNS records by name/type */
  private async listDnsRecordsByName(name: string, type: "CNAME") {
    const out: ListDnsRecordsResponse = await this.do(
      `/zones/${this.zoneId}/dns_records?type=${type}&name=${encodeURIComponent(name)}`,
      { method: "GET" },
      ListDnsRecordsResponseSchema,
    );
    return out.result;
  }

  /** Update DNS record by id */
  private async updateDnsRecord(recordId: string, req: UpdateDnsRecordRequest) {
    const body = UpdateDnsRecordRequestSchema.parse(req);
    const out: UpdateDnsRecordResponse = await this.do(
      `/zones/${this.zoneId}/dns_records/${encodeURIComponent(recordId)}`,
      { method: "PUT", body: JSON.stringify(body) },
      UpdateDnsRecordResponseSchema,
    );
    return out.result;
  }

  /**
   * Upsert a CNAME record (create if missing, otherwise update in place).
   * This makes /devices/enroll idempotent and safe to retry.
   */
  async upsertCnameRecord(req: CreateDnsRecordRequest) {
    try {
      const result = await this.createCnameRecord(req);
      console.log(`✅ Created new DNS record: ${req.name} -> ${req.content}`);
      return result;
    } catch (err: any) {
      // If creation failed (likely because record exists), try list+update
      const existing = await this.listDnsRecordsByName(req.name, "CNAME");
      if (!existing.length) throw err;
      const record = existing[0];
      const result = await this.updateDnsRecord(record.id, req);
      console.log(
        `🔄 Updated existing DNS record: ${req.name} -> ${req.content}`,
      );
      return result;
    }
  }

  /** Retrieve the tunnel token if it wasn’t returned at creation (or after rotation) */
  async getTunnelToken(tunnelId: string) {
    const out = await this.do(
      `/accounts/${this.accountId}/cfd_tunnel/${encodeURIComponent(tunnelId)}/token`,
      { method: "GET" },
      GetTunnelTokenResponseSchema,
    );
    return out.result; // token string
  }
}

/** Convenience builders for your enroll flow */

export function buildIngressForHost(
  hostname: string,
  service = "http://localhost:3030",
): PutTunnelConfigRequest {
  return PutTunnelConfigRequestSchema.parse({
    config: {
      ingress: [{ hostname, service }, { service: "http_status:404" }],
    },
  });
}

export function buildCnameForTunnel(
  hostname: string,
  tunnelId: string,
): CreateDnsRecordRequest {
  return CreateDnsRecordRequestSchema.parse({
    type: "CNAME",
    name: hostname,
    content: `${tunnelId}.cfargotunnel.com`,
    proxied: true,
    ttl: 1, // "auto"
  });
}
