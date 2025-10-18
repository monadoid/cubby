import "dotenv/config";
import { createClient } from "@cubby/js";

async function monitorEvents() {
  console.log("starting event stream...");

  const baseUrl = process.env.CUBBY_API_BASE_URL;
  const token = process.env.CUBBY_API_TOKEN;

  if (!baseUrl || !token) {
    console.error("error: CUBBY_API_BASE_URL and CUBBY_API_TOKEN must be set in .env");
    process.exit(1);
  }

  const client = createClient({ baseUrl, token });
  const devices = await client.listDevices();
  if (!devices?.devices?.length) {
    console.error("error: no devices found");
    process.exit(1);
  }
  client.setDeviceId(String(devices.devices[0].id));

  // no filtering: stream everything from the device as { name, data }
  // example event: { name: "ocr_result", data: { app_name, text, ... } }
  for await (const evt of client.streamEvents()) {
    // logs: [event_name] {...data}
    console.log(`[${evt?.name}] ${JSON.stringify(evt?.data)}`);
  }

  // how to filter only transcriptions (if your device emits them):
  // for await (const evt of client.streamEvents()) {
  //   if (evt?.name === "transcription") {
  //     // typical shape: { name: "transcription", data: { text: string, is_final?: boolean, ts?: number, ... } }
  //     const { text, is_final } = evt.data || {};
  //     console.log(`[transcription] ${is_final ? "final:" : "partial:"} ${text || ""}`);
  //   }
  // }

  // how to filter only vision/ocr frames:
  // for await (const evt of client.streamEvents()) {
  //   if (evt?.name === "ocr_result" || evt?.name === "ui_frame") {
  //     // ocr_result example (from ws): { name: "ocr_result", data: { app_name: string, text: string, confidence: number, ... } }
  //     const { app_name, text, confidence } = evt.data || {};
  //     console.log(`[ocr] app=${app_name || "unknown"} conf=${confidence ?? "?"} text=${(text || "").slice(0, 120)}`);
  //   }
  // }
}

monitorEvents().catch(console.error);
