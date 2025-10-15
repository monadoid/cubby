# Cubby Frontend

Simple hello world HTMX frontend for Cubby.

## Development

```bash
# Install dependencies
pnpm install

# Run local development server
pnpm dev

# Type check
pnpm type-check

# Generate Cloudflare types
pnpm cf-typegen
```

## Deployment

```bash
# Deploy to Cloudflare Workers
pnpm deploy
```

## Custom Domain

The frontend is served at `cubby.sh` through Cloudflare Workers. The domain routing is configured in `wrangler.toml`.



