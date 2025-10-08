import * as oauth from 'oauth4webapi'

export type OAuthConfig = {
  authorizationEndpoint: string
  tokenEndpoint: string
  clientId: string
  clientSecret: string
  redirectUri: string
  scopes: string[]
  issuer: string
}

export type OAuthContext = {
  as: oauth.AuthorizationServer
  client: oauth.Client
  clientAuth: oauth.ClientAuth
}

export type AuthorizationSession = {
  state: string
  codeVerifier: string
  issuedAt: number
}

export type Connection = {
  accessToken: string
  scope?: string
  expiresIn?: number
  receivedAt: number
}

export function createOAuthContext(config: OAuthConfig): OAuthContext {
  const tokenUrl = new URL(config.tokenEndpoint)
  const authorizationUrl = new URL(config.authorizationEndpoint)
  const issuer = config.issuer.trim()
  if (!issuer) {
    throw new Error('OAuth issuer is required')
  }

  return {
    as: {
      issuer,
      authorization_endpoint: authorizationUrl.toString(),
      token_endpoint: tokenUrl.toString(),
    },
    client: {
      client_id: config.clientId,
      client_secret: config.clientSecret,
    },
    // Use ClientSecretBasic for confidential client authentication
    clientAuth: oauth.ClientSecretBasic(config.clientSecret),
  }
}

export function buildAuthorizationUrl(config: OAuthConfig, state: string, codeChallenge: string): URL {
  const url = new URL(config.authorizationEndpoint)
  url.searchParams.set('client_id', config.clientId)
  url.searchParams.set('redirect_uri', config.redirectUri)
  url.searchParams.set('response_type', 'code')
  // OAuth 2.0 spec requires 'scope' (singular) as space-separated string
  url.searchParams.set('scope', config.scopes.join(' '))
  url.searchParams.set('code_challenge', codeChallenge)
  url.searchParams.set('code_challenge_method', 'S256')
  url.searchParams.set('state', state)
  return url
}

export function validateCallbackParameters(
  context: OAuthContext,
  callbackUrl: URL,
  expectedState: string,
): URLSearchParams {
  return oauth.validateAuthResponse(context.as, context.client, callbackUrl, expectedState)
}

export async function exchangeAuthorizationCode(
  context: OAuthContext,
  callbackParameters: URLSearchParams,
  redirectUri: string,
  codeVerifier: string,
  timeoutMs = 10_000,
): Promise<Connection> {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), timeoutMs)

  try {
    const response = await oauth.authorizationCodeGrantRequest(
      context.as,
      context.client,
      context.clientAuth,
      callbackParameters,
      redirectUri,
      codeVerifier,
      { signal: controller.signal },
    )

    const result = await oauth.processAuthorizationCodeResponse(context.as, context.client, response)
    if (!result.access_token) {
      throw new Error('Token response missing access token')
    }

    return {
      accessToken: result.access_token,
      scope: result.scope,
      expiresIn: result.expires_in,
      receivedAt: Date.now(),
    }
  } finally {
    clearTimeout(timeout)
  }
}

export const { generateRandomState, generateRandomCodeVerifier, calculatePKCECodeChallenge } = oauth
