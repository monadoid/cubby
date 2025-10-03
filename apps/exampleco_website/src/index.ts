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
  <script src="https://unpkg.com/htmx.org@2.0.4"></script>
  <style>
    body { font-family: system-ui, sans-serif; margin: 2rem auto; max-width: 800px; padding: 0 1rem; }
    h1 { font-size: 1.75rem; margin-bottom: 1rem; }
    h2 { font-size: 1.25rem; margin-top: 2rem; margin-bottom: 1rem; }
    button, a.button { display: inline-flex; align-items: center; justify-content: center; gap: 0.5rem; padding: 0.75rem 1.5rem; background: #2563eb; color: #fff; border: none; border-radius: 0.375rem; font-size: 1rem; cursor: pointer; text-decoration: none; }
    button.secondary { background: #4b5563; }
    button:disabled { opacity: 0.65; cursor: not-allowed; }
    pre { background: #0f172a; color: #f8fafc; padding: 1rem; border-radius: 0.375rem; min-height: 7rem; overflow-x: auto; white-space: pre-wrap; word-wrap: break-word; }
    .cta { display: flex; gap: 1rem; flex-wrap: wrap; margin: 1.5rem 0; }
    .form-group { display: flex; flex-direction: column; gap: 0.5rem; margin-bottom: 1rem; }
    label { font-weight: 500; font-size: 0.875rem; }
    input { padding: 0.5rem; border: 1px solid #d1d5db; border-radius: 0.375rem; font-size: 1rem; }
    input:focus { outline: none; border-color: #2563eb; }
    .htmx-request .htmx-indicator { display: inline; }
    .htmx-indicator { display: none; margin-left: 0.5rem; }
    .section { margin-bottom: 2rem; padding: 1rem; background: #f9fafb; border-radius: 0.5rem; }
  </style>
</head>
<body>
  <h1>Connected App OAuth + PKCE Demo</h1>
  <p>This page demonstrates ExampleCo acting as an OAuth client against Stytch to obtain a Cubby access token.</p>
  
  <div class="cta">
    <a class="button" href="/connect">Connect Cubby</a>
    <button type="button" class="secondary" id="call-cubby">Call Cubby (/whoami)</button>
  </div>
  
  <div class="section">
    <h2>Test Device Search</h2>
    <p>Search your Screenpipe device using the proxied API endpoint.</p>
    <div id="device-status" style="margin-bottom: 1rem; padding: 0.5rem; background: #fef3c7; border-radius: 0.375rem;">
      Loading devices...
    </div>
    <form hx-post="/api/search" hx-target="#search-result" hx-indicator="#search-indicator">
      <div class="form-group">
        <label for="device-id">Select Device</label>
        <select id="device-id" name="deviceId" required style="padding: 0.5rem; border: 1px solid #d1d5db; border-radius: 0.375rem; font-size: 1rem; width: 100%;">
          <option value="">-- Select a device --</option>
        </select>
      </div>
      <div class="form-group">
        <label for="search-query">Search Query</label>
        <input type="text" id="search-query" name="q" placeholder="Search your device..." required />
      </div>
      <div class="form-group">
        <label for="limit">Limit (optional)</label>
        <input type="number" id="limit" name="limit" placeholder="10" min="1" max="100" />
      </div>
      <button type="submit">
        Search Device
        <span id="search-indicator" class="htmx-indicator">⏳</span>
      </button>
    </form>
    <pre id="search-result">Search results will appear here...</pre>
  </div>

  <div class="section">
    <h2>Whoami Result</h2>
    <pre id="result">Click "Call Cubby" to fetch the protected endpoint.</pre>
  </div>

  <script type="module">
    const cubbyApiUrl = ${JSON.stringify(c.env.CUBBY_API_URL)};
    const result = document.getElementById('result');
    const callButton = document.getElementById('call-cubby');
    const whoamiUrl = new URL('/whoami', cubbyApiUrl).toString();
    const deviceSelect = document.getElementById('device-id');
    const deviceStatus = document.getElementById('device-status');

    // Fetch devices on page load
    async function loadDevices() {
      const token = sessionStorage.getItem('cubby_access_token');
      if (!token) {
        deviceStatus.textContent = '⚠️ No access token found. Connect Cubby first.';
        deviceStatus.style.background = '#fee2e2';
        return;
      }

      try {
        const devicesUrl = new URL('/devices', cubbyApiUrl).toString();
        const response = await fetch(devicesUrl, {
          headers: { Authorization: 'Bearer ' + token },
        });

        if (!response.ok) {
          const error = await response.text();
          deviceStatus.textContent = \`⚠️ Failed to load devices: \${error}\`;
          deviceStatus.style.background = '#fee2e2';
          return;
        }

        const data = await response.json();
        
        if (data.devices && data.devices.length > 0) {
          // Populate dropdown
          data.devices.forEach(device => {
            const option = document.createElement('option');
            option.value = device.id;
            option.textContent = \`\${device.id} (created: \${new Date(device.createdAt).toLocaleDateString()})\`;
            deviceSelect.appendChild(option);
          });
          
          deviceStatus.textContent = \`✅ Found \${data.devices.length} device(s)\`;
          deviceStatus.style.background = '#d1fae5';
        } else {
          deviceStatus.textContent = '⚠️ No devices found. Please enroll a device first.';
          deviceStatus.style.background = '#fef3c7';
        }
      } catch (error) {
        console.error('Error loading devices:', error);
        deviceStatus.textContent = \`❌ Error loading devices: \${error.message}\`;
        deviceStatus.style.background = '#fee2e2';
      }
    }

    // Load devices on page load
    loadDevices();

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

    // Intercept htmx requests to add auth token
    document.body.addEventListener('htmx:configRequest', (event) => {
      const token = sessionStorage.getItem('cubby_access_token');
      if (!token) {
        event.preventDefault();
        document.getElementById('search-result').textContent = '⚠️ No access token found. Connect Cubby first.';
        return;
      }
      event.detail.headers['Authorization'] = 'Bearer ' + token;
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

app.post('/api/search', async (c) => {
  const authHeader = c.req.header('Authorization')
  if (!authHeader) {
    return c.text('⚠️ Missing Authorization header', 401)
  }

  const formData = await c.req.formData()
  const deviceId = formData.get('deviceId')?.toString()
  const q = formData.get('q')?.toString()
  const limit = formData.get('limit')?.toString()

  if (!deviceId || !q) {
    return c.text('⚠️ Missing required fields: deviceId and q', 400)
  }

  try {
    // Build search URL with query parameters
    const searchUrl = new URL(`/devices/${deviceId}/search`, c.env.CUBBY_API_URL)
    searchUrl.searchParams.set('q', q)
    if (limit) {
      searchUrl.searchParams.set('limit', limit)
    }

    console.log(`Proxying search request to: ${searchUrl.toString()}`)

    const response = await fetch(searchUrl.toString(), {
      method: 'GET',
      headers: {
        'Authorization': authHeader,
      },
    })

    const body = await response.text()
    
    if (!response.ok) {
      return c.text(`❌ Error (${response.status}): ${body}`)
    }

    // Try to pretty-print JSON
    try {
      const json = JSON.parse(body)
      return c.text(JSON.stringify(json, null, 2))
    } catch {
      return c.text(body)
    }
  } catch (error) {
    console.error('Search proxy error:', error)
    return c.text(`❌ Failed to search: ${getErrorMessage(error)}`, 502)
  }
})

function getOAuthConfig(env: Env): {
  authorizationEndpoint: string
  tokenEndpoint: string
  clientId: string
  clientSecret: string
  redirectUri: string
  scopes: string[]
  issuer: string
} {
  return {
    authorizationEndpoint: env.STYTCH_AUTH_URL,
    tokenEndpoint: env.STYTCH_TOKEN_URL,
    clientId: env.STYTCH_CLIENT_ID,
    clientSecret: env.STYTCH_CLIENT_SECRET,
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
