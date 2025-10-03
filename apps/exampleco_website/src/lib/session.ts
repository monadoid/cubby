import type { Context } from 'hono'
import { deleteCookie, getSignedCookie, setSignedCookie } from 'hono/cookie'
import { z } from 'zod'
import type { AuthorizationSession } from './oauth'

export const SESSION_COOKIE_NAME = 'cubby_oauth_session'
export const SESSION_TTL_SECONDS = 600

const COOKIE_OPTIONS = {
  httpOnly: true,
  maxAge: SESSION_TTL_SECONDS,
  path: '/',
  sameSite: 'Lax' as const,
}

const sessionSchema = z.object({
  state: z.string().min(1),
  codeVerifier: z.string().min(1),
  issuedAt: z.number(),
})

type SessionPayload = z.infer<typeof sessionSchema>

export async function writeSessionCookie(
  c: Context,
  session: AuthorizationSession,
  secret: string,
  secure: boolean,
): Promise<void> {
  await setSignedCookie(c, SESSION_COOKIE_NAME, JSON.stringify(session), secret, {
    ...COOKIE_OPTIONS,
    secure,
  })
}

export async function readSessionCookie(
  c: Context,
  secret: string,
): Promise<AuthorizationSession | null> {
  const raw = await getSignedCookie(c, secret, SESSION_COOKIE_NAME)
  if (typeof raw !== 'string') {
    return null
  }

  try {
    const candidate = JSON.parse(raw)
    const parsed = sessionSchema.safeParse(candidate)
    if (!parsed.success) {
      return null
    }

    const session = parsed.data
    if (isExpired(session)) {
      return null
    }

    return session
  } catch {
    return null
  }
}

export function clearSessionCookie(c: Context, secure: boolean): void {
  deleteCookie(c, SESSION_COOKIE_NAME, {
    path: '/',
    secure,
  })
}

function isExpired(session: SessionPayload): boolean {
  const ageMs = Date.now() - session.issuedAt
  return ageMs > SESSION_TTL_SECONDS * 1000
}
