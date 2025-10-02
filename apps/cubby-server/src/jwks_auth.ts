import type { MiddlewareHandler } from 'hono'
import { createMiddleware } from 'hono/factory'
import { createRemoteJWKSet, jwtVerify, type JWTPayload } from 'jose'

export type JWKSAuthOptions = {
    audience?: string | string[]
    requiredScopes?: string[]
}
const cloudflareCacheTtl = 3600
const jwksURL = 'https://your-stytch-domain/.well-known/jwks.json';
const issuer = 'https://your-stytch-domain/';

export const jwksAuth = (opts: JWKSAuthOptions): MiddlewareHandler => {
    const JWKS = createRemoteJWKSet(new URL(jwksURL))

    return createMiddleware(async (c, next) => {
        const token = c.req.header('Authorization')?.replace(/^Bearer\s+/i, '')
        if (!token) return c.text('Missing bearer token', 401, { 'WWW-Authenticate': `Bearer error="invalid_token", error_description="missing"` })

        //  prime cloudflare edge cache for the JWKS URL
        try { await fetch(jwksURL, { cf: { cacheEverything: true, cacheTtl: cloudflareCacheTtl } } as any) } catch {}

        try {
            const { payload } = await jwtVerify(token, JWKS, {
                issuer: issuer,
                audience: opts.audience,
            })

            const scopes = extractScopes(payload)
            if (opts.requiredScopes?.length && !opts.requiredScopes.every(s => scopes.includes(s))) {
                return c.text('Insufficient scope', 403, {
                    'WWW-Authenticate': `Bearer error="insufficient_scope", scope="${opts.requiredScopes.join(' ')}"`,
                })
            }

            c.set('auth', {
                userId: payload.sub,
                issuer: payload.iss,
                audiences: toArray(payload.aud),
                scopes,
                claims: payload,
            })
            c.set('userId', payload.sub)
            await next()
        } catch {
            return c.text('Invalid token', 401, { 'WWW-Authenticate': 'Bearer error="invalid_token"' })
        }
    })
}

const extractScopes = (p: JWTPayload): string[] => {
    const str = (p as any).scope as string | undefined
    const arr = (p as any).scp as string[] | undefined
    return str ? str.split(' ').filter(Boolean) : Array.isArray(arr) ? arr.filter(s => typeof s === 'string') : []
}
const toArray = (aud?: unknown): string[] => Array.isArray(aud) ? aud.map(String) : aud ? [String(aud)] : []

