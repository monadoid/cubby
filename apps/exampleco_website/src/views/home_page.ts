export function renderHomePage(cubbyApiUrl: string): string {
  return `<!doctype html>
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
    pre { background: #0f172a; color: #f8fafc; padding: 1rem; border-radius: 0.375rem; min-height: 7rem; max-height: 500px; overflow: auto; white-space: pre-wrap; word-wrap: break-word; }
    .summary-container { background: white; padding: 1rem; border-radius: 0.5rem; border: 1px solid #e5e7eb; }
    .summary-container h3 { margin: 0 0 0.5rem 0; font-size: 1.125rem; color: #111827; }
    .summary-text { padding: 1rem; background: #f3f4f6; border-radius: 0.375rem; color: #374151; line-height: 1.6; }
    .cta { display: flex; gap: 1rem; flex-wrap: wrap; margin: 1.5rem 0; }
    .form-group { display: flex; flex-direction: column; gap: 0.5rem; margin-bottom: 1rem; }
    label { font-weight: 500; font-size: 0.875rem; }
    input, select { padding: 0.5rem; border: 1px solid #d1d5db; border-radius: 0.375rem; font-size: 1rem; }
    input:focus, select:focus { outline: none; border-color: #2563eb; }
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
    <form hx-post="/api/search" hx-target="#search-result" hx-swap="innerHTML" hx-indicator="#search-indicator">
      <div class="form-group">
        <label for="device-id">Select Device</label>
        <select 
          id="device-id" 
          name="deviceId" 
          required 
          hx-get="/api/devices-fragment" 
          hx-trigger="load" 
          hx-target="this" 
          hx-swap="innerHTML"
        >
          <option value="">Loading devices...</option>
        </select>
      </div>
      <div class="form-group">
        <label for="search-query">Search Query (leave empty for recent activity)</label>
        <input type="text" id="search-query" name="q" placeholder="Leave empty to show recent activity..." />
      </div>
      <div class="form-group">
        <label for="limit">Limit</label>
        <input type="number" id="limit" name="limit" value="10" min="1" max="100" />
      </div>
      <button type="submit">
        Search Device
        <span id="search-indicator" class="htmx-indicator">⏳</span>
      </button>
    </form>
    <div id="search-result" style="background: #f9fafb; padding: 1rem; border-radius: 0.5rem; min-height: 7rem;">
      <p style="color: #6b7280;">Search results will appear here...</p>
    </div>
  </div>

  <div class="section">
    <h2>Whoami Result</h2>
    <pre id="result">Click "Call Cubby" to fetch the protected endpoint.</pre>
  </div>

  <script type="module">
    const cubbyApiUrl = ${JSON.stringify(cubbyApiUrl)};
    const result = document.getElementById('result');
    const callButton = document.getElementById('call-cubby');
    const whoamiUrl = new URL('/whoami', cubbyApiUrl).toString();

    // Configure HTMX to add Authorization header
    document.body.addEventListener('htmx:configRequest', (event) => {
      const token = sessionStorage.getItem('cubby_access_token');
      if (token) {
        event.detail.headers['Authorization'] = 'Bearer ' + token;
      }
    });

    // Whoami button handler
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
</html>`
}

