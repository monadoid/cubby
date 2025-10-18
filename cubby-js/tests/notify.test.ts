import { describe, it, expect } from "vitest";
import { createClient } from "../src";

describe("notify", () => {
  it("posts to /notify", async () => {
    let posted = false;
    const fetchImpl = async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      if (url.includes("/notify") && init?.method === "POST") {
        posted = true;
        return new Response(JSON.stringify({ success: true }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      }
      return new Response("not found", { status: 404 });
    };
    const client = createClient({ fetchImpl });
    const res = await client.notify({ title: "hi", body: "test" } as any);
    expect(posted).toBe(true);
    expect(res.success).toBe(true);
  });
});


