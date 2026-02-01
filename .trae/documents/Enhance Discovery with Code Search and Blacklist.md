Based on your request, I will enhance the discovery and sync tasks and verify them using the configuration.

### 1. Enhance GitHub Client
*   **Add `search_code` method**: Implement code search functionality to support searching for files like `SKILL.md`.
*   **Add `get_repository` method**: Needed to fetch full repo details when a code search result returns only basic info or file info.

### 2. Implement Blacklist Mechanism
*   **Create `blacklist` entity**: Add a new table `blacklist` to store invalid repositories.
*   **Update DB Schema**: Add `blacklist` table migration in `db.rs`.
*   **Update Discovery Task**: Check against blacklist before adding new repositories.
*   **Update Sync Task**: If validation fails (no `SKILL.md` or invalid format), add the repo to the blacklist and remove it from `skill_registry`.
*   **Cleanup Task**: Implement a periodic cleanup for the blacklist (though the user said "can be cleared periodically", I'll add a simple function or separate task for it).

### 3. Enhance Discovery Task
*   **Multiple Queries**: Update `worker/src/tasks/discovery.rs` to split `SEARCH_KEYWORDS` by comma and execute multiple searches.
*   **Code Search Support**: If a keyword contains `filename:` or `path:`, use `search_code` instead of `search_repositories`.
*   **Sorting**: For `search_repositories`, ensure `sort:updated` is used. For `search_code`, sorting is less flexible but we can fetch results and process.

### 4. Configuration & Verification
*   **Configuration**: Ensure `common/src/config.rs` and `.env` support the new requirements.
*   **Verification**:
    *   Create a `.env` file with test configuration (using `topic:agent-skill` and a code search query like `filename:SKILL.md path:/`).
    *   Run the worker and verify logs to ensure it discovers repos and handles validation.
    *   (Self-Correction): Since I cannot authenticate with a real GitHub token, I will implement the logic robustly. If execution fails due to auth, I will explain that the logic is in place but requires a valid token. However, `search/repositories` works without auth (rate limited), but `search/code` **requires authentication**. I will add a fallback or clear warning.

### Plan Steps:
1.  Create `blacklist` entity and update `db.rs`.
2.  Update `worker/src/github.rs` to add `search_code`.
3.  Update `worker/src/tasks/discovery.rs` to support multiple queries and code search.
4.  Update `worker/src/tasks/sync.rs` to implement validation and blacklisting logic.
5.  Create `.env` file.
6.  Run and verify.
