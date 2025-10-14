import * as oauth from 'oauth4webapi';
export type OAuthConfig = {
    authorizationEndpoint: string;
    tokenEndpoint: string;
    clientId: string;
    redirectUri: string;
    scope: string;
    issuer: string;
};
export type OAuthContext = {
    as: oauth.AuthorizationServer;
    client: oauth.Client;
    clientAuth: oauth.ClientAuth;
};
export type AuthorizationSession = {
    state: string;
    codeVerifier: string;
    issuedAt: number;
};
export type Connection = {
    accessToken: string;
    scope?: string;
    expiresIn?: number;
    receivedAt: number;
};
export declare function createOAuthContext(config: OAuthConfig): OAuthContext;
export declare function buildAuthorizationUrl(config: OAuthConfig, state: string, codeChallenge: string): URL;
export declare function validateCallbackParameters(context: OAuthContext, callbackUrl: URL, expectedState: string): URLSearchParams;
export declare function exchangeAuthorizationCode(context: OAuthContext, callbackParameters: URLSearchParams, redirectUri: string, codeVerifier: string, timeoutMs?: number): Promise<Connection>;
export declare const generateRandomState: typeof oauth.generateRandomState, generateRandomCodeVerifier: typeof oauth.generateRandomCodeVerifier, calculatePKCECodeChallenge: typeof oauth.calculatePKCECodeChallenge;
//# sourceMappingURL=oauth.d.ts.map