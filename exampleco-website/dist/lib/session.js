import { deleteCookie, getSignedCookie, setSignedCookie } from 'hono/cookie';
import { z } from 'zod';
export const SESSION_COOKIE_NAME = 'cubby_oauth_session';
export const SESSION_TTL_SECONDS = 600;
const COOKIE_OPTIONS = {
    httpOnly: true,
    maxAge: SESSION_TTL_SECONDS,
    path: '/',
    sameSite: 'Lax',
};
const sessionSchema = z.object({
    state: z.string().min(1),
    codeVerifier: z.string().min(1),
    issuedAt: z.number(),
});
export async function writeSessionCookie(c, session, secret, secure) {
    await setSignedCookie(c, SESSION_COOKIE_NAME, JSON.stringify(session), secret, {
        ...COOKIE_OPTIONS,
        secure,
    });
}
export async function readSessionCookie(c, secret) {
    const raw = await getSignedCookie(c, secret, SESSION_COOKIE_NAME);
    if (typeof raw !== 'string') {
        return null;
    }
    try {
        const candidate = JSON.parse(raw);
        const parsed = sessionSchema.safeParse(candidate);
        if (!parsed.success) {
            return null;
        }
        const session = parsed.data;
        if (isExpired(session)) {
            return null;
        }
        return session;
    }
    catch {
        return null;
    }
}
export function clearSessionCookie(c, secure) {
    deleteCookie(c, SESSION_COOKIE_NAME, {
        path: '/',
        secure,
    });
}
function isExpired(session) {
    const ageMs = Date.now() - session.issuedAt;
    return ageMs > SESSION_TTL_SECONDS * 1000;
}
//# sourceMappingURL=session.js.map