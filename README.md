# Cubby

A secure desktop tunnel service for remote access to local applications.

## Overview
- Rust desktop agent (`apps/cubby`) runs Screenpipe capture, manages Cloudflared, and provisions devices
- Cloudflare Workers power the public API (`apps/cubby-server`) and installer endpoint (`apps/cubby-installer`)
- ExampleCo demo site (`apps/exampleco_website`) showcases the Stytch Connected Apps authorization flow
- Neon PostgreSQL backs both environments; Stytch handles account creation and app consent

## Quick Start

### End Users
```bash
curl -fsSL https://get.cubby.sh/cli | sh
```
Installs the Cubby CLI, walks the user through Stytch-powered signup, and launches the tunnel with Screenpipe + Cloudflared.

### Developers
**Prerequisites:** Rust (stable), Node.js 20+, pnpm, a Cloudflare account, and `just` (`cargo install just`).

**Core commands:**
- `just install` – installs all JS/TS dependencies across the workspace
- `just cubby-start` – runs the desktop agent locally (dev mode)
- `just server-dev` – starts the Cubby API Worker with Wrangler
- `just example-dev` – serves the ExampleCo Connected App demo
- `just get-cubby-dev` – runs the installer Worker (CLI download endpoint)
- `just fmt` – runs `cargo fmt` and `pnpm run format` (Prettier across workspace TypeScript)
- `just lint` – runs `cargo clippy --workspace -- -D warnings` plus workspace lint scripts


## Project Structure
```
apps/
  cubby/             # Rust CLI (desktop agent)
  cubby-server/      # Cloudflare Worker API
  cubby-installer/   # Cloudflare Worker serving the installer script
  exampleco_website/ # Example Stytch Connected App walkthrough
check_screenpipe_db.sh
justfile
pnpm-workspace.yaml
```

## Core Components
- **Desktop agent (`apps/cubby`)** starts Screenpipe capture, brokers auth with Stytch, and maintains the Cloudflared tunnel.
- **API Worker (`apps/cubby-server`)** exposes enrollment + OAuth flows, persists data in Neon, and brokers access tokens for Connected Apps.
- **Installer Worker (`apps/cubby-installer`)** hosts the install shell script and binary artifacts served from R2.
- **ExampleCo demo (`apps/exampleco_website`)** exercises the Stytch Connected Apps flow so developers can see end-to-end consent.

## Auth & Data
- Stytch powers both CLI sign-up/login and Connected Apps authorization; new devices create accounts via the CLI, then users grant third-party access through ExampleCo (or another integrated app).
- Neon PostgreSQL hosts two main databases: `main` (production, tied to the `main` branch) and `test` (shared by local/dev and the `develop` branch).
  - Develop locally against the `test` Neon database or run locally.
  - Database migrations live in `apps/cubby-server` and run through `pnpm db:migrate`.

## Deployment
- Work off of the `develop` branch; open pull requests into `main` when changes are ready to ship.
- Merging to `main` triggers the release pipeline that builds the CLI, publishes Worker updates, and refreshes installer artifacts.
- Use `wrangler deploy` from each Worker project (or the automation in CI) when you need a manual rollout.

## Notes
- Environment variables are managed per Worker via Wrangler `.env` files/Cloudflare dashboards; database URLs and Stytch keys live there rather than in GitHub repository secrets.
- `just fmt` covers Rust via `cargo fmt` and TypeScript via Prettier; extend package-level scripts if new surface areas appear.
