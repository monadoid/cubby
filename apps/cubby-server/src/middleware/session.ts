import { HTTPException } from 'hono/http-exception'
import { getCookie } from 'hono/cookie'
import stytch from 'stytch'
import type { Context, MiddlewareHandler } from 'hono'
import type { Bindings, Variables } from '../index'

/**
 * Session Authentication Middleware
 * 
 * Validates Stytch session JWTs from first-party clients (Rust CLI, browser sessions).
 * 
 * Token Details:
 * - Issuer: stytch.com/{PROJECT_ID}
 * - Accepts from: Authorization header (Bearer token) OR cookie (stytch_session_jwt)
 * - Validation: Uses Stytch SDK client.sessions.authenticateJwt()
 * 
 * Sets in context:
 * - c.set('session', sessionData) - Full Stytch session object
 * - c.set('userId', string) - Extracted from session.custom_claims.user_id
 * 
 * @example
 * app.post('/devices/enroll', session(), async (c) => {
 *   const userId = c.get('userId')
 *   // ... use userId
 * })
 */
export function session(): MiddlewareHandler<{ Bindings: Bindings; Variables: Variables }> {
    return async (c: Context, next) => {
        // Try to get JWT from Authorization header first (for API clients like Rust CLI)
        const authHeader = c.req.header('authorization')
        let sessionJwt: string | undefined
        
        if (authHeader?.startsWith('Bearer ')) {
            sessionJwt = authHeader.substring(7)
        } else {
            // Fall back to cookie (for browser clients)
            sessionJwt = getCookie(c, 'stytch_session_jwt')
        }
        
        if (!sessionJwt) {
            throw new HTTPException(401, { message: 'Missing session JWT' })
        }
        
        // Validate the session JWT with Stytch
        const client = new stytch.Client({
            project_id: c.env.STYTCH_PROJECT_ID,
            secret: c.env.STYTCH_PROJECT_SECRET,
        })
        
        try {
            const response = await client.sessions.authenticateJwt({
                session_jwt: sessionJwt,
            })
            
            // Extract user_id from custom claims
            const userId = response.session.custom_claims?.user_id as string | undefined
            
            if (!userId) {
                console.error('No user_id in session custom claims')
                throw new HTTPException(401, { message: 'Invalid session: missing user_id' })
            }
            
            // Store session info in context
            c.set('session', response.session)
            c.set('userId', userId)
            
            await next()
        } catch (error: any) {
            console.error('Session JWT validation failed:', error)
            throw new HTTPException(401, { message: 'Invalid or expired session' })
        }
    }
}

