import { Hono } from 'hono'
import { getCookie } from 'hono/cookie'
import { HTTPException } from 'hono/http-exception'
import { z } from 'zod'
import type { Bindings, Variables } from '../index'

const app = new Hono<{ Bindings: Bindings, Variables: Variables }>()

// OAuth schemas
const authorizeQuerySchema = z.object({
    client_id: z.string().min(1),
    redirect_uri: z.string().url(),
    response_type: z.string().optional(),
    scope: z.string().optional(),
    state: z.string().optional(),
    code_challenge: z.string().min(1),
    code_challenge_method: z.string().optional(),
})

const authorizeSubmitSchema = authorizeQuerySchema.extend({
    consent_granted: z.string().optional(),
})

// OAuth helper functions
function parseScopes(scope: string | undefined): string[] {
    if (!scope) {
        return []
    }
    return scope
        .split(' ')
        .map((s) => s.trim())
        .filter((s) => s.length > 0)
}

async function stytchPost(env: Bindings, path: string, payload: Record<string, unknown>): Promise<Record<string, unknown>> {
    const response = await fetch(`${env.STYTCH_BASE_URL}${path}`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'Authorization': `Basic ${btoa(`${env.STYTCH_PROJECT_ID}:${env.STYTCH_PROJECT_SECRET}`)}`,
        },
        body: JSON.stringify(payload),
    })

    const json = await response.json().catch(() => null)

    if (!response.ok) {
        console.error('Stytch API error', {
            path,
            status: response.status,
            body: json,
        })
        throw new HTTPException(502, { message: 'Authorization service error' })
    }

    return (json ?? {}) as Record<string, unknown>
}

async function finalizeAuthorization(
    env: Bindings,
    params: Record<string, unknown>,
    consentGranted: boolean,
) {
    const payload = {
        ...params,
        consent_granted: consentGranted,
    }
    const response = await stytchPost(env, '/v1/public/oauth/authorize', payload)
    const redirectUri = typeof response.redirect_uri === 'string' ? (response.redirect_uri as string) : null
    if (!redirectUri) {
        console.error('Missing redirect_uri from Stytch authorize response', response)
        throw new HTTPException(502, { message: 'Authorization service error' })
    }
    return { redirect_uri: redirectUri }
}

function inferScopes(scopesFromStart: unknown, fallback: string[]): string[] {
    if (Array.isArray(scopesFromStart)) {
        const values = scopesFromStart
            .map((scope) => {
                if (typeof scope === 'string') {
                    return scope
                }
                if (
                    scope &&
                    typeof scope === 'object' &&
                    'scope' in scope &&
                    typeof (scope as Record<string, unknown>).scope === 'string'
                ) {
                    return (scope as Record<string, string>).scope
                }
                return null
            })
            .filter((scope): scope is string => typeof scope === 'string' && scope.length > 0)
        if (values.length > 0) {
            return values
        }
    }
    return fallback
}

function renderConsentPage(scopes: string[], hidden: Record<string, string>): string {
    const hiddenInputs = Object.entries(hidden)
        .map(([name, value]) => `<input type="hidden" name="${escapeHtml(name)}" value="${escapeHtml(value)}" />`)
        .join('\n')

    const scopesList = scopes.length
        ? scopes.map((scope) => `<li>${escapeHtml(scope)}</li>`).join('\n')
        : '<li>Default access</li>'

    return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Authorize Connected App</title>
  <style>
    body { font-family: system-ui, sans-serif; margin: 3rem auto; max-width: 480px; padding: 0 1.5rem; }
    h1 { font-size: 1.5rem; margin-bottom: 1rem; }
    form { margin-top: 2rem; display: flex; flex-direction: column; gap: 0.75rem; }
    button { padding: 0.75rem 1.25rem; border: none; border-radius: 0.5rem; font-size: 1rem; cursor: pointer; }
    button[type="submit"] { background: #2563eb; color: #fff; }
    button.secondary { background: #e5e7eb; color: #111827; }
  </style>
</head>
<body>
  <h1>Authorize Connected App</h1>
  <p>This application is requesting access to:</p>
  <ul>
    ${scopesList}
  </ul>
  <form method="post" action="/oauth/authorize/submit">
    ${hiddenInputs}
    <div>
      <button type="submit">Allow</button>
    </div>
    <div>
      <button type="submit" name="consent_granted" value="false" class="secondary">Cancel</button>
    </div>
  </form>
</body>
</html>`
}

function escapeHtml(value: string): string {
    return value
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;')
}

// OAuth routes
app.get('/authorize', async (c) => {
    const queryParams = Object.fromEntries(new URL(c.req.url).searchParams.entries())
    const parsed = authorizeQuerySchema.safeParse(queryParams)
    if (!parsed.success) {
        return c.text('Invalid authorization request', 400)
    }

    const sessionJwt = getCookie(c, 'stytch_session_jwt')
    if (!sessionJwt) {
        return c.text('User session required. Please sign in first.', 401)
    }

    const { client_id, redirect_uri, response_type, scope, state, code_challenge, code_challenge_method } = parsed.data
    const resolvedResponseType = response_type ?? 'code'
    const resolvedCodeChallengeMethod = code_challenge_method ?? 'S256'
    const scopes = parseScopes(scope)

    const baseParams = {
        client_id,
        redirect_uri,
        response_type: resolvedResponseType,
        scopes,
        state,
        code_challenge,
        code_challenge_method: resolvedCodeChallengeMethod,
        session_jwt: sessionJwt,
    }

    const startResponse = await stytchPost(c.env, '/v1/public/oauth/authorize/start', baseParams)
    const consentRequired = Boolean((startResponse as { consent_required?: boolean }).consent_required)

    if (!consentRequired) {
        const authorizeResponse = await finalizeAuthorization(c.env, baseParams, true)
        return c.redirect(authorizeResponse.redirect_uri, 307)
    }

    const requestedScopes = inferScopes((startResponse as { scopes?: unknown }).scopes, scopes)
    return c.html(renderConsentPage(requestedScopes, {
        client_id,
        redirect_uri,
        response_type: resolvedResponseType,
        scope: scopes.join(' '),
        state: state ?? '',
        code_challenge,
        code_challenge_method: resolvedCodeChallengeMethod,
    }))
})

app.post('/authorize/submit', async (c) => {
    const body = await c.req.parseBody()
    const normalized = Object.fromEntries(
        Object.entries(body).map(([key, value]) => [key, typeof value === 'string' ? value : ''])
    )
    const parsed = authorizeSubmitSchema.safeParse(normalized)
    if (!parsed.success) {
        return c.text('Invalid authorization submission', 400)
    }

    const sessionJwt = getCookie(c, 'stytch_session_jwt')
    if (!sessionJwt) {
        return c.text('User session required. Please sign in first.', 401)
    }

    const { client_id, redirect_uri, response_type, scope, state, code_challenge, code_challenge_method } = parsed.data
    const scopes = parseScopes(scope)
    const consentGranted = (parsed.data.consent_granted ?? 'true').toLowerCase() !== 'false'
    const resolvedResponseType = response_type ?? 'code'
    const resolvedCodeChallengeMethod = code_challenge_method ?? 'S256'

    const baseParams = {
        client_id,
        redirect_uri,
        response_type: resolvedResponseType,
        scopes,
        state,
        code_challenge,
        code_challenge_method: resolvedCodeChallengeMethod,
        session_jwt: sessionJwt,
    }

    const authorizeResponse = await finalizeAuthorization(c.env, baseParams, consentGranted)
    return c.redirect(authorizeResponse.redirect_uri, 307)
})

export default app