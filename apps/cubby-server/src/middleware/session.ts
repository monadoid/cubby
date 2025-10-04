import { HTTPException } from 'hono/http-exception'
import { getCookie } from 'hono/cookie'
import stytch from 'stytch'
import type { Context, MiddlewareHandler } from 'hono'
import type { Bindings, Variables } from '../index'

/**
 * Session Authentication Middleware
 * 
 * For first-party clients (Rust CLI, browser sessions). Both token types include user_id claim.
 * 
 * Token Differences vs OAuth:
 * - Issuer: stytch.com/{PROJECT_ID} vs PROJECT_DOMAIN (e.g., https://login.cubby.sh)
 * - Source: Authorization header OR cookie vs header only
 * - No OAuth scopes
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

