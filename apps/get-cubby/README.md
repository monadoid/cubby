# get-cubby

Cloudflare Worker that serves the Cubby CLI install script and binaries.

## Usage

Users can install Cubby CLI with:

```bash
curl -fsSL https://get.cubby.sh/cli | sh
```

## How it works

1. The `/cli` endpoint serves the install script (`src/install.sh`) that:
   - Detects the user's OS and architecture
   - Downloads the appropriate binary from `/binaries/:filename`
   - Installs it to `/usr/local/bin` or `~/.local/bin`

2. The `/binaries/:filename` endpoint serves binaries from Cloudflare R2 storage

3. Binaries are uploaded to R2 via GitHub Actions on every push to main

## Project Structure

```
apps/get-cubby/
├── src/
│   ├── index.ts          # Main worker code
│   ├── install.sh        # Install script (imported as text)
│   └── install.sh.d.ts   # TypeScript declarations for .sh imports
├── package.json
├── tsconfig.json
└── wrangler.jsonc        # Includes rule to bundle .sh files as text
```

The install script is kept in a separate `.sh` file for:
- IDE syntax highlighting and linting
- Better shell script editing experience
- Easier maintenance and testing

It's imported as a text string and served directly by the worker.

## Development

```bash
pnpm install
pnpm dev
```

## Deployment

```bash
pnpm deploy
```

Or push to main branch - GitHub Actions will deploy automatically.

## Setup Requirements

### Cloudflare R2 Bucket

Create an R2 bucket named `cubby-releases`:

```bash
wrangler r2 bucket create cubby-releases
```

### GitHub Secrets

Set these secrets in your GitHub repository:

- `R2_ACCESS_KEY_ID` - R2 access key ID
- `R2_SECRET_ACCESS_KEY` - R2 secret access key
- `R2_ENDPOINT` - R2 endpoint (e.g., `https://<account_id>.r2.cloudflarestorage.com`)
- `CLOUDFLARE_API_TOKEN` - Cloudflare API token with Workers write access
- `CLOUDFLARE_ACCOUNT_ID` - Your Cloudflare account ID

### DNS Setup

Point `get.cubby.sh` to your Cloudflare Worker:

1. In Cloudflare Dashboard, go to Workers & Pages
2. Select the `get-cubby` worker
3. Go to Settings > Triggers
4. Add custom domain: `get.cubby.sh`

