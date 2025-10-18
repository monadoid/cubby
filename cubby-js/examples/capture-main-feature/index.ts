import 'dotenv/config';
import { createClient } from "@cubby/js";

async function start() {
  const baseUrl = process.env.CUBBY_API_BASE_URL;
  const token = process.env.CUBBY_API_TOKEN;
  if (!baseUrl || !token) {
    console.error('error: set CUBBY_API_BASE_URL and CUBBY_API_TOKEN in .env');
    process.exit(1);
  }

  console.log("sending demo notifications via gateway...");
  const client = createClient({ baseUrl, token });
  await client.notify({ title: "less useful feature", body: "dog: woof" } as any);
  await client.notify({ title: "very useful feature", body: "cat: meow" } as any);
}

start().catch(console.error);
