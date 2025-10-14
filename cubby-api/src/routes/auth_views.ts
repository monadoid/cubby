import { Hono } from "hono";
import type { Bindings, Variables } from "../index";

const app = new Hono<{ Bindings: Bindings; Variables: Variables }>();

app.get("/login", (c) => {
  const redirectTo = c.req.query("redirect_to");

  const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Login - Cubby</title>
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <link rel="stylesheet" href="/tailwind.css" />
  <script src="/htmx.min.js"></script>
  <style>
    .htmx-request .htmx-default { display: none; }
    .htmx-indicator { display: none; }
    .htmx-request .htmx-indicator { display: inline; }
  </style>
</head>
<body class="font-sans mx-auto my-8 max-w-md px-4">
  <h1 class="text-3xl font-semibold mb-4">Login</h1>
  <p class="text-gray-700 mb-6">Sign in to your account to continue.</p>
  <form 
    class="flex flex-col gap-4"
    hx-post="/login" 
    hx-target="#error-container"
    hx-swap="innerHTML"
    hx-indicator="#login-btn"
  >
    ${redirectTo ? `<input type="hidden" name="redirect_to" value="${redirectTo}" />` : ""}
    <input 
      type="email" 
      name="email" 
      placeholder="Email" 
      required 
      autocomplete="email"
      class="px-3 py-2 border border-gray-300 rounded-md text-base focus:outline-none focus:border-blue-600 focus:ring-2 focus:ring-blue-500"
    />
    <input 
      type="password" 
      name="password" 
      placeholder="Password" 
      minlength="8" 
      required 
      autocomplete="current-password"
      class="px-3 py-2 border border-gray-300 rounded-md text-base focus:outline-none focus:border-blue-600 focus:ring-2 focus:ring-blue-500"
    />
    <div id="error-container"></div>
    <button 
      type="submit" 
      id="login-btn"
      class="px-3 py-2 bg-blue-600 text-white border-none rounded-md text-base font-medium cursor-pointer hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed htmx-request:opacity-50 htmx-request:cursor-wait"
    >
      <span class="htmx-indicator">Logging in...</span>
      <span class="htmx-default">Login</span>
    </button>
  </form>
  <a 
    href="/sign-up${redirectTo ? `?redirect_to=${encodeURIComponent(redirectTo)}` : ""}" 
    class="text-blue-600 no-underline text-sm mt-4 inline-block hover:underline"
  >
    Don't have an account? Sign up
  </a>
</body>
</html>`;

  return c.html(html);
});

app.get("/sign-up", (c) => {
  const redirectTo = c.req.query("redirect_to");

  const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Sign Up - Cubby</title>
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <link rel="stylesheet" href="/tailwind.css" />
  <script src="/htmx.min.js"></script>
  <style>
    .htmx-request .htmx-default { display: none; }
    .htmx-indicator { display: none; }
    .htmx-request .htmx-indicator { display: inline; }
  </style>
</head>
<body class="font-sans mx-auto my-8 max-w-md px-4">
  <h1 class="text-3xl font-semibold mb-4">Sign Up</h1>
  <p class="text-gray-700 mb-6">Create your account to continue.</p>
  <form 
    class="flex flex-col gap-4"
    hx-post="/sign-up" 
    hx-target="#error-container"
    hx-swap="innerHTML"
    hx-indicator="#signup-btn"
  >
    ${redirectTo ? `<input type="hidden" name="redirect_to" value="${redirectTo}" />` : ""}
    <input 
      type="email" 
      name="email" 
      placeholder="Email" 
      required 
      autocomplete="email"
      class="px-3 py-2 border border-gray-300 rounded-md text-base focus:outline-none focus:border-blue-600 focus:ring-2 focus:ring-blue-500"
    />
    <input 
      type="password" 
      name="password" 
      placeholder="Password" 
      minlength="8" 
      required 
      autocomplete="new-password"
      class="px-3 py-2 border border-gray-300 rounded-md text-base focus:outline-none focus:border-blue-600 focus:ring-2 focus:ring-blue-500"
    />
    <div id="error-container"></div>
    <button 
      type="submit" 
      id="signup-btn"
      class="px-3 py-2 bg-blue-600 text-white border-none rounded-md text-base font-medium cursor-pointer hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed htmx-request:opacity-50 htmx-request:cursor-wait"
    >
      <span class="htmx-indicator">Creating account...</span>
      <span class="htmx-default">Create Account</span>
    </button>
  </form>
  <a 
    href="/login${redirectTo ? `?redirect_to=${encodeURIComponent(redirectTo)}` : ""}" 
    class="text-blue-600 no-underline text-sm mt-4 inline-block hover:underline"
  >
    Already have an account? Login
  </a>
</body>
</html>`;

  return c.html(html);
});

export default app;
