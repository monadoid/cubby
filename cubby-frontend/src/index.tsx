// cubby-frontend - simple hello world htmx frontend
import { Hono } from "hono";
import { Scalar } from "@scalar/hono-api-reference";
import { Content } from "./components/Content";
import { Fluid } from "./components/Fluid";
import { TopBar } from "./components/TopBar";

type Bindings = {
  API_URL: string;
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
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>cubby - login</title>
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
            #login-error {
              min-height: 24px;
            }
          `}
        </style>
      </head>
      <body hx-boost="true">
        <TopBar />
        <div style="position: fixed; top: 48px; left: 0; width: 100vw; height: calc(100vh - 48px); display: flex; align-items: center; justify-content: center;">
          <div style="background-color: #000; padding: 2rem; max-width: 400px; width: 100%; border: 1px solid #fff;">
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
                  required
                  class="w-full px-3 py-2 border border-gray-600 bg-black text-white rounded focus:outline-none focus:border-white"
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
                  required
                  class="w-full px-3 py-2 border border-gray-600 bg-black text-white rounded focus:outline-none focus:border-white"
                  placeholder="••••••••"
                />
              </div>
              <div id="login-error" class="text-red-500 text-sm"></div>
              <button 
                id="login-button"
                type="submit"
                class="w-full px-4 py-2 bg-white text-black font-bold rounded hover:bg-gray-200 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
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
    
    // Store token in a cookie for the dashboard to access
    c.header("Set-Cookie", `cubby_token=${data.session_jwt}; Path=/; HttpOnly; SameSite=Lax; Max-Age=3600`);
    
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
  const cookies = c.req.header("cookie") || "";
  const tokenMatch = cookies.match(/cubby_token=([^;]+)/);
  const token = tokenMatch ? tokenMatch[1] : null;

  if (!token) {
    return c.redirect("/login");
  }

  return c.html(
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>cubby - dashboard</title>
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
            .token-display {
              word-break: break-all;
              background-color: #111;
              padding: 1rem;
              border: 1px solid #333;
              border-radius: 4px;
              font-family: 'Courier New', monospace;
              font-size: 0.875rem;
            }
          `}
        </style>
      </head>
      <body hx-boost="true">
        <TopBar />
        <div style="position: fixed; top: 48px; left: 0; width: 100vw; height: calc(100vh - 48px); overflow-y: auto;">
          <div style="background-color: #000; padding: 2rem; max-width: 800px; margin: 0 auto;">
            <h1 class="text-3xl font-bold mb-8 pixelated">dashboard</h1>
            <div class="space-y-6">
              <div>
                <h2 class="text-xl font-bold mb-4">your session token</h2>
                <div class="token-display">{token}</div>
              </div>
              <div>
                <p class="text-gray-400 mb-4">
                  you&apos;re logged in! use this token for api requests.
                </p>
                <a 
                  href="/login" 
                  class="inline-block px-4 py-2 bg-white text-black font-bold rounded hover:bg-gray-200 transition-colors"
                  onclick="document.cookie = 'cubby_token=; Path=/; Max-Age=0'"
                >
                  logout
                </a>
              </div>
            </div>
          </div>
        </div>
      </body>
    </html>
  );
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
        <link rel="stylesheet" href="tailwind.css" />
        <style>
          {`
            body {
              font-family: 'Courier New', monospace;
              background-color: #0a0a0a;
              color: #fff;
            }
            .pixelated {
              image-rendering: pixelated;
              image-rendering: -moz-crisp-edges;
              image-rendering: crisp-edges;
            }
            .docs-sidebar {
              background-color: #111;
              border-right: 1px solid #333;
            }
            .docs-nav-item {
              display: block;
              padding: 0.75rem 1rem;
              color: #ccc;
              text-decoration: none;
              border-left: 3px solid transparent;
              transition: all 0.2s;
            }
            .docs-nav-item:hover {
              color: #fff;
              background-color: #1a1a1a;
              border-left-color: #666;
            }
            .docs-nav-item.active {
              color: #fff;
              background-color: #1a1a1a;
              border-left-color: #fff;
            }
            .docs-content {
              background-color: #0a0a0a;
            }
            .docs-card {
              background-color: #111;
              border: 1px solid #333;
              border-radius: 8px;
              padding: 1.5rem;
              margin-bottom: 1.5rem;
            }
            .docs-code-block {
              background-color: #1a1a1a;
              border: 1px solid #333;
              border-radius: 6px;
              padding: 1rem;
              font-family: 'Courier New', monospace;
              font-size: 0.875rem;
              overflow-x: auto;
            }
          `}
        </style>
      </head>
      <body hx-boost="true">
        <TopBar />
        <div class="fixed top-12 left-0 w-full h-[calc(100vh-48px)] flex">
          {/* Sidebar */}
          <div class="docs-sidebar w-64 overflow-y-auto">
            <div class="p-4 pt-8">
              <h2 class="text-lg font-bold mb-4">documentation</h2>
              <nav class="space-y-1">
                <a href="#getting-started" class="docs-nav-item active">getting started</a>
                <a href="#typescript-sdk" class="docs-nav-item">typescript sdk</a>
                <a href="#mcp-integration" class="docs-nav-item">mcp integration</a>
                <a href="#rest-api" class="docs-nav-item">rest api</a>
                <a href="#local-vs-remote" class="docs-nav-item">local vs remote</a>
                <div class="border-t border-gray-700 my-4"></div>
                <a href="/docs/api" class="docs-nav-item text-blue-400 hover:text-blue-300">api reference →</a>
              </nav>
            </div>
          </div>
          
          {/* Main Content */}
          <div class="docs-content flex-1 overflow-y-auto">
            <div class="max-w-4xl mx-auto p-8 pt-16">
              <div class="mb-8">
                <h1 class="text-4xl font-bold pixelated mb-2">cubby documentation</h1>
                <p class="text-gray-400 text-lg">comprehensive guide to using cubby</p>
              </div>
              
              {/* Getting Started Section */}
              <div id="getting-started" class="docs-card">
                <h2 class="text-2xl font-bold mb-4">getting started</h2>
                <p class="text-gray-300 mb-4">cubby works on macos & linux, windows is coming soon.</p>
                <div class="docs-code-block mb-4">
                  <code>curl -s https://get.cubby.sh/cli | sh</code>
                </div>
                <p class="text-gray-300 mb-4">once you&apos;ve installed it, you can view your data a few ways:</p>
                <ol class="list-decimal list-inside space-y-2 text-gray-300 ml-4">
                  <li>connect to your cubby via mcp tool</li>
                  <li>connect to your cubby via rest api</li>
                  <li>connect to your cubby via ts sdk</li>
                </ol>
              </div>

              {/* TypeScript SDK Section */}
              <div id="typescript-sdk" class="docs-card">
                <h2 class="text-2xl font-bold mb-4">typescript sdk</h2>
                <p class="text-gray-300 mb-6">the cubby js sdk works in node, cloudflare workers, and in the browser.</p>
                
                <div class="space-y-6">
                  <div>
                    <h3 class="text-xl font-bold mb-3">installation</h3>
                    <div class="docs-code-block">
                      <code>npm install @cubby/js</code>
                    </div>
                  </div>
                  
                  <div>
                    <h3 class="text-xl font-bold mb-3">basic usage</h3>
                    <div class="docs-code-block">
                      <code>{`import { createClient } from "@cubby/js";

const client = createClient({
  baseUrl: "https://api.cubby.sh", // or http://localhost:3030 for local
  token: "your-oauth-token"
});

// search through your recorded content
const results = await client.search({
  startTime: new Date(Date.now() - 5 * 60 * 1000).toISOString(), // last 5 minutes
  limit: 10,
  contentType: "all" // "ocr", "audio", "ui", or "all"
});

console.log(\`found \${results.pagination.total} items\`);
for (const item of results.data) {
  console.log(\`[\${item.type}] \${item.content.timestamp}\`);
}`}</code>
                    </div>
                  </div>

                  <div>
                    <h3 class="text-xl font-bold mb-3">streaming transcriptions</h3>
                    <div class="docs-code-block">
                      <code>{`// stream real-time audio transcriptions
for await (const chunk of client.streamTranscriptions()) {
  const text = chunk.choices[0].text;
  const isFinal = chunk.choices[0].finish_reason === "stop";
  const device = chunk.metadata?.device;
  
  console.log(\`[\${device}] \${isFinal ? "final:" : "partial:"} \${text}\`);
}`}</code>
                    </div>
                  </div>

                  <div>
                    <h3 class="text-xl font-bold mb-3">streaming vision data</h3>
                    <div class="docs-code-block">
                      <code>{`// stream ocr and ui frame data
for await (const event of client.streamVision(true)) { // true = include images
  if (event.name === "ocr_result") {
    console.log("ocr:", event.data.text);
  } else if (event.name === "ui_frame") {
    console.log("ui frame:", event.data);
  }
}`}</code>
                    </div>
                  </div>

                  <div>
                    <h3 class="text-xl font-bold mb-3">device control</h3>
                    <div class="docs-code-block">
                      <code>{`// open applications and urls
await client.device.openApplication("Visual Studio Code");
await client.device.openUrl("https://github.com", "Safari");

// send notifications
await client.notify({
  title: "hello from cubby",
  body: "this is a test notification"
});`}</code>
                    </div>
                  </div>

                  <div>
                    <h3 class="text-xl font-bold mb-3">environment configuration</h3>
                    <div class="docs-code-block">
                      <code>{`// set environment variables
export CUBBY_API_BASE_URL="https://api.cubby.sh"
export CUBBY_API_TOKEN="your-token"

// or in browser/workers
globalThis.__CUBBY_ENV__ = {
  CUBBY_API_BASE_URL: "https://api.cubby.sh",
  CUBBY_API_TOKEN: "your-token"
};

// then use without explicit config
const client = createClient();`}</code>
                    </div>
                  </div>
                </div>
              </div>

              {/* MCP Integration Section */}
              <div id="mcp-integration" class="docs-card">
                <h2 class="text-2xl font-bold mb-4">mcp integration</h2>
                <p class="text-gray-300 mb-6">model context protocol (mcp) allows ai assistants to interact with cubby tools.</p>
                
                <div class="space-y-6">
                  <div>
                    <h3 class="text-xl font-bold mb-3">claude desktop</h3>
                    <p class="text-gray-300 mb-3">add to your claude desktop config:</p>
                    <div class="docs-code-block">
                      <code>{`{
  "mcpServers": {
    "cubby": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-fetch", "http://localhost:3030/mcp"],
      "env": {}
    }
  }
}`}</code>
                    </div>
                  </div>

                  <div>
                    <h3 class="text-xl font-bold mb-3">cursor ide</h3>
                    <p class="text-gray-300 mb-3">add to your cursor mcp config:</p>
                    <div class="docs-code-block">
                      <code>{`{
  "mcpServers": {
    "cubby": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-fetch", "http://localhost:3030/mcp"]
    }
  }
}`}</code>
                    </div>
                  </div>

                  <div>
                    <h3 class="text-xl font-bold mb-3">available mcp tools</h3>
                    <ul class="space-y-2 text-gray-300">
                      <li><span class="font-semibold text-gray-200">search-content</span> - search through ocr text, audio transcriptions, ui elements</li>
                      <li><span class="font-semibold text-gray-200">open-application</span> - open applications by name (macos only)</li>
                      <li><span class="font-semibold text-gray-200">open-url</span> - open urls in browser (cross-platform)</li>
                      <li><span class="font-semibold text-gray-200">pixel-control</span> - control mouse and keyboard (cross-platform)</li>
                      <li><span class="font-semibold text-gray-200">find-elements</span> - find ui elements by role (macos only)</li>
                      <li><span class="font-semibold text-gray-200">click-element</span> - click ui elements by id (macos only)</li>
                      <li><span class="font-semibold text-gray-200">fill-element</span> - type into ui elements (macos only)</li>
                      <li><span class="font-semibold text-gray-200">scroll-element</span> - scroll ui elements (macos only)</li>
                    </ul>
                  </div>
                </div>
              </div>

              {/* REST API Section */}
              <div id="rest-api" class="docs-card">
                <h2 class="text-2xl font-bold mb-4">rest api</h2>
                <p class="text-gray-300 mb-6">direct api access for custom integrations.</p>
                
                <div class="space-y-6">
                  <div>
                    <h3 class="text-xl font-bold mb-3">local development</h3>
                    <div class="docs-code-block">
                      <code>{`# start cubby service
cubby start

# api documentation
curl http://localhost:3030/openapi.json

# mcp server
curl http://localhost:3030/mcp`}</code>
                    </div>
                  </div>

                  <div>
                    <h3 class="text-xl font-bold mb-3">authentication</h3>
                    <div class="docs-code-block">
                      <code>{`# get oauth token
curl -X POST https://api.cubby.sh/dev/token \\
  -H "Content-Type: application/json" \\
  -d '{"email": "your@email.com", "password": "your-password"}'

# use token in requests
curl -H "Authorization: Bearer YOUR_TOKEN" \\
  https://api.cubby.sh/devices`}</code>
                    </div>
                  </div>

                  <div>
                    <h3 class="text-xl font-bold mb-3">search endpoints</h3>
                    <div class="docs-code-block">
                      <code>{`# search content
curl -H "Authorization: Bearer YOUR_TOKEN" \\
  "https://api.cubby.sh/devices/DEVICE_ID/search?q=hello&limit=10"

# semantic search
curl -H "Authorization: Bearer YOUR_TOKEN" \\
  "https://api.cubby.sh/devices/DEVICE_ID/semantic-search?q=meeting notes"`}</code>
                    </div>
                  </div>
                </div>
              </div>

              {/* Local vs Remote Section */}
              <div id="local-vs-remote" class="docs-card">
                <h2 class="text-2xl font-bold mb-4">local vs remote</h2>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                  <div class="bg-gray-800/50 rounded-lg p-6">
                    <h3 class="text-lg font-bold text-gray-200 mb-4">local usage</h3>
                    <ul class="space-y-2 text-sm text-gray-300">
                      <li>mcp client: <code class="bg-gray-700 px-1 rounded text-gray-200">http://localhost:3030/mcp</code></li>
                      <li>no authentication required</li>
                      <li>tools require no device_id parameter</li>
                      <li>direct access to local cubby instance</li>
                    </ul>
                  </div>
                  <div class="bg-gray-800/50 rounded-lg p-6">
                    <h3 class="text-lg font-bold text-gray-200 mb-4">remote usage</h3>
                    <ul class="space-y-2 text-sm text-gray-300">
                      <li>mcp client: <code class="bg-gray-700 px-1 rounded text-gray-200">https://api.cubby.sh/mcp</code></li>
                      <li>oauth authentication required</li>
                      <li>tools require device_id parameter</li>
                      <li>access from anywhere via cloud</li>
                    </ul>
                  </div>
                </div>
              </div>

              {/* Footer Navigation */}
              <div class="pt-8 text-center">
                <div class="flex justify-center gap-6">
                  <a href="/" class="text-gray-300 underline hover:text-gray-100">← back to home</a>
                  <a href="/login" class="text-gray-300 underline hover:text-gray-100">login</a>
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