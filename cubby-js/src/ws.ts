import { WebSocket as IsoWebSocket } from "isows";
import type { TokenManager } from "./auth";

export type IsoWebSocketInstance = InstanceType<typeof IsoWebSocket>;

export interface CreateSocketOptions {
  baseUrl: string;
  deviceId: string;
  includeImages?: boolean;
  tokenManager?: TokenManager;
}

export function toWsUrl(baseHttpUrl: string, path: string, query: Record<string, string | number | boolean | undefined>): string {
  const httpUrl = new URL(path.startsWith("/") ? path : `/${path}`, baseHttpUrl);
  const isSecure = httpUrl.protocol === "https:";
  const wsProtocol = isSecure ? "wss:" : "ws:";
  const wsUrl = new URL(httpUrl.toString());
  wsUrl.protocol = wsProtocol;
  Object.entries(query).forEach(([k, v]) => {
    if (v === undefined) return;
    wsUrl.searchParams.set(k, String(v));
  });
  return wsUrl.toString();
}

export async function createEventSocketAsync(opts: CreateSocketOptions): Promise<IsoWebSocketInstance> {
  let token: string | null = null;
  if (opts.tokenManager) {
    try {
      token = await opts.tokenManager.getToken();
    } catch (error) {
      console.error("failed to get token for websocket:", error);
      token = null;
    }
  }
  const path = `/devices/${encodeURIComponent(opts.deviceId)}/ws/events`;
  const url = toWsUrl(opts.baseUrl, path, {
    images: Boolean(opts.includeImages),
    access_token: token || undefined,
  });
  return new (IsoWebSocket as any)(url) as IsoWebSocketInstance;
}


