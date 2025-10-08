import { Hono } from 'hono'
import { zValidator } from '@hono/zod-validator'
import { z } from 'zod'
import OpenAI from 'openai'
import type { Bindings, Variables } from '../index'
import { renderDevicesFragment } from '../views/devices_fragment'
import { callMcpTool } from '../lib/mcp_client'

// Schemas matching screenpipe OpenAPI spec
const contentTypeSchema = z.enum(['all', 'ocr', 'audio', 'ui', 'audio+ui', 'ocr+ui', 'audio+ocr']).optional()

const searchRequestSchema = z.object({
  deviceId: z.string().min(1, 'Device ID is required'),
  q: z.string().optional().default(''), // Optional - when empty, returns recent activity
  limit: z
    .string()
    .optional()
    .transform((val: string | undefined) => {
      if (!val || val === '') return 10
      const num = Number(val)
      return isNaN(num) ? 10 : num
    }),
  content_type: contentTypeSchema.default('all'),
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

// HTML escape helper
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
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
    
    const data = await response.json() as { devices: any[] }
    return c.html(renderDevicesFragment(data.devices || []))
  } catch (error) {
    console.error('Error loading devices:', error)
    return c.html('<option value="">❌ Error loading devices</option>')
  }
})

// MCP search endpoint - uses JSON-RPC 2.0 to call MCP tools
app.post(
  '/mcp-search',
  zValidator('form', searchRequestSchema),
  async (c) => {
    const authHeader = c.req.header('Authorization')
    if (!authHeader) {
      return c.text('⚠️ Missing Authorization header', 401)
    }

    const { deviceId, q, limit, content_type } = c.req.valid('form')
    const accessToken = authHeader.replace(/^Bearer\s+/i, '')

    try {
      // Construct MCP endpoint URL
      const mcpUrl = new URL('/mcp', c.env.CUBBY_API_URL)

      // Call MCP search tool via JSON-RPC 2.0
      const result = await callMcpTool(
        mcpUrl.toString(),
        accessToken,
        'search',
        {
          deviceId,
          q: q && q.trim() !== '' ? q : undefined,
          limit,
          content_type,
        }
      )

      // Extract structured content (SearchResponse)
      const searchResponse = result.structuredContent
      const textSummary = result.content.find(c => c.type === 'text')?.text || 'No summary available'

      // Format response as HTML
      const html = `
<div class="summary-container">
  <h3>MCP Tool Result</h3>
  <div class="summary-text">
    <strong>Tool:</strong> search<br/>
    <strong>Summary:</strong> ${escapeHtml(textSummary)}
  </div>
  
  <details>
    <summary style="cursor: pointer; margin-top: 1rem; padding: 0.5rem; background: #374151; color: white; border-radius: 0.25rem; user-select: none;">
      View Structured Response
    </summary>
    <pre style="margin-top: 0.5rem;">${escapeHtml(JSON.stringify(searchResponse, null, 2))}</pre>
  </details>
</div>
`
      
      return c.html(html)
    } catch (error) {
      console.error('MCP search error:', error)
      return c.text(`❌ MCP search failed: ${getErrorMessage(error)}`, 502)
    }
  }
)

app.post(
  '/search',
  zValidator('form', searchRequestSchema),
  async (c) => {
    const authHeader = c.req.header('Authorization')
    if (!authHeader) {
      return c.text('⚠️ Missing Authorization header', 401)
    }

    const { deviceId, q, limit, content_type } = c.req.valid('form')

    try {
      // Build search URL with query parameters matching screenpipe API
      const searchUrl = new URL(`/devices/${deviceId}/search`, c.env.CUBBY_API_URL)
      
      // Set query parameters
      if (q && q.trim() !== '') {
        // Remove FTS5 special characters that cause syntax errors
        // FTS5 uses: ? * + - " for special syntax, we strip them to avoid errors
        // This allows word-level matching while preventing syntax errors
        const sanitizedQuery = q.replace(/[?*+"'-]/g, ' ').trim()
        searchUrl.searchParams.set('q', sanitizedQuery)
        console.log(`[exampleco_website] Using text search with query: "${sanitizedQuery}"`)
      } else {
        // When q is empty, explicitly omit it (screenpipe returns recent activity chronologically)
        // Note: Not setting q parameter at all, as per OpenAPI spec where it's nullable
        console.log(`[exampleco_website] No search query - will return recent activity`)
      }
      searchUrl.searchParams.set('limit', limit.toString())
      searchUrl.searchParams.set('content_type', content_type)
      // Note: include_frames causes screenpipe to crash/hang, omitting for now
      
      console.log(`Proxying search request to: ${searchUrl.toString()}`)

      const response = await fetch(searchUrl.toString(), {
        method: 'GET',
        headers: {
          'Authorization': authHeader,
        },
      })

      console.log(`[exampleco_website] Response status: ${response.status}`)
      
      const body = await response.text()
      
      if (!response.ok) {
        console.error(`[exampleco_website] Error response body: ${body}`)
        return c.text(`❌ Error (${response.status}): ${body}`)
      }

      // Parse the screenpipe response
      let screenpipeData
      try {
        screenpipeData = JSON.parse(body)
      } catch {
        return c.text(body)
      }

      // Extract OCR text from the results for AI summarization
      const data = screenpipeData.data as any[]
      const ocrTexts = data
        ?.filter((item: any) => item.type === 'OCR')
        .map((item: any) => ({
          timestamp: item.content?.timestamp,
          app: item.content?.app_name,
          window: item.content?.window_name,
          text: item.content?.text?.slice(0, 500) // Limit to 500 chars per frame
        }))
        .slice(0, 5) // Only use first 5 frames for summarization

      // Generate AI summary if we have OCR data
      let summary = 'No context available to summarize.'
      
      if (ocrTexts && ocrTexts.length > 0) {
        try {
          const openai = new OpenAI({
            apiKey: c.env.OPENAI_API_KEY,
          })

          const contextText = ocrTexts
            .map((item: any, i: number) => 
              `[${i + 1}] ${item.app} - ${item.window}\n${item.text}\n`
            )
            .join('\n---\n')

          const completion = await openai.chat.completions.create({
            model: 'gpt-4o-mini',
            messages: [
              {
                role: 'system',
                content: 'You are a helpful assistant that summarizes screen content. Provide a brief 2-3 sentence summary of what the user is currently working on based on their screen captures.',
              },
              {
                role: 'user',
                content: `Here is the user's current screen context from their device:\n\n${contextText}\n\nSummarize in 2-3 sentences what they are currently working on.`,
              },
            ],
            temperature: 0.7,
            max_tokens: 150,
          })

          summary = completion.choices[0]?.message?.content || 'Failed to generate summary.'
        } catch (error) {
          console.error('[exampleco_website] OpenAI error:', error)
          summary = 'Failed to generate AI summary. Check API key configuration.'
        }
      }

      // Return HTML with both summary and raw data
      const rawDataJson = JSON.stringify(screenpipeData, null, 2)
      const html = `
<div class="summary-container">
  <h3>AI Summary</h3>
  <div class="summary-text">${escapeHtml(summary)}</div>
  
  <details>
    <summary style="cursor: pointer; margin-top: 1rem; padding: 0.5rem; background: #374151; color: white; border-radius: 0.25rem; user-select: none;">
      View Raw Data
    </summary>
    <pre style="margin-top: 0.5rem;">${escapeHtml(rawDataJson)}</pre>
  </details>
</div>
`
      
      return c.html(html)
    } catch (error) {
      console.error('Search proxy error:', error)
      return c.text(`❌ Failed to search: ${getErrorMessage(error)}`, 502)
    }
  }
)

export default app

