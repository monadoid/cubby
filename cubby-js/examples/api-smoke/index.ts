import 'dotenv/config';
import { createClient } from '@cubby/js';

async function main() {
  const baseUrl = process.env.CUBBY_API_BASE_URL;
  const token = process.env.CUBBY_API_TOKEN;
  if (!baseUrl || !token) {
    console.error('error: set CUBBY_API_BASE_URL and CUBBY_API_TOKEN in .env');
    process.exit(1);
  }

  const client = createClient({ baseUrl, token });

  console.log('whoami:');
  const who = await fetch(new URL('/whoami', baseUrl).toString(), {
    headers: { Authorization: `Bearer ${token}` },
  }).then((r) => r.json());
  console.log(who);

  console.log('devices:');
  const devices = await client.listDevices();
  console.log(devices);
  if (!devices?.devices?.length) {
    throw new Error('no devices found');
  }
  const deviceId = String(devices.devices[0].id);
  client.setDeviceId(deviceId);
  console.log('using deviceId:', deviceId);

  console.log('search:');
  const res = await client.search({ q: 'hello', limit: 1 });
  console.log(JSON.stringify(res, null, 2));

}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
