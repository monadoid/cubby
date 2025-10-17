# cubby 


## quick start

```bash
 curl -s https://get.cubby.sh/cli | sh
```

## local server

when you run `cubby start`, the local rust server runs on `localhost:3030`

**rest api documentation:** `http://localhost:3030/openapi.yaml` or `http://localhost:3030/openapi.json`

**mcp server:** `http://localhost:3030/mcp`

### mcp tools available

- **search-content** - search through ocr text, audio transcriptions, ui elements

[//]: # (- **pixel-control** - control mouse and keyboard &#40;cross-platform&#41;)

[//]: # (- **find-elements** - find ui elements by role &#40;macos only&#41;)

[//]: # (- **click-element** - click ui elements by id &#40;macos only&#41;)

[//]: # (- **fill-element** - type into ui elements &#40;macos only&#41;  )

[//]: # (- **scroll-element** - scroll ui elements &#40;macos only&#41;)
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
