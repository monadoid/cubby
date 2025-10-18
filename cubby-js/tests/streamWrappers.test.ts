import { describe, it, expect } from "vitest";
import { createClient } from "../src";
import { WebSocketServer } from "ws";

describe("stream wrappers", () => {
  it("streamTranscriptions filters transcription events", async () => {
    const wss = new WebSocketServer({ port: 8789 });
    wss.on("connection", (ws) => {
      ws.send(JSON.stringify({ name: "noop", data: {} }));
      ws.send(JSON.stringify({ name: "transcription", data: { transcription: "ok", is_final: true } }));
    });
    const client = createClient({ env: { CUBBY_API_BASE_URL: "http://localhost:8789" } });
    const got: any[] = [];
    for await (const evt of client.streamTranscriptions()) {
      got.push(evt);
      break;
    }
    expect(got.length).toBe(1);
    wss.close();
  });

  it("streamVision filters ocr/ui events", async () => {
    const wss = new WebSocketServer({ port: 8790 });
    wss.on("connection", (ws) => {
      ws.send(JSON.stringify({ name: "ocr_result", data: { text: "hi", timestamp: Date.now() } }));
    });
    const client = createClient({ env: { CUBBY_API_BASE_URL: "http://localhost:8790" } });
    const got: any[] = [];
    for await (const evt of client.streamVision()) {
      got.push(evt);
      break;
    }
    expect(got.length).toBe(1);
    wss.close();
  });
});


