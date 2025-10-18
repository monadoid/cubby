import { getDefaultBaseUrlSync } from "./config";

export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

export interface TokenManagerOptions {
  clientId?: string;
  clientSecret?: string;
  baseUrl?: string;
  fetchImpl?: FetchLike;
}

interface TokenResponse {
  access_token: string;
  token_type: string;
  expires_in: number; // seconds
  scope?: string;
}

/**
 * manages oauth2 client credentials token lifecycle
 * automatically exchanges credentials for access tokens and refreshes before expiry
 */
export class TokenManager {
  private clientId: string | null;
  private clientSecret: string | null;
  private baseUrl: string;
  private fetchImpl: FetchLike;
  
  private token: string | null = null;
  private expiresAt: number | null = null;
  private refreshPromise: Promise<void> | null = null;

  // refresh token 60 seconds before expiry to prevent request failures
  private readonly REFRESH_BUFFER_SECONDS = 60;

  constructor(options: TokenManagerOptions = {}) {
    this.clientId = options.clientId ?? null;
    this.clientSecret = options.clientSecret ?? null;
    this.baseUrl = options.baseUrl || getDefaultBaseUrlSync();
    // bind fetch to globalThis to avoid Illegal invocation in Cloudflare Workers
    this.fetchImpl = options.fetchImpl || ((globalThis.fetch as unknown as FetchLike).bind(globalThis) as FetchLike);
  }

  /**
   * get a valid access token, refreshing if necessary
   * returns null if no credentials are configured
   */
  async getToken(): Promise<string | null> {
    // no credentials configured
    if (!this.clientId || !this.clientSecret) {
      return null;
    }

    // token is valid and not expiring soon
    if (this.token && this.expiresAt && Date.now() < this.expiresAt - this.REFRESH_BUFFER_SECONDS * 1000) {
      return this.token;
    }

    // if already refreshing, wait for that
    if (this.refreshPromise) {
      await this.refreshPromise;
      return this.token;
    }

    // exchange for new token
    this.refreshPromise = this.exchangeToken();
    try {
      await this.refreshPromise;
      return this.token;
    } finally {
      this.refreshPromise = null;
    }
  }

  /**
   * exchange client credentials for access token
   */
  private async exchangeToken(): Promise<void> {
    if (!this.clientId || !this.clientSecret) {
      throw new Error(
        "cubby sdk: missing client credentials. set CUBBY_CLIENT_ID and CUBBY_CLIENT_SECRET environment variables, " +
        "or pass { clientId, clientSecret } to createClient(). get credentials at https://cubby.sh/dashboard"
      );
    }

    const tokenUrl = new URL("/oauth/token", this.baseUrl).toString();
    
    const body = new URLSearchParams({
      grant_type: "client_credentials",
      client_id: this.clientId,
      client_secret: this.clientSecret,
      scope: "read:cubby",
    });

    try {
      const response = await this.fetchImpl(tokenUrl, {
        method: "POST",
        headers: {
          "Content-Type": "application/x-www-form-urlencoded",
        },
        body: body.toString(),
      });

      if (!response.ok) {
        const error = await response.text().catch(() => "unknown error");
        throw new Error(
          `cubby sdk: token exchange failed (${response.status}): ${error}. ` +
          `verify your credentials at https://cubby.sh/dashboard`
        );
      }

      const data: TokenResponse = await response.json();
      
      this.token = data.access_token;
      // set expiry time: current time + expires_in seconds
      this.expiresAt = Date.now() + data.expires_in * 1000;

    } catch (error: any) {
      // clear token on failure
      this.token = null;
      this.expiresAt = null;
      
      if (error.message?.includes("cubby sdk:")) {
        throw error;
      }
      throw new Error(
        `cubby sdk: failed to exchange credentials for token: ${error.message || "unknown error"}. ` +
        `check your network connection and verify credentials at https://cubby.sh/dashboard`
      );
    }
  }

  /**
   * clear cached token, forcing refresh on next getToken() call
   */
  clearToken(): void {
    this.token = null;
    this.expiresAt = null;
  }

  /**
   * update credentials
   */
  setCredentials(clientId: string, clientSecret: string): void {
    this.clientId = clientId;
    this.clientSecret = clientSecret;
    this.clearToken();
  }
}

