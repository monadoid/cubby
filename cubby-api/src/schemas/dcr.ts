/**
 * Dynamic Client Registration (DCR) schemas
 *
 * Type-safe Zod schemas matching Stytch's DCR API specification.
 * https://stytch.com/docs/api/oauth2-register
 */

import { z } from "zod";

/**
 * Request schema for POST /oauth/register
 * Matches Stytch's DCR registration endpoint
 */
export const DcrRegisterRequestSchema = z.object({
  redirect_uris: z
    .array(z.string().url())
    .min(1, "At least one redirect URI is required"),
  client_name: z.string().optional(),
  client_uri: z.string().url().optional(),
});

export type DcrRegisterRequest = z.infer<typeof DcrRegisterRequestSchema>;

/**
 * Success response schema for DCR registration
 */
export const DcrRegisterResponseSchema = z.object({
  client_id: z.string(),
  client_name: z.string(),
  grant_types: z.array(z.string()),
  redirect_uris: z.array(z.string()),
  response_types: z.array(z.string()),
  token_endpoint_auth_method: z.string(),
  request_id: z.string().optional(),
  status_code: z.number().optional(),
  client_uri: z.string().optional(),
});

export type DcrRegisterResponse = z.infer<typeof DcrRegisterResponseSchema>;

/**
 * Error response schema from Stytch
 * Used for 429, 500, and other error responses
 */
export const DcrErrorResponseSchema = z.object({
  status_code: z.number(),
  request_id: z.string(),
  error_type: z.string(),
  error_message: z.string(),
  error_url: z.string().optional(),
});

export type DcrErrorResponse = z.infer<typeof DcrErrorResponseSchema>;
