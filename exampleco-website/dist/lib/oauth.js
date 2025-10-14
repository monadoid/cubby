import * as oauth from 'oauth4webapi';
const publicClientAuth = (_as, client, body) => {
    if (!body.has('client_id')) {
        body.set('client_id', client.client_id);
    }
};
export function createOAuthContext(config) {
    const tokenUrl = new URL(config.tokenEndpoint);
    const authorizationUrl = new URL(config.authorizationEndpoint);
    const issuer = config.issuer.trim();
    if (!issuer) {
        throw new Error('OAuth issuer is required');
    }
    return {
        as: {
            issuer,
            authorization_endpoint: authorizationUrl.toString(),
            token_endpoint: tokenUrl.toString(),
        },
        client: {
            client_id: config.clientId,
        },
        clientAuth: publicClientAuth,
    };
}
export function buildAuthorizationUrl(config, state, codeChallenge) {
    const url = new URL(config.authorizationEndpoint);
    url.searchParams.set('client_id', config.clientId);
    url.searchParams.set('redirect_uri', config.redirectUri);
    url.searchParams.set('response_type', 'code');
    url.searchParams.set('scope', config.scope);
    url.searchParams.set('code_challenge', codeChallenge);
    url.searchParams.set('code_challenge_method', 'S256');
    url.searchParams.set('state', state);
    return url;
}
export function validateCallbackParameters(context, callbackUrl, expectedState) {
    return oauth.validateAuthResponse(context.as, context.client, callbackUrl, expectedState);
}
export async function exchangeAuthorizationCode(context, callbackParameters, redirectUri, codeVerifier, timeoutMs = 10000) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), timeoutMs);
    try {
        const response = await oauth.authorizationCodeGrantRequest(context.as, context.client, context.clientAuth, callbackParameters, redirectUri, codeVerifier, { signal: controller.signal });
        const result = await oauth.processAuthorizationCodeResponse(context.as, context.client, response);
        if (!result.access_token) {
            throw new Error('Token response missing access token');
        }
        return {
            accessToken: result.access_token,
            scope: result.scope,
            expiresIn: result.expires_in,
            receivedAt: Date.now(),
        };
    }
    finally {
        clearTimeout(timeout);
    }
}
export const { generateRandomState, generateRandomCodeVerifier, calculatePKCECodeChallenge } = oauth;
//# sourceMappingURL=oauth.js.map