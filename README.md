# cubby 

Turn your computer into a secure, remote MCP server.

**local-first data with cloud access** - your screen and audio recordings stay on your device, but you control who can access them securely via oauth and mcp tools.

## quick start
(macos + linux, windows incoming)

```bash
curl -s https://get.cubby.sh/cli | sh
```

this installs the cubby binary and starts recording your screen and audio in the background. all data stays local in `~/.cubby/`, unless you grant OAuth access.

## how it works

cubby continuously records your screen (ocr + screenshots) and audio (transcriptions). everything is stored in a local database.

you can then access your data in three ways:

### 1. typescript sdk

```bash
pnpm i @cubby/js
```

common ways to use cubby:

**search** - query your history:
```typescript
import { createClient } from '@cubby/js';

const client = createClient({ 
  baseUrl: 'https://api.cubby.sh',
  token: 'your-oauth-token'
});

// list devices and select one (for remote usage)
const { devices } = await client.listDevices();
client.setDeviceId(devices[0].id);

// find that article about dolphins you read last week
const results = await client.search({
  q: 'find me that website about dolphins',
  contentType: 'ocr',
  limit: 5
});
```

**watch** - process live events and trigger actions:
```typescript
// auto-create todoist tasks from spoken todos with ai
for await (const event of client.streamTranscriptions()) {
  if (event.text?.toLowerCase().includes('todo') || event.text?.toLowerCase().includes('remind me')) {
    // extract structured task details with ai
    const task = await ai.generateStructuredOutput({
      prompt: `extract task from: "${event.text}"`,
      schema: { title: 'string', priority: 'high|medium|low', dueDate: 'ISO date' }
    });
    
    // add to todoist
    await todoist.create(task);
    await client.notify({ 
      title: 'task added', 
      body: `"${task.title}" - ${task.priority} priority` 
    });
  }
}
```

**contextualize** - power ai with your personal context:
```typescript
// smart email responses based on recent conversations
const recentChats = await client.search({
  q: 'slack messages project alpha',
  contentType: 'ocr',
  limit: 15
});

// generate contextual reply
const draft = await ai.chat.completions.create({
  messages: [
    { role: 'system', content: 'draft professional email responses' },
    { role: 'user', content: `recent context: ${JSON.stringify(recentChats)}. draft reply to: "${emailContent}"` }
  ]
});

// send via gmail
await gmail.users.messages.send({ userId: 'me', raw: encodeDraft(draft) });
```

**automate** - build smart automations:
```typescript
// auto-log work hours when specific apps are active
for await (const event of client.streamVision()) {
  if (event.data.app_name === 'Linear' && event.data.text?.match(/ENG-\d+/)) {
    const ticketId = event.data.text.match(/ENG-\d+/)[0];
    await timeTracker.startTimer({ project: 'engineering', ticket: ticketId });
    await client.notify({ title: 'timer started', body: `tracking time on ${ticketId}` });
  }
}
```

full sdk docs at [npmjs.com/package/@cubby/js](https://www.npmjs.com/package/@cubby/js)

### 2. mcp server

**local:** `http://localhost:3030/mcp` (no auth required)

**remote:** `https://api.cubby.sh/mcp` (oauth via [cubby.sh](https://cubby.sh))

**available tools:**
- `devices/list` - list your enrolled devices
- `devices/set` - select a device for subsequent calls
- `device/search` - search content across screen + audio
- `device/search-keyword` - fast keyword search
- `device/speakers/search` - find speakers by name
- `device/speakers/similar` - find similar voices
- `device/speakers/unnamed` - get unidentified speakers
- `device/audio/list` - list audio devices
- `device/vision/list` - list monitors
- `device/frames/get` - retrieve specific frame data
- `device/tags/get` - get content tags
- `device/embeddings` - generate text embeddings
- `device/add` - add custom content to database
- `device/open-application` - launch applications
- `device/open-url` - open urls
- `device/notify` - send notifications

### 3. rest api

full openapi spec at `http://cubby.sh/docs/api`

**key endpoints:**
- `GET /search` - search across screen captures, audio, and ui elements
- `GET /search/keyword` - fast keyword search with fuzzy matching
- `GET /speakers/search` - find speakers by name
- `GET /audio/list` - list audio devices
- `GET /vision/list` - list monitors
- `POST /open-application` - launch apps
- `POST /open-url` - open urls
- `POST /notify` - send desktop notifications
- `WS /events` - stream live events (transcriptions, ocr, screenshots)

**remote usage:** `https://api.cubby.sh/devices/{deviceId}/search`

## architecture

```
┌──────────────┐
│  SQLite DB   │
│  ~/.cubby/   │
└──────────────┘
        │
        │ local data access
        ↓
┌────────────────────────────┐
│        Cubby Server        │
│        MCP / REST          │
└────────────────────────────┘
        │
        ↓
┌────────────────────────────┐
│          Tunnel            │
└────────────────────────────┘
        │
        ↓
┌────────────────────────────┐           ┌────────────────────────────┐
│       api.cubby.sh/mcp     │ ←───────  │     Remote MCP Client      │
│          (Cubby API)       │           │     (OAuth)                │
└────────────────────────────┘           │     JS SDK                 │
                                         └────────────────────────────┘
```

## development

cubby is written in rust + typescript:
- **cubby-server** - rust binary for recording, ocr, stt, database, rest api + mcp server
- **cubby-api** - typescript cloudflare worker for oauth + remote mcp proxy
- **cubby-js** - typescript sdk for building integrations
