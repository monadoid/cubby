import type { Context } from 'hono';
import type { AuthorizationSession } from './oauth';
export declare const SESSION_COOKIE_NAME = "cubby_oauth_session";
export declare const SESSION_TTL_SECONDS = 600;
export declare function writeSessionCookie(c: Context, session: AuthorizationSession, secret: string, secure: boolean): Promise<void>;
export declare function readSessionCookie(c: Context, secret: string): Promise<AuthorizationSession | null>;
export declare function clearSessionCookie(c: Context, secure: boolean): void;
//# sourceMappingURL=session.d.ts.map