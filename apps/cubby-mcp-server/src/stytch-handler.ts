import { env } from "cloudflare:workers";
import type { AuthRequest, OAuthHelpers } from "@cloudflare/workers-oauth-provider";
import { Hono } from "hono";
import { fetchUpstreamAuthToken, getUpstreamAuthorizeUrl, type Props } from "./utils";
import {
	clientIdAlreadyApproved,
	parseRedirectApproval,
	renderApprovalDialog,
} from "./workers-oauth-utils";

const app = new Hono<{ Bindings: Env & { OAUTH_PROVIDER: OAuthHelpers } }>();

app.get("/authorize", async (c) => {
	const oauthReqInfo = await c.env.OAUTH_PROVIDER.parseAuthRequest(c.req.raw);
	const { clientId } = oauthReqInfo;
	if (!clientId) {
		return c.text("Invalid request", 400);
	}

	if (
		await clientIdAlreadyApproved(c.req.raw, oauthReqInfo.clientId, env.COOKIE_ENCRYPTION_KEY)
	) {
		return redirectToStytch(c.req.raw, oauthReqInfo);
	}

	return renderApprovalDialog(c.req.raw, {
		client: await c.env.OAUTH_PROVIDER.lookupClient(clientId),
		server: {
			description: "This is a demo MCP Remote Server using Stytch for authentication.",
			logo: "https://stytch.com/favicon.ico",
			name: "Cloudflare Stytch MCP Server", // optional
		},
		state: { oauthReqInfo }, // arbitrary data that flows through the form submission below
	});
});

app.post("/authorize", async (c) => {
	// Validates form submission, extracts state, and generates Set-Cookie headers to skip approval dialog next time
	const { state, headers } = await parseRedirectApproval(c.req.raw, env.COOKIE_ENCRYPTION_KEY);
	if (!state.oauthReqInfo) {
		return c.text("Invalid request", 400);
	}

	return redirectToStytch(c.req.raw, state.oauthReqInfo, headers);
});

async function redirectToStytch(
	request: Request,
	oauthReqInfo: AuthRequest,
	headers: Record<string, string> = {},
) {
	return new Response(null, {
		headers: {
			...headers,
			location: getUpstreamAuthorizeUrl({
				client_id: env.STYTCH_TEST_PROJECT_ID,
				redirect_uri: new URL("/callback", request.url).href,
				scope: "openid email profile",
				state: btoa(JSON.stringify(oauthReqInfo)),
				upstream_url: "https://test.stytch.com/v1/oauth/authorize",
				response_type: "code",
			}),
		},
		status: 302,
	});
}

/**
 * OAuth Callback Endpoint
 *
 * This route handles the callback from Stytch Connected Apps after user authentication.
 * Stytch will redirect here with stytch_token_type=oauth and token=... query parameters.
 */
app.get("/callback", async (c) => {
	// Get the oauthReqInfo from state parameter
	const state = c.req.query("state");
	if (!state) {
		return c.text("Missing state parameter", 400);
	}
	
	const oauthReqInfo = JSON.parse(atob(state)) as AuthRequest;
	if (!oauthReqInfo.clientId) {
		return c.text("Invalid state", 400);
	}

	// Check for Stytch token type and token
	const stytchTokenType = c.req.query("stytch_token_type");
	const token = c.req.query("token");
	
	if (stytchTokenType !== "oauth" || !token) {
		return c.text("Invalid callback parameters", 400);
	}

	// Use Stytch OAuth authenticate endpoint to validate the token
	const stytchResponse = await fetch(`https://test.stytch.com/v1/oauth/authenticate`, {
		method: "POST",
		headers: {
			"Content-Type": "application/json",
			"Authorization": `Basic ${btoa(`${c.env.STYTCH_TEST_PROJECT_ID}:${c.env.STYTCH_TEST_SECRET}`)}`
		},
		body: JSON.stringify({ 
			token: token,
			session_duration_minutes: 60
		})
	});

	if (!stytchResponse.ok) {
		console.log(await stytchResponse.text());
		return c.text("Failed to authenticate with Stytch", 500);
	}

	const stytchData: any = await stytchResponse.json();
	const { user_id: login, user, session_token } = stytchData;
	const { name, emails } = user;
	const email = emails?.[0]?.email || '';

	// Return back to the MCP client a new token
	const { redirectTo } = await c.env.OAUTH_PROVIDER.completeAuthorization({
		metadata: {
			label: name,
		},
		// This will be available on this.props inside MyMCP
		props: {
			accessToken: session_token,
			email,
			login,
			name,
		} as Props,
		request: oauthReqInfo,
		scope: oauthReqInfo.scope,
		userId: login,
	});

	return Response.redirect(redirectTo);
});

// Add OAuth metadata endpoint to the Hono app
app.get("/.well-known/oauth-authorization-server", async (c) => {
	const url = new URL(c.req.url);
	const baseUrl = `${url.protocol}//${url.host}`;
	
	return c.json({
		issuer: baseUrl,
		authorization_endpoint: `${baseUrl}/authorize`,
		token_endpoint: `${baseUrl}/token`,
		registration_endpoint: `${baseUrl}/register`,
		response_types_supported: ["code"],
		grant_types_supported: ["authorization_code"],
		code_challenge_methods_supported: ["S256"],
		scopes_supported: ["read", "write"],
		token_endpoint_auth_methods_supported: ["client_secret_basic", "client_secret_post"],
	});
});

export { app as StytchHandler };
