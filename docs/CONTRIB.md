# Contributor Guide

This guide covers the development workflow, available scripts, environment setup, and testing procedures for the Agent Skill Registry project.

## Table of Contents

- [Quick Start](#quick-start)
- [Development Workflow](#development-workflow)
- [Available Scripts](#available-scripts)
- [Environment Setup](#environment-setup)
- [Testing Procedures](#testing-procedures)
- [Code Organization](#code-organization)
- [Common Tasks](#common-tasks)

## Quick Start

### Prerequisites

- **Rust** (latest stable)
- **Node.js & npm**
- **PostgreSQL** database
- **S3-compatible storage** (MinIO, AWS S3, etc.)
- **Docker & Docker Compose** (for containerized development)

### Initial Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/Lynskylate/skillgregistry.git
   cd skillgregistry
   ```

2. **Configure environment variables**
   ```bash
   cp .env.example backend/.env
   # Edit backend/.env with your configuration
   ```

3. **Start services with Docker Compose**
   ```bash
   make dev
   ```

This will build and start all services (API, Worker, Database, S3) in the background and follow the logs.

## Development Workflow

### Backend Development

The backend uses a Rust workspace with multiple crates:

```
backend/
├── api/           # HTTP API server (Axum)
├── worker/        # Background job processor
├── common/        # Shared types and utilities
├── migration/     # Database migrations
├── setup/         # Database setup utilities
└── e2e-tests/     # End-to-end tests
```

**Development cycle:**
1. Make changes to source code
2. Run `cargo test` to verify
3. Run `cargo run --bin api` or `--bin worker` to test locally
4. Rebuild Docker containers: `make rebuild-backend`

### Frontend Development

The frontend is a React + Vite application with Tailwind CSS and shadcn/ui components.

**Development cycle:**
1. Make changes to source code
2. Vite hot-reloads automatically
3. Build for production: `npm run build`

### Git Workflow

1. Create a feature branch from `main`
2. Make atomic commits with clear messages
3. Run tests before pushing
4. Create a pull request for review

## Available Scripts

### Docker Compose Commands (via Makefile)

| Command | Description |
|---------|-------------|
| `make help` | Show all available commands |
| `make build` | Build all Docker images |
| `make build-plain` | Build with BuildKit and plain output |
| `make up` | Start all services in background |
| `make up-build` | Start services and rebuild images |
| `make down` | Stop and remove services |
| `make down-v` | Stop services and remove volumes |
| `make ps` | Show service status |
| `make config` | Validate and view Compose configuration |
| `make logs` | Follow all service logs |
| `make logs-app` | Follow backend + worker logs |
| `make logs-backend` | Follow backend service logs |
| `make logs-worker` | Follow worker service logs |
| `make shell-backend` | Open shell in backend container |
| `make shell-worker` | Open shell in worker container |
| `make shell-setup` | Open shell in setup container |
| `make rebuild-backend` | Rebuild and restart backend |
| `make dev` | Build, start, and follow app logs |
| `make reset` | Recreate entire stack (down -v, build, up) |
| `make clean` | Show Docker disk usage |
| `make prune` | Remove unused Docker data |
| `make prune-all` | Remove all Docker data |

### Frontend NPM Scripts

| Command | Description |
|---------|-------------|
| `npm run dev` | Start Vite dev server (hot reload) |
| `npm run build` | Build for production (TypeScript + Vite) |
| `npm run preview` | Preview production build locally |

### Backend Cargo Commands

| Command | Description |
|---------|-------------|
| `cargo run --bin api` | Run the API server |
| `cargo run --bin worker` | Run the background worker |
| `cargo test` | Run unit and integration tests |
| `cargo clippy` | Run linter checks |
| `cargo fmt` | Format code |

### Test Scripts

| Script | Description |
|--------|-------------|
| `./scripts/run-e2e-tests.sh` | Run full E2E test suite |

## Environment Setup

### Environment Variables

The application uses the `SKILLREGISTRY_` prefix for all configuration variables. Nested configuration uses double underscores (`__`).

#### Database Configuration

```bash
# SQLite (default in .env.example)
SKILLREGISTRY_DATABASE__URL=sqlite://skillregistry.db?mode=rwc

# PostgreSQL (recommended for production)
SKILLREGISTRY_DATABASE__URL=postgres://user:password@localhost/skillregistry
```

#### Server Configuration

```bash
SKILLREGISTRY_PORT=3000
RUST_LOG=worker=info,common=info,api=info
```

#### S3 Storage Configuration

```bash
SKILLREGISTRY_S3__BUCKET=skill-registry-bucket
SKILLREGISTRY_S3__REGION=us-east-1
SKILLREGISTRY_S3__ACCESS_KEY_ID=your_access_key
SKILLREGISTRY_S3__SECRET_ACCESS_KEY=your_secret_key
SKILLREGISTRY_S3__ENDPOINT=http://localhost:9000  # MinIO default
SKILLREGISTRY_S3__FORCE_PATH_STYLE=true
```

#### GitHub Integration

```bash
SKILLREGISTRY_GITHUB__TOKEN=your_github_token
SKILLREGISTRY_GITHUB__SEARCH_KEYWORDS=topic:agent-skill
```

#### Worker Configuration

```bash
SKILLREGISTRY_WORKER__SCAN_INTERVAL_SECONDS=3600
```

#### Temporal Configuration

```bash
SKILLREGISTRY_TEMPORAL__SERVER_URL=http://localhost:7233
SKILLREGISTRY_TEMPORAL__TASK_QUEUE=skill-registry-queue
```

#### Authentication

```bash
SKILLREGISTRY_AUTH__JWT__SIGNING_KEY=dev-secret-key-change-in-production
SKILLREGISTRY_AUTH__JWT__ISSUER=skillregistry
SKILLREGISTRY_AUTH__JWT__AUDIENCE=skillregistry
```

#### CORS & Security

```bash
# Enable for local HTTP development
SKILLREGISTRY_DEBUG=true

# Comma-separated allowlist for frontend origins
SKILLREGISTRY_AUTH__FRONTEND_ORIGIN=http://localhost:8080,http://127.0.0.1:8080

# Optional: Share cookies across subdomains
SKILLREGISTRY_AUTH__COOKIE_DOMAIN=
```

> **Important**: After the config refactor, only `SKILLREGISTRY_*` variables are read by the application. Legacy variable names are no longer supported.

## Testing Procedures

### Unit & Integration Tests

Run the full test suite:

```bash
cd backend
cargo test
```

Run tests for a specific crate:

```bash
cargo test -p api
cargo test -p worker
cargo test -p common
```

Run tests with output:

```bash
cargo test -- --nocapture
```

### End-to-End Tests

The project includes comprehensive E2E tests that validate:
- Temporal workflow execution
- API endpoint functionality
- Database operations
- S3 storage interactions

**Run E2E tests locally:**

```bash
./scripts/run-e2e-tests.sh
```

**E2E Test Documentation:**
- [E2E Testing Guide](E2E_TESTING.md) - How to run and write E2E tests
- [E2E Test Design](E2E_TEST_DESIGN.md) - Architecture and design decisions
- [E2E Test Examples](E2E_TEST_EXAMPLES.md) - Example test scenarios

### Coverage

Generate coverage reports:

```bash
cd backend
cargo tarpaulin --out Html
```

## Code Organization

### Backend Structure

```
backend/
├── api/
│   ├── src/
│   │   ├── main.rs          # API server entry point
│   │   ├── handlers/        # Request handlers
│   │   ├── routes/          # Route definitions
│   │   └── middleware/      # Middleware (auth, CORS)
│   └── Cargo.toml
├── worker/
│   ├── src/
│   │   ├── main.rs          # Worker entry point
│   │   ├── activities/      # Temporal activities
│   │   └── workflows/       # Temporal workflows
│   └── Cargo.toml
├── common/
│   ├── src/
│   │   ├── config/          # Configuration structs
│   │   ├── models/          # Database models
│   │   ├── db/              # Database utilities
│   │   └── s3/              # S3 client wrapper
│   └── Cargo.toml
├── migration/
│   ├── src/
│   │   └── main.rs          # Migration runner
│   └── migrations/          # SQL migration files
├── setup/
│   ├── src/
│   │   └── main.rs          # Setup utilities
│   └── Cargo.toml
├── e2e-tests/
│   ├── src/
│   │   └── ...
│   └── Cargo.toml
├── Cargo.toml               # Workspace definition
└── .env.example             # Environment template
```

### Frontend Structure

```
frontend/
├── src/
│   ├── components/          # React components
│   ├── pages/               # Page components
│   ├── lib/                 # Utility functions
│   ├── App.tsx              # Root component
│   └── main.tsx             # Entry point
├── public/                  # Static assets
├── package.json
├── vite.config.ts
├── tailwind.config.js
└── tsconfig.json
```

## Common Tasks

### Adding a New Environment Variable

1. Add the variable to `.env.example`
2. Update the config struct in `backend/common/src/config/`
3. Document the variable in this guide

### Running Database Migrations

```bash
docker-compose exec skillregistry-setup sh
cd /app
cargo run --bin setup
```

### Rebuilding After Dependency Changes

```bash
make rebuild-backend
```

### Viewing Real-Time Logs

```bash
make logs-app       # Backend + Worker
make logs-backend   # Backend only
make logs-worker    # Worker only
```

### Debugging in Containers

```bash
make shell-backend  # Access backend container
make shell-worker   # Access worker container
```

### Resetting the Development Environment

This will remove all volumes and rebuild:

```bash
make reset
```

## Additional Resources

- [Backend Architecture Notes](BACKEND_ARCHITECTURE_NOTE.md)
- [Docker Commands Reference](DOCKER_COMMANDS.md)
- [E2E Testing Documentation](E2E_TESTING.md)
- [Agent Skills Specification](https://agentskills.io/specification)
