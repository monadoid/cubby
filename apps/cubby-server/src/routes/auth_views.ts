import { Hono } from 'hono'
import type { Bindings, Variables } from '../index'

const app = new Hono<{ Bindings: Bindings; Variables: Variables }>()

const SHARED_STYLES = `
  body { font-family: system-ui, sans-serif; margin: 2rem auto; max-width: 400px; padding: 0 1rem; }
  h1 { font-size: 1.75rem; margin-bottom: 1rem; }
  form { display: flex; flex-direction: column; gap: 1rem; margin-top: 1.5rem; }
  input { padding: 0.75rem; border: 1px solid #d1d5db; border-radius: 0.375rem; font-size: 1rem; }
  input:focus { outline: none; border-color: #2563eb; ring: 2px solid #3b82f6; }
  button { padding: 0.75rem; background: #2563eb; color: white; border: none; border-radius: 0.375rem; font-size: 1rem; font-weight: 500; cursor: pointer; }
  button:hover { background: #1d4ed8; }
  button:disabled { opacity: 0.5; cursor: not-allowed; }
  .error { color: #dc2626; font-size: 0.875rem; padding: 0.75rem; background: #fee2e2; border-radius: 0.375rem; }
  .link { color: #2563eb; text-decoration: none; font-size: 0.875rem; margin-top: 1rem; display: inline-block; }
  .link:hover { text-decoration: underline; }
  .htmx-request button { opacity: 0.5; cursor: wait; }
`

app.get('/login', (c) => {
  const redirectTo = c.req.query('redirect_to')

  const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Login - Cubby</title>
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <script src="/htmx.min.js"></script>
  <style>${SHARED_STYLES}</style>
</head>
<body>
  <h1>Login</h1>
  <p>Sign in to your account to continue.</p>
  <form 
    hx-post="/login" 
    hx-target="#error-container"
    hx-swap="innerHTML"
    hx-indicator="#login-btn"
  >
    ${redirectTo ? `<input type="hidden" name="redirect_to" value="${redirectTo}" />` : ''}
    <input type="email" name="email" placeholder="Email" required autocomplete="email" />
    <input type="password" name="password" placeholder="Password" minlength="8" required autocomplete="current-password" />
    <div id="error-container"></div>
    <button type="submit" id="login-btn">
      <span class="htmx-indicator">Logging in...</span>
      <span class="htmx-default">Login</span>
    </button>
  </form>
  <a href="/sign-up${redirectTo ? `?redirect_to=${encodeURIComponent(redirectTo)}` : ''}" class="link">
    Don't have an account? Sign up
  </a>
  <style>
    .htmx-request .htmx-default { display: none; }
    .htmx-indicator { display: none; }
    .htmx-request .htmx-indicator { display: inline; }
  </style>
</body>
</html>`

  return c.html(html)
})

app.get('/sign-up', (c) => {
  const redirectTo = c.req.query('redirect_to')

  const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Sign Up - Cubby</title>
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <script src="/htmx.min.js"></script>
  <style>${SHARED_STYLES}</style>
</head>
<body>
  <h1>Sign Up</h1>
  <p>Create your account to continue.</p>
  <form 
    hx-post="/sign-up" 
    hx-target="#error-container"
    hx-swap="innerHTML"
    hx-indicator="#signup-btn"
  >
    ${redirectTo ? `<input type="hidden" name="redirect_to" value="${redirectTo}" />` : ''}
    <input type="email" name="email" placeholder="Email" required autocomplete="email" />
    <input type="password" name="password" placeholder="Password" minlength="8" required autocomplete="new-password" />
    <div id="error-container"></div>
    <button type="submit" id="signup-btn">
      <span class="htmx-indicator">Creating account...</span>
      <span class="htmx-default">Create Account</span>
    </button>
  </form>
  <a href="/login${redirectTo ? `?redirect_to=${encodeURIComponent(redirectTo)}` : ''}" class="link">
    Already have an account? Login
  </a>
  <style>
    .htmx-request .htmx-default { display: none; }
    .htmx-indicator { display: none; }
    .htmx-request .htmx-indicator { display: inline; }
  </style>
</body>
</html>`

  return c.html(html)
})

export default app

