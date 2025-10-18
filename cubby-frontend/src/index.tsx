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
                <a href="#getting-started" class="block p-3 rounded-lg bg-primary text-primary-content font-medium mb-1">getting started</a>
                <a href="#typescript-sdk" class="block p-3 rounded-lg hover:bg-base-300 text-base-content transition-colors mb-1">typescript sdk</a>
                <a href="#mcp-integration" class="block p-3 rounded-lg hover:bg-base-300 text-base-content transition-colors mb-1">mcp integration</a>
                <a href="#rest-api" class="block p-3 rounded-lg hover:bg-base-300 text-base-content transition-colors mb-1">rest api</a>
                <a href="#local-vs-remote" class="block p-3 rounded-lg hover:bg-base-300 text-base-content transition-colors mb-1">local vs remote</a>
                <div class="divider my-4"></div>
                <a href="/docs/api" class="block p-3 rounded-lg bg-accent text-accent-content font-medium">api reference →</a>
              </nav>
            </div>
          </div>
          
          {/* Main Content */}
          <div class="flex-1 overflow-y-auto bg-base-100">
            <div class="max-w-4xl mx-auto p-8 pt-16">
              <div class="mb-8">
                <h1 class="text-4xl font-bold pixelated mb-2 text-base-content">cubby documentation</h1>
                <p class="text-base-content/70 text-lg">comprehensive guide to using cubby</p>
              </div>
              
              {/* Getting Started Section */}
              <div id="getting-started" class="card bg-base-200 shadow-xl mb-6">
                <div class="card-body">
                  <h2 class="card-title text-2xl font-bold text-base-content">getting started</h2>
                  <p class="text-base-content/80 mb-4">cubby works on macos & linux, windows is coming soon.</p>
                  <div class="mockup-code w-full mb-4">
                    <pre data-prefix="$"><code>curl -s https://get.cubby.sh/cli | sh</code></pre>
                  </div>
                  <p class="text-base-content/80 mb-4">once you&apos;ve installed it, you can view your data a few ways:</p>
                  <ol class="list-decimal list-inside space-y-2 text-base-content/80 ml-4">
                    <li>connect to your cubby via mcp tool</li>
                    <li>connect to your cubby via rest api</li>
                    <li>connect to your cubby via ts sdk</li>
                  </ol>
                </div>
              </div>

              {/* TypeScript SDK Section */}
              <div id="typescript-sdk" class="card bg-base-200 shadow-xl mb-6">
                <div class="card-body">
                  <h2 class="card-title text-2xl font-bold text-base-content">typescript sdk</h2>
                  <p class="text-base-content/80 mb-6">the cubby js sdk works in node, cloudflare workers, and in the browser.</p>
                  
                  <div>
                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">installation</h3>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>npm install @cubby/js</code></pre>
                      </div>
                    </div>
                    
                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">basic usage</h3>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`import { createClient } from "@cubby/js";

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
}`}</code></pre>
                      </div>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">streaming transcriptions</h3>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`// stream real-time audio transcriptions
for await (const chunk of client.streamTranscriptions()) {
  const text = chunk.choices[0].text;
  const isFinal = chunk.choices[0].finish_reason === "stop";
  const device = chunk.metadata?.device;
  
  console.log(\`[\${device}] \${isFinal ? "final:" : "partial:"} \${text}\`);
}`}</code></pre>
                      </div>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">streaming vision data</h3>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`// stream ocr and ui frame data
for await (const event of client.streamVision(true)) { // true = include images
  if (event.name === "ocr_result") {
    console.log("ocr:", event.data.text);
  } else if (event.name === "ui_frame") {
    console.log("ui frame:", event.data);
  }
}`}</code></pre>
                      </div>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">device control</h3>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`// open applications and urls
await client.device.openApplication("Visual Studio Code");
await client.device.openUrl("https://github.com", "Safari");

// send notifications
await client.notify({
  title: "hello from cubby",
  body: "this is a test notification"
});`}</code></pre>
                      </div>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">environment configuration</h3>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`// set environment variables
export CUBBY_API_BASE_URL="https://api.cubby.sh"
export CUBBY_API_TOKEN="your-token"

// or in browser/workers
globalThis.__CUBBY_ENV__ = {
  CUBBY_API_BASE_URL: "https://api.cubby.sh",
  CUBBY_API_TOKEN: "your-token"
};

// then use without explicit config
const client = createClient();`}</code></pre>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              {/* MCP Integration Section */}
              <div id="mcp-integration" class="card bg-base-200 shadow-xl mb-6">
                <div class="card-body">
                  <h2 class="card-title text-2xl font-bold text-base-content">mcp integration</h2>
                  <p class="text-base-content/80 mb-6">model context protocol (mcp) allows ai assistants to interact with cubby tools.</p>
                  
                  <div>
                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">claude desktop</h3>
                      <p class="text-base-content/80 mb-3">add to your claude desktop config:</p>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`{
  "mcpServers": {
    "cubby": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-fetch", "http://localhost:3030/mcp"],
      "env": {}
    }
  }
}`}</code></pre>
                      </div>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">cursor ide</h3>
                      <p class="text-base-content/80 mb-3">add to your cursor mcp config:</p>
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
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">available mcp tools</h3>
                      <ul class="text-base-content/80">
                        <li class="mb-2"><span class="font-semibold text-base-content">search-content</span> - search through ocr text, audio transcriptions, ui elements</li>
                        <li class="mb-2"><span class="font-semibold text-base-content">open-application</span> - open applications by name (macos only)</li>
                        <li class="mb-2"><span class="font-semibold text-base-content">open-url</span> - open urls in browser (cross-platform)</li>
                        <li class="mb-2"><span class="font-semibold text-base-content">pixel-control</span> - control mouse and keyboard (cross-platform)</li>
                        <li class="mb-2"><span class="font-semibold text-base-content">find-elements</span> - find ui elements by role (macos only)</li>
                        <li class="mb-2"><span class="font-semibold text-base-content">click-element</span> - click ui elements by id (macos only)</li>
                        <li class="mb-2"><span class="font-semibold text-base-content">fill-element</span> - type into ui elements (macos only)</li>
                        <li class="mb-2"><span class="font-semibold text-base-content">scroll-element</span> - scroll ui elements (macos only)</li>
                      </ul>
                    </div>
                  </div>
                </div>
              </div>

              {/* REST API Section */}
              <div id="rest-api" class="card bg-base-200 shadow-xl mb-6">
                <div class="card-body">
                  <h2 class="card-title text-2xl font-bold text-base-content">rest api</h2>
                  <p class="text-base-content/80 mb-6">direct api access for custom integrations.</p>
                  
                  <div>
                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">local development</h3>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`# start cubby service
cubby start

# api documentation
curl http://localhost:3030/openapi.json

# mcp server
curl http://localhost:3030/mcp`}</code></pre>
                      </div>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">authentication</h3>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`# get oauth token
curl -X POST https://api.cubby.sh/dev/token \\
  -H "Content-Type: application/json" \\
  -d '{"email": "your@email.com", "password": "your-password"}'

# use token in requests
curl -H "Authorization: Bearer YOUR_TOKEN" \\
  https://api.cubby.sh/devices`}</code></pre>
                      </div>
                    </div>

                    <div class="mb-6">
                      <h3 class="text-xl font-bold mb-3 text-base-content">search endpoints</h3>
                      <div class="mockup-code w-full">
                        <pre data-prefix="$"><code>{`# search content
curl -H "Authorization: Bearer YOUR_TOKEN" \\
  "https://api.cubby.sh/devices/DEVICE_ID/search?q=hello&limit=10"

# semantic search
curl -H "Authorization: Bearer YOUR_TOKEN" \\
  "https://api.cubby.sh/devices/DEVICE_ID/semantic-search?q=meeting notes"`}</code></pre>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              {/* Local vs Remote Section */}
              <div id="local-vs-remote" class="card bg-base-200 shadow-xl mb-6">
                <div class="card-body">
                  <h2 class="card-title text-2xl font-bold text-base-content">local vs remote</h2>
                  <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                    <div class="card bg-base-300 shadow-md">
                      <div class="card-body">
                        <h3 class="card-title text-lg font-bold text-base-content">local usage</h3>
                        <ul class="text-sm text-base-content/80">
                          <li class="mb-2">mcp client: <code class="bg-base-100 px-1 rounded text-base-content">http://localhost:3030/mcp</code></li>
                          <li class="mb-2">no authentication required</li>
                          <li class="mb-2">tools require no device_id parameter</li>
                          <li class="mb-2">direct access to local cubby instance</li>
                        </ul>
                      </div>
                    </div>
                    <div class="card bg-base-300 shadow-md">
                      <div class="card-body">
                        <h3 class="card-title text-lg font-bold text-base-content">remote usage</h3>
                        <ul class="text-sm text-base-content/80">
                          <li class="mb-2">mcp client: <code class="bg-base-100 px-1 rounded text-base-content">https://api.cubby.sh/mcp</code></li>
                          <li class="mb-2">oauth authentication required</li>
                          <li class="mb-2">tools require device_id parameter</li>
                          <li class="mb-2">access from anywhere via cloud</li>
                        </ul>
                      </div>
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