import { describe, it, expect } from "vitest";
import { createClient } from "../src";

describe("search endpoints", () => {
  it("search returns data", async () => {
    const fetchImpl = async (input: RequestInfo | URL) => {
      const url = typeof input === "string" ? input : input.toString();
      if (url.includes("/search?")) {
        return new Response(
          JSON.stringify({ data: [], pagination: { total: 0 } }),
          { status: 200, headers: { "Content-Type": "application/json" } }
        );
      }
      return new Response("not found", { status: 404 });
    };
    const client = createClient({ fetchImpl });
    const res = await client.search({ contentType: "all", limit: 1 });
    expect(res).toBeDefined();
    expect(res.pagination.total).toBeDefined();
  });

  it("speakersSearch returns ok", async () => {
    const fetchImpl = async (input: RequestInfo | URL) => {
      const url = typeof input === "string" ? input : input.toString();
      if (url.includes("/speakers/search")) {
        return new Response(JSON.stringify({ data: [] }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      }
      return new Response("not found", { status: 404 });
    };
    const client = createClient({ fetchImpl });
    const res = await client.speakersSearch({ q: "alice" });
    expect(res).toBeDefined();
    expect(Array.isArray(res.data)).toBe(true);
  });

  it("semanticSearch returns ok", async () => {
    const fetchImpl = async (input: RequestInfo | URL) => {
      const url = typeof input === "string" ? input : input.toString();
      if (url.includes("/semantic-search")) {
        return new Response(JSON.stringify({ data: [] }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      }
      return new Response("not found", { status: 404 });
    };
    const client = createClient({ fetchImpl });
    const res = await client.semanticSearch({ q: "test" });
    expect(res).toBeDefined();
    expect(Array.isArray(res.data)).toBe(true);
  });
});


