# cubby - context layer for agi

cubby is the context layer for agi - recording screens, audio 24/7, extracting ocr & stt, saving to local db, and connecting to ai.

## quick start

```bash
curl -fsSL https://get.cubby.sh/cli | sh
```

## local server

when you run `cubby start`, the local rust server runs on `localhost:3030`

**rest api documentation:** `http://localhost:3030/openapi.yaml` or `http://localhost:3030/openapi.json`

**mcp server:** `http://localhost:3030/mcp`

### mcp tools available

- **search-content** - search through ocr text, audio transcriptions, ui elements
- **pixel-control** - control mouse and keyboard (cross-platform)
- **find-elements** - find ui elements by role (macos only)
- **click-element** - click ui elements by id (macos only)
- **fill-element** - type into ui elements (macos only)  
- **scroll-element** - scroll ui elements (macos only)
- **open-application** - open applications by name (macos only)
- **open-url** - open urls in browser (cross-platform)

### mcp access patterns

**local usage (claude desktop, etc):**
- configure mcp client to: `http://localhost:3030/mcp`
- no authentication required
- tools require no device_id parameter

**remote usage (from anywhere):**
- configure mcp client to: `https://api.cubby.sh/mcp`
- oauth authentication required (configure once via cubby.sh)
- tools require `deviceId` parameter to specify which device to control
- get your device ids from: `https://api.cubby.sh/devices` (authenticated)

## architecture

```
┌─────────────┐                    ┌──────────────┐
│ local mcp   │──────────────────→ │ cubby-server │
│ client      │  localhost:3030/mcp│ (rust)       │
└─────────────┘                    └──────────────┘
                                          │
                                          │ local data access
                                          ↓
                                   ┌──────────────┐
                                   │ sqlite db    │
                                   │ ~/.cubby/    │
                                   └──────────────┘

┌─────────────┐                    ┌──────────────┐
│ remote mcp  │──→ oauth ──────→   │ cubby-api    │
│ client      │    api.cubby.sh/mcp│ (cloudflare) │
└─────────────┘                    └──────────────┘
                                          │
                                          │ proxy with device_id
                                          ↓
                              ┌──────────────────────┐
                              │ cloudflare tunnel    │
                              │ {deviceId}.cubby.sh  │
                              └──────────────────────┘
                                          │
                                          ↓
                              ┌──────────────────────┐
                              │ cubby-server (rust)  │
                              │ localhost:3030       │
                              └──────────────────────┘
```

## development

cubby is written in rust + typescript:
- **cubby-server** - rust binary recording, ocr, stt, database, rest api + mcp server
- **cubby-api** - typescript cloudflare worker for remote oauth + mcp proxy
- **cubby-js** - typescript sdk for building integrations
