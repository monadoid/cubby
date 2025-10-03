import { Hono } from 'hono'
import { zValidator } from '@hono/zod-validator'
import { z } from 'zod'
import type { Bindings, Variables } from '../index'
import { renderDevicesFragment } from '../views/devices_fragment'

// Schemas
const searchRequestSchema = z.object({
  deviceId: z.string().min(1, 'Device ID is required'),
  q: z.string().min(1, 'Search query is required'),
  limit: z
    .string()
    .optional()
    .transform((val) => {
      if (!val || val === '') return 10
      const num = Number(val)
      return isNaN(num) ? 10 : num
    }),
})

type SearchRequest = z.infer<typeof searchRequestSchema>

const app = new Hono<{ Bindings: Bindings; Variables: Variables }>()

// Error message helper
function getErrorMessage(error: unknown): string {
  if (error instanceof DOMException && error.name === 'AbortError') {
    return 'Request timed out'
  }

  if (error instanceof Error) {
    return error.message
  }

  return 'Unknown error'
}

// Server-side device list HTML fragment
app.get('/devices-fragment', async (c) => {
  const authHeader = c.req.header('Authorization')
  if (!authHeader) {
    return c.html('<option value="">⚠️ Not authenticated - Connect Cubby first</option>')
  }
  
  try {
    const devicesUrl = new URL('/devices', c.env.CUBBY_API_URL)
    const response = await fetch(devicesUrl.toString(), {
      headers: { Authorization: authHeader },
    })
    
    if (!response.ok) {
      const error = await response.text()
      console.error('Failed to load devices:', error)
      return c.html('<option value="">⚠️ Failed to load devices</option>')
    }
    
    const data = await response.json()
    return c.html(renderDevicesFragment(data.devices || []))
  } catch (error) {
    console.error('Error loading devices:', error)
    return c.html('<option value="">❌ Error loading devices</option>')
  }
})

app.post(
  '/search',
  zValidator('form', searchRequestSchema),
  async (c) => {
    const authHeader = c.req.header('Authorization')
    if (!authHeader) {
      return c.text('⚠️ Missing Authorization header', 401)
    }

    const { deviceId, q, limit } = c.req.valid('form')

    try {
      // Build search URL with query parameters
      const searchUrl = new URL(`/devices/${deviceId}/search`, c.env.CUBBY_API_URL)
      searchUrl.searchParams.set('q', q)
      if (limit) {
        searchUrl.searchParams.set('limit', limit.toString())
      }

      console.log(`Proxying search request to: ${searchUrl.toString()}`)

      const response = await fetch(searchUrl.toString(), {
        method: 'GET',
        headers: {
          'Authorization': authHeader,
        },
      })

      const body = await response.text()
      
      if (!response.ok) {
        return c.text(`❌ Error (${response.status}): ${body}`)
      }

      // Try to pretty-print JSON
      try {
        const json = JSON.parse(body)
        return c.text(JSON.stringify(json, null, 2))
      } catch {
        return c.text(body)
      }
    } catch (error) {
      console.error('Search proxy error:', error)
      return c.text(`❌ Failed to search: ${getErrorMessage(error)}`, 502)
    }
  }
)

export default app

