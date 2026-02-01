Based on the analysis of the existing code and your requirements, here is the implementation plan. I will refactor the existing `backend` workspace to support SQLite and implement the robust worker tasks.

### Phase 1: Infrastructure & Database Migration
1.  **Dependencies Update**: Modify `common/Cargo.toml` to replace `sqlx-postgres` with `sqlx-sqlite` and add necessary features to `sea-orm`.
2.  **Database Schema**:
    *   Refactor `common/src/entities/repositories.rs` to `skill_registry.rs` to match the requested table name.
    *   Ensure `skills` and `skill_versions` entities match the requirements (adding `file_hash`, `updated_at`, etc.).
    *   Update `common/src/db.rs` to initialize a SQLite database file (`skillregistry.db`) instead of connecting to Postgres.
3.  **OSS/S3 Service Upgrade**:
    *   Enhance `common/src/s3.rs` to support multipart uploads, retry logic, and MD5 checksum verification for file integrity.

### Phase 2: Enhanced GitHub Client
1.  **Client Implementation** (`worker/src/github.rs`):
    *   Implement robust error handling with exponential backoff for rate limits (403/429).
    *   Add specific search filters: `created`, `fork:false`, `topic:agent-skill`.
    *   Implement pagination support to fetch all results, not just the first page.

### Phase 3: Core Task Implementation
1.  **Task 1: Discovery Task** (`worker/src/tasks/discovery.rs`):
    *   Implement the logic to search GitHub based on config (env vars).
    *   Filter out already existing repositories in `skill_registry`.
    *   Parse metadata and insert new records into SQLite.
2.  **Task 2: Synchronization Task** (`worker/src/tasks/sync.rs`):
    *   Iterate through registered skills.
    *   Detect changes by comparing the calculated hash of the `skill/` directory (or repo root) with the stored hash.
    *   If changed:
        *   Package the skill folder into a ZIP file.
        *   Upload to OSS with integrity check.
        *   Update `skills` and `skill_versions` tables with new version, hash, and OSS URL.
3.  **Refactor Worker Entrypoint** (`worker/src/main.rs`):
    *   Replace the simple loop with a structured scheduler (or managed loop) that handles the two tasks independently.
    *   Implement comprehensive logging (processed count, errors, success rate).

### Phase 4: Configuration & Verification
1.  **Configuration**: Ensure all parameters (cron schedule, search keywords, OSS credentials) are loaded from environment variables (`.env`).
2.  **Verification**:
    *   Create a local SQLite DB.
    *   Run the worker to discover real repositories.
    *   Verify data in SQLite and files in the configured OSS bucket (or mock).
