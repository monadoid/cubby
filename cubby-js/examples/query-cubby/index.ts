import 'dotenv/config';
import { createClient } from "@cubby/js";

async function queryCubby() {
  const baseUrl = process.env.CUBBY_API_BASE_URL;
  const token = process.env.CUBBY_API_TOKEN;
  if (!baseUrl || !token) {
    console.error('error: set CUBBY_API_BASE_URL and CUBBY_API_TOKEN in .env');
    process.exit(1);
  }

  console.log("starting query cubby...");
  console.log("------------------------------");
  console.log("querying last 5 minutes of activity...");
  console.log("------------------------------");

  // get content from last 5 minutes
  const fiveMinutesAgo = new Date(Date.now() - 5 * 60 * 1000).toISOString();

  const client = createClient({ baseUrl, token });
  const devices = await client.listDevices();
  if (!devices?.devices?.length) {
    console.error("no devices found");
    process.exit(1);
  }
  client.setDeviceId(String(devices.devices[0].id));
  const results = await client.search({
    startTime: fiveMinutesAgo,
    limit: 10,
    contentType: "all", // can be "ocr", "audio", "ui", or "all"
  });

  if (!results) {
    console.log("no results found or error occurred");
    return;
  }

  console.log(`found ${results.pagination.total} items`);

  // process each result
  for (const item of results.data) {
    console.log("\n--- new item ---");
    console.log(`type: ${item.type}`);
    console.log(`timestamp: ${item.content.timestamp}`);

    if (item.type === "OCR") {
      console.log(`OCR: ${JSON.stringify(item.content)}`);
    } else if (item.type === "Audio") {
      console.log(`transcript: ${JSON.stringify(item.content)}`);
    } else if (item.type === "UI") {
      console.log(`UI: ${JSON.stringify(item.content)}`);
    }

    // here you could send to openai or other ai service
    // example pseudo-code:
    // const aiResponse = await openai.chat.completions.create({
    //   messages: [{ role: "user", content: item.content }],
    //   model: "gpt-4"
    // });
  }
}

queryCubby().catch((e) => {
  console.error(e);
  process.exit(1);
});
