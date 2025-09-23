import { Hono } from 'hono'
import * as client from 'openid-client'

type Bindings = {
	// Example binding to KV. Learn more at https://developers.cloudflare.com/workers/runtime-apis/kv/
	// MY_KV_NAMESPACE: KVNamespace;
	//
	// Example binding to Durable Object. Learn more at https://developers.cloudflare.com/workers/runtime-apis/durable-objects/
	// MY_DURABLE_OBJECT: DurableObjectNamespace;
	//
	// Example binding to R2. Learn more at https://developers.cloudflare.com/workers/runtime-apis/r2/
	// MY_BUCKET: R2Bucket;
	//
	// Example binding to a Service. Learn more at https://developers.cloudflare.com/workers/runtime-apis/service-bindings/
	// MY_SERVICE: Fetcher;
	//
	// Example binding to a Queue. Learn more at https://developers.cloudflare.com/workers/runtime-apis/queues/
	// MY_QUEUE: Queue;


	// For production you'd use KV or similar for session storage
}

// In-memory storage for OAuth state (for local dev only)
const oauthStates = new Map<string, { code_verifier: string; state: string }>()
const accessTokens = new Map<string, string>()

// OAuth Configuration - matches Bruno test environment
const OAUTH_CONFIG = {
	base_url: 'http://localhost:5150',
	project_domain: 'surf-cap-4473.customers.stytch.dev',
	client_id: 'connected-app-test-f019736e-3425-4e27-a24f-7c235d9d058b',
	client_secret: 'Lf2x5Ca1DOwfTGCRHNB0x8Q6naamYs7-qhokDvHqEK_ffpY-',
	scope: 'openid',
	redirect_uri: 'http://localhost:8670/oauth/callback'
}

const app = new Hono<{ Bindings: Bindings }>()

// Simple session management using cookies
function getSessionId(c: any): string | null {
	const cookie = c.req.header('Cookie')
	if (!cookie) return null
	const match = cookie.match(/session_id=([^;]+)/)
	return match ? match[1] : null
}

function setSessionId(c: any, sessionId: string) {
	c.header('Set-Cookie', `session_id=${sessionId}; Path=/; HttpOnly; Max-Age=3600`)
}

app.get('/', (c) => {
	return c.html(`
		<!DOCTYPE html>
		<html>
		<head>
			<title>ExampleCo</title>
			<style>
				body { font-family: Arial, sans-serif; max-width: 800px; margin: 40px auto; padding: 20px; }
				button { background: #007bff; color: white; border: none; padding: 12px 24px; border-radius: 4px; cursor: pointer; }
				button:hover { background: #0056b3; }
			</style>
		</head>
		<body>
			<h1>ExampleCo</h1>
			<p>Welcome to ExampleCo! Click below to access your dashboard:</p>
			<a href="/home"><button>Go to Dashboard</button></a>
		</body>
		</html>
	`)
})

app.get('/home', (c) => {
	const sessionId = getSessionId(c)
	const hasAccess = sessionId && accessTokens.has(sessionId)
	
	return c.html(`
		<!DOCTYPE html>
		<html>
		<head>
			<title>Dashboard - ExampleCo</title>
			<style>
				body { font-family: Arial, sans-serif; max-width: 800px; margin: 40px auto; padding: 20px; }
				button { background: #007bff; color: white; border: none; padding: 12px 24px; border-radius: 4px; cursor: pointer; margin: 8px; }
				button:hover { background: #0056b3; }
				.success { background: #28a745; }
				.success:hover { background: #1e7e34; }
				.movies { margin-top: 20px; padding: 20px; border: 1px solid #ddd; border-radius: 4px; }
			</style>
		</head>
		<body>
			<h1>Dashboard</h1>
			${hasAccess ? `
				<p>✅ Connected to Cubby API!</p>
				<button class="success" onclick="fetchMovies()">Fetch Movies from Cubby API</button>
				<div id="movies" class="movies" style="display: none;"></div>
			` : `
				<p>Connect to Cubby API to access your data:</p>
				<a href="/oauth/authorize"><button>Connect to Cubby API</button></a>
			`}
			<script>
				async function fetchMovies() {
					try {
						const response = await fetch('/api/movies')
						const data = await response.json()
						document.getElementById('movies').style.display = 'block'
						document.getElementById('movies').innerHTML = '<h3>Movies from Cubby API:</h3><pre>' + JSON.stringify(data, null, 2) + '</pre>'
					} catch (error) {
						document.getElementById('movies').style.display = 'block'
						document.getElementById('movies').innerHTML = '<h3>Error:</h3><p>' + error.message + '</p>'
					}
				}
			</script>
		</body>
		</html>
	`)
})

app.get('/oauth/authorize', async (c) => {
	try {
		// Generate PKCE parameters
		const code_verifier = client.randomPKCECodeVerifier()
		const code_challenge = await client.calculatePKCECodeChallenge(code_verifier)
		const state = client.randomState()
		
		// Store PKCE parameters for callback
		oauthStates.set(state, { code_verifier, state })
		
		// Build authorization URL
		const authUrl = new URL(`${OAUTH_CONFIG.base_url}/oauth/authorize`)
		authUrl.searchParams.set('client_id', OAUTH_CONFIG.client_id)
		authUrl.searchParams.set('redirect_uri', OAUTH_CONFIG.redirect_uri)
		authUrl.searchParams.set('response_type', 'code')
		authUrl.searchParams.set('scope', OAUTH_CONFIG.scope)
		authUrl.searchParams.set('state', state)
		authUrl.searchParams.set('code_challenge', code_challenge)
		authUrl.searchParams.set('code_challenge_method', 'S256')
		
		return c.redirect(authUrl.toString())
	} catch (error) {
		return c.json({ error: 'Failed to initiate OAuth flow', details: error instanceof Error ? error.message : 'Unknown error' }, 500)
	}
})

app.get('/oauth/callback', async (c) => {
	try {
		const url = new URL(c.req.url)
		const code = url.searchParams.get('code')
		const state = url.searchParams.get('state')
		
		if (!code || !state) {
			return c.json({ error: 'Missing code or state parameter' }, 400)
		}
		
		// Retrieve stored PKCE parameters
		const storedState = oauthStates.get(state)
		if (!storedState) {
			return c.json({ error: 'Invalid or expired state' }, 400)
		}
		
		// Exchange code for token
		const tokenUrl = `https://${OAUTH_CONFIG.project_domain}/v1/oauth2/token`
		const tokenResponse = await fetch(tokenUrl, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/x-www-form-urlencoded',
			},
			body: new URLSearchParams({
				client_id: OAUTH_CONFIG.client_id,
				client_secret: OAUTH_CONFIG.client_secret,
				grant_type: 'authorization_code',
				code: code,
				redirect_uri: OAUTH_CONFIG.redirect_uri,
				code_verifier: storedState.code_verifier,
			}),
		})
		
		if (!tokenResponse.ok) {
			const errorText = await tokenResponse.text()
			return c.json({ error: 'Token exchange failed', details: errorText }, 500)
		}
		
		const tokens = await tokenResponse.json() as { access_token: string }
		
		// Create session and store access token
		const sessionId = crypto.randomUUID()
		accessTokens.set(sessionId, tokens.access_token)
		setSessionId(c, sessionId)
		
		// Clean up state
		oauthStates.delete(state)
		
		return c.redirect('/home')
	} catch (error) {
		return c.json({ error: 'OAuth callback failed', details: error instanceof Error ? error.message : 'Unknown error' }, 500)
	}
})

app.get('/api/movies', async (c) => {
	const sessionId = getSessionId(c)
	if (!sessionId) {
		return c.json({ error: 'No session' }, 401)
	}
	
	const accessToken = accessTokens.get(sessionId)
	if (!accessToken) {
		return c.json({ error: 'No access token' }, 401)
	}
	
	try {
		const response = await fetch(`${OAUTH_CONFIG.base_url}/api/movies/list`, {
			headers: {
				'Authorization': `Bearer ${accessToken}`
			}
		})
		
		if (!response.ok) {
			const errorText = await response.text()
			return c.json({ error: 'Failed to fetch movies', details: errorText }, 500)
		}
		
		const movies = await response.json()
		return c.json(movies)
	} catch (error) {
		return c.json({ error: 'Failed to call Cubby API', details: error instanceof Error ? error.message : 'Unknown error' }, 500)
	}
})

export default app