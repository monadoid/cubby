// cubby-frontend - simple hello world htmx frontend
import { Hono } from "hono";
import { Content } from "./components/Content";
import { Fluid } from "./components/Fluid";
import { TopBar } from "./components/TopBar";

type Bindings = {
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
        <script src="https://unpkg.com/htmx.org@1.9.10"></script>
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

// Docs page
app.get("/docs", (c) => {
  return c.html(
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>cubby docs</title>
        <script src="https://unpkg.com/htmx.org@1.9.10"></script>
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
          `}
        </style>
      </head>
      <body hx-boost="true" hx-ext="sse,ws">
        <TopBar />
        <div style="position: fixed; top: 48px; left: 0; width: 100vw; height: calc(100vh - 48px); z-index: 30; pointer-events: none; display: flex; align-items: center; justify-content: center;">
          <div style="background-color: #000; padding: 2rem; pointer-events: auto; max-width: 800px; margin: 0 auto;">
            <h1 class="text-4xl font-bold mb-8 pixelated">cubby docs</h1>
            <div class="space-y-4">
              <p>welcome to the cubby documentation.</p>
              <p>this is a hello world docs page. more documentation coming soon.</p>
              <a href="/" class="text-white underline hover:text-gray-300">
                ← back to home
              </a>
            </div>
          </div>
        </div>
      </body>
    </html>
  );
});

export default app;