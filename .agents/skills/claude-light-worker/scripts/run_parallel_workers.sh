#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<"USAGE"
Run multiple Claude workers concurrently from a jobs file.

Usage:
  run_parallel_workers.sh --jobs-file JOBS.tsv [--log-dir DIR] [--default-timeout SECONDS] [--dry-run]

Jobs file format (tab-separated, one job per line):
  job_id<TAB>workdir<TAB>prompt_file[<TAB>timeout_seconds]

Examples:
  worker_unit\t/tmp/.wortree/w-unit\t/tmp/prompts/unit.txt\t150
  worker_docs\t/tmp/.wortree/w-docs\t/tmp/prompts/docs.txt

Notes:
- Lines starting with # and empty lines are ignored.
- Output files are written to <log-dir>/<job_id>.json and <log-dir>/<job_id>.err.
USAGE
}

jobs_file=""
log_dir="/tmp/.wortree/worker-logs"
default_timeout="120"
dry_run="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --jobs-file)
      jobs_file="${2:-}"
      shift 2
      ;;
    --log-dir)
      log_dir="${2:-}"
      shift 2
      ;;
    --default-timeout)
      default_timeout="${2:-}"
      shift 2
      ;;
    --dry-run)
      dry_run="true"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$jobs_file" ]]; then
  echo "Missing required --jobs-file" >&2
  usage >&2
  exit 1
fi
if [[ ! -f "$jobs_file" ]]; then
  echo "Jobs file not found: $jobs_file" >&2
  exit 1
fi
if [[ ! "$default_timeout" =~ ^[0-9]+$ ]]; then
  echo "default-timeout must be an integer" >&2
  exit 1
fi

mkdir -p "$log_dir"

declare -a pids=()
declare -a ids=()

while IFS=$'\t' read -r job_id workdir prompt_file timeout_sec _extra; do
  # Skip comments and empty lines.
  if [[ -z "${job_id}" || "${job_id}" =~ ^# ]]; then
    continue
  fi

  if [[ -z "${workdir:-}" || -z "${prompt_file:-}" ]]; then
    echo "Invalid row in jobs file: $job_id" >&2
    echo "Expected: job_id<TAB>workdir<TAB>prompt_file[<TAB>timeout_seconds]" >&2
    exit 1
  fi
  if [[ ! -d "$workdir" ]]; then
    echo "Workdir not found for $job_id: $workdir" >&2
    exit 1
  fi
  if [[ ! -f "$prompt_file" ]]; then
    echo "Prompt file not found for $job_id: $prompt_file" >&2
    exit 1
  fi

  timeout_value="${timeout_sec:-$default_timeout}"
  if [[ -z "$timeout_value" ]]; then
    timeout_value="$default_timeout"
  fi
  if [[ ! "$timeout_value" =~ ^[0-9]+$ ]]; then
    echo "Timeout must be integer for $job_id (got: $timeout_value)" >&2
    exit 1
  fi

  out_json="$log_dir/$job_id.json"
  out_err="$log_dir/$job_id.err"

  cmd="cd \"$workdir\" && timeout ${timeout_value}s claude -p --output-format json \"\$(cat \"$prompt_file\")\" > \"$out_json\" 2> \"$out_err\""

  if [[ "$dry_run" == "true" ]]; then
    echo "[DRY-RUN][$job_id] $cmd"
    continue
  fi

  bash -lc "$cmd" &
  pid=$!
  pids+=("$pid")
  ids+=("$job_id")
  echo "[STARTED][$job_id] pid=$pid"
done < "$jobs_file"

if [[ "$dry_run" == "true" ]]; then
  exit 0
fi

if [[ ${#pids[@]} -eq 0 ]]; then
  echo "No jobs launched (jobs file had no runnable rows)." >&2
  exit 1
fi

echo "[WAIT] Waiting for ${#pids[@]} worker(s) to finish..."
failures=0

for i in "${!pids[@]}"; do
  pid="${pids[$i]}"
  job_id="${ids[$i]}"
  if wait "$pid"; then
    echo "[DONE][$job_id] exit=0"
  else
    code=$?
    echo "[DONE][$job_id] exit=$code"
    failures=$((failures + 1))
  fi
done

if [[ $failures -gt 0 ]]; then
  echo "Completed with $failures failure(s)." >&2
  exit 1
fi

echo "All workers completed successfully."
