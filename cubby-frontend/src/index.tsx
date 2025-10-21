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
    <html lang="en" data-theme="dark" style="--root-bg:#000; --color-base-100:#000; --color-base-content:#fff;">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>cubby</title>
        <link rel="icon" href="/favicon.svg" type="image/svg+xml" />
        <link rel="icon" href="/favicon.png" type="image/png" />
        <link rel="icon" href="/favicon.ico" />
        <script src="/htmx.min.js"></script>
        <link rel="stylesheet" href="/output.css" />
        <script dangerouslySetInnerHTML={{
          __html: `
            document.addEventListener('htmx:historyRestore', function(evt){
              var path = (evt && evt.detail && evt.detail.path) || location.pathname;
              if (path === '/') { window.location.replace('/'); }
            });
          `
        }}></script>
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
        <script src="/htmx.min.js"></script>
        <link rel="stylesheet" href="/output.css" />
        <script dangerouslySetInnerHTML={{
          __html: `
            document.addEventListener('htmx:historyRestore', function(evt){
              var path = (evt && evt.detail && evt.detail.path) || location.pathname;
              if (path === '/') { window.location.replace('/'); }
            });
          `
        }}></script>
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
        <script src="/htmx.min.js"></script>
        <link rel="stylesheet" href="/output.css" />
        <script dangerouslySetInnerHTML={{
          __html: `
            document.addEventListener('htmx:historyRestore', function(evt){
              var path = (evt && evt.detail && evt.detail.path) || location.pathname;
              if (path === '/') { window.location.replace('/'); }
            });
          `
        }}></script>
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
    <html lang="en" data-theme="dark">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>cubby docs</title>
        <link rel="icon" href="/favicon.svg" type="image/svg+xml" />
        <link rel="icon" href="/favicon.png" type="image/png" />
        <link rel="icon" href="/favicon.ico" />
        <script src="/htmx.min.js"></script>
        <link rel="stylesheet" href="/output.css" />
        <script dangerouslySetInnerHTML={{
          __html: `
            document.addEventListener('htmx:historyRestore', function(evt){
              var path = (evt && evt.detail && evt.detail.path) || location.pathname;
              if (path === '/') { window.location.replace('/'); }
            });
          `
        }}></script>
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
      <body hx-boost="true">
        <TopBar />
        <div class="fixed top-12 left-0 w-full h-[calc(100vh-48px)] flex bg-base-100">
          {/* Sidebar */}
          <div class="w-64 bg-base-200 border-r border-base-300 overflow-y-auto">
            <div class="p-4 pt-8">
              <h2 class="text-lg font-bold mb-4 text-base-content">documentation</h2>
              <nav>
                <a href="#getting-started" class="block p-3 rounded-lg hover:bg-base-300 text-base-content transition-colors mb-1">getting started</a>
                <a href="#data-shape" class="block p-3 rounded-lg hover:bg-base-300 text-base-content transition-colors mb-1">how it works</a>
                <a href="#typescript-sdk" class="block p-3 rounded-lg hover:bg-base-300 text-base-content transition-colors mb-1">build with cubby</a>
                <a href="#mcp-integration" class="block p-3 rounded-lg hover:bg-base-300 text-base-content transition-colors mb-1">use with ai assistants</a>
                <a href="#rest-api" class="block p-3 rounded-lg hover:bg-base-300 text-base-content transition-colors mb-1">rest api</a>
                <div class="divider my-4"></div>
                <a href="/docs/api" data-hx-boost="false" class="block p-3 rounded-lg bg-accent text-accent-content font-medium">full api reference →</a>
              </nav>
            </div>
          </div>
          
          {/* Main Content */}
          <div class="flex-1 overflow-y-auto bg-base-100">
            <div class="max-w-4xl mx-auto p-8 pt-16">
              {/* Hero Section */}
              <div class="mb-16">
                <h1 class="text-5xl font-bold pixelated mb-8 text-base-content text-center">
                  cubby turns your screen and microphone data into context, so you can let your favorite AI know what you're working on.
                </h1>
                <img src="/cubby_explainer_no_bg.png" alt="cubby overview" class="w-full h-auto pixelated mb-8" />
                <div class="prose prose-lg max-w-none text-center">
                  <p class="text-xl text-base-content/80 mb-4">
                    don't panic - your data is only stored locally, and cubby is completely open source.
                  </p>
                </div>
              </div>

              {/* Why Use Cubby Section */}
              <div class="card bg-base-200 shadow-xl mb-12">
                <div class="card-body">
                  <h2 class="card-title text-3xl font-bold mb-6">why use cubby?</h2>
                  <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                    <div class="space-y-4">
                      <div class="flex items-start gap-3">
                        <span class="text-primary text-xl">→</span>
                        <div>
                          <h3 class="font-semibold text-lg">ai with memory</h3>
                          <p class="text-base-content/80 text-sm">your ai assistant knows what you're working on, what you've read, and what you've discussed</p>
                        </div>
                      </div>
                      <div class="flex items-start gap-3">
                        <span class="text-primary text-xl">→</span>
                        <div>
                          <h3 class="font-semibold text-lg">local-first privacy</h3>
                          <p class="text-base-content/80 text-sm">all data stays on your device. you control who accesses it</p>
                        </div>
                      </div>
                    </div>
                    <div class="space-y-4">
                      <div class="flex items-start gap-3">
                        <span class="text-primary text-xl">→</span>
                        <div>
                          <h3 class="font-semibold text-lg">instant setup</h3>
                          <p class="text-base-content/80 text-sm">one install command. works with claude, cursor, and any ai tool</p>
                        </div>
                      </div>
                      <div class="flex items-start gap-3">
                        <span class="text-primary text-xl">→</span>
                        <div>
                          <h3 class="font-semibold text-lg">open source</h3>
                          <p class="text-base-content/80 text-sm">fully transparent. self-host or use our cloud tunnel</p>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              {/* Getting Started Section */}
              <div id="getting-started" class="card bg-base-200 shadow-xl mb-12">
                <div class="card-body">
                  <h2 class="card-title text-3xl font-bold mb-8">getting started</h2>
                          
                          <div class="space-y-12">
                            {/* Step 1 */}
                            <div class="card bg-base-100 shadow-lg">
                              <div class="card-body">
                                <div class="flex items-center gap-4 mb-6">
                                  <span class="badge badge-lg badge-primary font-bold text-lg px-4 py-2">step 1</span>
                                  <h3 class="text-2xl font-bold">install cubby</h3>
                                </div>
                                <p class="text-lg mb-6">one command installs everything. works on macos & linux.</p>
                                <div class="mockup-code w-full mb-6">
                                  <pre data-prefix="$"><code>curl -fsSL https://cubby.sh/install.sh | sh</code></pre>
                                </div>
                                <div class="alert alert-info">
                                  <div>
                                    <div class="font-bold">what you get:</div>
                                    <ul class="text-sm mt-2 space-y-1">
                                      <li>✓ installs cubby binary and starts background recording</li>
                                      <li>✓ provides your <code class="bg-base-200 px-1 rounded">CLIENT_ID</code> and <code class="bg-base-200 px-1 rounded">CLIENT_SECRET</code></li>
                                      <li>✓ stores all data locally in <code class="bg-base-200 px-1 rounded">~/.cubby/</code></li>
                                      <li>✓ starts local server on <code class="bg-base-200 px-1 rounded">localhost:3030</code></li>
                                      <li>✓ creates secure tunnel at <code class="bg-base-200 px-1 rounded">api.cubby.sh</code></li>
                                    </ul>
                                  </div>
                                </div>
                                <div class="alert alert-warning">
                                  <span>save your credentials - you'll need them for step 2</span>
                                </div>
                              </div>
                            </div>

                            {/* Step 2 */}
                            <div class="card bg-base-100 shadow-lg">
                              <div class="card-body">
                                <div class="flex items-center gap-4 mb-6">
                                  <span class="badge badge-lg badge-secondary font-bold text-lg px-4 py-2">step 2</span>
                                  <h3 class="text-2xl font-bold">deploy ai agent</h3>
                                </div>
                                <p class="text-lg mb-6">one-click deploy gives your ai access to your personal memory:</p>
                                <div class="bg-base-200 rounded-lg p-6 mb-6">
                                  <ul class="space-y-2 text-base">
                                    <li class="flex items-center gap-2">
                                      <span class="text-primary">→</span>
                                      search your screen and audio history
                                    </li>
                                    <li class="flex items-center gap-2">
                                      <span class="text-primary">→</span>
                                      send desktop notifications to your devices
                                    </li>
                                    <li class="flex items-center gap-2">
                                      <span class="text-primary">→</span>
                                      open applications and urls on your devices
                                    </li>
                                    <li class="flex items-center gap-2">
                                      <span class="text-primary">→</span>
                                      human-in-the-loop confirmations for sensitive actions
                                    </li>
                                  </ul>
                                </div>
                                <div class="flex flex-col sm:flex-row gap-4">
                                  <a href="https://github.com/monadoid/cubby-starter" target="_blank" rel="noopener noreferrer" class="btn btn-primary flex-1">
                                    view cubby-starter on github
                                  </a>
                                  <a href="https://deploy.workers.cloudflare.com/?url=https://github.com/monadoid/cubby-starter" target="_blank" rel="noopener noreferrer" class="btn btn-accent flex-1">
                                    deploy to cloudflare (one-click)
                                  </a>
                                </div>
                                <div class="alert alert-info mt-4">
                                  <span>you'll use the credentials from step 1 to configure the agent</span>
                                </div>
                              </div>
                            </div>

                            {/* Alternative: Build Your Own */}
                            <div class="divider text-base-content/50">or build your own</div>
                            
                            <div class="card bg-base-100 shadow-lg">
                              <div class="card-body">
                                <h3 class="text-2xl font-bold mb-6">access your data using:</h3>
                                <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
                                  <div class="card bg-base-200 shadow-md">
                                    <div class="card-body">
                                      <h4 class="card-title text-lg">typescript sdk</h4>
                                      <div class="mockup-code w-full mb-3">
                                        <pre data-prefix="$"><code>npm i @cubby/js</code></pre>
                                      </div>
                                      <p class="text-sm text-base-content/70">node, cloudflare, browser</p>
                                    </div>
                                  </div>
                                  <div class="card bg-base-200 shadow-md">
                                    <div class="card-body">
                                      <h4 class="card-title text-lg">rest api</h4>
                                      <div class="mockup-code w-full mb-3">
                                        <pre data-prefix="$"><code>api.cubby.sh</code></pre>
                                      </div>
                                      <p class="text-sm text-base-content/70">openapi spec available</p>
                                    </div>
                                  </div>
                                  <div class="card bg-base-200 shadow-md">
                                    <div class="card-body">
                                      <h4 class="card-title text-lg">mcp server</h4>
                                      <div class="mockup-code w-full mb-3">
                                        <pre data-prefix="$"><code>localhost:3030/mcp</code></pre>
                                      </div>
                                      <p class="text-sm text-base-content/70">claude, cursor, etc</p>
                                    </div>
                                  </div>
                                </div>
                              </div>
                            </div>
                          </div>
                        </div>
                      </div>

                      {/* Data Shape Section */}
                      <div id="data-shape" class="card bg-base-200 shadow-xl mb-12">
                        <div class="card-body">
                          <h2 class="card-title text-3xl font-bold mb-6">how it works</h2>
                          <p class="text-lg mb-6">live events streamed from your screen and microphone</p>
                          <div class="mockup-code w-full">
                            <pre data-prefix="$"><code>{`// ocr event
const ocrEvent = {
  name: 'ocr_result',
  data: {
    text: 'design doc - project alpha',
    timestamp: '2025-10-18T12:34:56Z',
    app_name: 'chrome',
    window_name: 'docs.google.com',
    browser_url: 'https://docs.google.com/document/d/...',
  }
};

// audio transcription event
const transcriptionEvent = {
  name: 'realtime_transcription',
  data: {
    transcription: 'let's ship this today',
    timestamp: '2025-10-18T12:35:10Z',
    device: 'macbook-pro',
    is_input: false,
    is_final: true,
    speaker: 'sam',
  }
};

// ui frame event
const uiFrameEvent = {
  name: 'ui_frame',
  data: {
    window: 'zoom meeting',
    app: 'zoom',
    text_output: 'Recording… | Mute | Share Screen',
    initial_traversal_at: '2025-10-18T12:35:20Z'
  }
};`}</code></pre>
                          </div>
                        </div>
                      </div>

                      {/* TypeScript SDK Section */}
                      <div id="typescript-sdk" class="card bg-base-200 shadow-xl mb-12">
                        <div class="card-body">
                          <h2 class="card-title text-3xl font-bold mb-6">build with cubby</h2>
                          <p class="text-lg mb-8">use the cubby js sdk in node, cloudflare workers, and browsers. published as <code class="bg-base-100 px-2 py-1 rounded">@cubby/js</code></p>
                          
                          <div class="space-y-8">
                            <div>
                              <h3 class="text-2xl font-bold mb-4">installation</h3>
                              <div class="mockup-code w-full">
                                <pre data-prefix="$"><code>npm i @cubby/js</code></pre>
                              </div>
                            </div>

                            <div>
                              <h3 class="text-2xl font-bold mb-4">authentication</h3>
                              <p class="text-base mb-4">get credentials at <a href="https://cubby.sh/dashboard" class="link link-primary">cubby.sh/dashboard</a></p>
                              <div class="mockup-code w-full">
                                <pre data-prefix="$"><code>export CUBBY_CLIENT_ID="your_client_id"</code></pre>
                                <pre data-prefix="$"><code>export CUBBY_CLIENT_SECRET="your_client_secret"</code></pre>
                              </div>
                            </div>

                            <div>
                              <h3 class="text-2xl font-bold mb-4">common ways to use cubby:</h3>
                              
                              <div class="space-y-8">
                                <div class="card bg-base-100 shadow-md">
                                  <div class="card-body">
                                    <h4 class="card-title text-xl">search</h4>
                                    <p class="text-sm text-base-content/70 mb-4">query your history</p>
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
                                </div>

                                <div class="card bg-base-100 shadow-md">
                                  <div class="card-body">
                                    <h4 class="card-title text-xl">watch</h4>
                                    <p class="text-sm text-base-content/70 mb-4">process live events and trigger actions</p>
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
                                </div>

                                <div class="card bg-base-100 shadow-md">
                                  <div class="card-body">
                                    <h4 class="card-title text-xl">contextualize</h4>
                                    <p class="text-sm text-base-content/70 mb-4">power ai with your personal context</p>
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
                                </div>

                                <div class="card bg-base-100 shadow-md">
                                  <div class="card-body">
                                    <h4 class="card-title text-xl">automate</h4>
                                    <p class="text-sm text-base-content/70 mb-4">build smart automations</p>
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
                          </div>
                        </div>
                      </div>

                      {/* MCP Integration Section */}
                      <div id="mcp-integration" class="card bg-base-200 shadow-xl mb-12">
                        <div class="card-body">
                          <h2 class="card-title text-3xl font-bold mb-6">use with ai assistants</h2>
                          <p class="text-lg mb-8">model context protocol (mcp) gives ai assistants direct access to your cubby data.</p>
                          
                          <div class="space-y-8">
                            <div class="card bg-base-100 shadow-md">
                              <div class="card-body">
                                <h3 class="text-2xl font-bold mb-4">local access (claude desktop, cursor)</h3>
                                <p class="text-base mb-4">add to your mcp config:</p>
                                <div class="mockup-code w-full mb-4">
                                  <pre data-prefix="$"><code>{`{
  "mcpServers": {
    "cubby": {
      "type": "streamable-http",
      "url": "http://localhost:3030/mcp"
    }
  }
}`}</code></pre>
                                </div>
                                <div class="alert alert-success">
                                  <span>no authentication required for local use</span>
                                </div>
                              </div>
                            </div>

                            <div class="card bg-base-100 shadow-md">
                              <div class="card-body">
                                <h3 class="text-2xl font-bold mb-4">remote access</h3>
                                <p class="text-base mb-4">use <code class="bg-base-200 px-2 py-1 rounded">https://api.cubby.sh/mcp</code> with bearer token authentication</p>
                                <ol class="list-decimal list-inside space-y-2 text-base mb-4">
                                  <li>get credentials at <a href="https://cubby.sh/dashboard" class="link link-primary">cubby.sh/dashboard</a></li>
                                  <li>exchange for token: <code class="text-sm bg-base-200 px-2 py-1 rounded">curl -X POST https://api.cubby.sh/oauth/token -d "grant_type=client_credentials&client_id=ID&client_secret=SECRET&scope=read:cubby"</code></li>
                                  <li>add to mcp config:</li>
                                </ol>
                                <div class="mockup-code w-full mb-4">
                                  <pre data-prefix="$"><code>{`{
  "mcpServers": {
    "cubby": {
      "type": "streamable-http",
      "url": "https://api.cubby.sh/mcp",
      "headers": {
        "Authorization": "Bearer YOUR_TOKEN"
      }
    }
  }
}`}</code></pre>
                                </div>
                                <div class="alert alert-info">
                                  <span>remote tools require <code class="bg-base-200 px-1 rounded">deviceId</code> parameter</span>
                                </div>
                              </div>
                            </div>

                            <div class="card bg-base-100 shadow-md">
                              <div class="card-body">
                                <h3 class="text-2xl font-bold mb-6">available tools</h3>
                                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                                  <div class="card bg-base-200 shadow-sm">
                                    <div class="card-body p-4">
                                      <span class="font-semibold text-base-content">devices/list</span>
                                      <p class="text-sm text-base-content/70 mt-1">list enrolled devices</p>
                                    </div>
                                  </div>
                                  <div class="card bg-base-200 shadow-sm">
                                    <div class="card-body p-4">
                                      <span class="font-semibold text-base-content">devices/set</span>
                                      <p class="text-sm text-base-content/70 mt-1">select device for calls</p>
                                    </div>
                                  </div>
                                  <div class="card bg-base-200 shadow-sm">
                                    <div class="card-body p-4">
                                      <span class="font-semibold text-base-content">device/search</span>
                                      <p class="text-sm text-base-content/70 mt-1">search content</p>
                                    </div>
                                  </div>
                                  <div class="card bg-base-200 shadow-sm">
                                    <div class="card-body p-4">
                                      <span class="font-semibold text-base-content">device/search-keyword</span>
                                      <p class="text-sm text-base-content/70 mt-1">fast keyword search</p>
                                    </div>
                                  </div>
                                  <div class="card bg-base-200 shadow-sm">
                                    <div class="card-body p-4">
                                      <span class="font-semibold text-base-content">device/speakers/search</span>
                                      <p class="text-sm text-base-content/70 mt-1">find speakers by name</p>
                                    </div>
                                  </div>
                                  <div class="card bg-base-200 shadow-sm">
                                    <div class="card-body p-4">
                                      <span class="font-semibold text-base-content">device/open-application</span>
                                      <p class="text-sm text-base-content/70 mt-1">launch applications</p>
                                    </div>
                                  </div>
                                  <div class="card bg-base-200 shadow-sm">
                                    <div class="card-body p-4">
                                      <span class="font-semibold text-base-content">device/open-url</span>
                                      <p class="text-sm text-base-content/70 mt-1">open urls in browser</p>
                                    </div>
                                  </div>
                                  <div class="card bg-base-200 shadow-sm">
                                    <div class="card-body p-4">
                                      <span class="font-semibold text-base-content">device/notify</span>
                                      <p class="text-sm text-base-content/70 mt-1">send notifications</p>
                                    </div>
                                  </div>
                                  <div class="card bg-base-200 shadow-sm">
                                    <div class="card-body p-4">
                                      <span class="font-semibold text-base-content">device/audio/list</span>
                                      <p class="text-sm text-base-content/70 mt-1">list audio devices</p>
                                    </div>
                                  </div>
                                  <div class="card bg-base-200 shadow-sm">
                                    <div class="card-body p-4">
                                      <span class="font-semibold text-base-content">device/vision/list</span>
                                      <p class="text-sm text-base-content/70 mt-1">list monitors</p>
                                    </div>
                                  </div>
                                  <div class="card bg-base-200 shadow-sm">
                                    <div class="card-body p-4">
                                      <span class="font-semibold text-base-content">+ more</span>
                                      <p class="text-sm text-base-content/70 mt-1">frames, tags, embeddings</p>
                                    </div>
                                  </div>
                                </div>
                              </div>
                            </div>
                          </div>
                        </div>
                      </div>

                      {/* REST API Section */}
                      <div id="rest-api" class="card bg-base-200 shadow-xl mb-12">
                        <div class="card-body">
                          <h2 class="card-title text-3xl font-bold mb-6">rest api</h2>
                          <p class="text-lg mb-8">full openapi spec for custom integrations</p>
                          
                          <div class="space-y-8">
                            <div class="card bg-base-100 shadow-md">
                              <div class="card-body">
                                <h3 class="text-2xl font-bold mb-4">local server</h3>
                                <div class="mockup-code w-full">
                                  <pre data-prefix="$"><code>{`# runs on localhost:3030 after install
curl http://localhost:3030/openapi.json

# example: search content
curl "http://localhost:3030/search?q=project&limit=10"`}</code></pre>
                                </div>
                              </div>
                            </div>

                            <div class="card bg-base-100 shadow-md">
                              <div class="card-body">
                                <h3 class="text-2xl font-bold mb-4">key endpoints</h3>
                                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                                  <div class="space-y-2">
                                    <div class="flex items-center gap-2">
                                      <code class="bg-base-200 px-2 py-1 rounded text-sm">GET /search</code>
                                      <span class="text-sm text-base-content/70">search across screen, audio, ui</span>
                                    </div>
                                    <div class="flex items-center gap-2">
                                      <code class="bg-base-200 px-2 py-1 rounded text-sm">GET /search/keyword</code>
                                      <span class="text-sm text-base-content/70">fast keyword search</span>
                                    </div>
                                    <div class="flex items-center gap-2">
                                      <code class="bg-base-200 px-2 py-1 rounded text-sm">GET /speakers/search</code>
                                      <span class="text-sm text-base-content/70">find speakers</span>
                                    </div>
                                    <div class="flex items-center gap-2">
                                      <code class="bg-base-200 px-2 py-1 rounded text-sm">POST /open-application</code>
                                      <span class="text-sm text-base-content/70">launch apps</span>
                                    </div>
                                  </div>
                                  <div class="space-y-2">
                                    <div class="flex items-center gap-2">
                                      <code class="bg-base-200 px-2 py-1 rounded text-sm">POST /open-url</code>
                                      <span class="text-sm text-base-content/70">open urls</span>
                                    </div>
                                    <div class="flex items-center gap-2">
                                      <code class="bg-base-200 px-2 py-1 rounded text-sm">POST /notify</code>
                                      <span class="text-sm text-base-content/70">desktop notifications</span>
                                    </div>
                                    <div class="flex items-center gap-2">
                                      <code class="bg-base-200 px-2 py-1 rounded text-sm">WS /events</code>
                                      <span class="text-sm text-base-content/70">stream live events</span>
                                    </div>
                                  </div>
                                </div>
                              </div>
                            </div>

                            <div class="card bg-base-100 shadow-md">
                              <div class="card-body">
                                <h3 class="text-2xl font-bold mb-4">remote api</h3>
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
                            </div>

                            <div class="alert alert-info">
                              <span>full api reference: <a href="/docs/api" class="link link-primary">api.cubby.sh/openapi.json</a></span>
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