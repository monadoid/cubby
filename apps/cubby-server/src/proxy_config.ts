/**
 * Proxy configuration for device endpoint allowlist.
 * 
 * This module defines which Screenpipe API endpoints can be proxied through
 * the Cubby API server to user devices. Security model: deny by default,
 * explicitly allow safe endpoints.
 */

type AllowedRoute = {
  pattern: RegExp
  methods: ('GET' | 'POST' | 'PUT' | 'DELETE')[]
  description: string
}

/**
 * Allowlist of routes that can be proxied to devices.
 * Based on Screenpipe OpenAPI specification.
 */
const ALLOWED_ROUTES: AllowedRoute[] = [
  // Core health and search endpoints
  { pattern: /^\/health$/, methods: ['GET'], description: 'Health check' },
  { pattern: /^\/search$/, methods: ['GET'], description: 'Search content' },
  { pattern: /^\/search\/keyword$/, methods: ['GET'], description: 'Keyword search' },
  
  // Media listings (read-only)
  { pattern: /^\/audio\/list$/, methods: ['GET'], description: 'List audio recordings' },
  { pattern: /^\/vision\/list$/, methods: ['GET'], description: 'List vision/screen captures' },
  { pattern: /^\/frames\/[a-zA-Z0-9-]+$/, methods: ['GET'], description: 'Get specific frame by ID' },
  
  // Pipes (read-only operations)
  { pattern: /^\/pipes\/list$/, methods: ['GET'], description: 'List available pipes' },
  { pattern: /^\/pipes\/info\/[a-zA-Z0-9-]+$/, methods: ['GET'], description: 'Get pipe info by ID' },
  
  // Tags
  { pattern: /^\/tags\/[a-zA-Z0-9_-]+\/[a-zA-Z0-9-]+$/, methods: ['GET'], description: 'Get tags by content type and ID' },
  
  // Speakers (read-only)
  { pattern: /^\/speakers\/search$/, methods: ['GET'], description: 'Search speakers' },
  { pattern: /^\/speakers\/similar$/, methods: ['GET'], description: 'Find similar speakers' },
  
  // Experimental features (read-only)
  { pattern: /^\/experimental\/validate\/media$/, methods: ['GET'], description: 'Validate media files' },
  { pattern: /^\/experimental\/frames\/merge$/, methods: ['GET'], description: 'Merge frames' },
  
  // Embeddings API
  { pattern: /^\/v1\/embeddings$/, methods: ['POST'], description: 'Generate embeddings' },
  // Add content
  { pattern: /^\/add$/, methods: ['POST'], description: 'Add content to Screenpipe' },
]

/**
 * Check if a given path and HTTP method combination is allowed.
 * 
 * @param path - The request path (e.g., "/health", "/search")
 * @param method - The HTTP method (e.g., "GET", "POST")
 * @returns true if the path/method combination is allowed, false otherwise
 */
export function isPathAllowed(path: string, method: string): boolean {
  const upperMethod = method.toUpperCase()
  
  return ALLOWED_ROUTES.some(route => 
    route.pattern.test(path) && route.methods.includes(upperMethod as any)
  )
}

/**
 * Get all allowed routes (useful for debugging or documentation).
 */
export function getAllowedRoutes(): ReadonlyArray<AllowedRoute> {
  return ALLOWED_ROUTES
}
