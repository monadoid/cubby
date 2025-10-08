import { Hono } from "hono";
import { zValidator } from "@hono/zod-validator";
import { z } from "zod";
import OpenAI from "openai";
import type { Bindings, Variables } from "../index";
import { renderDevicesFragment } from "../views/devices_fragment";
import { callMcpTool } from "../lib/mcp_client";

// Schemas matching screenpipe OpenAPI spec
const contentTypeSchema = z
  .enum(["all", "ocr", "audio", "ui", "audio+ui", "ocr+ui", "audio+ocr"])
  .optional();

const searchRequestSchema = z.object({
  deviceId: z.string().min(1, "Device ID is required"),
  q: z.string().optional().default(""),
  limit: z
    .string()
    .optional()
    .transform((val: string | undefined) => {
      if (!val || val === "") return 10;
      const num = Number(val);
      return Number.isNaN(num) ? 10 : num;
    }),
  content_type: contentTypeSchema.default("all"),
});

type SearchRequest = z.infer<typeof searchRequestSchema>;

const app = new Hono<{ Bindings: Bindings; Variables: Variables }>();

// Error message helper
function getErrorMessage(error: unknown): string {
  if (error instanceof DOMException && error.name === "AbortError") {
    return "Request timed out";
  }

  if (error instanceof Error) {
    return error.message;
  }

  return "Unknown error";
}

// HTML escape helper
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function sanitizeSearchQuery(query: string): string {
  // Remove FTS5 special characters that cause syntax errors
  // FTS5 uses: ? * + - " for special syntax, we strip them to avoid errors
  return query.replace(/[?*+"'-]/g, " ").trim();
}

// Server-side device list HTML fragment
app.get("/devices-fragment", async (c) => {
  const authHeader = c.req.header("Authorization");
  if (!authHeader) {
    console.error("[devices-fragment] No Authorization header provided");
    return c.text("Missing Authorization header", 401);
  }

  try {
    const devicesUrl = new URL("/devices", c.env.CUBBY_API_URL);
    console.log(
      `[devices-fragment] Fetching devices from: ${devicesUrl.toString()}`,
    );

    const response = await fetch(devicesUrl.toString(), {
      headers: { Authorization: authHeader },
    });

    console.log(`[devices-fragment] Response status: ${response.status}`);

    if (!response.ok) {
      const error = await response.text();
      console.error("[devices-fragment] Failed to load devices:", error);
      return c.text(`Failed to load devices: ${error}`, 502);
    }

    const data = (await response.json()) as { devices: unknown[] };
    console.log(
      `[devices-fragment] Loaded ${data.devices?.length || 0} devices`,
    );

    return c.html(renderDevicesFragment((data.devices as unknown[]) || []));
  } catch (error) {
    console.error("[devices-fragment] Error loading devices:", error);
    return c.text(`Error loading devices: ${getErrorMessage(error)}`, 500);
  }
});

// MCP search endpoint - uses JSON-RPC 2.0 to call MCP tools
app.post(
  "/mcp-search",
  zValidator("form", searchRequestSchema),
  async (c) => {
    const authHeader = c.req.header("Authorization");
    if (!authHeader) {
      return c.text("⚠️ Missing Authorization header", 401);
    }

    const { deviceId, q, limit, content_type } = c.req.valid("form");
    const accessToken = authHeader.replace(/^Bearer\s+/i, "");

    try {
      const mcpUrl = new URL("/mcp", c.env.CUBBY_API_URL);

      const result = await callMcpTool(
        mcpUrl.toString(),
        accessToken,
        "search",
        {
          deviceId,
          q: q && q.trim() !== "" ? q : undefined,
          limit,
          content_type,
        },
      );

      const searchResponse = result.structuredContent;
      const textSummary =
        result.content.find((content) => content.type === "text")?.text ||
        "No summary available";

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
`;

      return c.html(html);
    } catch (error) {
      console.error("MCP search error:", error);
      return c.text(`❌ MCP search failed: ${getErrorMessage(error)}`, 502);
    }
  },
);

// Screenpipe REST search proxy
app.post(
  "/search",
  zValidator("form", searchRequestSchema),
  async (c) => {
    const authHeader = c.req.header("Authorization");
    if (!authHeader) {
      return c.text("⚠️ Missing Authorization header", 401);
    }

    const { deviceId, q, limit, content_type } = c.req.valid("form");

    try {
      // Build search URL with query parameters matching screenpipe API
      const searchUrl = new URL(
        `/devices/${deviceId}/search`,
        c.env.CUBBY_API_URL,
      );

      if (q && q.trim() !== "") {
        const sanitizedQuery = sanitizeSearchQuery(q);
        searchUrl.searchParams.set("q", sanitizedQuery);
        console.log(
          `[exampleco_website] Using text search with query: "${sanitizedQuery}"`,
        );
      } else {
        console.log(
          `[exampleco_website] No search query - will return recent activity`,
        );
      }

      searchUrl.searchParams.set("limit", limit.toString());
      searchUrl.searchParams.set("content_type", content_type);

      console.log(`Proxying search request to: ${searchUrl.toString()}`);

      const response = await fetch(searchUrl.toString(), {
        method: "GET",
        headers: {
          Authorization: authHeader,
        },
      });

      console.log(`[exampleco_website] Response status: ${response.status}`);

      const body = await response.text();

      if (!response.ok) {
        console.error(`[exampleco_website] Error response body: ${body}`);
        return c.text(`❌ Error (${response.status}): ${body}`);
      }

      let screenpipeData: unknown;
      try {
        screenpipeData = JSON.parse(body) as Record<string, unknown>;
      } catch {
        return c.text(body);
      }

      const data = (screenpipeData as { data?: unknown[] }).data as any[];
      const ocrTexts = data
        ?.filter((item: any) => item.type === "OCR")
        .map((item: any) => ({
          timestamp: item.content?.timestamp,
          app: item.content?.app_name,
          window: item.content?.window_name,
          text: item.content?.text?.slice(0, 500),
        }))
        .slice(0, 5);

      let summary = "No context available to summarize.";

      if (ocrTexts && ocrTexts.length > 0) {
        try {
          const openai = new OpenAI({
            apiKey: c.env.OPENAI_API_KEY,
          });

          const contextText = ocrTexts
            .map(
              (item: any, index: number) =>
                `[${index + 1}] ${item.app} - ${item.window}\n${item.text}\n`,
            )
            .join("\n---\n");

          const completion = await openai.chat.completions.create({
            model: "gpt-4o-mini",
            messages: [
              {
                role: "system",
                content:
                  "You are a helpful assistant that summarizes screen content. Provide a brief 2-3 sentence summary of what the user is currently working on based on their screen captures.",
              },
              {
                role: "user",
                content: `Here is the user's current screen context from their device:\n\n${contextText}\n\nSummarize in 2-3 sentences what they are currently working on.`,
              },
            ],
            temperature: 0.7,
            max_tokens: 150,
          });

          summary =
            completion.choices[0]?.message?.content ||
            "Failed to generate summary.";
        } catch (error) {
          console.error("[exampleco_website] OpenAI error:", error);
          summary =
            "Failed to generate AI summary. Check API key configuration.";
        }
      }

      const rawDataJson = JSON.stringify(screenpipeData, null, 2);
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
`;

      return c.html(html);
    } catch (error) {
      console.error("Search proxy error:", error);
      return c.text(`❌ Failed to search: ${getErrorMessage(error)}`, 502);
    }
  },
);

export default app;
