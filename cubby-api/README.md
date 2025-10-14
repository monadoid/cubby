# Cubby API Server

Cloudflare Worker that backs the Cubby API.

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

## Database Management

```bash
# Generate migration files
pnpm db:generate

# Run migrations
pnpm db:migrate
```

## Deployment

```bash
# Deploy to Cloudflare Workers
pnpm deploy
```

## Environment Variables

The following environment variables need to be configured in Cloudflare:

- `DATABASE_URL`: Neon database connection URL
- `CLOUDFLARE_API_TOKEN`: For Cloudflare API access
- `CLOUDFLARE_ACCOUNT_ID`: Your Cloudflare account ID
- `STYTCH_PROJECT_ID`: Stytch project ID for authentication
- `STYTCH_SECRET`: Stytch secret key
- `MCP_AUTH_SECRET`: Secret for MCP authentication

## Custom Domain

The API is served at `api.cubby.sh` through Cloudflare Workers. The domain routing is configured in `wrangler.toml`.



