# Database Structure Review Report

## 1. Missing Fields
- **Skills Table**:
  - `status`: Missing. Recommended to add `status` (VARCHAR) to track `active`, `deprecated`, `archived`.
  - `installs`: Missing. Recommended to add `installs` (INT) for leaderboard sorting.
- **Skill Versions Table**:
  - `changelog`: Missing. Useful for displaying update history in UI.

## 2. Performance & Indices
- **Current State**:
  - `skills.name` is UNIQUE (Global). **Issue**: Prevents same-named skills in different repos.
- **Recommendations**:
  - **Remove** Unique constraint on `skills.name`.
  - **Add** Composite Unique Index on `skills(skill_registry_id, name)`.
  - **Add** Index on `skills(created_at)` for sorting.
  - **Add** Index on `skill_registry(owner)` for Org List filtering.
  - **Add** Index on `skill_registry(owner, name)` for Repo List filtering.

## 3. Integrity
- Foreign Keys seem correct (`skill_registry_id`, `skill_id`).
- Ensure `ON DELETE CASCADE` is set for `skill_versions` when deleting a `skill`.

## 4. Schema Modification Plan
- Modify `backend/common/src/entities/skills.rs` to remove unique constraint on `name`.
- Update API logic to identify skills by `owner/repo/name` triple instead of just `name`.
