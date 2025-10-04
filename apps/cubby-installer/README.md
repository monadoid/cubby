# Cubby Installer Worker

This Cloudflare Worker serves the Cubby CLI installation script and binaries.

## Endpoints

- `GET /` - Health check and info
- `GET /cli` - Serves the installation script
- `GET /binaries/:filename` - Serves binaries from R2 (requires R2 bucket configuration)

## Deployment

```bash
pnpm run deploy
```

## Important Notes

### Shell Script Handling

The installation script is embedded as a TypeScript string constant in `src/install.sh.ts` rather than imported as a separate `.sh` file. This is a **workaround for a Cloudflare API restriction**:

- ❌ Importing `.sh` files directly (even with `rules` configuration) causes a `403 Forbidden` error
- ✅ Embedding the script as a string constant works perfectly

If you need to update the install script:
1. Edit `src/install.sh.ts`
2. Be careful with escaping: `${}` becomes `\${}`  and backslashes need doubling

### R2 Bucket Configuration

The R2 bucket for binary distribution is currently commented out in `wrangler.jsonc`. To enable it:

1. Uncomment the `r2_buckets` section in `wrangler.jsonc`
2. Ensure the `cubby-releases` bucket exists in your Cloudflare account
3. Deploy again

## Configuration

Key settings in `wrangler.jsonc`:
- `account_id`: Required to avoid interactive account selection
- `observability`: Enabled for monitoring
- Worker name: `cubby-installer`
- Deployed URL: `https://cubby-installer.prosammer.workers.dev`

## Development

```bash
pnpm run dev     # Start local development server
pnpm run deploy  # Deploy to production
```

