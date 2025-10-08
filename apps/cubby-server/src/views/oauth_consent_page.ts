import type { IDPOAuthAuthorizeStartResponse } from 'stytch'
import {BaseOAuthParams} from '../routes/oauth_routes'

/**
 * Renders an interactive HTML consent page where users can approve/deny scopes.
 * The page displays grantable scopes as checkboxes and submits to /oauth/authorize/submit.
 * OAuth state fields are preserved as hidden form inputs.
 */
export function renderOAuthConsentPage(
    startResponse: IDPOAuthAuthorizeStartResponse,
    params: BaseOAuthParams
): string {
    // Build hidden inputs from all params except scopes (which are checkboxes)
    const hiddenInputs = Object.entries(params)
        .filter(([key, value]) => key !== 'scopes' && value !== undefined)
        .map(([name, value]) => 
            `<input type="hidden" name="${escapeHtml(name)}" value="${escapeHtml(String(value))}" />`
        )
        .join('\n')

    const connectedApp = startResponse.client
    const appHeader = connectedApp
        ? `<div style="margin-bottom: .5rem;">
        <div style="font-weight:600">${escapeHtml(connectedApp.client_name ?? 'Connected App')}</div>
        <div style="font-size:.9rem;color:#6b7280;">${escapeHtml(connectedApp.client_description ?? '')}</div>
      </div>`
        : ''

    const grantableScopes = (startResponse.scope_results ?? []).filter(s => s.is_grantable)

    const scopesList = grantableScopes.length
        ? grantableScopes
            .map(s => {
                const checked = params.scopes.includes(s.scope) ? 'checked' : ''
                return `<label style="display:flex;align-items:flex-start;gap:.5rem;margin:.25rem 0;">
            <input type="checkbox" class="scope-checkbox" value="${escapeHtml(s.scope)}" ${checked} />
            <span>
              <div style="font-weight:600">${escapeHtml(s.scope)}</div>
              <div style="color:#6b7280;font-size:.9rem">${escapeHtml(s.description ?? '')}</div>
            </span>
          </label>`
            })
            .join('\n')
        : '<div>Default access</div>'

    return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Authorize Connected App</title>
  <style>
    body { font-family: system-ui, -apple-system, Segoe UI, Roboto, Ubuntu, Cantarell, Noto Sans, Helvetica Neue, Arial, "Apple Color Emoji", "Segoe UI Emoji"; margin: 3rem auto; max-width: 560px; padding: 0 1.25rem; }
    h1 { font-size: 1.5rem; margin-bottom: .75rem; }
    form { margin-top: 1.25rem; display: flex; flex-direction: column; gap: .75rem; }
    .actions { display: flex; gap: .75rem; }
    button { padding: .7rem 1.25rem; border: none; border-radius: .5rem; font-size: 1rem; cursor: pointer; }
    .primary { background: #2563eb; color: #fff; }
    .secondary { background: #e5e7eb; color: #111827; }
    .card { border: 1px solid #e5e7eb; border-radius: .75rem; padding: 1rem; }
  </style>
</head>
<body>
  <h1>Authorize Connected App</h1>
  ${appHeader}
  <form method="post" action="/oauth/authorize/submit" id="consent-form">
    <div class="card">
      <div style="margin-bottom:.5rem; font-weight:600;">This application is requesting access to:</div>
      <div>${scopesList}</div>
    </div>
    ${hiddenInputs}
    <!-- OAuth 2.0 standard: scope as space-separated string -->
    <input type="hidden" name="scope" id="scope-field" value="" />
    <div class="actions">
      <button class="primary" type="submit" name="consent_granted" value="true">Allow</button>
      <button class="secondary" type="submit" name="consent_granted" value="false">Deny</button>
    </div>
  </form>
  <script>
    // Convert checked scopes to space-separated string on submit (OAuth 2.0 standard)
    document.getElementById('consent-form').addEventListener('submit', function(e) {
      const checkboxes = document.querySelectorAll('.scope-checkbox:checked');
      const scopes = Array.from(checkboxes).map(cb => cb.value).join(' ');
      document.getElementById('scope-field').value = scopes;
    });
  </script>
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
