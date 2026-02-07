#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
EXAMPLES_DIR="${SKILL_ROOT}/examples"

ALL_PACKAGES=(
  helloworld
  batch-sliding-window
  saga
  localactivity
  struct-activity
)

PACKAGES=(helloworld struct-activity)
CUSTOM_PACKAGES=0
RUNTIME_PACKAGE="helloworld"
ENABLE_RUNTIME=1
SKIP_COMPILE=0
DRY_RUN=0

usage() {
  cat <<USAGE
Usage: $(basename "$0") [options]

Options:
  --package <name>        Add package for compile smoke checks. Repeatable.
  --all-packages          Compile smoke check all example packages.
  --runtime-package <name> Package used for runtime smoke check.
  --skip-runtime          Skip runtime smoke check even if server is available.
  --skip-compile          Skip compile smoke checks.
  --dry-run               Print commands without executing.
  -h, --help              Show this help.
USAGE
}

contains_package() {
  local package="$1"
  shift
  local entry
  for entry in "$@"; do
    if [[ "${entry}" == "${package}" ]]; then
      return 0
    fi
  done
  return 1
}

require_command() {
  local cmd="$1"
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "[smoke] Missing required command: ${cmd}" >&2
    exit 1
  fi
}

temporal_server_up() {
  if command -v nc >/dev/null 2>&1; then
    if nc -z 127.0.0.1 7233 >/dev/null 2>&1; then
      return 0
    fi
  fi
  if (echo >/dev/tcp/127.0.0.1/7233) >/dev/null 2>&1; then
    return 0
  fi
  return 1
}

starter_args_for_package() {
  local package="$1"
  case "${package}" in
    helloworld|struct-activity)
      echo "--name SmokeUser"
      ;;
    *)
      echo ""
      ;;
  esac
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --package)
      shift
      if [[ $# -eq 0 ]]; then
        echo "[smoke] Missing value for --package" >&2
        exit 1
      fi
      if [[ ${CUSTOM_PACKAGES} -eq 0 ]]; then
        PACKAGES=()
        CUSTOM_PACKAGES=1
      fi
      PACKAGES+=("$1")
      ;;
    --all-packages)
      PACKAGES=("${ALL_PACKAGES[@]}")
      CUSTOM_PACKAGES=1
      ;;
    --runtime-package)
      shift
      if [[ $# -eq 0 ]]; then
        echo "[smoke] Missing value for --runtime-package" >&2
        exit 1
      fi
      RUNTIME_PACKAGE="$1"
      ;;
    --skip-runtime)
      ENABLE_RUNTIME=0
      ;;
    --skip-compile)
      SKIP_COMPILE=1
      ;;
    --dry-run)
      DRY_RUN=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[smoke] Unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
  shift
done

require_command cargo

for package in "${PACKAGES[@]}"; do
  if ! contains_package "${package}" "${ALL_PACKAGES[@]}"; then
    echo "[smoke] Unsupported package: ${package}" >&2
    echo "[smoke] Supported packages: ${ALL_PACKAGES[*]}" >&2
    exit 1
  fi
done

if ! contains_package "${RUNTIME_PACKAGE}" "${ALL_PACKAGES[@]}"; then
  echo "[smoke] Unsupported runtime package: ${RUNTIME_PACKAGE}" >&2
  exit 1
fi

if [[ ${DRY_RUN} -eq 1 ]]; then
  echo "[smoke] Dry run mode enabled"
fi

pushd "${EXAMPLES_DIR}" >/dev/null

if [[ ${SKIP_COMPILE} -eq 0 ]]; then
  for package in "${PACKAGES[@]}"; do
    echo "[smoke] Compile check package: ${package}"
    if [[ ${DRY_RUN} -eq 1 ]]; then
      echo "[dry-run] cargo check -p ${package}"
    else
      cargo check -p "${package}"
    fi
  done
else
  echo "[smoke] Compile checks skipped"
fi

if [[ ${ENABLE_RUNTIME} -eq 1 ]]; then
  if temporal_server_up; then
    echo "[smoke] Temporal server detected at 127.0.0.1:7233"
    echo "[smoke] Runtime check package: ${RUNTIME_PACKAGE}"

    runtime_log_dir="${EXAMPLES_DIR}/target/smoke-logs"
    worker_log="${runtime_log_dir}/${RUNTIME_PACKAGE}-worker.log"
    starter_log="${runtime_log_dir}/${RUNTIME_PACKAGE}-starter.log"
    mkdir -p "${runtime_log_dir}"

    starter_args="$(starter_args_for_package "${RUNTIME_PACKAGE}")"

    if [[ ${DRY_RUN} -eq 1 ]]; then
      echo "[dry-run] timeout 45s cargo run -p ${RUNTIME_PACKAGE} -- worker > ${worker_log} 2>&1 &"
      echo "[dry-run] timeout 45s cargo run -p ${RUNTIME_PACKAGE} -- starter ${starter_args} > ${starter_log} 2>&1"
    else
      timeout 45s cargo run -p "${RUNTIME_PACKAGE}" -- worker >"${worker_log}" 2>&1 &
      worker_pid=$!
      sleep 5

      set +e
      if [[ -n "${starter_args}" ]]; then
        timeout 45s cargo run -p "${RUNTIME_PACKAGE}" -- starter ${starter_args} >"${starter_log}" 2>&1
      else
        timeout 45s cargo run -p "${RUNTIME_PACKAGE}" -- starter >"${starter_log}" 2>&1
      fi
      starter_status=$?
      set -e

      kill "${worker_pid}" >/dev/null 2>&1 || true
      wait "${worker_pid}" 2>/dev/null || true

      if [[ ${starter_status} -ne 0 ]]; then
        echo "[smoke] Runtime check failed for package ${RUNTIME_PACKAGE}" >&2
        echo "[smoke] Worker log: ${worker_log}" >&2
        echo "[smoke] Starter log: ${starter_log}" >&2
        exit 1
      fi

      echo "[smoke] Runtime check passed for package ${RUNTIME_PACKAGE}"
      echo "[smoke] Worker log: ${worker_log}"
      echo "[smoke] Starter log: ${starter_log}"
    fi
  else
    echo "[smoke] Temporal server is not reachable at 127.0.0.1:7233"
    echo "[smoke] Runtime check skipped"
    echo "[smoke] Start server and rerun for runtime validation"
  fi
else
  echo "[smoke] Runtime check skipped by option"
fi

popd >/dev/null

echo "[smoke] Done"
