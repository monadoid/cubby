import {Hono} from 'hono'
import {HTTPException} from 'hono/http-exception'
import {describeRoute, resolver, openAPIRouteHandler} from 'hono-openapi'
import {zValidator} from '@hono/zod-validator'
import {z} from 'zod/v4'
import {buildCnameForTunnel, buildIngressForHost, CloudflareClient} from './clients/cloudflare'
import {fetchDeviceHealth} from './clients/tunnel'
import {createDbClient} from './db/client'
import {createDevice} from './db/devices_repo'
import {createUser, createUserSchema} from './db/users_repo'
import {jwksAuth, type AuthUser} from "./jwks_auth";
import oauthRoutes from './routes/oauth_routes'
import {strictJSONResponse} from "./helpers";

type Bindings = CloudflareBindings

type Variables = {
    auth: AuthUser
    userId: string
}

export type { Bindings, Variables }

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

// OAuth routes
app.route('/oauth', oauthRoutes)

app.get('/sign-up', async (c) => {
    const redirectTo = c.req.query('redirect_to')
    
    const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Sign Up - Cubby</title>
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <style>
    body { font-family: system-ui, sans-serif; margin: 2rem auto; max-width: 400px; padding: 0 1rem; }
    h1 { font-size: 1.75rem; margin-bottom: 1rem; }
    form { display: flex; flex-direction: column; gap: 1rem; margin-top: 1.5rem; }
    input { padding: 0.75rem; border: 1px solid #d1d5db; border-radius: 0.375rem; font-size: 1rem; }
    input:focus { outline: none; border-color: #2563eb; ring: 2px solid #3b82f6; }
    button { padding: 0.75rem; background: #2563eb; color: white; border: none; border-radius: 0.375rem; font-size: 1rem; font-weight: 500; cursor: pointer; }
    button:hover { background: #1d4ed8; }
    button:disabled { opacity: 0.5; cursor: not-allowed; }
    .error { color: #dc2626; font-size: 0.875rem; margin-top: -0.5rem; }
  </style>
</head>
<body>
  <h1>Sign Up</h1>
  <p>Create your account to continue.</p>
  <form id="signup-form">
    <input type="email" name="email" placeholder="Email" required autocomplete="email" />
    <input type="password" name="password" placeholder="Password" minlength="8" required autocomplete="new-password" />
    <div id="error-message" class="error" style="display: none;"></div>
    <button type="submit">Create Account</button>
  </form>
  <script>
    const form = document.getElementById('signup-form');
    const errorMessage = document.getElementById('error-message');
    const redirectTo = ${JSON.stringify(redirectTo || null)};
    const submitButton = form.querySelector('button[type="submit"]');
    
    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      errorMessage.style.display = 'none';
      submitButton.disabled = true;
      submitButton.textContent = 'Creating account...';
      
      const formData = new FormData(form);
      
      try {
        const response = await fetch('/sign-up', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            email: formData.get('email'),
            password: formData.get('password'),
          }),
        });
        
        const data = await response.json();
        
        if (!response.ok) {
          errorMessage.textContent = data.error || 'Sign up failed. Please try again.';
          errorMessage.style.display = 'block';
          submitButton.disabled = false;
          submitButton.textContent = 'Create Account';
          return;
        }
        
        // Store session JWT in cookie (Stytch session)
        document.cookie = \`stytch_session_jwt=\${data.session_jwt}; Path=/; SameSite=Lax; Secure\`;
        
        // Redirect to OAuth flow or home
        if (redirectTo) {
          window.location.href = redirectTo;
        } else {
          window.location.href = '/';
        }
      } catch (error) {
        console.error('Sign up error:', error);
        errorMessage.textContent = 'An error occurred during sign up. Please try again.';
        errorMessage.style.display = 'block';
        submitButton.disabled = false;
        submitButton.textContent = 'Create Account';
      }
    });
  </script>
</body>
</html>`
    
    return c.html(html)
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
                    'Authorization': `Basic ${btoa(`${c.env.STYTCH_PROJECT_ID}:${c.env.STYTCH_PROJECT_SECRET}`)}`
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


export default app
