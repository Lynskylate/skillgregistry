# Agent Skill Registry - Project Guide

This project is a registry system for indexing, discovering, and distributing AI Agent skills, following the [Agent Skills specification](https://agentskills.io/specification).

## Language Policy
- English is required across this repository.
- Do not introduce Chinese (or other non-English text) outside the `.trae/` directory.

## Mission
Provide a standardized skill repository for AI Agents, supporting automatic discovery (from GitHub), verification, versioning, and distribution.

## Tech Stack
- **Backend**: Rust
  - `api`: Axum REST API.
  - `worker`: Async task processor for skill discovery and syncing.
  - `common`: Shared library with database entities (SeaORM), S3 storage, and configuration.
- **Frontend**: React (Vite)
  - **Styling**: [Tailwind CSS](https://tailwindcss.com/docs/installation/using-vite)
  - **UI components**: [shadcn/ui](https://ui.shadcn.com/llms.txt)
- **Database**: SQLite (local development) / PostgreSQL (production)
- **Object storage**: S3-compatible storage (for skill archives)

## Core Logic
### 1. Discovery
The `worker` periodically searches GitHub repositories tagged with `agent-skill`, or searches code based on configured keywords.

### 2. Sync & Verify
- **Download**: Download the skill repository ZIP from GitHub.
- **Verify**: Call `verify_skill` to validate whether the `SKILL.md` frontmatter matches the specification.
- **Package**: Call `package_skill` to re-package the skill directory and compute the MD5 hash.
- **Distribute**: Upload the packaged artifact to S3 and record version information in the database.

## Development Guide
### Run Tests
```bash
cd backend/worker
cargo test
```

### Add New Features
- Database changes: modify models in `backend/common/src/entities/`.
- API changes: add handler functions in `backend/api/src/handlers.rs`.
- Worker logic: update task logic in `backend/worker/src/tasks/`.

## Agent Skills Specification Summary
Agent Skills is a standardized format for defining AI Agent skills.

### Directory Structure
- `skill-name/`
    - `SKILL.md` (required, contains YAML frontmatter and Markdown body)
    - `scripts/` (optional)
    - `references/` (optional)
    - `assets/` (optional)

### `SKILL.md` Constraints
- `name`: 1-64 characters, lowercase letters, digits, and hyphens.
- `description`: 1-1024 characters.
- Allowed fields: `name`, `description`, `license`, `compatibility`, `allowed-tools`, `metadata`.

## References
- **Agent Skills specification**: [https://agentskills.io/specification](https://agentskills.io/specification)
- **Anthropic Skills repo**: [https://github.com/anthropics/skills](https://github.com/anthropics/skills)
- **Anthropic official plugins repo**: [https://github.com/anthropics/claude-plugins-official](https://github.com/anthropics/claude-plugins-official)
