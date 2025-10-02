import type { MiddlewareHandler } from 'hono'
import { createMiddleware } from 'hono/factory'
import { createRemoteJWKSet, jwtVerify, type JWTPayload } from 'jose'
import type { Bindings, Variables } from './index'

export type JWKSAuthOptions = {
    audience?: string | string[]
    requiredScopes?: string[]
}

export type AuthUser = {
    userId: string
    issuer: string
    audiences: string[]
    scopes: string[]
    claims: JWTPayload
}

const cloudflareCacheTtl = 3600

export const jwksAuth = (opts: JWKSAuthOptions): MiddlewareHandler<{ Bindings: Bindings, Variables: Variables }> => {
    return createMiddleware(async (c, next) => {
        const env = c.env
        const jwksURL = `${env.STYTCH_BASE_URL}/v1/sessions/jwks/${env.STYTCH_PROJECT_ID}`
        const issuer = `${env.STYTCH_BASE_URL}/`
        const JWKS = createRemoteJWKSet(new URL(jwksURL))
        
        const token = c.req.header('Authorization')?.replace(/^Bearer\s+/i, '')
        if (!token) return c.text('Missing bearer token', 401, { 'WWW-Authenticate': `Bearer error="invalid_token", error_description="missing"` })

        //  prime cloudflare edge cache for the JWKS URL
        try { 
            await fetch(jwksURL, { 
                cf: { cacheEverything: true, cacheTtl: cloudflareCacheTtl },
                headers: {
                    'Authorization': `Basic ${btoa(`${env.STYTCH_PROJECT_ID}:${env.STYTCH_SECRET}`)}`
                }
            } as any) 
        } catch {}

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

            const authUser: AuthUser = {
                userId: payload.sub!,
                issuer: payload.iss!,
                audiences: toArray(payload.aud),
                scopes,
                claims: payload,
            }
            c.set('auth', authUser)
            c.set('userId', payload.sub!)
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
const toArray = (aud?: string | string[] | undefined): string[] => Array.isArray(aud) ? aud.map(String) : aud ? [String(aud)] : []

