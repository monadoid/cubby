import type { MiddlewareHandler } from 'hono'
import { createMiddleware } from 'hono/factory'
import { createRemoteJWKSet, jwtVerify, type JWTPayload } from 'jose'
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

        const env = c.env
        const audience = env.STYTCH_PROJECT_ID;
        const jwksURL = `${env.STYTCH_PROJECT_DOMAIN}/.well-known/jwks.json`
        // OAuth IDP tokens use the custom domain as issuer, not stytch.com/project-id
        const issuer = env.STYTCH_PROJECT_DOMAIN
        console.log(`jwksURL: ${jwksURL}`)
        const JWKS = createRemoteJWKSet(new URL(jwksURL))
        
        const token = c.req.header('Authorization')?.replace(/^Bearer\s+/i, '')
        if (!token) throw errors.auth.MISSING_TOKEN()

        //  prime cloudflare edge cache for the JWKS URL
        try { 
            await fetch(jwksURL, { 
                cf: { cacheEverything: true, cacheTtl: cloudflareCacheTtl },
            })
        } catch (error){
            console.error(`Failed to prime cloudflare edge cache for ${jwksURL}: ${error}`)
        }

        let payload;
        try {
            const result = await jwtVerify(token, JWKS, {
                issuer: issuer,
                audience: audience,
            })
            payload = result.payload;
            console.log('JWT payload:', JSON.stringify(payload, null, 2))
        } catch (error) {
            console.error('JWT verification failed:', error)
            throw errors.auth.INVALID_TOKEN()
        }

        const scopes = extractScopes(payload)
        if (opts.requiredScopes?.length && !opts.requiredScopes.every(s => scopes.includes(s))) {
            throw errors.auth.INSUFFICIENT_SCOPE(opts.requiredScopes)
        }

        const getAppUserId = (p: JWTPayload, claimName = 'user_id'): string | undefined => {
            const v = (p as any)[claimName]
            return typeof v === 'string' && v.length > 0 ? v : undefined
        }
        const dbUserId = getAppUserId(payload, 'user_id')
        console.log('Extracted user_id:', dbUserId)
        if (!dbUserId) {
            console.error('No user_id found in token. Payload:', payload)
            throw errors.auth.INVALID_AUTH_DATA()
        }

        const parseResult = AuthUserSchema.safeParse({
            userId: dbUserId,
            issuer: payload.iss,
            audiences: toArray(payload.aud),
            scopes,
            claims: payload,
        })
        if (!parseResult.success) {
            throw errors.auth.INVALID_AUTH_DATA()
        }

        c.set('auth', parseResult.data)
        c.set('userId', parseResult.data.userId)
        await next()
    })
}

const extractScopes = (p: JWTPayload): string[] => {
    const str = (p as any).scope as string | undefined
    const arr = (p as any).scp as string[] | undefined
    return str ? str.split(' ').filter(Boolean) : Array.isArray(arr) ? arr.filter(s => typeof s === 'string') : []
}
const toArray = (aud?: string | string[] | undefined): string[] => Array.isArray(aud) ? aud.map(String) : aud ? [String(aud)] : []

