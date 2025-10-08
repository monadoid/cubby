/**
 * MCP HTTP Handler
 * 
 * Binds the MCP server to HTTP transport for request/response handling.
 */

import { StreamableHttpTransport } from 'mcp-lite'
import { mcpServer } from './server'

// Import tools to register them
import './tools/search'

/**
 * HTTP transport for MCP server.
 * Uses stateless mode (no session adapter) for simplicity.
 */
const transport = new StreamableHttpTransport()

/**
 * HTTP handler for MCP requests.
 * Pass Web API Request objects to this handler.
 */
export const mcpHttpHandler = transport.bind(mcpServer)



