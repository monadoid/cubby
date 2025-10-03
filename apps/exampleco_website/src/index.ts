import { Hono } from 'hono'
import {
  AuthorizationSession,
  buildAuthorizationUrl,
  calculatePKCECodeChallenge,
  createOAuthContext,
  exchangeAuthorizationCode,
  generateRandomCodeVerifier,
  generateRandomState,
  validateCallbackParameters,
} from './lib/oauth'
import { clearSessionCookie, readSessionCookie, writeSessionCookie } from './lib/session'

type Bindings = Env
type Variables = {
  secure: boolean
}

export type { Bindings, Variables }

const app = new Hono<{ Bindings: Bindings; Variables: Variables }>()

app.get('/', (c) => {
  return c.html(`<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>ExampleCo OAuth Demo</title>
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <style>
    body { font-family: system-ui, sans-serif; margin: 2rem auto; max-width: 640px; padding: 0 1rem; }
    h1 { font-size: 1.75rem; margin-bottom: 1rem; }
    button, a.button { display: inline-flex; align-items: center; justify-content: center; gap: 0.5rem; padding: 0.75rem 1.5rem; background: #2563eb; color: #fff; border: none; border-radius: 0.375rem; font-size: 1rem; cursor: pointer; text-decoration: none; }
    button.secondary { background: #4b5563; }
    button:disabled { opacity: 0.65; cursor: not-allowed; }
    pre { background: #0f172a; color: #f8fafc; padding: 1rem; border-radius: 0.375rem; min-height: 7rem; overflow-x: auto; }
    .cta { display: flex; gap: 1rem; flex-wrap: wrap; margin: 1.5rem 0; }
  </style>
</head>
<body>
  <h1>Connected App OAuth + PKCE Demo</h1>
  <p>This page demonstrates ExampleCo acting as an OAuth client against Stytch to obtain a Cubby access token.</p>
  <div class="cta">
    <a class="button" href="/connect">Connect Cubby</a>
    <button type="button" class="secondary" id="call-cubby">Call Cubby (/whoami)</button>
  </div>
  <pre id="result">Click "Call Cubby" to fetch the protected endpoint.</pre>
  <script type="module">
    const cubbyApiUrl = ${JSON.stringify(c.env.CUBBY_API_URL)};
    const result = document.getElementById('result');
    const callButton = document.getElementById('call-cubby');
    const whoamiUrl = new URL('/whoami', cubbyApiUrl).toString();

    callButton?.addEventListener('click', async () => {
      const token = sessionStorage.getItem('cubby_access_token');
      if (!token) {
        result.textContent = '⚠️ No access token found. Connect Cubby first.';
        return;
      }

      try {
        const response = await fetch(whoamiUrl, {
          headers: { Authorization: 'Bearer ' + token },
        });

        const body = await response.json().catch(() => ({ error: 'Failed to parse response body' }));
        if (!response.ok) {
          result.textContent = JSON.stringify({ status: response.status, body }, null, 2);
          return;
        }

        result.textContent = JSON.stringify(body, null, 2);
      } catch (error) {
        console.error('Error calling Cubby', error);
        result.textContent = JSON.stringify({ error: String(error) }, null, 2);
      }
    });
  </script>
</body>
</html>`)
})

app.get('/connect', async (c) => {
  const oauthConfig = getOAuthConfig(c.env)

  const codeVerifier = generateRandomCodeVerifier()
  const codeChallenge = await calculatePKCECodeChallenge(codeVerifier)
  const state = generateRandomState()

  const session: AuthorizationSession = {
    state,
    codeVerifier,
    issuedAt: Date.now(),
  }

  const secureCookies = c.env.SECURE_COOKIES === 'true'
  await writeSessionCookie(c, session, c.env.SESSION_SECRET, secureCookies)

  const authorizationUrl = buildAuthorizationUrl(oauthConfig, state, codeChallenge)
  return c.redirect(authorizationUrl.toString(), 302)
})

app.get('/callback', async (c) => {
  const oauthConfig = getOAuthConfig(c.env)
  const context = createOAuthContext(oauthConfig)
  const secureCookies = c.env.SECURE_COOKIES === 'true'

  const session = await readSessionCookie(c, c.env.SESSION_SECRET)

  if (!session) {
    clearSessionCookie(c, secureCookies)
    return c.text('Invalid or expired OAuth session. Start over from /connect', 400)
  }

  let callbackParameters: URLSearchParams
  try {
    callbackParameters = validateCallbackParameters(context, new URL(c.req.url), session.state)
  } catch (error) {
    console.error('Invalid callback parameters', error)
    clearSessionCookie(c, secureCookies)
    return c.text('Invalid callback parameters', 400)
  }

  try {
    const connection = await exchangeAuthorizationCode(
      context,
      callbackParameters,
      oauthConfig.redirectUri,
      session.codeVerifier,
    )

    clearSessionCookie(c, secureCookies)

    const accessToken = JSON.stringify(connection.accessToken)
    const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Completing OAuth</title>
</head>
<body>
  <script>
    sessionStorage.setItem('cubby_access_token', ${accessToken});
    window.location.href = '/';
  </script>
</body>
</html>`

    return c.html(html)
  } catch (error) {
    console.error('Token exchange failed', error)
    clearSessionCookie(c, secureCookies)
    const message = getErrorMessage(error)
    return c.text(`Token exchange failed: ${message}`, 502)
  }
})

function getOAuthConfig(env: Env): {
  authorizationEndpoint: string
  tokenEndpoint: string
  clientId: string
  redirectUri: string
  scopes: string[]
  issuer: string
} {
  return {
    authorizationEndpoint: env.STYTCH_AUTH_URL,
    tokenEndpoint: env.STYTCH_TOKEN_URL,
    clientId: env.STYTCH_CLIENT_ID,
    redirectUri: env.REDIRECT_URI,
    scopes: env.REQUESTED_SCOPES.split(',').map((s) => s.trim()),
    issuer: env.STYTCH_ISSUER,
  }
}

function getErrorMessage(error: unknown): string {
  if (error instanceof DOMException && error.name === 'AbortError') {
    return 'Token endpoint request timed out'
  }

  if (error instanceof Error) {
    return error.message
  }

  return 'Unknown error'
}

export default app
