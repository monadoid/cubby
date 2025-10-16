/**
 * Gateway MCP Session Store
 *
 * In-memory mapping from gateway session IDs to device sessions and auth.
 * Maps: gw_session_id → { userId, deviceId?, deviceSessionId?, accessToken?, scopes? }
 */

export interface GatewaySession {
  userId: string;
  deviceId?: string;
  deviceSessionId?: string;
  accessToken?: string;
  scopes?: string[];
}

// In-memory store (module-level Map)
const sessionStore = new Map<string, GatewaySession>();

/**
 * Extract the Mcp-Session-Id header from a request
 */
export function getGwSessionId(req: Request): string | undefined {
  return req.headers.get("mcp-session-id") || undefined;
}

/**
 * Get or create a gateway session
 */
export function getOrCreateSession(
  gwSessionId: string,
  userId: string,
): GatewaySession {
  let session = sessionStore.get(gwSessionId);
  if (!session) {
    session = { userId };
    sessionStore.set(gwSessionId, session);
  }
  return session;
}

/**
 * Set the selected device for a gateway session
 */
export function setDevice(
  gwSessionId: string,
  deviceId: string,
  deviceSessionId: string,
): void {
  const session = sessionStore.get(gwSessionId);
  if (!session) {
    throw new Error("session not found");
  }
  session.deviceId = deviceId;
  session.deviceSessionId = deviceSessionId;
}

/**
 * Get an existing session (returns undefined if not found)
 */
export function getSession(gwSessionId: string): GatewaySession | undefined {
  return sessionStore.get(gwSessionId);
}

/**
 * Set authentication info for a gateway session
 */
export function setSessionAuth(
  gwSessionId: string,
  accessToken: string,
  scopes: string[],
): void {
  const session = sessionStore.get(gwSessionId);
  if (!session) {
    throw new Error("session not found");
  }
  session.accessToken = accessToken;
  session.scopes = scopes;
}

/**
 * Get authentication info from a gateway session
 */
export function getSessionAuth(
  gwSessionId: string,
): { accessToken: string; scopes: string[] } | undefined {
  const session = sessionStore.get(gwSessionId);
  if (!session?.accessToken) {
    return undefined;
  }
  return {
    accessToken: session.accessToken,
    scopes: session.scopes || [],
  };
}

