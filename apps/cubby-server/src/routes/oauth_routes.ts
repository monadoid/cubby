import { Hono} from 'hono'
import { HTTPException } from 'hono/http-exception'
import { z } from 'zod'
import stytch from 'stytch'
import type { Bindings, Variables } from '../index'
import type {
    IDPOAuthAuthorizeRequest,
    IDPOAuthAuthorizeStartRequest,
    IDPOAuthAuthorizeStartResponse,
} from 'stytch'
import { renderOAuthConsentPage } from '../views/oauth_consent_page'


const app = new Hono<{ Bindings: Bindings; Variables: Variables }>()

export const baseOAuthSchema = z.object({
    client_id: z.string().min(1, 'client_id is required'),
    redirect_uri: z.url('redirect_uri must be a valid URL'),
    response_type: z.literal('code').default('code'),
    scopes: z.preprocess(
        (val) => Array.isArray(val) ? val : val ? [val] : undefined,
        z.array(z.string()).min(1, 'At least one scope is required')
    ),
    state: z.string().optional(),
    nonce: z.string().optional(),
    code_challenge: z.string().optional(),
    code_challenge_method: z.enum(['S256', 'plain']).optional(),
    prompt: z.string().optional(),
})

export type BaseOAuthParams = z.infer<typeof baseOAuthSchema>

const submitSchema = baseOAuthSchema.extend({
    consent_granted: z
        .union([z.literal('true'), z.literal('false')])
        .optional()
        .default('true')
        .transform((val) => val !== 'false'),
})

app.get('/authorize', async (c) => {
    const urlParams = new URL(c.req.url).searchParams
    // Build an object that handles multiple scopes values
    const paramsObj: any = {}
    for (const [key, value] of urlParams.entries()) {
        if (key === 'scopes') {
            if (!paramsObj.scopes) paramsObj.scopes = []
            paramsObj.scopes.push(value)
        } else {
            paramsObj[key] = value
        }
    }
    
    const parsed = baseOAuthSchema.safeParse(paramsObj)
    if (!parsed.success) {
        throw new HTTPException(400, { message: z.prettifyError(parsed.error) })
    }

    const client = new stytch.Client({
        project_id: c.env.STYTCH_PROJECT_ID,
        secret: c.env.STYTCH_PROJECT_SECRET,
    })

    const startReq: IDPOAuthAuthorizeStartRequest = {
        client_id: parsed.data.client_id,
        redirect_uri: parsed.data.redirect_uri,
        response_type: 'code',
        scopes: parsed.data.scopes
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

app.post('/authorize/submit', async (c) => {
    const body = await c.req.parseBody()
    const parsed = submitSchema.safeParse(body)
    if (!parsed.success) {
        throw new HTTPException(400, { message: z.prettifyError(parsed.error) })
    }

    const client = new stytch.Client({
        project_id: c.env.STYTCH_PROJECT_ID,
        secret: c.env.STYTCH_PROJECT_SECRET,
    })

    const authReq: IDPOAuthAuthorizeRequest = {
        ...parsed.data,
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
