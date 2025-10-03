import {Hono} from 'hono'
import { getCookie } from 'hono/cookie'
import {HTTPException} from 'hono/http-exception'
import {describeRoute, resolver, openAPIRouteHandler} from 'hono-openapi'
import {zValidator} from '@hono/zod-validator'
import {z} from 'zod'
import type { Context } from "hono";
import type { ContentfulStatusCode } from "hono/utils/http-status";
import type { ZodSchema } from "zod";
import {buildCnameForTunnel, buildIngressForHost, CloudflareClient} from './clients/cloudflare'
import {fetchDeviceHealth} from './clients/tunnel'
import {createDbClient} from './db/client'
import {createDevice} from './db/devices_repo'
import {createUser, createUserSchema} from './db/users_repo'
import {jwksAuth, type AuthUser} from "./jwks_auth";

type Bindings = CloudflareBindings

type Variables = {
    auth: AuthUser
    userId: string
}

export type { Bindings, Variables }

export function strictJSONResponse<
    C extends Context,
    S extends ZodSchema,
    D extends Parameters<Context["json"]>[0] & z.infer<S>,
    U extends ContentfulStatusCode
>(c: C, schema: S, data: D, statusCode?: U) {
    const validatedResponse = schema.safeParse(data);

    if (!validatedResponse.success) {
        return c.json(
            {
                message: "Strict response validation failed",
            },
            500
        );
    }

    return c.json(validatedResponse.data, statusCode);
}

const signUpRequestSchema = createUserSchema.pick({ email: true }).extend({
    password: z.string()
})

const signUpResponseSchema = z.object({
    user_id: z.string(),
    session_token: z.string(),
    session_jwt: z.string()
})

const stytchSuccessResponseSchema = z.object({
    user_id: z.string(),
    session_token: z.string(),
    session_jwt: z.string(),
}).transform((data) => ({
    auth_id: data.user_id,
    ...data
}));

const stytchErrorResponseSchema = z.object({
    error_message: z.string(),
    error_type: z.string(),
})

// Device enrollment schemas
const deviceEnrollRequestSchema = z.object({})

const deviceEnrollResponseSchema = z.object({
    device_id: z.string(),
    hostname: z.string(),
    tunnel_token: z.string(),
})

const whoamiResponseSchema = z.object({
    ok: z.boolean(),
    sub: z.string(),
    iss: z.string(),
    aud: z.array(z.string()),
    scopes: z.array(z.string()),
    claims: z.any(),
})

const authorizeQuerySchema = z.object({
    client_id: z.string().min(1),
    redirect_uri: z.string().url(),
    response_type: z.string().optional(),
    scope: z.string().optional(),
    state: z.string().optional(),
    code_challenge: z.string().min(1),
    code_challenge_method: z.string().optional(),
})

const authorizeSubmitSchema = authorizeQuerySchema.extend({
    consent_granted: z.string().optional(),
})

const app = new Hono<{ Bindings: Bindings, Variables: Variables }>()

// Global error handler
app.onError((err, c) => {
    if (err instanceof HTTPException) {
        console.error('HTTP Exception:', err.status, err.message)
        return err.getResponse()
    }
    console.error('Unhandled error:', err)
    return c.text('Internal Server Error', 500)
})

app.post(
    '/sign-up',
    describeRoute({
        description: 'Create a new user account',
        responses: {
            201: {
                description: 'User created successfully',
                content: {
                    'application/json': {schema: resolver(signUpResponseSchema)},
                },
            },
            400: {
                description: 'Bad request - invalid email or duplicate user',
            },
            500: {
                description: 'Internal server error',
            },
        },
        tags: ['Authentication'],
    }),
    zValidator('json', signUpRequestSchema),
    async (c) => {
        const {email, password} = c.req.valid('json')

        const newUserId = crypto.randomUUID();
        try {
            const stytchResponse = await fetch(`${c.env.STYTCH_BASE_URL}/v1/passwords`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'Authorization': `Basic ${btoa(`${c.env.STYTCH_PROJECT_ID}:${c.env.STYTCH_SECRET}`)}`
                },
                body: JSON.stringify({
                    email,
                    password,
                    session_duration_minutes: 60,
                    trusted_metadata: { user_id: newUserId },
                    session_custom_claims: { user_id: newUserId },
                })
            })

            const rawData = await stytchResponse.json()

            if (!stytchResponse.ok) {
                const errorParseResult = stytchErrorResponseSchema.safeParse(rawData)
                if (errorParseResult.success) {
                    console.error('Stytch API error:', errorParseResult.data)
                    return c.json({error: errorParseResult.data.error_message}, 400)
                }
                console.error('Failed to parse Stytch error response:', rawData)
                return c.json({error: 'Authentication service error'}, 400)
            }

            console.log(rawData)
            const successParseResult = stytchSuccessResponseSchema.safeParse(rawData)
            if (!successParseResult.success) {
                console.error('Failed to parse Stytch success response:', successParseResult.error, 'Raw data:', rawData)
                throw new Error('Invalid response from authentication service')
            }

            const {auth_id, session_token, session_jwt} = successParseResult.data

            const db = createDbClient(c.env.DATABASE_URL)
            await createUser(db, {
                id: newUserId,
                authId: auth_id,
                email,
            })

            return strictJSONResponse(c, signUpResponseSchema, {
                user_id: newUserId,
                session_token,
                session_jwt
            }, 201)
        } catch (error) {
            console.error('Sign-up error:', error)
            return c.json({error: 'Internal server error'}, 500)
        }
    }
)

app.post(
    '/devices/enroll',
    describeRoute({
        description: 'Enroll a new device and create Cloudflare tunnel',
        responses: {
            200: {
                description: 'Device enrolled successfully',
                content: {
                    'application/json': {schema: resolver(deviceEnrollResponseSchema)},
                },
            },
            400: {
                description: 'Bad request - invalid device information',
            },
            500: {
                description: 'Internal server error',
            },
        },
        tags: ['Devices'],
    }),
    jwksAuth({}),
    zValidator('json', deviceEnrollRequestSchema),
    async (c) => {
        const _ = c.req.valid('json')

        try {
            const userId = c.get('userId')
            console.log('The "userId" from the auth gaurd is:', userId)

            const db = createDbClient(c.env.DATABASE_URL)

            const device = await createDevice(db, {
                userId
            })

            const device_id = device.id

            // Initialize Cloudflare client
            const cf = new CloudflareClient({
                apiToken: c.env.CF_API_TOKEN,
                accountId: c.env.CF_ACCOUNT_ID,
                zoneId: c.env.CF_ZONE_ID,
            })

            const name = `cubby-${device_id}`
            const hostname = `${device_id}.cubby.sh`

            // 1) Create or reuse tunnel (idempotent)
            const createdOrExisting = await cf.createOrGetTunnel(name)
            const tunnel_id = createdOrExisting.id

            // 2) Ensure config is correct (PUT is idempotent)
            await cf.putTunnelConfig(tunnel_id, buildIngressForHost(hostname, 'http://localhost:3030'))

            // 3) Ensure DNS points to the tunnel (idempotent)
            await cf.upsertCnameRecord(buildCnameForTunnel(hostname, tunnel_id))

            // 4) Ensure we have a token (create may not return it)
            const tunnel_token = createdOrExisting.token ?? (await cf.getTunnelToken(tunnel_id))

            return strictJSONResponse(c, deviceEnrollResponseSchema, {
                device_id,
                hostname,
                tunnel_token
            }, 200)
        } catch (error) {
            console.error('Device enrollment error:', error)
            return c.json({error: 'Failed to enroll device'}, 500)
        }
    }
)

app.get('/oauth/authorize', async (c) => {
    const queryParams = Object.fromEntries(new URL(c.req.url).searchParams.entries())
    const parsed = authorizeQuerySchema.safeParse(queryParams)
    if (!parsed.success) {
        return c.text('Invalid authorization request', 400)
    }

    const sessionJwt = getCookie(c, 'stytch_session_jwt')
    if (!sessionJwt) {
        return c.text('User session required. Please sign in first.', 401)
    }

    const { client_id, redirect_uri, response_type, scope, state, code_challenge, code_challenge_method } = parsed.data
    const resolvedResponseType = response_type ?? 'code'
    const resolvedCodeChallengeMethod = code_challenge_method ?? 'S256'
    const scopes = parseScopes(scope)

    const baseParams = {
        client_id,
        redirect_uri,
        response_type: resolvedResponseType,
        scopes,
        state,
        code_challenge,
        code_challenge_method: resolvedCodeChallengeMethod,
        session_jwt: sessionJwt,
    }

    const startResponse = await stytchPost(c.env, '/v1/public/oauth/authorize/start', baseParams)
    const consentRequired = Boolean((startResponse as { consent_required?: boolean }).consent_required)

    if (!consentRequired) {
        const authorizeResponse = await finalizeAuthorization(c.env, baseParams, true)
        return c.redirect(authorizeResponse.redirect_uri, 307)
    }

    const requestedScopes = inferScopes((startResponse as { scopes?: unknown }).scopes, scopes)
    return c.html(renderConsentPage(requestedScopes, {
        client_id,
        redirect_uri,
        response_type: resolvedResponseType,
        scope: scopes.join(' '),
        state: state ?? '',
        code_challenge,
        code_challenge_method: resolvedCodeChallengeMethod,
    }))
})

app.post('/oauth/authorize/submit', async (c) => {
    const body = await c.req.parseBody()
    const normalized = Object.fromEntries(
        Object.entries(body).map(([key, value]) => [key, typeof value === 'string' ? value : ''])
    )
    const parsed = authorizeSubmitSchema.safeParse(normalized)
    if (!parsed.success) {
        return c.text('Invalid authorization submission', 400)
    }

    const sessionJwt = getCookie(c, 'stytch_session_jwt')
    if (!sessionJwt) {
        return c.text('User session required. Please sign in first.', 401)
    }

    const { client_id, redirect_uri, response_type, scope, state, code_challenge, code_challenge_method } = parsed.data
    const scopes = parseScopes(scope)
    const consentGranted = (parsed.data.consent_granted ?? 'true').toLowerCase() !== 'false'
    const resolvedResponseType = response_type ?? 'code'
    const resolvedCodeChallengeMethod = code_challenge_method ?? 'S256'

    const baseParams = {
        client_id,
        redirect_uri,
        response_type: resolvedResponseType,
        scopes,
        state,
        code_challenge,
        code_challenge_method: resolvedCodeChallengeMethod,
        session_jwt: sessionJwt,
    }

    const authorizeResponse = await finalizeAuthorization(c.env, baseParams, consentGranted)
    return c.redirect(authorizeResponse.redirect_uri, 307)
})

app.get('/devices/:deviceId/health',
    jwksAuth({
        requiredScopes: ['read:user']
    }),
    async (c) => {
    const {deviceId} = c.req.param()

    try {
        return await fetchDeviceHealth(deviceId, {
            ACCESS_CLIENT_ID: c.env.ACCESS_CLIENT_ID,
            ACCESS_CLIENT_SECRET: c.env.ACCESS_CLIENT_SECRET,
            TUNNEL_DOMAIN: c.env.TUNNEL_DOMAIN,
        })
    } catch (error) {
        console.error('Device health proxy error:', error)
        return c.json({error: 'Failed to fetch device health'}, 502)
    }
})

app.get(
    '/whoami',
    jwksAuth({
        // requiredScopes: undefined  // add later in Step 4
    }),
    (c) => {
        const auth = c.get('auth')
        return strictJSONResponse(c, whoamiResponseSchema, {
            ok: true,
            sub: auth.userId,
            iss: auth.issuer,
            aud: auth.audiences,
            scopes: auth.scopes,
            // for debugging only; remove later:
            claims: auth.claims,
        })
    }
)


app.get(
    '/openapi',
    openAPIRouteHandler(app, {
        documentation: {
            openapi: '3.0.0',
            info: {
                title: 'Cubby API',
                version: '1.0.0',
                description: 'Authentication API for Cubby',
            },
            servers: [
                {url: 'http://localhost:8787', description: 'Local Development Server'},
            ],
        },
    })
)

app.get('/', (c) => {
    return c.text('Cubby API - Visit /openapi for OpenAPI documentation')
})

function parseScopes(scope: string | undefined): string[] {
    if (!scope) {
        return []
    }
    return scope
        .split(' ')
        .map((s) => s.trim())
        .filter((s) => s.length > 0)
}

async function stytchPost(env: Bindings, path: string, payload: Record<string, unknown>): Promise<Record<string, unknown>> {
    const response = await fetch(`${env.STYTCH_BASE_URL}${path}`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'Authorization': `Basic ${btoa(`${env.STYTCH_PROJECT_ID}:${env.STYTCH_SECRET}`)}`,
        },
        body: JSON.stringify(payload),
    })

    const json = await response.json().catch(() => null)

    if (!response.ok) {
        console.error('Stytch API error', {
            path,
            status: response.status,
            body: json,
        })
        throw new HTTPException(502, { message: 'Authorization service error' })
    }

    return (json ?? {}) as Record<string, unknown>
}

async function finalizeAuthorization(
    env: Bindings,
    params: Record<string, unknown>,
    consentGranted: boolean,
) {
    const payload = {
        ...params,
        consent_granted: consentGranted,
    }
    const response = await stytchPost(env, '/v1/public/oauth/authorize', payload)
    const redirectUri = typeof response.redirect_uri === 'string' ? (response.redirect_uri as string) : null
    if (!redirectUri) {
        console.error('Missing redirect_uri from Stytch authorize response', response)
        throw new HTTPException(502, { message: 'Authorization service error' })
    }
    return { redirect_uri: redirectUri }
}

function inferScopes(scopesFromStart: unknown, fallback: string[]): string[] {
    if (Array.isArray(scopesFromStart)) {
        const values = scopesFromStart
            .map((scope) => {
                if (typeof scope === 'string') {
                    return scope
                }
                if (
                    scope &&
                    typeof scope === 'object' &&
                    'scope' in scope &&
                    typeof (scope as Record<string, unknown>).scope === 'string'
                ) {
                    return (scope as Record<string, string>).scope
                }
                return null
            })
            .filter((scope): scope is string => typeof scope === 'string' && scope.length > 0)
        if (values.length > 0) {
            return values
        }
    }
    return fallback
}

function renderConsentPage(scopes: string[], hidden: Record<string, string>): string {
    const hiddenInputs = Object.entries(hidden)
        .map(([name, value]) => `<input type="hidden" name="${escapeHtml(name)}" value="${escapeHtml(value)}" />`)
        .join('\n')

    const scopesList = scopes.length
        ? scopes.map((scope) => `<li>${escapeHtml(scope)}</li>`).join('\n')
        : '<li>Default access</li>'

    return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Authorize Connected App</title>
  <style>
    body { font-family: system-ui, sans-serif; margin: 3rem auto; max-width: 480px; padding: 0 1.5rem; }
    h1 { font-size: 1.5rem; margin-bottom: 1rem; }
    form { margin-top: 2rem; display: flex; flex-direction: column; gap: 0.75rem; }
    button { padding: 0.75rem 1.25rem; border: none; border-radius: 0.5rem; font-size: 1rem; cursor: pointer; }
    button[type="submit"] { background: #2563eb; color: #fff; }
    button.secondary { background: #e5e7eb; color: #111827; }
  </style>
</head>
<body>
  <h1>Authorize Connected App</h1>
  <p>This application is requesting access to:</p>
  <ul>
    ${scopesList}
  </ul>
  <form method="post" action="/oauth/authorize/submit">
    ${hiddenInputs}
    <div>
      <button type="submit">Allow</button>
    </div>
    <div>
      <button type="submit" name="consent_granted" value="false" class="secondary">Cancel</button>
    </div>
  </form>
</body>
</html>`
}

function escapeHtml(value: string): string {
    return value
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;')
}

export default app
