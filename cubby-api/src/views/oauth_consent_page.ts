import type { IDPOAuthAuthorizeStartResponse } from "stytch";
import type { BaseOAuthParams } from "../routes/oauth_routes";

/**
 * Renders an interactive HTML consent page where users can approve/deny scopes.
 * The page displays grantable scopes as checkboxes and submits to /oauth/authorize/submit.
 * OAuth state fields are preserved as hidden form inputs.
 */
export function renderOAuthConsentPage(
  startResponse: IDPOAuthAuthorizeStartResponse,
  params: BaseOAuthParams,
): string {
  const hiddenInputs = Object.entries(params)
    .filter(
      ([key, value]) =>
        key !== "scopes" && key !== "scope" && value !== undefined,
    )
    .map(
      ([name, value]) =>
        `<input type="hidden" name="${escapeHtml(name)}" value="${escapeHtml(String(value))}" />`,
    )
    .join("\n");

  const connectedApp = startResponse.client;
  const appHeader = connectedApp
    ? `<div class="mb-2">
        <div class="font-semibold">${escapeHtml(connectedApp.client_name ?? "Connected App")}</div>
        <div class="text-sm text-gray-500">${escapeHtml(connectedApp.client_description ?? "")}</div>
      </div>`
    : "";

  const requestedScopes = new Set(params.scopes);
  const grantableScopes = (startResponse.scope_results ?? []).filter(
    (scope) => scope.is_grantable,
  );

  const scopesList = grantableScopes.length
    ? grantableScopes
        .map((scope) => {
          const checked = requestedScopes.has(scope.scope) ? "checked" : "";
          return `<label class="flex items-start gap-2 my-1">
            <input type="checkbox" class="scope-checkbox mt-1" name="scopes" value="${escapeHtml(scope.scope)}" ${checked} />
            <span>
              <div class="font-semibold">${escapeHtml(scope.scope)}</div>
              <div class="text-gray-500 text-sm">${escapeHtml(scope.description ?? "")}</div>
            </span>
          </label>`;
        })
        .join("\n")
    : "<div>Default access</div>";

  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Authorize Connected App</title>
  <link rel="stylesheet" href="/tailwind.css" />
</head>
<body class="font-sans mx-auto my-12 max-w-lg px-5">
  <h1 class="text-2xl font-semibold mb-3">Authorize Connected App</h1>
  ${appHeader}
  <form method="post" action="/oauth/authorize/submit" id="consent-form" class="mt-5 flex flex-col gap-3">
    <div class="border border-gray-200 rounded-xl p-4">
      <div class="mb-2 font-semibold">This application is requesting access to:</div>
      <div>${scopesList}</div>
    </div>
    ${hiddenInputs}
    <input type="hidden" name="scope" id="scope-field" value="${escapeHtml(params.scope)}" />
    <div class="flex gap-3">
      <button class="px-5 py-3 border-none rounded-lg text-base cursor-pointer bg-blue-600 text-white hover:bg-blue-700" type="submit" name="consent_granted" value="true">Allow</button>
      <button class="px-5 py-3 border-none rounded-lg text-base cursor-pointer bg-gray-200 text-gray-900 hover:bg-gray-300" type="submit" name="consent_granted" value="false">Deny</button>
    </div>
  </form>
  <script>
    document.getElementById('consent-form').addEventListener('submit', function () {
      const checkboxes = document.querySelectorAll('.scope-checkbox:checked');
      const scopes = Array.from(checkboxes).map(function (checkbox) { return checkbox.value; });
      document.getElementById('scope-field').value = scopes.join(' ');
    });
  </script>
</body>
</html>`;
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}
