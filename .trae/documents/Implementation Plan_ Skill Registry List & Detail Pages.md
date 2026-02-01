# Full Stack Implementation Plan for Skill Registry List & Detail Pages

## 1. Database Structure Verification & Report

* **Action**: Analyze `skills` and `skill_versions` tables.

* **Output**: Create `DB_REVIEW.md` containing:

  * **Missing Fields**: `status` (active/deprecated) in `skills` table.

  * **Performance**: Recommendation for indices on `skills(created_at)` for sorting and `skill_versions(skill_id, version)` for fast lookups.

  * **Integrity**: Verify FK constraints between `skills` and `skill_versions`.

## 2. Backend API Enhancement (Rust/Axum)

* **Goal**: Align API with RESTful requirements and standard response format.

* **Files**: `backend/api/src/handlers.rs`, `backend/api/src/models.rs` (to be created)

* **Tasks**:

  1. **Standardize Response**: Create a generic `ApiResponse<T>` struct `{ code, data, message, timestamp }`.
  2. **Enhance** **`GET /api/skills`**:

     * Add `sort_by` (name, created\_at) and `order` (asc, desc) parameters.

     * Optimize `latest_version` description fetching (replace N+1 loop with Join or efficient map).
  3. **Enhance** **`GET /api/skills/:name`**:

     * Ensure complete object graph is returned.
  4. **Testing**: Create `api_tests.http` (Postman alternative) for VS Code REST Client to verify endpoints.

## 3. Frontend Implementation (React/Vite)

* **Goal**: Create high-performance List and Detail pages.

* **Infrastructure**:

  * Setup `react-router-dom` in `App.tsx`.

  * Create reusable UI components in `src/components/ui` (Button, Input, Table, Card, Badge, Skeleton) following shadcn/ui patterns.

* **Page: Skill List (`/skills`)**:

  * **Features**:

    * **Virtual Scrolling / Infinite Scroll**: Implement efficient rendering for large lists using backend pagination.

    * **Advanced Search**: Sidebar/Panel with keyword and sort options.

    * **Columns**: Toggleable columns for "Name", "Latest Version", "Created At".

* **Page: Skill Detail (`/skills/:name`)**:

  * **Features**:

    * **Header**: Skill info, status badge, install command.

    * **Content**: README rendering (using `react-markdown` if available, or plain text for now).

    * **Versions**: List of historical versions with download links.

    * **Optimization**: Skeleton loaders while fetching data.

## 4. Quality Assurance

* **Backend**: Run `cargo test` and verify API manually with http client.

* **Frontend**: Ensure clean console, responsive design check (1280px+).

