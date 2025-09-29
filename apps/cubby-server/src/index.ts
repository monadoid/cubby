import { Hono } from 'hono'
import { describeRoute, resolver, validator, openAPIRouteHandler } from 'hono-openapi'
import { z } from 'zod'

type Env = {
  STYTCH_PROJECT_ID: string
  STYTCH_SECRET: string
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

const app = new Hono<{ Bindings: Env }>()

app.post(
  '/sign-up',
  describeRoute({
    description: 'Create a new user account',
    responses: {
      201: {
        description: 'User created successfully',
        content: {
          'application/json': { schema: resolver(signUpResponseSchema) },
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
    const { email, password } = c.req.valid('json')
    
    try {
      const stytchResponse = await fetch('https://test.stytch.com/v1/passwords', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Basic ${btoa(`${c.env.STYTCH_PROJECT_ID}:${c.env.STYTCH_SECRET}`)}`
        },
        body: JSON.stringify({ email, password, session_duration_minutes: 60 })
      })

      const rawData = await stytchResponse.json()

      if (!stytchResponse.ok) {
        const errorParseResult = stytchErrorResponseSchema.safeParse(rawData)
        if (errorParseResult.success) {
          console.error('Stytch API error:', errorParseResult.data)
          return c.json({ error: errorParseResult.data.error_message }, 400)
        }
        console.error('Failed to parse Stytch error response:', rawData)
        return c.json({ error: 'Authentication service error' }, 400)
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
      return c.json({ error: 'Internal server error' }, 500)
    }
  }
)

app.get(
  '/openapi',
  openAPIRouteHandler(app, {
    documentation: {
      info: {
        title: 'Cubby API',
        version: '1.0.0',
        description: 'Authentication API for Cubby',
      },
      servers: [
        { url: 'http://localhost:8787', description: 'Local Development Server' },
      ],
    },
  })
)

app.get('/', (c) => {
  return c.text('Cubby API - Visit /openapi for OpenAPI documentation')
})

export default app