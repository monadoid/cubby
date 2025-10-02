import { AuthorizationSession } from './oauth'

const encoder = new TextEncoder()

export const SESSION_COOKIE_NAME = 'cubby_oauth_session'
export const SESSION_TTL_SECONDS = 600

export async function encodeSession(session: AuthorizationSession, secret: string): Promise<string> {
  const payload = JSON.stringify(session)
  const data = base64UrlEncode(payload)
  const signature = await sign(data, secret)
  return `${data}.${signature}`
}

export async function decodeSession(value: string | null | undefined, secret: string): Promise<AuthorizationSession | null> {
  if (!value) return null

  const [data, signature] = value.split('.')
  if (!data || !signature) {
    return null
  }

  const expectedSignature = await sign(data, secret)
  if (!constantTimeEqual(signature, expectedSignature)) {
    return null
  }

  try {
    const payload = base64UrlDecodeToString(data)
    const parsed = JSON.parse(payload) as AuthorizationSession
    if (!parsed?.state || !parsed?.codeVerifier || typeof parsed.issuedAt !== 'number') {
      return null
    }

    const age = Date.now() - parsed.issuedAt
    if (age > SESSION_TTL_SECONDS * 1000) {
      return null
    }

    return parsed
  } catch {
    return null
  }
}

function base64UrlEncode(input: ArrayBuffer | string): string {
  let bytes: Uint8Array
  if (typeof input === 'string') {
    bytes = encoder.encode(input)
  } else {
    bytes = new Uint8Array(input)
  }

  let binary = ''
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i])
  }

  const base64 = btoa(binary)
  return base64.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

function base64UrlDecodeToString(input: string): string {
  const padded = padBase64(input.replace(/-/g, '+').replace(/_/g, '/'))
  const binary = atob(padded)
  let result = ''
  for (let i = 0; i < binary.length; i++) {
    result += String.fromCharCode(binary.charCodeAt(i))
  }
  return result
}

function padBase64(input: string): string {
  const remainder = input.length % 4
  if (remainder === 0) return input
  if (remainder === 2) return `${input}==`
  if (remainder === 3) return `${input}=`
  return `${input}===`
}

async function sign(input: string, secret: string): Promise<string> {
  const key = await crypto.subtle.importKey('raw', encoder.encode(secret), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign'])
  const signature = await crypto.subtle.sign('HMAC', key, encoder.encode(input))
  return base64UrlEncode(signature)
}

function constantTimeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false
  let result = 0
  for (let i = 0; i < a.length; i++) {
    result |= a.charCodeAt(i) ^ b.charCodeAt(i)
  }
  return result === 0
}
