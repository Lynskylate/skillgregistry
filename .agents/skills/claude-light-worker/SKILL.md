---
name: claude-light-worker
description: Delegate lightweight tasks to Claude CLI as a lower-cost worker in isolated git worktrees. Use when dispatching unit-test drafting, docs updates, quick codebase exploration, small refactors, or patch proposals, especially when you want parallel background workers and safe merge-back workflow.
---

# Claude Light Worker

Use Claude as a cheaper subordinate worker for small, well-scoped tasks. Isolate each worker in its own worktree, validate outputs, and merge back only verified changes.

## Good Task Types

- Unit-test drafting for a specific module.
- Documentation edits or short writing tasks.
- Codebase exploration and structured findings.
- Small patch proposals with strict file scope.

Keep each assignment narrow and independent.

## Delegation Workflow

1. Define one concrete objective and acceptance checks.
2. Create a dedicated worktree at `/tmp/.wortree/{feature}`.
3. Run Claude in non-interactive mode with timeout.
4. Collect output into per-worker log files.
5. Validate results (tests, lint, or manual inspection).
6. Commit in worker branch.
7. Merge verified branch into main branch.
8. Remove worker worktree and branch.

## Worktree Setup

```bash
mkdir -p /tmp/.wortree
git worktree add -b <feature> /tmp/.wortree/<feature> HEAD
```

Use short feature names like `chore/claude-unit-test-skill`.

## Prompt Templates

Patch-oriented task:

```text
You are a coding worker in an isolated worktree.
Task: <exact objective>
Files in scope: <path list>
Constraints:
- Match current implementation behavior unless explicitly changing behavior.
- Keep output deterministic and minimal.
- Return ONLY a unified diff patch against <target paths>. No prose.
Validation target:
- <exact command>
```

Exploration or docs task with structured output:

```text
You are a lightweight worker.
Task: <analysis or docs objective>
Return ONLY valid JSON with keys:
- summary
- actions
- risks
```

## Claude Command Patterns

Single worker:

```bash
timeout 120s claude -p --output-format json "$PROMPT"
```

Parallel background workers:

```bash
(
  cd /tmp/.wortree/<feature-a> && timeout 120s claude -p --output-format json "$PROMPT_A" > worker-a.json
) &
(
  cd /tmp/.wortree/<feature-b> && timeout 120s claude -p --output-format json "$PROMPT_B" > worker-b.json
) &
wait
```

Reusable launcher script:

```bash
./scripts/run_parallel_workers.sh --jobs-file /tmp/.wortree/jobs.tsv
```

`jobs.tsv` rows are tab-separated: `job_id`, `workdir`, `prompt_file`, optional `timeout_seconds`.

Optional schema enforcement for structured output:

```bash
claude -p --output-format json --json-schema "$SCHEMA" "$PROMPT"
```

If schema mode is slow or unstable, keep `--output-format json` and enforce JSON shape in the prompt.

Normalize worker output before applying it:

```bash
./scripts/extract_result.py /tmp/.wortree/worker-logs/<job>.json --strip-fences --expect diff
./scripts/extract_result.py /tmp/.wortree/worker-logs/<job>.json --strip-fences --expect json
```

If extraction fails, tighten prompt and rerun that worker.

If sandbox blocks writes under `~/.claude`, rerun with escalated permissions.

## Validation and Merge

In worker worktree:

```bash
cargo test -p <crate> <filter> -- --nocapture
git add <files>
git commit -m "<message>"
```

Back in main workspace:

```bash
git merge --no-ff <feature>
git worktree remove /tmp/.wortree/<feature>
git branch -d <feature>
```

## Quality Gates

- Reject outputs outside declared file scope.
- Reject prose-only answers when patch was requested.
- Require deterministic assertions for unit tests.
- Require concise, machine-readable structure for explore/docs tasks.
- Add one prompt constraint after each failed iteration.

## Common Failure Patterns

- Worker infers desired behavior instead of current behavior.
  - Add explicit line: "match current implementation behavior."
- Worker emits markdown fences around patch.
  - Add: "Return ONLY unified diff patch. No prose."
- Worker hangs.
  - Enforce `timeout` and narrow prompt scope.
- `$skill-name` expands in shell unexpectedly.
  - Use single-quoted heredoc or escape `$` in prompt construction.
