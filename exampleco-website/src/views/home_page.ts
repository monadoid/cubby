export function renderHomePage(cubbyApiUrl: string): string {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>ExampleCo</title>
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <script src="/htmx.min.js"></script>
  <style>
    body { font-family: system-ui, sans-serif; margin: 2rem auto; max-width: 800px; padding: 0 1rem; }
    h1 { font-size: 1.75rem; margin-bottom: 1rem; }
    h2 { font-size: 1.25rem; margin-top: 2rem; margin-bottom: 1rem; }
    button, a.button { display: inline-flex; align-items: center; justify-content: center; gap: 0.5rem; padding: 0.75rem 1.5rem; background: #2563eb; color: #fff; border: none; border-radius: 0.375rem; font-size: 1rem; cursor: pointer; text-decoration: none; }
    button.secondary { background: #4b5563; }
    button:disabled { opacity: 0.65; cursor: not-allowed; }
    button.danger { background: #dc2626; }
    pre { background: #0f172a; color: #f8fafc; padding: 1rem; border-radius: 0.375rem; min-height: 7rem; max-height: 500px; overflow: auto; white-space: pre-wrap; word-wrap: break-word; }
    .summary-container { background: white; padding: 1rem; border-radius: 0.5rem; border: 1px solid #e5e7eb; }
    .summary-container h3 { margin: 0 0 0.5rem 0; font-size: 1.125rem; color: #111827; }
    .summary-text { padding: 1rem; background: #f3f4f6; border-radius: 0.375rem; color: #374151; line-height: 1.6; }
    .cta { display: flex; gap: 1rem; flex-wrap: wrap; margin: 1.5rem 0; }
    .form-group { display: flex; flex-direction: column; gap: 0.5rem; margin-bottom: 1rem; }
    label { font-weight: 500; font-size: 0.875rem; }
    input, select { padding: 0.5rem; border: 1px solid #d1d5db; border-radius: 0.375rem; font-size: 1rem; }
    input:focus, select:focus { outline: none; border-color: #2563eb; }
    .htmx-request button { opacity: 0.7; cursor: wait; }
    .htmx-request .htmx-default { display: none; }
    .htmx-indicator { display: none; }
    .htmx-request .htmx-indicator { display: inline; }
    .section { margin-bottom: 2rem; padding: 1rem; background: #f9fafb; border-radius: 0.5rem; }
    .hidden { display: none; }
    .alert { padding: 1rem; border-radius: 0.375rem; margin-bottom: 1rem; }
    .alert-info { background: #dbeafe; color: #1e40af; border: 1px solid #93c5fd; }
    .alert-error { background: #fee2e2; color: #991b1b; border: 1px solid #fecaca; }
    .status-badge { display: inline-block; padding: 0.25rem 0.75rem; border-radius: 9999px; font-size: 0.875rem; font-weight: 500; }
    .status-connected { background: #d1fae5; color: #065f46; }
    .status-disconnected { background: #fee2e2; color: #991b1b; }
  </style>
</head>
<body>
  <h1>ExampleCo</h1>
  <p>This page demonstrates ExampleCo acting as an OAuth client against Stytch to obtain a Cubby access token.</p>
  
  <div id="connection-status" class="cta">
    <span class="status-badge status-disconnected">Not Connected</span>
  </div>
  
  <div class="alert alert-info">
    <strong>Privacy-first:</strong> Your data stays on your device. Cubby.sh is only a secure proxy—your data passes through in-transit and is never stored on our servers. ExampleCo doesn't store any data either.
  </div>
  
  <div id="connect-section" class="cta">
    <a class="button" href="/connect">Connect Cubby</a>
    <a class="button secondary" href="/mcp-demo">Try MCP Tools Demo</a>
  </div>
  
  <div id="search-section" class="section hidden">
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;">
      <h2 style="margin: 0;">Test Device Search</h2>
      <button type="button" class="secondary" onclick="disconnectCubby()">Disconnect</button>
    </div>
    <p>Search your Screenpipe device using the proxied API endpoint.</p>
    
    <form hx-post="/api/search" hx-target="#search-result" hx-swap="innerHTML">
      <div class="form-group">
        <label for="device-id">Select Device</label>
        <select 
          id="device-id" 
          name="deviceId" 
          required
        >
          <option value="">Select a device...</option>
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
        <span class="htmx-indicator">Searching...</span>
        <span class="htmx-default">Search Device</span>
      </button>
    </form>
    <div id="search-result" style="background: #f9fafb; padding: 1rem; border-radius: 0.5rem; min-height: 7rem; margin-top: 1rem;">
      <p style="color: #6b7280;">Search results will appear here...</p>
    </div>
  </div>

  <script type="module">
    // Configure HTMX to add Authorization header
    document.body.addEventListener('htmx:configRequest', (event) => {
      const token = localStorage.getItem('cubby_access_token');
      if (token) {
        event.detail.headers['Authorization'] = 'Bearer ' + token;
      }
    });

    // Handle HTMX errors
    document.body.addEventListener('htmx:responseError', (event) => {
      console.error('[HTMX Error]', event.detail);
      const targetId = event.detail.target?.id;
      
      if (targetId === 'device-id') {
        // Device loading failed
        const select = document.getElementById('device-id');
        if (select) {
          select.innerHTML = '<option value="">❌ Failed to load devices - check console</option>';
        }
      }
    });

    // Function to disconnect
    window.disconnectCubby = function() {
      localStorage.removeItem('cubby_access_token');
      window.location.reload();
    };

    // Function to load devices
    async function loadDevices() {
      const select = document.getElementById('device-id');
      if (!select) return;
      
      select.innerHTML = '<option value="">Loading devices...</option>';
      
      try {
        const token = localStorage.getItem('cubby_access_token');
        const response = await fetch('/api/devices-fragment', {
          headers: {
            'Authorization': 'Bearer ' + token
          }
        });
        
        if (!response.ok) {
          throw new Error(\`HTTP \${response.status}: \${await response.text()}\`);
        }
        
        const html = await response.text();
        select.innerHTML = html;
        
        // Check if we got an error message
        if (html.includes('❌') || html.includes('⚠️')) {
          console.error('[Device Load] Error in response:', html);
        }
      } catch (error) {
        console.error('[Device Load] Failed to load devices:', error);
        select.innerHTML = '<option value="">❌ Failed to load devices</option>';
        
        // Show alert
        const alert = document.createElement('div');
        alert.className = 'alert alert-error';
        alert.textContent = 'Failed to load devices: ' + error.message;
        select.parentElement.insertBefore(alert, select);
      }
    }

    // Check auth status on page load
    function checkAuthStatus() {
      const token = localStorage.getItem('cubby_access_token');
      const connectSection = document.getElementById('connect-section');
      const searchSection = document.getElementById('search-section');
      const statusBadge = document.getElementById('connection-status');
      
      if (token) {
        // Authenticated - show search section, hide connect
        connectSection.classList.add('hidden');
        searchSection.classList.remove('hidden');
        statusBadge.innerHTML = '<span class="status-badge status-connected">Connected</span>';
        
        // Load devices
        loadDevices();
      } else {
        // Not authenticated - show connect, hide search
        connectSection.classList.remove('hidden');
        searchSection.classList.add('hidden');
        statusBadge.innerHTML = '<span class="status-badge status-disconnected">Not Connected</span>';
      }
    }

    // Run on page load
    checkAuthStatus();
  </script>
</body>
</html>`;
}
