# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.


### Tech Stack
- **Backend**: Loco-rs (Rust web framework)
- **Database**: PostgreSQL with SeaORM
- **Frontend**: Server-side rendered with Tera templates + HTMX
- **Styling**: Tailwind CSS + daisyUI
- **Authentication**: Stytch + Custom OAuth flow
- **Testing**: Bruno for HTTP API tests
- **Deployment**: Cloudflare Workers (exampleco_website)

## Development Commands

### Building and Running
- `just build` - Build all projects (Rust + TypeScript)
- `just dev-api` - Start the Loco development server at localhost:5150
- `just dev-worker` - Start Cloudflare Worker development server

### Testing
- `just test` - Run all tests (Rust + TypeScript)
- `just bruno-test` - Run Bruno HTTP API tests (requires server to be running)
- `cd apps/cubby-api/bruno && bru run . -r --env local` - Run Bruno tests manually

### Code Quality
- `just lint` - Run clippy, cargo fmt check, and pnpm lint
- `just fmt` - Format all code (cargo fmt + pnpm format)

### Database Operations
- `cargo loco db migrate` - Run database migrations
- `cargo loco db entities` - Generate SeaORM entities from database
- `cargo loco generate migration <NAME>` - Create new migration

### CSS Development
- `./tailwindcss -i input.css -o assets/static/output.css` - Generate CSS from Tailwind
- `./tailwindcss -i input.css -o assets/static/output.css --watch` - Watch mode for CSS

## Architecture

### Monorepo Structure
- **Root**: Cargo workspace with justfile for task automation
- **apps/cubby-api**: Main Loco Rust backend application
- **apps/exampleco_website**: Cloudflare Worker TypeScript application

### Backend Architecture (Loco Framework)
- **Controllers**: Handle HTTP routes and requests (`src/controllers/`)
  - `auth.rs` + `auth_htmx.rs` - Authentication endpoints
  - `oauth.rs` + `oauth_htmx.rs` - OAuth flow implementation
  - `movie.rs` + `movie_htmx.rs` - Movie management - movies are just examples for MVP
  - `pods.rs` + `pod_htmx.rs` - Solid pod integration
  - `css_proxy.rs` - CSS server proxy functionality
- **Models**: Database entities and business logic (`src/models/`)
  - Generated entities in `_entities/` subfolder
  - Custom model logic in individual files
- **Views**: Server-side template rendering (`src/views/`)
- **Data**: External service integrations (`src/data/`)
  - Stytch authentication
  - Solid server communication
  - DPoP key handling
- **Workers**: Background job processing (`src/workers/`)
- **Mailers**: Email functionality (`src/mailers/`)

### Authentication Flow
- Stytch integration for user authentication
- Custom OAuth implementation with client credentials
- DPoP (Demonstration of Proof of Possession) for Solid protocol
- JWT tokens for session management

### Frontend Integration
- Server-side rendering with Tera templates
- HTMX for dynamic interactions
- Tailwind CSS + daisyUI for styling
- Static assets served from `assets/static/`

## Bruno HTTP Tests
- Test scenarios include OAuth flows, authentication, and API endpoints
- Requires the development server to be running on localhost:5150 (allow the user to restart it themselves when you make code changes)
- Remember: when running bruno tests, you can look in `apps/cubby-api/logs/app.log` and tail the logs

## Important Notes
- View templates are located in `assets/views/`
- Static files are served from `assets/static/`
- CSS is generated via Tailwind CLI (not Node.js based)
- The project integrates with Solid protocol for decentralized data storage