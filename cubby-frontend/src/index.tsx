// cubby-frontend - simple hello world htmx frontend
import { Hono } from "hono";
import { Scalar } from "@scalar/hono-api-reference";
import { setCookie, getCookie } from "hono/cookie";
import { Content } from "./components/Content";
import { Fluid } from "./components/Fluid";
import { TopBar } from "./components/TopBar";

type Bindings = {
  API_URL: string;
  STYTCH_PROJECT_DOMAIN: string;
  // Add Cloudflare bindings here as needed
  // MY_KV: KVNamespace;
  // MY_D1: D1Database;
  // MY_R2: R2Bucket;
};

const app = new Hono<{ Bindings: Bindings }>();

// Main route - serve the hello world page with components
app.get("/", (c) => {
  return c.html(
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>cubby</title>
        <link rel="icon" href="/favicon.svg" type="image/svg+xml" />
        <link rel="icon" href="/favicon.png" type="image/png" />
        <link rel="icon" href="/favicon.ico" />
        <script src="htmx.min.js"></script>
        <link rel="stylesheet" href="tailwind.css" />
        <style>
          {`
            body {
              font-family: 'Courier New', monospace;
              background-color: #000;
              color: #fff;
            }
            .pixelated {
              image-rendering: pixelated;
              image-rendering: -moz-crisp-edges;
              image-rendering: crisp-edges;
            }
            /* allow dragging on canvas underneath while keeping ui usable */
            .content { pointer-events: none; }
            .content input, .content button, .content textarea, .content select, .content a { pointer-events: auto; }
          `}
        </style>
      </head>
      <body hx-boost="true" hx-ext="sse,ws">
        <TopBar />
        <Fluid />
        <Content />
      </body>
    </html>
  );
});

// Handle form submission
app.post("/posts", (c) => {
  return c.html(
    <div class="text-center">
      <h2 class="text-2xl font-bold mb-4">post submitted!</h2>
      <p class="text-gray-400 mb-8">thanks for sharing your thoughts</p>
      <a href="/" class="text-white underline hover:text-gray-300">
        ← back to home
      </a>
    </div>
  );
});

// Login page
app.get("/login", (c) => {
  return c.html(
    <html lang="en" data-theme="dark">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>cubby - login</title>
        <link rel="icon" href="/favicon.svg" type="image/svg+xml" />
        <link rel="icon" href="/favicon.png" type="image/png" />
        <link rel="icon" href="/favicon.ico" />
        <script src="htmx.min.js"></script>
        <link rel="stylesheet" href="output.css" />
        <style>
          {`
            body {
              font-family: 'Courier New', monospace;
            }
            .pixelated {
              image-rendering: pixelated;
              image-rendering: -moz-crisp-edges;
              image-rendering: crisp-edges;
            }
            #login-error {
              min-height: 24px;
            }
          `}
        </style>
      </head>
      <body hx-boost="true">
        <TopBar />
        <div style="position: fixed; top: 48px; left: 0; width: 100vw; height: calc(100vh - 48px); display: flex; align-items: center; justify-content: center;">
          <div class="bg-base-300 p-8 max-w-md w-full rounded-box">
            <h1 class="text-3xl font-bold mb-8 pixelated">login</h1>
            <form 
              id="login-form"
              hx-post="/api/login" 
              hx-target="#login-error"
              hx-swap="innerHTML"
              hx-indicator="#login-button"
              class="space-y-4"
            >
              <div>
                <label for="email" class="block text-sm font-medium mb-2">
                  email
                </label>
                <input 
                  type="email" 
                  name="email" 
                  id="email"
                  class="input input-bordered w-full"
                  placeholder="your@email.com"
                />
              </div>
              <div>
                <label for="password" class="block text-sm font-medium mb-2">
                  password
                </label>
                <input 
                  type="password" 
                  name="password" 
                  id="password"
                  class="input input-bordered w-full"
                  placeholder="••••••••"
                />
              </div>
              <div id="login-error" class="text-red-500 text-sm"></div>
              <button 
                id="login-button"
                type="submit"
                class="btn w-full"
              >
                <span class="htmx-indicator:hidden">login</span>
                <span class="hidden htmx-indicator:inline">logging in...</span>
              </button>
            </form>
          </div>
        </div>
      </body>
    </html>
  );
});

// Handle login form submission
app.post("/api/login", async (c) => {
  try {
    const formData = await c.req.parseBody();
    const email = formData.email as string;
    const password = formData.password as string;

    if (!email || !password) {
      return c.html(
        <div class="text-red-500">email and password are required</div>,
        400
      );
    }

    // Forward to the API - send as JSON to get token back
    const apiUrl = c.env.API_URL || "https://api.cubby.sh";
    const response = await fetch(`${apiUrl}/login`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        email,
        password,
      }),
    });

    if (!response.ok) {
      let errorMsg = "login failed";
      
      try {
        const data = await response.json() as { error?: string };
        errorMsg = data.error || errorMsg;
      } catch {
        // If JSON parse fails, use default error
      }
      
      return c.html(<div class="text-red-500">{errorMsg}</div>, response.status as any);
    }

    const data = await response.json() as { session_jwt: string; user_id: string; session_token: string };
    
    // Store session JWT in httpOnly cookie using Hono's cookie helper
    setCookie(c, "stytch_session_jwt", data.session_jwt, {
      path: "/",
      httpOnly: true,
      secure: true,
      sameSite: "Lax",
      maxAge: 3600, // 1 hour
    });
    
    // Trigger HTMX redirect to dashboard
    c.header("HX-Redirect", "/dashboard");
    return c.html("");
  } catch (error) {
    console.error("login error:", error);
    return c.html(
      <div class="text-red-500">an error occurred during login</div>,
      500
    );
  }
});

// Dashboard page
app.get("/dashboard", (c) => {
  const token = getCookie(c, "stytch_session_jwt");

  if (!token) {
    return c.redirect("/login");
  }

  return c.html(
    <html lang="en" data-theme="dark">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>cubby - dashboard</title>
        <link rel="icon" href="/favicon.svg" type="image/svg+xml" />
        <link rel="icon" href="/favicon.png" type="image/png" />
        <link rel="icon" href="/favicon.ico" />
        <script src="htmx.min.js"></script>
        <link rel="stylesheet" href="output.css" />
        <style>
          {`
            body {
              font-family: 'Courier New', monospace;
            }
            .pixelated {
              image-rendering: pixelated;
              image-rendering: -moz-crisp-edges;
              image-rendering: crisp-edges;
            }
            .code-display {
              word-break: break-all;
              font-family: 'Courier New', monospace;
              font-size: 0.75rem;
              line-height: 1.4;
            }
          `}
        </style>
        <script dangerouslySetInnerHTML={{
          __html: `
            function copyToClipboard(text) {
              navigator.clipboard.writeText(text).then(function() {
                alert('copied to clipboard!');
              });
            }
            function closeModal() {
              document.getElementById('credentials-modal').close();
            }
            function logout() {
              document.cookie = 'stytch_session_jwt=; Path=/; Max-Age=0';
              window.location.href = '/login';
            }
          `
        }}></script>
      </head>
      <body hx-boost="true">
        <TopBar />
        <div class="fixed top-12 left-0 w-full h-[calc(100vh-48px)] overflow-y-auto bg-base-100">
          <div class="bg-base-100 p-8 max-w-4xl mx-auto">
            <h1 class="text-3xl font-bold mb-8 pixelated text-base-content">dashboard</h1>
            <div class="space-y-8">
              <div class="card bg-base-200 shadow-xl">
                <div class="card-body">
                  <h2 class="card-title text-xl font-bold text-base-content">api credentials</h2>
                  <p class="text-base-content/70 mb-4 text-sm">
                    generate client credentials to use with cursor, mcp clients, or the rest api
                  </p>
                  <button 
                    hx-post="/api/m2m/create"
                    hx-target="#credentials-result"
                    hx-swap="innerHTML"
                    class="btn btn-primary"
                  >
                    generate new credentials
                  </button>
                  <div id="credentials-result" class="mt-6"></div>
                </div>
              </div>

              <div class="card bg-base-200 shadow-xl">
                <div class="card-body">
                  <p class="text-base-content/70 mb-4">
                    you are logged in!
                  </p>
                  <button 
                    onclick="logout()"
                    class="btn btn-outline"
                  >
                    logout
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </body>
    </html>
  );
});

// Create M2M client and exchange for access token
app.post("/api/m2m/create", async (c) => {
  try {
    const sessionJwt = getCookie(c, "stytch_session_jwt");
    if (!sessionJwt) {
      return c.html(<div class="text-red-500">not authenticated</div>, 401);
    }

    const apiUrl = c.env.API_URL || "https://api.cubby.sh";
    
    // Step 1: Create M2M client
    const createResponse = await fetch(`${apiUrl}/m2m/clients`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "Authorization": `Bearer ${sessionJwt}`,
      },
      body: JSON.stringify({ name: `cubby-client-${Date.now()}` }),
    });

    if (!createResponse.ok) {
      const error = await createResponse.json() as { error?: string };
      return c.html(
        <div class="text-red-500">{error.error || "failed to create credentials"}</div>,
        createResponse.status as any
      );
    }

    const clientData = await createResponse.json() as { 
      client_id: string; 
      client_secret: string; 
      name?: string;
      created_at: string;
    };

    // Step 2: Exchange for access token using Stytch's client_credentials flow
    const stytchDomain = c.env.STYTCH_PROJECT_DOMAIN || "https://login.cubby.sh";
    const tokenResponse = await fetch(`${stytchDomain}/v1/oauth2/token`, {
      method: "POST",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded",
      },
      body: new URLSearchParams({
        grant_type: "client_credentials",
        client_id: clientData.client_id,
        client_secret: clientData.client_secret,
        scope: "read:cubby",
      }).toString(),
    });

    let accessToken = "";
    let expiresIn = 0;
    if (tokenResponse.ok) {
      const tokenData = await tokenResponse.json() as { 
        access_token: string; 
        expires_in: number;
        token_type: string;
      };
      accessToken = tokenData.access_token;
      expiresIn = tokenData.expires_in;
    } else {
      console.error("failed to exchange token:", await tokenResponse.text());
    }

    const cursorConfig = JSON.stringify({
      mcpServers: {
        cubby: {
          url: "https://api.cubby.sh/mcp",
          headers: {
            Authorization: `Bearer ${accessToken}`
          }
        }
      }
    }, null, 2);

    return c.html(
      <div class="card bg-base-300 shadow-xl">
        <div class="card-body">
          <h2 class="card-title text-xl font-bold text-base-content">credentials created!</h2>
          <div class="alert alert-warning">
            <span>⚠️ save these now! the client secret and access token will not be shown again.</span>
          </div>
          <div class="space-y-4">
            <div>
              <h3 class="text-sm font-bold mb-2 text-base-content">client id</h3>
              <div class="mockup-code w-full">
                <pre data-prefix=""><code class="code-display">{clientData.client_id}</code></pre>
              </div>
              <button onclick={`navigator.clipboard.writeText('${clientData.client_id}')`} class="btn btn-sm btn-outline mt-2">copy</button>
            </div>
            <div>
              <h3 class="text-sm font-bold mb-2 text-base-content">client secret</h3>
              <div class="mockup-code w-full">
                <pre data-prefix=""><code class="code-display">{clientData.client_secret}</code></pre>
              </div>
              <button onclick={`navigator.clipboard.writeText('${clientData.client_secret}')`} class="btn btn-sm btn-outline mt-2">copy</button>
            </div>
            {accessToken ? (
              <>
                <div>
                  <h3 class="text-sm font-bold mb-2 text-base-content">access token (expires in {Math.floor(expiresIn / 60)} minutes)</h3>
                  <div class="mockup-code w-full">
                    <pre data-prefix=""><code class="code-display text-xs">{accessToken}</code></pre>
                  </div>
                  <button onclick={`navigator.clipboard.writeText('${accessToken}')`} class="btn btn-sm btn-outline mt-2">copy</button>
                </div>
                <div class="card bg-base-200">
                  <div class="card-body">
                    <h3 class="text-sm font-bold mb-2 text-base-content">use in cursor mcp.json</h3>
                    <div class="mockup-code w-full">
                      <pre data-prefix=""><code class="code-display text-xs">{cursorConfig}</code></pre>
                    </div>
                    <button onclick={`navigator.clipboard.writeText(\`${cursorConfig}\`)`} class="btn btn-sm btn-outline mt-2">copy config</button>
                    <p class="text-xs text-base-content/60 mt-2">regenerate token when expired</p>
                  </div>
                </div>
              </>
            ) : (
              <div class="alert alert-warning">
                <span>token exchange failed. use client_id and client_secret to get a token manually.</span>
              </div>
            )}
          </div>
        </div>
      </div>
    );
  } catch (error) {
    console.error("m2m creation error:", error);
    return c.html(
      <div class="text-red-500">an error occurred</div>,
      500
    );
  }
});

// Docs page
app.get("/docs", (c) => {
  return c.html(
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>cubby docs</title>
        <link rel="icon" href="/favicon.svg" type="image/svg+xml" />
        <link rel="icon" href="/favicon.png" type="image/png" />
        <link rel="icon" href="/favicon.ico" />
        <script src="htmx.min.js"></script>
        <link rel="stylesheet" href="output.css" />
        <style>
          {`
            body {
              font-family: 'Courier New', monospace;
            }
            .pixelated {
              image-rendering: pixelated;
              image-rendering: -moz-crisp-edges;
              image-rendering: crisp-edges;
            }
          `}
        </style>
      </head>
      <body hx-boost="true" data-theme="dark">
        <TopBar />
        <div class="fixed top-12 left-0 w-full h-[calc(100vh-48px)] flex bg-base-100">
          {/* Sidebar */}
          <div class="w-64 bg-base-200 border-r border-base-300 overflow-y-auto">
            <div class="p-4 pt-8">
              <h2 class="text-lg font-bold mb-4 text-base-content">documentation</h2>
              <nav>
                <a href="#getting-started" class="block p-3 rounded-lg hover:bg-base-300 text-base-content transition-colors mb-1">getting started</a>
                <a href="#typescript-sdk" class="block p-3 rounded-lg hover:bg-base-300 text-base-content transition-colors mb-1">typescript sdk</a>
                <a href="#mcp-integration" class="block p-3 rounded-lg hover:bg-base-300 text-base-content transition-colors mb-1">mcp server</a>
                <a href="#rest-api" class="block p-3 rounded-lg hover:bg-base-300 text-base-content transition-colors mb-1">rest api</a>
                <div class="divider my-4"></div>
                <a href="/docs/api" class="block p-3 rounded-lg bg-accent text-accent-content font-medium">full api reference →</a>
              </nav>
            </div>
          </div>
          
          {/* Main Content */}
          <div class="flex-1 overflow-y-auto bg-base-100">
            <div class="max-w-4xl mx-auto p-8 pt-16">
              <div class="mb-8">
                <h1 class="text-4xl font-bold pixelated mb-2 text-base-content">cubby documentation</h1>
                <p class="text-base-content/70 text-lg">comprehensive guide to using cubby</p>
                <p class="text-base-content/80 mt-4"><strong>local-first data with cloud access</strong> - your screen and audio recordings stay on your device, but you control who can access them securely via oauth and mcp tools. use it locally for instant access, or connect remotely from ai assistants and custom apps.</p>
              </div>
              
              {/* Getting Started Section */}
              <div id="getting-started" class="card bg-base-200 shadow-xl mb-6">
                <div class="card-body">
                  <h2 class="card-title text-2xl font-bold text-base-content">getting started</h2>
                  <p class="text-base-content/80 mb-4">install cubby to start capturing everything. works on macos & linux.</p>
                  <div class="mockup-code w-full mb-4">
                    <pre data-prefix="$"><code>curl -s https://get.cubby.sh/cli | sh</code></pre>
                  </div>
                  <p class="text-base-content/80 mb-4">this installs the cubby binary and starts recording your screen and audio in the background. all data stays local in <code class="bg-base-100 px-1 rounded">~/.cubby/</code></p>
                  <p class="text-base-content/80 mb-4">a local rust server runs on <code class="bg-base-100 px-1 rounded">localhost:3030</code> that continuously records screen (ocr + screenshots) and audio (transcriptions + speaker identification). everything is stored in sqlite.</p>
                  <p class="text-base-content/80 mb-2">access your data in three ways:</p>
                  <ol class="list-decimal list-inside space-y-2 text-base-content/80 ml-4">
                    <li>typescript sdk - <code class="bg-base-100 px-1 rounded">npm i @cubby/js</code></li>
                    <li>rest api - <code class="bg-base-100 px-1 rounded">localhost:3030/openapi.json</code></li>
                    <li>mcp server - configure ai assistants to use cubby tools</li>
                  </ol>
                </div>
              </div>

              {/* TypeScript SDK Section */}
              <div id="typescript-sdk" class="card bg-base-200 shadow-xl mb-6">
                <div class="card-body">
                  <h2 class="card-title text-2xl font-bold text-base-content">typescript sdk</h2>
                  <p class="text-base-content/80 mb-6">the cubby js sdk works in node, cloudflare workers, and browsers. published as <code class="bg-base-100 px-1 rounded">@cubby/js</code></p>
                  
                  <div>
                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">installation</h3>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>npm i @cubby/js</code></pre>
                      </div>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">authentication</h3>
                      <p class="text-sm text-base-content/70 mb-2">get credentials at <a href="https://cubby.sh/dashboard" class="link">cubby.sh/dashboard</a></p>
                      <div class="mockup-code w-full mb-2">
                        <pre data-prefix="$"><code>export CUBBY_CLIENT_ID="your_client_id"</code></pre>
                        <pre data-prefix="$"><code>export CUBBY_CLIENT_SECRET="your_client_secret"</code></pre>
                      </div>
                    </div>

                    <p class="text-base-content/80 mb-4">common ways to use cubby:</p>
                    
                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">search</h3>
                      <p class="text-sm text-base-content/70 mb-2">query your history</p>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`import { createClient } from '@cubby/js';

// credentials auto-detected from env
const client = createClient({ 
  baseUrl: 'https://api.cubby.sh',
  clientId: process.env.CUBBY_CLIENT_ID,
  clientSecret: process.env.CUBBY_CLIENT_SECRET,
});

// list devices and select one (for remote)
const { devices } = await client.listDevices();
client.setDeviceId(devices[0].id);

// find that article you read last week
const results = await client.search({
  q: 'dolphins',
  content_type: 'ocr',
  limit: 5
});`}</code></pre>
                      </div>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">watch</h3>
                      <p class="text-sm text-base-content/70 mb-2">process live events and trigger actions</p>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`// auto-create todoist tasks from spoken todos
for await (const event of client.streamTranscriptions()) {
  if (event.text?.toLowerCase().includes('todo')) {
    const task = await ai.generateStructuredOutput({
      prompt: \`extract task from: "\${event.text}"\`,
      schema: { title: 'string', priority: 'high|medium|low' }
    });
    await todoist.create(task);
    await client.notify({ 
      title: 'task added', 
      body: \`"\${task.title}" - \${task.priority} priority\` 
    });
  }
}`}</code></pre>
                      </div>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">contextualize</h3>
                      <p class="text-sm text-base-content/70 mb-2">power ai with your personal context</p>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`// smart email responses based on recent chats
const recentChats = await client.search({
  q: 'slack messages project alpha',
  content_type: 'ocr',
  limit: 15
});

const draft = await ai.chat.completions.create({
  messages: [
    { role: 'system', content: 'draft professional email' },
    { role: 'user', content: \`context: \${JSON.stringify(recentChats)}\` }
  ]
});
await gmail.users.messages.send({ raw: encodeDraft(draft) });`}</code></pre>
                      </div>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">automate</h3>
                      <p class="text-sm text-base-content/70 mb-2">build smart automations</p>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`// auto-log work hours when specific apps are active
for await (const event of client.streamVision()) {
  if (event.data.app_name === 'Linear' && event.data.text?.match(/ENG-\\d+/)) {
    const ticketId = event.data.text.match(/ENG-\\d+/)[0];
    await timeTracker.startTimer({ project: 'engineering', ticket: ticketId });
    await client.notify({ title: 'timer started', body: \`tracking time on \${ticketId}\` });
  }
}`}</code></pre>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              {/* MCP Integration Section */}
              <div id="mcp-integration" class="card bg-base-200 shadow-xl mb-6">
                <div class="card-body">
                  <h2 class="card-title text-2xl font-bold text-base-content">mcp server</h2>
                  <p class="text-base-content/80 mb-6">model context protocol (mcp) allows ai assistants to access cubby tools directly.</p>
                  
                  <div>
                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">local access (claude desktop, cursor)</h3>
                      <p class="text-base-content/80 mb-3">add to your mcp config:</p>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`{
  "mcpServers": {
    "cubby": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-fetch", "http://localhost:3030/mcp"]
    }
  }
}`}</code></pre>
                      </div>
                      <p class="text-base-content/80 mt-3">no authentication required for local use</p>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">remote access</h3>
                      <p class="text-base-content/80 mb-3">use <code class="bg-base-100 px-1 rounded">https://api.cubby.sh/mcp</code> with bearer token authentication</p>
                      <ol class="list-decimal list-inside space-y-1 text-sm text-base-content/80 mb-3">
                        <li>get credentials at <a href="https://cubby.sh/dashboard" class="link">cubby.sh/dashboard</a></li>
                        <li>exchange for token: <code class="text-xs bg-base-100 px-1">curl -X POST https://api.cubby.sh/oauth/token -d "grant_type=client_credentials&client_id=ID&client_secret=SECRET&scope=read:cubby"</code></li>
                        <li>add to mcp config: <code class="text-xs bg-base-100 px-1">"headers": {{"Authorization": "Bearer TOKEN"}}</code></li>
                      </ol>
                      <p class="text-base-content/80 text-sm">remote tools require <code class="bg-base-100 px-1 rounded">deviceId</code> parameter</p>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">available tools</h3>
                      <div class="grid grid-cols-1 md:grid-cols-2 gap-3 text-sm">
                        <div class="bg-base-300 p-3 rounded">
                          <span class="font-semibold text-base-content">devices/list</span>
                          <p class="text-base-content/70 mt-1">list enrolled devices</p>
                        </div>
                        <div class="bg-base-300 p-3 rounded">
                          <span class="font-semibold text-base-content">devices/set</span>
                          <p class="text-base-content/70 mt-1">select device for calls</p>
                        </div>
                        <div class="bg-base-300 p-3 rounded">
                          <span class="font-semibold text-base-content">device/search</span>
                          <p class="text-base-content/70 mt-1">search content</p>
                        </div>
                        <div class="bg-base-300 p-3 rounded">
                          <span class="font-semibold text-base-content">device/search-keyword</span>
                          <p class="text-base-content/70 mt-1">fast keyword search</p>
                        </div>
                        <div class="bg-base-300 p-3 rounded">
                          <span class="font-semibold text-base-content">device/speakers/search</span>
                          <p class="text-base-content/70 mt-1">find speakers by name</p>
                        </div>
                        <div class="bg-base-300 p-3 rounded">
                          <span class="font-semibold text-base-content">device/open-application</span>
                          <p class="text-base-content/70 mt-1">launch applications</p>
                        </div>
                        <div class="bg-base-300 p-3 rounded">
                          <span class="font-semibold text-base-content">device/open-url</span>
                          <p class="text-base-content/70 mt-1">open urls in browser</p>
                        </div>
                        <div class="bg-base-300 p-3 rounded">
                          <span class="font-semibold text-base-content">device/notify</span>
                          <p class="text-base-content/70 mt-1">send notifications</p>
                        </div>
                        <div class="bg-base-300 p-3 rounded">
                          <span class="font-semibold text-base-content">device/audio/list</span>
                          <p class="text-base-content/70 mt-1">list audio devices</p>
                        </div>
                        <div class="bg-base-300 p-3 rounded">
                          <span class="font-semibold text-base-content">device/vision/list</span>
                          <p class="text-base-content/70 mt-1">list monitors</p>
                        </div>
                        <div class="bg-base-300 p-3 rounded">
                          <span class="font-semibold text-base-content">+ more</span>
                          <p class="text-base-content/70 mt-1">frames, tags, embeddings</p>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              {/* REST API Section */}
              <div id="rest-api" class="card bg-base-200 shadow-xl mb-6">
                <div class="card-body">
                  <h2 class="card-title text-2xl font-bold text-base-content">rest api</h2>
                  <p class="text-base-content/80 mb-6">full openapi spec available for custom integrations</p>
                  
                  <div>
                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">local server</h3>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`# runs on localhost:3030 after install
curl http://localhost:3030/openapi.json

# example: search content
curl "http://localhost:3030/search?q=project&limit=10"`}</code></pre>
                      </div>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">key endpoints</h3>
                      <ul class="text-sm text-base-content/80 space-y-2">
                        <li><code class="bg-base-100 px-1 rounded">GET /search</code> - search across screen, audio, ui</li>
                        <li><code class="bg-base-100 px-1 rounded">GET /search/keyword</code> - fast keyword search</li>
                        <li><code class="bg-base-100 px-1 rounded">GET /speakers/search</code> - find speakers</li>
                        <li><code class="bg-base-100 px-1 rounded">POST /open-application</code> - launch apps</li>
                        <li><code class="bg-base-100 px-1 rounded">POST /open-url</code> - open urls</li>
                        <li><code class="bg-base-100 px-1 rounded">POST /notify</code> - desktop notifications</li>
                        <li><code class="bg-base-100 px-1 rounded">WS /events</code> - stream live events</li>
                      </ul>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">remote api</h3>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`# authenticate
curl -X POST https://api.cubby.sh/login \\
  -H "Content-Type: application/json" \\
  -d '{"email": "you@email.com", "password": "pass"}'

# list devices
curl -H "Authorization: Bearer TOKEN" \\
  https://api.cubby.sh/devices

# search on specific device
curl -H "Authorization: Bearer TOKEN" \\
  "https://api.cubby.sh/devices/DEVICE_ID/search?q=hello"`}</code></pre>
                      </div>
                    </div>

                    <div class="mb-6">
                      <p class="text-base-content/80">full api reference: <a href="/docs/api" class="link">api.cubby.sh/openapi.json</a></p>
                    </div>
                  </div>
                </div>
              </div>

              {/* Footer Navigation */}
              <div class="pt-8 text-center">
                <div class="flex justify-center">
                  <a href="/" class="link link-hover text-base-content/70 mr-6">← back to home</a>
                  <a href="/login" class="link link-hover text-base-content/70">login</a>
                </div>
              </div>
            </div>
          </div>
        </div>
      </body>
    </html>
  );
});

// API Reference using Scalar
app.get(
  "/docs/api",
  Scalar({
    theme: "none",
    pageTitle: "cubby api reference",
    url: "/docs/openapi.json",
  })
);

// Proxy the OpenAPI spec from the API
app.get("/docs/openapi.json", async (c) => {
  const apiUrl = c.env.API_URL || "https://api.cubby.sh";
  const response = await fetch(`${apiUrl}/openapi.json`);
  const spec = await response.json();
  return c.json(spec);
});

export default app;