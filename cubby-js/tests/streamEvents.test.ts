import { describe, it, expect } from "vitest";
import { createClient } from "../src";
import { WebSocketServer } from "ws";

describe("streamEvents", () => {
  it("receives events over ws", async () => {
    const wss = new WebSocketServer({ port: 8788 });
    wss.on("connection", (ws) => {
      ws.send(JSON.stringify({ name: "transcription", data: { transcription: "hi", is_final: true } }));
      ws.send(JSON.stringify({ name: "ocr_result", data: { text: "hello", timestamp: Date.now() } }));
    });

    const client = createClient({ env: { CUBBY_API_BASE_URL: "http://localhost:8788" } });
    const received: any[] = [];
    for await (const evt of client.streamEvents()) {
      received.push(evt);
      if (received.length === 2) break;
    }
    expect(received.length).toBe(2);
    wss.close();
  }, 10000);
});


