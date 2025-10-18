import { toSnakeCase } from "./utils";
import { getDefaultBaseUrlSync } from "./config";

export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

export interface HttpClientOptions {
  baseUrl?: string;
  token?: string | null;
  tokenProvider?: () => Promise<string | null> | string | null;
  fetchImpl?: FetchLike;
  credentials?: RequestCredentials;
}

export class HttpClient {
  private baseUrl: string;
  private token: string | null;
  private tokenProvider?: () => Promise<string | null> | string | null;
  private fetchImpl: FetchLike;
  private credentials?: RequestCredentials;

  constructor(options: HttpClientOptions = {}) {
    const envBase = getDefaultBaseUrlSync();
    this.baseUrl = options.baseUrl || envBase;
    this.token = options.token ?? null;
    this.tokenProvider = options.tokenProvider;
    this.fetchImpl = options.fetchImpl || (globalThis.fetch as FetchLike);
    this.credentials = options.credentials;
  }

  public setBaseUrl(url: string) {
    this.baseUrl = url;
  }

  public setToken(token: string | null) {
    this.token = token;
  }

  public setTokenProvider(provider: (() => Promise<string | null> | string | null) | undefined) {
    this.tokenProvider = provider;
  }

  private async resolveToken(): Promise<string | null> {
    if (this.token != null) return this.token;
    if (!this.tokenProvider) return null;
    try {
      const t = await Promise.resolve(this.tokenProvider());
      return t ?? null;
    } catch {
      return null;
    }
  }

  private buildUrl(path: string, query?: Record<string, unknown>): string {
    const url = new URL(path.startsWith("/") ? path : `/${path}`, this.baseUrl);
    if (query) {
      Object.entries(query).forEach(([key, value]) => {
        if (value === undefined || value === null || value === "") return;
        if (Array.isArray(value)) {
          if (value.length > 0) url.searchParams.append(toSnakeCase(key), value.join(","));
        } else {
          url.searchParams.append(toSnakeCase(key), String(value));
        }
      });
    }
    return url.toString();
  }

  public async get<T>(path: string, query?: Record<string, unknown>, init?: RequestInit): Promise<T> {
    const url = this.buildUrl(path, query);
    const token = await this.resolveToken();
    const headers: Record<string, string> = { "Content-Type": "application/json" };
    if (token) headers["Authorization"] = `Bearer ${token}`;
    const resp = await this.fetchImpl(url, {
      method: "GET",
      headers: { ...headers, ...(init?.headers || {}) },
      credentials: this.credentials,
      ...init,
    });
    if (!resp.ok) throw await this.errorFromResponse(resp);
    return (await resp.json()) as T;
  }

  public async post<T>(path: string, body?: unknown, init?: RequestInit): Promise<T> {
    const url = this.buildUrl(path);
    const token = await this.resolveToken();
    const headers: Record<string, string> = { "Content-Type": "application/json" };
    if (token) headers["Authorization"] = `Bearer ${token}`;
    const resp = await this.fetchImpl(url, {
      method: "POST",
      headers: { ...headers, ...(init?.headers || {}) },
      body: body === undefined ? undefined : JSON.stringify(body),
      credentials: this.credentials,
      ...init,
    });
    if (!resp.ok) throw await this.errorFromResponse(resp);
    return (await resp.json()) as T;
  }

  private async errorFromResponse(resp: Response): Promise<Error> {
    const text = await resp.text().catch(() => "");
    try {
      const json = text ? JSON.parse(text) : null;
      return new Error(`http ${resp.status}: ${json?.error || resp.statusText}`);
    } catch {
      return new Error(`http ${resp.status}: ${text || resp.statusText}`);
    }
  }
}


