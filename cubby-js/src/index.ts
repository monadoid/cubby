import type { cubbyQueryParams, cubbyResponse, NotificationOptions } from "./types";
import { HttpClient } from "./http";
import { createEventSocketAsync } from "./ws";
import { getDefaultBaseUrlSync, getClientIdSync, getClientSecretSync } from "./config";
import type { CubbyEnv } from "./config";
import { TokenManager } from "./auth";

export interface ClientOptions {
  baseUrl?: string;
  clientId?: string;
  clientSecret?: string;
  fetchImpl?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
  credentials?: RequestCredentials;
  env?: CubbyEnv;
}

export class CubbyClient {
  private http: HttpClient;
  private baseUrl: string;
  private tokenManager: TokenManager;
  private selectedDeviceId: string | null = null;
  public device: {
    openApplication: (appName: string) => Promise<boolean>;
    openUrl: (url: string, browser?: string) => Promise<boolean>;
  };

  constructor(opts: ClientOptions = {}) {
    const resolvedBaseUrl = (opts.env?.CUBBY_API_BASE_URL || opts.baseUrl || getDefaultBaseUrlSync()) as string;
    
    // resolve client credentials from options or environment
    const clientId = opts.clientId || getClientIdSync();
    const clientSecret = opts.clientSecret || getClientSecretSync();
    
    // initialize token manager with credentials
    this.tokenManager = new TokenManager({
      clientId,
      clientSecret,
      baseUrl: resolvedBaseUrl,
      fetchImpl: opts.fetchImpl,
    });
    
    this.http = new HttpClient({ 
      baseUrl: resolvedBaseUrl,
      tokenManager: this.tokenManager,
      fetchImpl: opts.fetchImpl,
      credentials: opts.credentials,
    });
    this.baseUrl = resolvedBaseUrl;

    // device control using gateway endpoints
    const fetchImpl = opts.fetchImpl || fetch.bind(globalThis);
    this.device = {
      openApplication: async (name: string): Promise<boolean> => {
        const resp = await fetchImpl(new URL("/open-application", this.baseUrl).toString(), {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ app_name: name }),
        });
        if (!resp.ok) return false;
        const data = await resp.json().catch(() => ({}));
        return Boolean((data && (data.success ?? true)) || resp.ok);
      },
      openUrl: async (url: string, browser?: string): Promise<boolean> => {
        const resp = await fetchImpl(new URL("/open-url", this.baseUrl).toString(), {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ url, browser }),
        });
        if (!resp.ok) return false;
        const data = await resp.json().catch(() => ({}));
        return Boolean((data && (data.success ?? true)) || resp.ok);
      },
    };
  }

  setBaseUrl(url: string) {
    this.baseUrl = url;
    this.http.setBaseUrl(url);
  }

  setCredentials(clientId: string, clientSecret: string) {
    this.tokenManager.setCredentials(clientId, clientSecret);
  }

  // device-scoped methods are defined later

  setDeviceId(deviceId: string) {
    this.selectedDeviceId = deviceId;
  }

  clearDeviceId() {
    this.selectedDeviceId = null;
  }

  private requireDeviceId(passed?: string): string {
    if (passed && String(passed).length > 0) return String(passed);
    if (this.selectedDeviceId && this.selectedDeviceId.length > 0) return this.selectedDeviceId;
    throw new Error("device not set: call listDevices() and client.setDeviceId(id) or pass deviceId explicitly");
  }

  async notify(options: NotificationOptions & { deviceId?: string }): Promise<{ success: boolean }>{
    const id = this.requireDeviceId(options.deviceId);
    const { deviceId, ...rest } = options as any;
    return await this.http.post<{ success: boolean }>(`/devices/${encodeURIComponent(id)}/notify`, rest);
  }

  streamEvents(includeImages: boolean = false, deviceId?: string): AsyncGenerator<any, void, unknown> {
    return (async function* (this: CubbyClient) {
      try {
        const id = this.requireDeviceId(deviceId);
        const ws = await createEventSocketAsync({ baseUrl: this.baseUrl, deviceId: id, includeImages, tokenManager: this.tokenManager });
        await new Promise<void>((resolve, reject) => {
          ws.addEventListener("open", () => resolve());
          ws.addEventListener("error", (e: Event) => reject(e));
        });
        const queue: MessageEvent[] = [];
        let resolveNext: ((val: MessageEvent) => void) | null = null;
        const onMessage = (ev: MessageEvent) => {
          if (resolveNext) {
            resolveNext(ev);
            resolveNext = null;
          } else {
            queue.push(ev);
          }
        };
        ws.addEventListener("message", onMessage);
        try {
          while (true) {
            const msg = await new Promise<MessageEvent>((resolve) => {
              if (queue.length > 0) resolve(queue.shift()!);
              else resolveNext = resolve;
            });
            yield JSON.parse((msg as any).data);
          }
        } finally {
          ws.removeEventListener("message", onMessage);
          ws.close();
        }
      } catch (err) {
        console.error("failed to open websocket:", err);
      }
    }).call(this);
  }

  async *streamTranscriptions(deviceId?: string): AsyncGenerator<any, void, unknown> {
    for await (const evt of this.streamEvents(false, deviceId)) {
      if (evt?.name === "transcription") yield evt;
    }
  }

  async *streamVision(includeImages: boolean = false, deviceId?: string): AsyncGenerator<any, void, unknown> {
    for await (const evt of this.streamEvents(includeImages, deviceId)) {
      if (evt?.name === "ocr_result" || evt?.name === "ui_frame") yield evt;
    }
  }

  /**
   * get current access token (mainly for debugging or advanced use cases)
   * the sdk automatically manages tokens, so you typically don't need this
   */
  async getAccessToken(): Promise<string | null> {
    return await this.tokenManager.getToken();
  }

  // device discovery helpers and device-scoped http
  async listDevices(): Promise<{ devices: Array<{ id: string }> }> {
    return await this.http.get("/devices");
  }

  async search(params: cubbyQueryParams & { deviceId?: string }): Promise<cubbyResponse> {
    const id = this.requireDeviceId((params as any).deviceId);
    const { deviceId, ...rest } = params as any;
    return await this.http.get<cubbyResponse>(`/devices/${encodeURIComponent(id)}/search`, rest);
  }

  // async semanticSearch(params: Record<string, unknown> & { deviceId?: string }): Promise<any> {
  //   const id = this.requireDeviceId((params as any).deviceId);
  //   const { deviceId, ...rest } = params as any;
  //   return await this.http.get<any>(`/devices/${encodeURIComponent(id)}/semantic-search`, rest);
  // }

  async speakersSearch(params: Record<string, unknown> & { deviceId?: string }): Promise<any> {
    const id = this.requireDeviceId((params as any).deviceId);
    const { deviceId, ...rest } = params as any;
    return await this.http.get<any>(`/devices/${encodeURIComponent(id)}/speakers/search`, rest);
  }
}

export function createClient(opts?: ClientOptions) {
  return new CubbyClient(opts);
}

export * from "./types";


