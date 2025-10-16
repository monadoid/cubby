/**
 * Device MCP Client
 *
 * HTTP helpers for communicating with device MCP servers.
 */

import type { Bindings } from "../index";

/**
 * Build the device origin (base URL)
 */
export function buildDeviceOrigin(env: Bindings, deviceId: string): string {
  return `https://${deviceId}.${env.TUNNEL_DOMAIN}`;
}

/**
 * POST a JSON-RPC request to a device's MCP endpoint
 */
export async function postMcp(
  env: Bindings,
  deviceId: string,
  body: string,
  options?: {
    sessionId?: string;
    userId?: string;
    gwSessionId?: string;
    accept?: string; // override Accept header when needed
  },
): Promise<Response> {
  const origin = buildDeviceOrigin(env, deviceId);
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    Accept: options?.accept || "application/json, text/event-stream",
    "CF-Access-Client-Id": env.ACCESS_CLIENT_ID,
    "CF-Access-Client-Secret": env.ACCESS_CLIENT_SECRET,
  };

  if (options?.sessionId) {
    headers["Mcp-Session-Id"] = options.sessionId;
  }
  if (options?.userId) {
    headers["X-Cubby-User-Id"] = options.userId;
  }
  if (options?.gwSessionId) {
    headers["X-Cubby-Session-Id"] = options.gwSessionId;
  }

  return fetch(`${origin}/mcp`, {
    method: "POST",
    headers,
    body,
    signal: AbortSignal.timeout(10000), // 10 second timeout
  });
}

/**
 * GET SSE stream from a device's MCP endpoint
 */
export async function getMcpSse(
  env: Bindings,
  deviceId: string,
  searchParams: URLSearchParams,
  options?: {
    sessionId?: string;
    userId?: string;
    gwSessionId?: string;
    timeoutMs?: number;
  },
): Promise<Response> {
  const origin = buildDeviceOrigin(env, deviceId);
  const params = new URLSearchParams(searchParams);
  if (options?.sessionId && !params.has("sessionId")) {
    params.set("sessionId", options.sessionId);
  }
  const headers: Record<string, string> = {
    Accept: "text/event-stream",
    "CF-Access-Client-Id": env.ACCESS_CLIENT_ID,
    "CF-Access-Client-Secret": env.ACCESS_CLIENT_SECRET,
  };

  if (options?.sessionId) {
    headers["Mcp-Session-Id"] = options.sessionId;
  }
  if (options?.userId) {
    headers["X-Cubby-User-Id"] = options.userId;
  }
  if (options?.gwSessionId) {
    headers["X-Cubby-Session-Id"] = options.gwSessionId;
  }

  const url = `${origin}/mcp?${params.toString()}`;
  return fetch(url, {
    method: "GET",
    headers,
    signal: AbortSignal.timeout(options?.timeoutMs ?? 10000),
  });
}

/**
 * Initialize a device MCP session and return the device session ID
 */
export async function initializeDeviceSession(
  env: Bindings,
  deviceId: string,
  options?: {
    userId?: string;
    gwSessionId?: string;
  },
): Promise<string> {
  const initRequest = {
    jsonrpc: "2.0",
    id: crypto.randomUUID(),
    method: "initialize",
    params: {
      protocolVersion: "2024-11-05",
      capabilities: {},
      clientInfo: {
        name: "cubby-gateway",
        version: "1.0.0",
      },
    },
  };

  const response = await postMcp(env, deviceId, JSON.stringify(initRequest), {
    userId: options?.userId,
    gwSessionId: options?.gwSessionId,
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(
      `device initialize failed: ${response.status} ${response.statusText} - ${text}`,
    );
  }

  // Extract session ID from response header
  const deviceSessionId = response.headers.get("mcp-session-id");
  if (!deviceSessionId) {
    throw new Error("device did not return mcp-session-id header");
  }

  // Send initialized notification as required by MCP spec
  const initializedNotification = {
    jsonrpc: "2.0",
    method: "notifications/initialized",
  };

  const initNotifyResponse = await postMcp(
    env,
    deviceId,
    JSON.stringify(initializedNotification),
    {
      sessionId: deviceSessionId,
      userId: options?.userId,
      gwSessionId: options?.gwSessionId,
    },
  );

  if (!initNotifyResponse.ok) {
    console.warn(
      `device initialized notification failed: ${initNotifyResponse.status}`,
    );
  }

  return deviceSessionId;
}

