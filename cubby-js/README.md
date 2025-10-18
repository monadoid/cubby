# @cubby/js

typescript sdk for [cubby](https://cubby.sh) - capture everything, search anything, automate everywhere.

works in node, cloudflare workers, and browsers.

## installation

```bash
npm i @cubby/js
```

## quick start

### local usage

```typescript
import { createClient } from '@cubby/js';

const client = createClient({ baseUrl: 'http://localhost:3030' });

// search your screen and audio history
const results = await client.search({ 
  q: 'project deadline', 
  limit: 10 
});

console.log(`found ${results.pagination.total} results`);
```

### remote usage

```typescript
import { createClient } from '@cubby/js';

const client = createClient({ 
  baseUrl: 'https://api.cubby.sh',
  token: 'your-oauth-token'
});

// list devices and select one
const { devices } = await client.listDevices();
client.setDeviceId(devices[0].id);

// now all methods use the selected device
await client.search({ q: 'hello' });
await client.notify({ title: 'test', body: 'works!' });
```

## api reference

### client creation

```typescript
import { createClient } from '@cubby/js';

const client = createClient({
  baseUrl: 'http://localhost:3030', // or 'https://api.cubby.sh'
  token: 'your-token', // optional for local, required for remote
  tokenProvider: async () => 'dynamic-token', // alternative to static token
});
```

### device management

```typescript
// list all enrolled devices
const { devices } = await client.listDevices();
// => { devices: [{ id: 'device-123' }, ...] }

// set device for subsequent calls (remote only)
client.setDeviceId('device-123');

// clear selected device
client.clearDeviceId();
```

### search

```typescript
// unified search across screen, audio, and ui
const results = await client.search({
  q: 'search query',
  limit: 50,
  offset: 0,
  content_type: 'all', // 'ocr' | 'audio' | 'ui' | 'all'
  start_time: '2024-01-01T00:00:00Z',
  end_time: '2024-01-02T00:00:00Z',
  app_name: 'Slack',
  window_name: 'general',
  include_frames: false,
  speaker_ids: [1, 2],
  deviceId: 'device-123' // optional override
});

// speaker search
const speakers = await client.speakersSearch({
  name: 'john',
  deviceId: 'device-123' // optional
});
```

### streaming

```typescript
// stream all events (transcriptions + vision)
for await (const event of client.streamEvents(false)) {
  console.log(event.name, event.data);
}

// stream transcriptions only
for await (const event of client.streamTranscriptions()) {
  console.log('transcription:', event.text);
}

// stream vision events (ocr + ui frames)
for await (const event of client.streamVision(false)) {
  if (event.name === 'ocr_result') {
    console.log('ocr:', event.data.text);
  }
}

// include images in stream
for await (const event of client.streamVision(true)) {
  console.log(event.data.image); // base64 image data
}
```

### device automation

```typescript
// open applications
await client.device.openApplication('Slack');
await client.device.openApplication('Visual Studio Code');

// open urls in browser
await client.device.openUrl('https://github.com');
await client.device.openUrl('https://google.com', 'Safari');

// send desktop notifications
await client.notify({
  title: 'reminder',
  body: 'meeting in 5 minutes'
});
```

### configuration

```typescript
// update base url
client.setBaseUrl('https://api.cubby.sh');

// update auth token
client.setAuthToken('new-token');

// set token provider for dynamic tokens
client.setTokenProvider(async () => {
  // fetch token from your auth system
  return await getToken();
});
```

## environment variables

the sdk automatically reads from environment variables:

```bash
# node / bun
export CUBBY_API_BASE_URL="https://api.cubby.sh"
export CUBBY_API_TOKEN="your-token"
```

```typescript
// cloudflare workers / browser
globalThis.__CUBBY_ENV__ = {
  CUBBY_API_BASE_URL: 'https://api.cubby.sh',
  CUBBY_API_TOKEN: 'your-token'
};

// then create client without config
const client = createClient();
```

## examples

### basic search

```typescript
import { createClient } from '@cubby/js';

const client = createClient({ baseUrl: 'http://localhost:3030' });

const results = await client.search({ 
  q: 'api keys',
  content_type: 'ocr',
  limit: 10 
});

for (const item of results.data) {
  console.log(`[${item.content.timestamp}] ${item.content.text}`);
}
```

### real-time transcription

```typescript
import { createClient } from '@cubby/js';

const client = createClient({ baseUrl: 'http://localhost:3030' });

console.log('listening for transcriptions...');

for await (const event of client.streamTranscriptions()) {
  console.log(event.text);
}
```

### remote device control

```typescript
import { createClient } from '@cubby/js';

const client = createClient({ 
  baseUrl: 'https://api.cubby.sh',
  token: process.env.CUBBY_TOKEN 
});

// list and select device
const { devices } = await client.listDevices();
console.log(`found ${devices.length} devices`);

client.setDeviceId(devices[0].id);

// search on remote device
const results = await client.search({ 
  q: 'meeting notes',
  limit: 5 
});

console.log(`found ${results.pagination.total} results`);

// automate remote device
await client.device.openApplication('Slack');
await client.notify({
  title: 'task complete',
  body: 'your automation finished'
});
```

## type definitions

the sdk is fully typed. import types for better intellisense:

```typescript
import { 
  createClient, 
  CubbyClient, 
  ClientOptions,
  cubbyQueryParams,
  cubbyResponse,
  NotificationOptions
} from '@cubby/js';
```

## links

- **docs**: [cubby.sh/docs](https://cubby.sh/docs)
- **rest api**: [api.cubby.sh/openapi.json](https://api.cubby.sh/openapi.json)
- **mcp server**: [cubby.sh/docs#mcp-integration](https://cubby.sh/docs#mcp-integration)
- **github**: [github.com/louis030195/cubby](https://github.com/louis030195/cubby)

## license

see [LICENSE.md](https://github.com/louis030195/cubby/blob/main/LICENSE.md)

