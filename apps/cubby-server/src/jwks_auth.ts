import type { MiddlewareHandler } from 'hono'
import { createMiddleware } from 'hono/factory'
import { createRemoteJWKSet, jwtVerify } from 'jose'
import { z } from 'zod'
import type { Bindings, Variables } from './index'
import { errors } from './errors'

export type JWKSAuthOptions = {
    requiredScopes?: string[]
}

export const AuthUserSchema = z.object({
    userId: z.uuid(),
    issuer: z.string(),
    audiences: z.array(z.string()),
    scopes: z.array(z.string()),
    claims: z.any()
})

export type AuthUser = z.infer<typeof AuthUserSchema>

const cloudflareCacheTtl = 3600

export const jwksAuth = (opts: JWKSAuthOptions): MiddlewareHandler<{ Bindings: Bindings, Variables: Variables }> => {
    return createMiddleware(async (c, next) => {
        // Extract token
        const token = c.req.header('Authorization')?.replace(/^Bearer\s+/i, '')
        if (!token) throw errors.auth.MISSING_TOKEN()

        // Prime Cloudflare edge cache for the JWKS URL
        const jwksURL = `${c.env.STYTCH_PROJECT_DOMAIN}/.well-known/jwks.json`
        try {
            await fetch(jwksURL, {
                cf: { cacheEverything: true, cacheTtl: cloudflareCacheTtl },
            })
        } catch (error) {
            console.error(`Failed to prime Cloudflare edge cache for ${jwksURL}:`, error)
        }

        // Validate JWT using jose with custom domain issuer
        // IDP OAuth tokens from Connected Apps use the custom domain as issuer
        const JWKS = createRemoteJWKSet(new URL(jwksURL))
        
        let payload
        try {
            const result = await jwtVerify(token, JWKS, {
                issuer: c.env.STYTCH_PROJECT_DOMAIN,
                audience: c.env.STYTCH_PROJECT_ID,
            })
            payload = result.payload
        } catch (error) {
            console.error('JWT verification failed:', error)
            throw errors.auth.INVALID_TOKEN()
        }

        // Extract user_id from custom claims
        const userId = (payload as any).user_id
        if (!userId || typeof userId !== 'string') {
            console.error('No user_id in JWT payload:', payload)
            throw errors.auth.INVALID_AUTH_DATA()
        }

        // Extract and parse scopes from space-separated string
        const scopeString = (payload as any).scope as string | undefined
        const scopes = scopeString ? scopeString.split(' ').filter(Boolean) : []
        
        // Validate required scopes
        if (opts.requiredScopes?.length && !opts.requiredScopes.every(s => scopes.includes(s))) {
            throw errors.auth.INSUFFICIENT_SCOPE(opts.requiredScopes)
        }

        // Normalize audiences to array
        const aud = payload.aud
        const audiences = Array.isArray(aud) ? aud.map(String) : aud ? [String(aud)] : []

        // Map to AuthUser schema
        const parseResult = AuthUserSchema.safeParse({
            userId,
            issuer: payload.iss,
            audiences,
            scopes,
            claims: payload,
        })
        
        if (!parseResult.success) {
            throw errors.auth.INVALID_AUTH_DATA()
        }

        // Set context variables
        c.set('auth', parseResult.data)
        c.set('userId', parseResult.data.userId)
        await next()
    })
}

