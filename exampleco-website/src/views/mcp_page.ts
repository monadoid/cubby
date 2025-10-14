export function renderMcpPage(cubbyApiUrl: string): string {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>ExampleCo - MCP Demo</title>
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
    .info-box { background: #eff6ff; border-left: 4px solid #2563eb; padding: 1rem; margin-bottom: 1.5rem; border-radius: 0.25rem; }
    .info-box h3 { margin: 0 0 0.5rem 0; font-size: 1rem; color: #1e40af; }
    .info-box p { margin: 0; color: #1e40af; font-size: 0.875rem; line-height: 1.5; }
  </style>
</head>
<body>
  <h1>ExampleCo - MCP Demo</h1>
  
  <div class="info-box">
    <h3>🔧 Model Context Protocol (MCP)</h3>
    <p>
      This page demonstrates using MCP tools through the JSON-RPC 2.0 protocol.
      MCP enables AI assistants and applications to call server-side tools in a standardized way.
      The search is performed using the same OAuth confidential flow, but through the MCP <code>/mcp</code> endpoint
      instead of the direct REST API.
    </p>
  </div>
  
  <div class="cta">
    <a class="button secondary" href="/">← Back to Home</a>
    <a class="button" href="/connect">Connect Cubby</a>
  </div>
  
  <div class="section">
    <h2>Test MCP Search Tool</h2>
    <p>Call the <code>search</code> tool via MCP JSON-RPC 2.0 protocol.</p>
    <form hx-post="/api/mcp-search" hx-target="#mcp-result" hx-swap="innerHTML" hx-indicator="#mcp-indicator">
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
        Call MCP Search Tool
        <span id="mcp-indicator" class="htmx-indicator">⏳</span>
      </button>
    </form>
    <div id="mcp-result" style="background: #f9fafb; padding: 1rem; border-radius: 0.5rem; min-height: 7rem;">
      <p style="color: #6b7280;">MCP tool results will appear here...</p>
    </div>
  </div>

  <div class="info-box">
    <h3>📋 How It Works</h3>
    <p>
      1. Click "Connect Cubby" to obtain an OAuth access token (confidential flow with client_secret)<br/>
      2. Select a device and enter a search query<br/>
      3. The backend makes a JSON-RPC 2.0 request to <code>${cubbyApiUrl}/mcp</code> with:<br/>
      &nbsp;&nbsp;• Method: <code>tools/call</code><br/>
      &nbsp;&nbsp;• Tool: <code>search</code><br/>
      &nbsp;&nbsp;• Args: <code>{ deviceId, q, limit, content_type }</code><br/>
      4. MCP server validates the OAuth token and executes the tool<br/>
      5. Results are returned in MCP format with structured content
    </p>
  </div>

  <script type="module">
    // Configure HTMX to add Authorization header
    document.body.addEventListener('htmx:configRequest', (event) => {
      const token = sessionStorage.getItem('cubby_access_token');
      if (token) {
        event.detail.headers['Authorization'] = 'Bearer ' + token;
      }
    });
  </script>
</body>
</html>`;
}
