import { Hono, type MiddlewareHandler } from 'hono'
import { HTTPException } from 'hono/http-exception'
import { getCookie } from 'hono/cookie'
import { z } from 'zod/v4'
import stytch from 'stytch'
import { Consumer } from '@hono/stytch-auth'
import type { Bindings, Variables } from '../index'
import type {
    IDPOAuthAuthorizeRequest,
    IDPOAuthAuthorizeStartRequest,
    IDPOAuthAuthorizeStartResponse,
} from 'stytch'
import { renderOAuthConsentPage } from '../views/oauth_consent_page'

export const baseOAuthSchema = z.object({
    client_id: z.string().min(1, 'client_id is required'),
    redirect_uri: z.url('redirect_uri must be a valid URL'),
    response_type: z.literal('code').default('code'),
    scopes: z.array(z.string()).min(1, 'At least one scope is required'),
    state: z.string().optional(),
    nonce: z.string().optional(),
    code_challenge: z.string().optional(),
    prompt: z.string().optional(),
})

export type BaseOAuthParams = z.infer<typeof baseOAuthSchema>

const submitSchema = baseOAuthSchema.extend({
    consent_granted: z
        .union([z.literal('true'), z.literal('false')])
        .optional()
        .default('true')
        .transform((val) => val !== 'false'),
}).extend({
    // Make scopes optional for submit - user might deny all scopes
    scopes: z.array(z.string()).default([])
})


const app = new Hono<{ Bindings: Bindings; Variables: Variables }>()

// Middleware to require authentication with redirect to sign-up
function requireAuthWithRedirect(): MiddlewareHandler {
    return async (c, next) => {
        const authMiddleware = Consumer.authenticateSessionLocal()
        
        try {
            await authMiddleware(c, async () => {
                await next()
            })
        } catch (error) {
            // Authentication failed - redirect to login with return URL
            const currentUrl = new URL(c.req.url)
            const redirectTo = `${currentUrl.pathname}${currentUrl.search}`
            const loginUrl = `/login?redirect_to=${encodeURIComponent(redirectTo)}`
            return c.redirect(loginUrl, 302)
        }
    }
}

app.get('/authorize', requireAuthWithRedirect(), async (c) => {
    const scopes = c.req.queries('scopes')
    if (!scopes) {
        throw new HTTPException(400, { message: 'scopes parameter is required' })
    }

    const params = {
        client_id: c.req.query('client_id'),
        redirect_uri: c.req.query('redirect_uri'),
        response_type: c.req.query('response_type'),
        scopes,
        state: c.req.query('state'),
        nonce: c.req.query('nonce'),
        code_challenge: c.req.query('code_challenge'),
        prompt: c.req.query('prompt'),
    }

    const parsed = baseOAuthSchema.safeParse(params)
    if (!parsed.success) {
        throw new HTTPException(400, { message: z.prettifyError(parsed.error) })
    }

    // Get authenticated session JWT from cookie
    const sessionJWT = getCookie(c, 'stytch_session_jwt')
    if (!sessionJWT) {
        throw new HTTPException(401, { message: 'Session JWT not found' })
    }

    const client = new stytch.Client({
        project_id: c.env.STYTCH_PROJECT_ID,
        secret: c.env.STYTCH_PROJECT_SECRET,
    })

    const startReq: IDPOAuthAuthorizeStartRequest = {
        client_id: parsed.data.client_id,
        redirect_uri: parsed.data.redirect_uri,
        response_type: 'code',
        scopes: parsed.data.scopes,
        session_jwt: sessionJWT,
    }
    let startResp: IDPOAuthAuthorizeStartResponse
    try {
        startResp = await client.idp.oauth.authorizeStart(startReq)
    } catch (err: any) {
        console.error('authorizeStart failed', err)
        throw new HTTPException(502, { message: 'Authorization service error (start)' })
    }

    // If no explicit consent is required, finalize immediately
    if (!startResp.consent_required) {
        const authReq: IDPOAuthAuthorizeRequest = {
            ...parsed.data,
            consent_granted: true,
            session_jwt: sessionJWT,
        }

        try {
            const authResp = await client.idp.oauth.authorize(authReq)
            // Stytch returns a redirect_uri with either an authorization_code or error params
            return c.redirect(authResp.redirect_uri, 302)
        } catch (err: any) {
            console.error('authorize failed', err)
            throw new HTTPException(502, { message: 'Authorization service error (authorize)' })
        }
    }

    // Render interactive consent page where user approves/denies requested scopes
    const html = renderOAuthConsentPage(startResp, parsed.data)
    return c.html(html)
})

app.post('/authorize/submit', requireAuthWithRedirect(), async (c) => {
    const body = await c.req.parseBody()
    
    // Normalize scopes to always be an array
    const normalizedBody = {
        ...body,
        scopes: Array.isArray(body.scopes) 
            ? body.scopes 
            : body.scopes 
                ? [body.scopes] 
                : []
    }
    
    const parsed = submitSchema.safeParse(normalizedBody)
    if (!parsed.success) {
        throw new HTTPException(400, { message: z.prettifyError(parsed.error) })
    }

    // Get authenticated session JWT from cookie
    const sessionJWT = getCookie(c, 'stytch_session_jwt')
    if (!sessionJWT) {
        throw new HTTPException(401, { message: 'Session JWT not found' })
    }

    const client = new stytch.Client({
        project_id: c.env.STYTCH_PROJECT_ID,
        secret: c.env.STYTCH_PROJECT_SECRET,
    })

    const authReq: IDPOAuthAuthorizeRequest = {
        ...parsed.data,
        session_jwt: sessionJWT,
    }

    try {
        const authResp = await client.idp.oauth.authorize(authReq)
        return c.redirect(authResp.redirect_uri, 302)
    } catch (err: any) {
        console.error('authorize (submit) failed', err)
        throw new HTTPException(502, { message: 'Authorization service error (submit)' })
    }
})

export default app
