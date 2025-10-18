type EnvValue = string | undefined | null;
export type EnvProvider = (key: string) => EnvValue | Promise<EnvValue>;
export interface CubbyEnv {
  CUBBY_API_BASE_URL?: string;
}

let userEnvProvider: EnvProvider | undefined;

function defaultEnvProvider(key: string): EnvValue {
  try {
    const g: any = typeof globalThis !== "undefined" ? (globalThis as any) : {};
    // explicit user-injected object for browsers or workers
    if (g.__CUBBY_ENV__ && typeof g.__CUBBY_ENV__ === "object") {
      const v = g.__CUBBY_ENV__[key];
      if (typeof v === "string") return v;
    }
    // node/bun environments via globalThis
    const proc = g.process as any;
    if (proc && proc.env) {
      const v = proc.env[key];
      if (typeof v === "string") return v;
    }
  } catch {}
  return undefined;
}

export function setEnvProvider(provider: EnvProvider | undefined) {
  userEnvProvider = provider;
}

export function setEnv(key: string, value: string) {
  const g: any = typeof globalThis !== "undefined" ? (globalThis as any) : {};
  if (!g.__CUBBY_ENV__) g.__CUBBY_ENV__ = {};
  g.__CUBBY_ENV__[key] = value;
}

export async function getEnv(key: string): Promise<string | undefined> {
  if (userEnvProvider) {
    try {
      const v = await Promise.resolve(userEnvProvider(key));
      if (v != null) return String(v);
    } catch {}
  }
  const v = defaultEnvProvider(key);
  return v == null ? undefined : String(v);
}

export function getEnvSync(key: string): string | undefined {
  if (userEnvProvider) {
    try {
      const v = userEnvProvider(key) as any;
      if (typeof v === "string") return v;
    } catch {}
  }
  const v = defaultEnvProvider(key);
  return v == null ? undefined : String(v);
}

export const BASE_URL_ENV_KEY = "CUBBY_API_BASE_URL";
export const CLIENT_ID_ENV_KEY = "CUBBY_CLIENT_ID";
export const CLIENT_SECRET_ENV_KEY = "CUBBY_CLIENT_SECRET";

export async function getDefaultBaseUrl(): Promise<string> {
  const fromEnv = await getEnv(BASE_URL_ENV_KEY);
  return fromEnv || "https://api.cubby.sh";
}

export function getDefaultBaseUrlSync(): string {
  const fromEnv = getEnvSync(BASE_URL_ENV_KEY);
  return fromEnv || "https://api.cubby.sh";
}

export async function getClientId(): Promise<string | undefined> {
  return await getEnv(CLIENT_ID_ENV_KEY);
}

export function getClientIdSync(): string | undefined {
  return getEnvSync(CLIENT_ID_ENV_KEY);
}

export async function getClientSecret(): Promise<string | undefined> {
  return await getEnv(CLIENT_SECRET_ENV_KEY);
}

export function getClientSecretSync(): string | undefined {
  return getEnvSync(CLIENT_SECRET_ENV_KEY);
}


