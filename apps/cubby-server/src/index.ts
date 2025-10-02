import {Hono} from 'hono'
import {describeRoute, resolver, validator, openAPIRouteHandler} from 'hono-openapi'
import {z} from 'zod'
import {buildCnameForTunnel, buildIngressForHost, CloudflareClient} from './clients/cloudflare'
import {fetchDeviceHealth} from './clients/tunnel'

type Env = {
    STYTCH_PROJECT_ID: string
    STYTCH_SECRET: string
    CF_API_TOKEN: string
    CF_ACCOUNT_ID: string
    CF_ZONE_ID: string
    ACCESS_CLIENT_ID: string
    ACCESS_CLIENT_SECRET: string
    TUNNEL_DOMAIN: string
}

const signUpRequestSchema = z.object({
    email: z.string(),
    password: z.string()
})

const signUpResponseSchema = z.object({
    user_id: z.string(),
    session_token: z.string(),
    session_jwt: z.string()
})

const stytchSuccessResponseSchema = z.object({
    user_id: z.string(),
    session_token: z.string().optional(),
    session_jwt: z.string().optional(),
})

const stytchErrorResponseSchema = z.object({
    error_message: z.string(),
    error_type: z.string(),
})

// Device enrollment schemas
const deviceEnrollRequestSchema = z.object({
    device_id: z.string().min(1),
})

const deviceEnrollResponseSchema = z.object({
    device_id: z.string(),
    hostname: z.string(),
    tunnel_token: z.string(),
    tunnel_url: z.string()
})

const app = new Hono<{ Bindings: Env }>()

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
    validator('json', signUpRequestSchema),
    async (c) => {
        const {email, password} = c.req.valid('json')

        try {
            const stytchResponse = await fetch('https://test.stytch.com/v1/passwords', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'Authorization': `Basic ${btoa(`${c.env.STYTCH_PROJECT_ID}:${c.env.STYTCH_SECRET}`)}`
                },
                body: JSON.stringify({email, password, session_duration_minutes: 60})
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

            const stytchData = successParseResult.data

            return c.json({
                user_id: stytchData.user_id,
                session_token: stytchData.session_token || '',
                session_jwt: stytchData.session_jwt || ''
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
    validator('json', deviceEnrollRequestSchema),
    async (c) => {
        const {device_id} = c.req.valid('json')

        try {
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

            return c.json({
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

app.get('/devices/:deviceId/health', async (c) => {
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

export default app
