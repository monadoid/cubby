/**
 * Screenpipe API schemas
 *
 * Type-safe Zod schemas matching the Screenpipe OpenAPI specification.
 * These schemas ensure proper validation and type safety when proxying
 * requests to Screenpipe devices.
 */

import { z } from "zod";

// Content type enum
export const ContentTypeSchema = z.enum([
  "all",
  "ocr",
  "audio",
  "ui",
  "audio+ui",
  "ocr+ui",
  "audio+ocr",
]);

export type ContentType = z.infer<typeof ContentTypeSchema>;

// Device type enum
export const DeviceTypeSchema = z.enum(["Input", "Output"]);

export type DeviceType = z.infer<typeof DeviceTypeSchema>;

// Speaker schema
export const SpeakerSchema = z.object({
  id: z.number(),
  name: z.string().nullable(),
});

export type Speaker = z.infer<typeof SpeakerSchema>;

// OCR content schema
export const OCRContentSchema = z.object({
  type: z.literal("OCR"),
  content: z.object({
    frame_id: z.number(),
    text: z.string(),
    timestamp: z.string().datetime(),
    file_path: z.string(),
    offset_index: z.number(),
    app_name: z.string(),
    window_name: z.string(),
    tags: z.array(z.string()),
    frame: z.string().nullable(),
    frame_name: z.string().nullable(),
    browser_url: z.string().nullable(),
    focused: z.boolean().nullable(),
  }),
});

export type OCRContent = z.infer<typeof OCRContentSchema>;

// Audio content schema
export const AudioContentSchema = z.object({
  type: z.literal("Audio"),
  content: z.object({
    chunk_id: z.number(),
    transcription: z.string(),
    timestamp: z.string().datetime(),
    file_path: z.string(),
    offset_index: z.number(),
    tags: z.array(z.string()),
    device_name: z.string(),
    device_type: DeviceTypeSchema,
    speaker: SpeakerSchema,
    start_time: z.number().nullable(),
    end_time: z.number().nullable(),
  }),
});

export type AudioContent = z.infer<typeof AudioContentSchema>;

// UI content schema
export const UIContentSchema = z.object({
  type: z.literal("UI"),
  content: z.object({
    id: z.number(),
    text: z.string(),
    timestamp: z.string().datetime(),
    app_name: z.string(),
    window_name: z.string(),
    initial_traversal_at: z.string().datetime().nullable(),
    file_path: z.string(),
    offset_index: z.number(),
    frame_name: z.string().nullable(),
    browser_url: z.string().nullable(),
  }),
});

export type UIContent = z.infer<typeof UIContentSchema>;

// Content item discriminated union
export const ContentItemSchema = z.discriminatedUnion("type", [
  OCRContentSchema,
  AudioContentSchema,
  UIContentSchema,
]);

export type ContentItem = z.infer<typeof ContentItemSchema>;

// Search response schema (normalized to { results, total })
const SearchResponseResultsSchema = z.object({
  results: z.array(ContentItemSchema),
  total: z.number(),
});

export type SearchResponse = z.infer<typeof SearchResponseResultsSchema>;

// Some Screenpipe deployments return a `data` array instead of `results`
const SearchResponseDataSchema = z.object({
  data: z.array(ContentItemSchema),
  total: z.number(),
});

const SearchResponseUnionSchema = z.union([
  SearchResponseResultsSchema,
  SearchResponseDataSchema,
]);

export const SearchResponseSchema = SearchResponseResultsSchema;

export function normalizeSearchResponse(input: unknown): SearchResponse {
  const parsed = SearchResponseUnionSchema.parse(input);
  if ("results" in parsed) {
    return parsed;
  }

  return {
    results: parsed.data,
    total: parsed.total,
  };
}

// Search query parameters schema
export const SearchQuerySchema = z.object({
  q: z.string().optional(),
  limit: z.number().optional(),
  offset: z.number().optional(),
  content_type: ContentTypeSchema.optional(),
  start_time: z.string().datetime().optional(),
  end_time: z.string().datetime().optional(),
  app_name: z.string().optional(),
  window_name: z.string().optional(),
  frame_name: z.string().optional(),
  include_frames: z.boolean().optional(),
  min_length: z.number().optional(),
  max_length: z.number().optional(),
  speaker_ids: z.array(z.number()).optional(),
  focused: z.boolean().optional(),
  browser_url: z.string().optional(),
});

export type SearchQuery = z.infer<typeof SearchQuerySchema>;

// MCP tool input schema (includes deviceId)
export const SearchToolInputSchema = SearchQuerySchema.extend({
  deviceId: z.string().min(1, "deviceId is required"),
});

export type SearchToolInput = z.infer<typeof SearchToolInputSchema>;
