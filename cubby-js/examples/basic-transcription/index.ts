import "dotenv/config";
import { createClient } from "@cubby/js";

async function monitorTranscriptions() {
  console.log("starting transcription monitor...");

  const baseUrl = process.env.CUBBY_API_BASE_URL;
  const token = process.env.CUBBY_API_TOKEN;

  if (!baseUrl || !token) {
    console.error("error: CUBBY_API_BASE_URL and CUBBY_API_TOKEN must be set in .env");
    process.exit(1);
  }

  const client = createClient({ baseUrl, token });
  for await (const chunk of client.streamTranscriptions()) {
    const text = chunk.choices[0].text;
    const isFinal = chunk.choices[0].finish_reason === "stop";
    const device = chunk.metadata?.device;

    console.log(`[${device}] ${isFinal ? "final:" : "partial:"} ${text}`);
  }
}

monitorTranscriptions().catch(console.error);
