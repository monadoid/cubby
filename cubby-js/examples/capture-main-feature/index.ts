import 'dotenv/config';
import { createClient } from "@cubby/js";

async function start() {
  const baseUrl = process.env.CUBBY_API_BASE_URL;
  const clientId = process.env.CUBBY_CLIENT_ID;
  const clientSecret = process.env.CUBBY_CLIENT_SECRET;
  
  if (!baseUrl || !clientId || !clientSecret) {
    console.error('error: set CUBBY_API_BASE_URL, CUBBY_CLIENT_ID, and CUBBY_CLIENT_SECRET in .env');
    console.error('get credentials at https://cubby.sh/dashboard');
    process.exit(1);
  }

  console.log("sending demo notifications via gateway...");
  const client = createClient({ baseUrl, clientId, clientSecret });
  const devices = await client.listDevices();
  if (!devices?.devices?.length) {
    console.error('error: no devices found');
    process.exit(1);
  }
  client.setDeviceId(String(devices.devices[0].id));
  await client.notify({ title: "less useful feature", body: "dog: woof" } as any);
  await client.notify({ title: "very useful feature", body: "cat: meow" } as any);
}

start().catch(console.error);
