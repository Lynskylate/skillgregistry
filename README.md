# Agent Skill Registry

[![CI](https://github.com/Lynskylate/skillgregistry/actions/workflows/ci.yml/badge.svg)](https://github.com/Lynskylate/skillgregistry/actions/workflows/ci.yml)
[![codecov](https://codecov.io/github/Lynskylate/skillgregistry/graph/badge.svg?token=XHBRCLST8W)](https://codecov.io/github/Lynskylate/skillgregistry)

A system to index, discover, and display skill projects hosted on GitHub/GitLab, following the [Agent Skills specification](https://agentskills.io/specification).

## Architecture

- **Backend**: Rust (Axum, SeaORM, Tokio)
- **Frontend**: React (Vite, Tailwind, shadcn/ui)
- **Database**: PostgreSQL
- **Storage**: S3-compatible Object Storage

## Prerequisites

- Rust (latest stable)
- Node.js & npm
- PostgreSQL database
- S3-compatible storage (MinIO, AWS S3, etc.)

## Getting Started

### 1. Database Setup

Ensure PostgreSQL is running and create a database named `skillregistry`.

```bash
# Example
createdb skillregistry
```

### 2. Backend Setup

Create a `.env` file in `backend/` or set environment variables:

```bash
DATABASE_URL=postgres://user:password@localhost/skillregistry
GITHUB_TOKEN=your_github_token
AWS_ACCESS_KEY_ID=your_key
AWS_SECRET_ACCESS_KEY=your_secret
AWS_REGION=us-east-1
```

Run the API:

```bash
cd backend
cargo run --bin api
```

Run the Worker (in a separate terminal):

```bash
cd backend
cargo run --bin worker
```

### 3. Frontend Setup

```bash
cd frontend
npm install
npm run dev
```

Visit `http://localhost:5173` to browse the registry.

## Features

- **Discovery**: Automatically finds repositories with `agent-skill` topic on GitHub.
- **Processing**: Downloads, parses `SKILL.md`, and validates structure.
- **Registry**: API to search and view skills.
- **Versioning**: Tracks versions of skills.

## Testing

### Unit and Integration Tests

Run the test suite:

```bash
cd backend
cargo test
```

### End-to-End Tests

The project includes comprehensive E2E tests that validate the complete system workflow including Temporal workflows, API endpoints, and database operations.

To run E2E tests locally:

```bash
./scripts/run-e2e-tests.sh
```

For detailed information about E2E testing:
- [E2E Testing Guide](docs/E2E_TESTING.md) - How to run and write E2E tests
- [E2E Test Design](docs/E2E_TEST_DESIGN.md) - Architecture and design decisions
