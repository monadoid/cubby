import type { cubbyQueryParams, cubbyResponse, NotificationOptions } from "./types";
import { HttpClient } from "./http";
import { createEventSocketAsync } from "./ws";
import { getDefaultBaseUrlSync } from "./config";
import type { CubbyEnv } from "./config";

export interface ClientOptions {
  baseUrl?: string;
  token?: string | null;
  tokenProvider?: () => Promise<string | null> | string | null;
  fetchImpl?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
  credentials?: RequestCredentials;
  env?: CubbyEnv;
}

export class CubbyClient {
  private http: HttpClient;
  private baseUrl: string;
  private token: string | null;
  public device: {
    openApplication: (appName: string) => Promise<boolean>;
    openUrl: (url: string, browser?: string) => Promise<boolean>;
  };

  constructor(opts: ClientOptions = {}) {
    const resolvedBaseUrl = (opts.env?.CUBBY_API_BASE_URL || opts.baseUrl || getDefaultBaseUrlSync()) as string;
    this.http = new HttpClient({ ...opts, baseUrl: resolvedBaseUrl });
    this.baseUrl = resolvedBaseUrl;
    this.token = opts.token ?? null;

    // device control using gateway endpoints
    const fetchImpl = (input: RequestInfo | URL, init?: RequestInit) => (this.http as any).fetchImpl(input, init) || fetch(input, init);
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

  setAuthToken(token: string | null) {
    this.token = token;
    this.http.setToken(token);
  }

  setTokenProvider(provider: (() => Promise<string | null> | string | null) | undefined) {
    this.http.setTokenProvider(provider);
  }

  // device-scoped methods are defined later

  async notify(options: NotificationOptions): Promise<{ success: boolean }>{
    return await this.http.post<{ success: boolean }>("/notify", options);
  }

  streamEvents(includeImages: boolean = false, deviceId?: string): AsyncGenerator<any, void, unknown> {
    return (async function* (this: CubbyClient) {
      try {
        const id = deviceId || (await this.getDefaultDeviceId());
        const ws = await createEventSocketAsync({ baseUrl: this.baseUrl, deviceId: id, includeImages, token: this.token });
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

  // device discovery helpers and device-scoped http
  async listDevices(): Promise<{ devices: Array<{ id: string }> }> {
    return await this.http.get("/devices");
  }

  private async getDefaultDeviceId(): Promise<string> {
    const res = await this.listDevices();
    if (!res?.devices?.length) throw new Error("no devices found");
    return String(res.devices[0].id);
  }

  async search(params: cubbyQueryParams & { deviceId?: string }): Promise<cubbyResponse> {
    const id = params.deviceId || (await this.getDefaultDeviceId());
    const { deviceId, ...rest } = params as any;
    return await this.http.get<cubbyResponse>(`/devices/${encodeURIComponent(id)}/search`, rest);
  }

  async semanticSearch(params: Record<string, unknown> & { deviceId?: string }): Promise<any> {
    const id = (params as any).deviceId || (await this.getDefaultDeviceId());
    const { deviceId, ...rest } = params as any;
    return await this.http.get<any>(`/devices/${encodeURIComponent(id)}/semantic-search`, rest);
  }

  async speakersSearch(params: Record<string, unknown> & { deviceId?: string }): Promise<any> {
    const id = (params as any).deviceId || (await this.getDefaultDeviceId());
    const { deviceId, ...rest } = params as any;
    return await this.http.get<any>(`/devices/${encodeURIComponent(id)}/speakers/search`, rest);
  }
}

export function createClient(opts?: ClientOptions) {
  return new CubbyClient(opts);
}

export * from "./types";


