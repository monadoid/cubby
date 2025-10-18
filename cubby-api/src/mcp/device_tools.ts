/**
 * Device MCP Tools (curated)
 *
 * Exposes a curated set of device tools via MCP that mirrors the gateway OpenAPI
 * spec (excluding websocket). These tools require a selected device (via devices/set)
 * and proxy to the device's REST endpoints, returning the upstream JSON as
 * structuredContent.
 */

import { z } from "zod";
import type { Bindings } from "../index";
import { callDeviceRest } from "./device_client";

// -----------------------------
// Zod Schemas (mirror OpenAPI)
// -----------------------------

const emptySchema = z.object({});

const searchSchema = z.object({
  q: z.string().optional(),
  limit: z.number().int().optional(),
  offset: z.number().int().optional(),
  content_type: z
    .enum([
      "all",
      "ocr",
      "audio",
      "ui",
      "audio+ui",
      "ocr+ui",
      "audio+ocr",
    ])
    .optional(),
  start_time: z.string().optional(),
  end_time: z.string().optional(),
  app_name: z.string().optional(),
  window_name: z.string().optional(),
  include_frames: z.boolean().optional(),
  speaker_ids: z.array(z.number().int()).optional(),
});

const keywordSearchSchema = z.object({
  query: z.string(),
  limit: z.number().int().optional(),
  offset: z.number().int().optional(),
  fuzzy_match: z.boolean().optional(),
  start_time: z.string().optional(),
  end_time: z.string().optional(),
});

const semanticSearchSchema = z.object({
  text: z.string(),
  limit: z.number().int().optional(),
  threshold: z.number().optional(),
});

const frameGetSchema = z.object({
  frameId: z.number().int(),
});

const speakersSearchSchema = z.object({
  name: z.string().optional(),
});

const speakersSimilarSchema = z.object({
  speaker_id: z.number().int(),
  limit: z.number().int().optional(),
});

const speakersUnnamedSchema = z.object({
  limit: z.number().int().optional(),
  offset: z.number().int().optional(),
});

const tagsGetSchema = z.object({
  contentType: z.enum(["vision", "audio"]),
  id: z.number().int(),
});

const embeddingsSchema = z.object({
  model: z.string(),
  input: z.union([z.string(), z.array(z.string())]),
  encoding_format: z.string().optional(),
});

const addContentSchema = z.object({
  device_name: z.string(),
  content: z.object({
    content_type: z.enum(["vision", "audio"]),
    data: z.record(z.string(), z.any()),
  }),
});

const openApplicationSchema = z.object({
  app_name: z.string(),
});

const openUrlSchema = z.object({
  url: z.string(),
  browser: z.string().optional(),
});

const notifySchema = z.object({
  title: z.string(),
  body: z.string(),
});

// -----------------------------
// Tool Definitions
// -----------------------------

export const DEVICE_TOOLS = [
  {
    name: "device/health",
    description: "[device] health check",
    inputSchema: z.toJSONSchema(emptySchema),
  },
  {
    name: "device/search",
    description: "[device] search content",
    inputSchema: z.toJSONSchema(searchSchema),
  },
  {
    name: "device/search-keyword",
    description: "[device] keyword search",
    inputSchema: z.toJSONSchema(keywordSearchSchema),
  },
  {
    name: "device/semantic-search",
    description: "[device] semantic search",
    inputSchema: z.toJSONSchema(semanticSearchSchema),
  },
  {
    name: "device/audio/list",
    description: "[device] list audio devices",
    inputSchema: z.toJSONSchema(emptySchema),
  },
  {
    name: "device/vision/list",
    description: "[device] list monitors",
    inputSchema: z.toJSONSchema(emptySchema),
  },
  {
    name: "device/frames/get",
    description: "[device] get frame data",
    inputSchema: z.toJSONSchema(frameGetSchema),
  },
  {
    name: "device/speakers/search",
    description: "[device] search speakers",
    inputSchema: z.toJSONSchema(speakersSearchSchema),
  },
  {
    name: "device/speakers/similar",
    description: "[device] find similar speakers",
    inputSchema: z.toJSONSchema(speakersSimilarSchema),
  },
  {
    name: "device/speakers/unnamed",
    description: "[device] get unnamed speakers",
    inputSchema: z.toJSONSchema(speakersUnnamedSchema),
  },
  {
    name: "device/tags/get",
    description: "[device] get tags",
    inputSchema: z.toJSONSchema(tagsGetSchema),
  },
  {
    name: "device/embeddings",
    description: "[device] generate embeddings",
    inputSchema: z.toJSONSchema(embeddingsSchema),
  },
  {
    name: "device/add",
    description: "[device] add content",
    inputSchema: z.toJSONSchema(addContentSchema),
  },
  {
    name: "device/open-application",
    description: "[device] open application",
    inputSchema: z.toJSONSchema(openApplicationSchema),
  },
  {
    name: "device/open-url",
    description: "[device] open url",
    inputSchema: z.toJSONSchema(openUrlSchema),
  },
  {
    name: "device/notify",
    description: "[device] send notification",
    inputSchema: z.toJSONSchema(notifySchema),
  },
];

// -----------------------------
// Dispatcher
// -----------------------------

export async function callDeviceTool(
  env: Bindings,
  deviceId: string,
  userId: string,
  gwSessionId: string,
  name: string,
  args: any,
) {
  // Validate and map to REST
  switch (name) {
    case "device/health": {
      emptySchema.parse(args || {});
      const resp = await callDeviceRest(env, deviceId, "GET", "/health", { userId, gwSessionId });
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "device health fetched" }], structuredContent: json };
    }
    case "device/search": {
      const parsed = searchSchema.parse(args || {});
      const qs = new URLSearchParams();
      if (parsed.q) qs.set("q", parsed.q);
      if (parsed.limit !== undefined) qs.set("limit", String(parsed.limit));
      if (parsed.offset !== undefined) qs.set("offset", String(parsed.offset));
      if (parsed.content_type) qs.set("content_type", parsed.content_type);
      if (parsed.start_time) qs.set("start_time", parsed.start_time);
      if (parsed.end_time) qs.set("end_time", parsed.end_time);
      if (parsed.app_name) qs.set("app_name", parsed.app_name);
      if (parsed.window_name) qs.set("window_name", parsed.window_name);
      if (parsed.include_frames !== undefined) qs.set("include_frames", String(parsed.include_frames));
      if (parsed.speaker_ids) parsed.speaker_ids.forEach((v) => qs.append("speaker_ids", String(v)));
      const resp = await callDeviceRest(env, deviceId, "GET", `/search?${qs.toString()}`, { userId, gwSessionId });
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "device search results" }], structuredContent: json };
    }
    case "device/search-keyword": {
      const parsed = keywordSearchSchema.parse(args || {});
      const qs = new URLSearchParams();
      qs.set("query", parsed.query);
      if (parsed.limit !== undefined) qs.set("limit", String(parsed.limit));
      if (parsed.offset !== undefined) qs.set("offset", String(parsed.offset));
      if (parsed.fuzzy_match !== undefined) qs.set("fuzzy_match", String(parsed.fuzzy_match));
      if (parsed.start_time) qs.set("start_time", parsed.start_time);
      if (parsed.end_time) qs.set("end_time", parsed.end_time);
      const resp = await callDeviceRest(env, deviceId, "GET", `/search/keyword?${qs.toString()}`, { userId, gwSessionId });
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "keyword search results" }], structuredContent: json };
    }
    case "device/semantic-search": {
      const parsed = semanticSearchSchema.parse(args || {});
      const qs = new URLSearchParams();
      qs.set("text", parsed.text);
      if (parsed.limit !== undefined) qs.set("limit", String(parsed.limit));
      if (parsed.threshold !== undefined) qs.set("threshold", String(parsed.threshold));
      const resp = await callDeviceRest(env, deviceId, "GET", `/semantic-search?${qs.toString()}`, { userId, gwSessionId });
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "semantic search results" }], structuredContent: json };
    }
    case "device/audio/list": {
      emptySchema.parse(args || {});
      const resp = await callDeviceRest(env, deviceId, "GET", "/audio/list", { userId, gwSessionId });
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "audio devices listed" }], structuredContent: json };
    }
    case "device/vision/list": {
      emptySchema.parse(args || {});
      const resp = await callDeviceRest(env, deviceId, "GET", "/vision/list", { userId, gwSessionId });
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "monitors listed" }], structuredContent: json };
    }
    case "device/frames/get": {
      const parsed = frameGetSchema.parse(args || {});
      const resp = await callDeviceRest(env, deviceId, "GET", `/frames/${parsed.frameId}`, { userId, gwSessionId });
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "frame retrieved" }], structuredContent: json };
    }
    case "device/speakers/search": {
      const parsed = speakersSearchSchema.parse(args || {});
      const qs = new URLSearchParams();
      if (parsed.name) qs.set("name", parsed.name);
      const resp = await callDeviceRest(env, deviceId, "GET", `/speakers/search?${qs.toString()}`, { userId, gwSessionId });
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "speakers search results" }], structuredContent: json };
    }
    case "device/speakers/similar": {
      const parsed = speakersSimilarSchema.parse(args || {});
      const qs = new URLSearchParams();
      qs.set("speaker_id", String(parsed.speaker_id));
      if (parsed.limit !== undefined) qs.set("limit", String(parsed.limit));
      const resp = await callDeviceRest(env, deviceId, "GET", `/speakers/similar?${qs.toString()}`, { userId, gwSessionId });
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "similar speakers" }], structuredContent: json };
    }
    case "device/speakers/unnamed": {
      const parsed = speakersUnnamedSchema.parse(args || {});
      const qs = new URLSearchParams();
      if (parsed.limit !== undefined) qs.set("limit", String(parsed.limit));
      if (parsed.offset !== undefined) qs.set("offset", String(parsed.offset));
      const resp = await callDeviceRest(env, deviceId, "GET", `/speakers/unnamed?${qs.toString()}`, { userId, gwSessionId });
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "unnamed speakers" }], structuredContent: json };
    }
    case "device/tags/get": {
      const parsed = tagsGetSchema.parse(args || {});
      const resp = await callDeviceRest(env, deviceId, "GET", `/tags/${parsed.contentType}/${parsed.id}`, { userId, gwSessionId });
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "tags fetched" }], structuredContent: json };
    }
    case "device/embeddings": {
      const parsed = embeddingsSchema.parse(args || {});
      const resp = await callDeviceRest(env, deviceId, "POST", "/v1/embeddings", { userId, gwSessionId }, parsed);
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "embeddings generated" }], structuredContent: json };
    }
    case "device/add": {
      const parsed = addContentSchema.parse(args || {});
      const resp = await callDeviceRest(env, deviceId, "POST", "/add", { userId, gwSessionId }, parsed);
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "content added" }], structuredContent: json };
    }
    case "device/open-application": {
      const parsed = openApplicationSchema.parse(args || {});
      const resp = await callDeviceRest(env, deviceId, "POST", "/open-application", { userId, gwSessionId }, parsed);
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "application opened" }], structuredContent: json };
    }
    case "device/open-url": {
      const parsed = openUrlSchema.parse(args || {});
      const resp = await callDeviceRest(env, deviceId, "POST", "/open-url", { userId, gwSessionId }, parsed);
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "url opened" }], structuredContent: json };
    }
    case "device/notify": {
      const parsed = notifySchema.parse(args || {});
      const resp = await callDeviceRest(env, deviceId, "POST", "/notify", { userId, gwSessionId }, parsed);
      const json = await resp.json();
      return { content: [{ type: "text" as const, text: "notification sent" }], structuredContent: json };
    }
    default: {
      throw new Error(`unknown device tool: ${name}`);
    }
  }
}


