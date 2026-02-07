# UI Enhancements Requiring Backend Support (Next Iteration)

This document tracks UI improvements that were intentionally deferred because they require new or expanded backend capabilities.

## 1) Real Skill Download / Install Action

### User-facing goal
Provide a working primary action on each skill row/detail page (download package or fetch install artifact), instead of copy-only guidance.

### Backend requirements
- Add a download endpoint for the latest skill artifact.
- Recommended route:
  - `GET /api/skills/{owner}/{repo}/{name}/download`
- Response options:
  - `302` redirect to a signed S3 URL, or
  - JSON payload with `download_url`, `expires_at`, `md5`, `version`.
- Return clear errors for missing artifact / unauthorized / expired URLs.

## 2) Leaderboard Sorting and Filtering

### User-facing goal
Enable real `Filter` and `Sort` controls on the leaderboard.

### Backend requirements
- Support sort keys and directions:
  - `installs`, `updated_at`, `created_at`, `stars`, `name`
  - `sort_by`, `sort_order`
- Support filters:
  - owner, repo, compatibility/tool tags, version presence
- Return pagination metadata:
  - `total`, `page`, `per_page`, `has_next`
- Optional facet counts for better filter UX.

## 3) Rich Skill Metadata for Detail Sidebar

### User-facing goal
Replace placeholder sidebar values with real metadata.

### Backend requirements
Include these fields in skill detail responses:
- `license`
- `compatibility`
- `allowed_tools`
- `install_count`
- `last_synced_at`
- optional `homepage` / `documentation_url`

## 4) Discovery Registry Operational Signals

### User-facing goal
Show high-signal operations status in admin UI (not just raw `ok/unknown`).

### Backend requirements
- Add structured status payload fields per registry:
  - `last_health_status`
  - `last_health_message`
  - `last_health_checked_at`
  - `last_run_at`
  - `last_run_status`
  - `last_run_message`
- Provide trigger/test response payloads with stable shape:
  - `ok`, `message`, `workflow_id`, `started_at`

## 5) Safer Destructive Actions

### User-facing goal
Use typed confirmation (e.g. enter registry ID) for deletion in admin UI.

### Backend requirements
- Option A: support pre-delete validation endpoint.
- Option B: extend delete endpoint to accept confirmation token/challenge.
- Return structured error reasons for display in UI.

## Suggested Delivery Order
1. Download endpoint + metadata fields (highest visible impact)
2. Sorting/filtering + pagination metadata
3. Discovery operational status enrichment
4. Safer delete confirmation flow
