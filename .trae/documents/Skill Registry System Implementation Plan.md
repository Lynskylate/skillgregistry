# Skill Registry System Design & Implementation Plan

This plan outlines the steps to build a Skill Registry system using Rust (Axum), React (Vite + Tailwind + shadcn/ui), and a PostgreSQL database with S3-compatible storage.

## 1. Project Initialization & Structure
We will use a monorepo structure for easier management of the full stack.
*   **Root Directory**: `skill-registry`
    *   `backend/` (Rust Workspace)
        *   `crates/api` (Axum REST API)
        *   `crates/worker` (Background Task Processor)
        *   `crates/common` (Shared Database Entities, Config, S3 Utils)
    *   `frontend/` (React + TypeScript + Vite)

## 2. Database Design (PostgreSQL)
We will use `sea-orm` for ORM capabilities to ensure type safety and ease of migration.

### Core Tables:
1.  **`repositories`**: Tracks source code repositories (GitHub/GitLab).
    *   Fields: `id`, `platform` (enum), `owner`, `name`, `url`, `last_scanned_at`, `status`.
2.  **`skills`**: Unique skills identified by name.
    *   Fields: `id`, `name` (unique), `latest_version`, `repository_id`.
3.  **`skill_versions`**: Specific versions of a skill.
    *   Fields: `id`, `skill_id`, `version` (semver), `description`, `readme_content` (SKILL.md body), `s3_key` (zip download link), `license`, `metadata` (JSON), `created_at`.
4.  **`dependencies`**: Tracks skill dependencies.
    *   Fields: `skill_version_id`, `dependency_name`, `constraint`.

## 3. Backend Implementation (Rust)

### 3.1 Common Crate
*   Setup `sea-orm` entities and migration logic.
*   Implement S3 client wrapper (using `aws-sdk-s3`) for multipart uploads.
*   Define shared configuration structure (DB URL, S3 Creds, GitHub Token).

### 3.2 Worker Crate (The "Crawler")
*   **Task Scheduler**: Implement a robust loop or use a library like `apalis` (Redis/Postgres backed) or a custom poll loop for task management.
*   **Discovery Task (Job 1)**:
    *   Use GitHub/GitLab APIs to search for repositories with `SKILL.md` or specific topics (e.g., `agent-skill`).
    *   Validate repository structure matches `https://agentskills.io/specification`.
*   **Processing Task (Job 2)**:
    *   Clone/Download target repositories.
    *   Parse `SKILL.md` YAML frontmatter (name, description, etc.).
    *   Zip the skill directory.
    *   Upload Zip to S3 (with multipart support).
    *   Update Database.

### 3.3 API Crate (Axum)
*   **Endpoints**:
    *   `GET /api/skills`: List skills with pagination and search (name/description).
    *   `GET /api/skills/:name`: Get skill details and version history.
    *   `GET /api/skills/:name/versions/:version`: Get specific version details.
*   **Middleware**: CORS, Logging (Tracing), Error Handling (Custom `AppError` -> JSON).

## 4. Frontend Implementation (React)

### 4.1 Setup
*   Initialize Vite project with React & TypeScript.
*   Install & Configure `Tailwind CSS`.
*   Initialize `shadcn/ui` and add core components (Card, Button, Input, Table, Badge, Skeleton).

### 4.2 Features
*   **Home Page**:
    *   Hero section with search bar.
    *   "Trending/Recent" skills grid using Card components.
*   **Search Results**:
    *   Filterable list of skills.
*   **Skill Detail Page**:
    *   Readme renderer (Markdown).
    *   Installation instructions.
    *   Metadata sidebar (License, Version, Author).
    *   Download button (links to S3).

## 5. Development Steps
1.  **Scaffold**: Create directories and initialize Rust workspace and React app.
2.  **Database**: Setup PostgreSQL (via Docker if needed) and write migration scripts.
3.  **Backend Core**: Implement `common` crate (DB & S3).
4.  **Worker**: Implement the Discovery logic first, then the Processing logic.
5.  **API**: Expose the data via REST endpoints.
6.  **Frontend**: Build the UI to consume the API.
7.  **Integration**: Verify the end-to-end flow (Scan -> Store -> Serve -> View).
