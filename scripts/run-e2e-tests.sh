#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND_DIR="$ROOT_DIR/backend"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_info() {
  echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
  echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
  echo -e "${RED}[ERROR]${NC} $1"
}

cleanup() {
  print_info "Cleaning up test environment..."

  if [ -f /tmp/e2e_worker.pid ]; then
    kill "$(cat /tmp/e2e_worker.pid)" 2>/dev/null || true
    rm -f /tmp/e2e_worker.pid
  fi

  if [ -f /tmp/e2e_api.pid ]; then
    kill "$(cat /tmp/e2e_api.pid)" 2>/dev/null || true
    rm -f /tmp/e2e_api.pid
  fi

  docker-compose -f "$ROOT_DIR/docker-compose.test.yml" down -v >/dev/null 2>&1 || true
}

load_env_token() {
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    return
  fi

  if [ -f "$ROOT_DIR/.env" ]; then
    set -a
    # shellcheck source=/dev/null
    source "$ROOT_DIR/.env"
    set +a
  fi

  if [ -z "${GITHUB_TOKEN:-}" ]; then
    print_error "GITHUB_TOKEN is required. Set it in environment or in $ROOT_DIR/.env"
    exit 1
  fi
}

wait_for_port() {
  local host="$1"
  local port="$2"
  local name="$3"
  local max_attempts="${4:-60}"

  for ((i=1; i<=max_attempts; i++)); do
    if bash -c "</dev/tcp/${host}/${port}" >/dev/null 2>&1; then
      print_info "$name is ready on ${host}:${port}"
      return 0
    fi
    sleep 2
  done

  print_error "$name did not become ready on ${host}:${port}"
  return 1
}

wait_for_temporal_health() {
  local max_attempts="${1:-60}"

  for ((i=1; i<=max_attempts; i++)); do
    if docker exec temporal-test temporal operator cluster health >/dev/null 2>&1; then
      print_info "Temporal cluster health is SERVING"
      return 0
    fi
    sleep 2
  done

  print_error "Temporal health did not become SERVING"
  return 1
}

run_test_or_fail() {
  local test_name="$1"
  local human_name="$2"

  print_info "Running E2E test ($human_name)..."
  set +e
  cargo test -p e2e-tests --test e2e "$test_name" -- --ignored --nocapture
  local test_exit_code=$?
  set -e

  if [ $test_exit_code -ne 0 ]; then
    print_error "$human_name E2E test failed"

    if [ -f /tmp/e2e_api.log ]; then
      print_warn "API log tail:"
      tail -n 200 /tmp/e2e_api.log || true
    fi

    if [ -f /tmp/e2e_worker.log ]; then
      print_warn "Worker log tail:"
      tail -n 200 /tmp/e2e_worker.log || true
    fi

    exit $test_exit_code
  fi
}

trap cleanup EXIT

print_info "Checking prerequisites..."
command -v docker >/dev/null 2>&1 || { print_error "Docker is not installed"; exit 1; }
command -v docker-compose >/dev/null 2>&1 || { print_error "Docker Compose is not installed"; exit 1; }
command -v cargo >/dev/null 2>&1 || { print_error "Cargo is not installed"; exit 1; }

load_env_token

print_info "Starting test services (RustFS + Temporal)..."
docker-compose -f "$ROOT_DIR/docker-compose.test.yml" down -v >/dev/null 2>&1 || true
docker-compose -f "$ROOT_DIR/docker-compose.test.yml" up -d

print_info "Waiting for services to be ready..."
wait_for_port "127.0.0.1" "9002" "RustFS" 60
wait_for_port "127.0.0.1" "7234" "Temporal" 60
wait_for_temporal_health 60

export SKILLREGISTRY_DATABASE__URL="sqlite:///tmp/skillregistry-e2e.db?mode=rwc"
export SKILLREGISTRY_TEMPORAL__SERVER_URL="http://localhost:7234"
export SKILLREGISTRY_TEMPORAL__TASK_QUEUE="skill-registry-queue"
export SKILLREGISTRY_S3__ENDPOINT="http://localhost:9002"
export SKILLREGISTRY_S3__BUCKET="skills"
export SKILLREGISTRY_S3__REGION="us-east-1"
export SKILLREGISTRY_S3__FORCE_PATH_STYLE="true"
export SKILLREGISTRY_S3__ACCESS_KEY_ID="rustfsadmin"
export SKILLREGISTRY_S3__SECRET_ACCESS_KEY="rustfsadmin"
export SKILLREGISTRY_GITHUB__TOKEN="$GITHUB_TOKEN"
export SKILLREGISTRY_GITHUB__SEARCH_KEYWORDS="${SKILLREGISTRY_GITHUB__SEARCH_KEYWORDS:-repo:anthropics/skills}"
export SKILLREGISTRY_AUTH__JWT__SIGNING_KEY="${SKILLREGISTRY_AUTH__JWT__SIGNING_KEY:-dev-secret-key-change-in-production}"
export SKILLREGISTRY_AUTH__JWT__ISSUER="${SKILLREGISTRY_AUTH__JWT__ISSUER:-skillregistry}"
export SKILLREGISTRY_AUTH__JWT__AUDIENCE="${SKILLREGISTRY_AUTH__JWT__AUDIENCE:-skillregistry}"

# Legacy aliases used by e2e-tests test harness.
export DATABASE_URL="$SKILLREGISTRY_DATABASE__URL"
export TEMPORAL_SERVER_URL="$SKILLREGISTRY_TEMPORAL__SERVER_URL"
export SKILLREGISTRY_TEMPORAL_TASK_QUEUE="$SKILLREGISTRY_TEMPORAL__TASK_QUEUE"
export S3_ENDPOINT="$SKILLREGISTRY_S3__ENDPOINT"
export S3_BUCKET="$SKILLREGISTRY_S3__BUCKET"
export S3_REGION="$SKILLREGISTRY_S3__REGION"
export S3_FORCE_PATH_STYLE="$SKILLREGISTRY_S3__FORCE_PATH_STYLE"
export AWS_ACCESS_KEY_ID="$SKILLREGISTRY_S3__ACCESS_KEY_ID"
export AWS_SECRET_ACCESS_KEY="$SKILLREGISTRY_S3__SECRET_ACCESS_KEY"

# API and test-specific settings.
export E2E_API_BASE_URL="${E2E_API_BASE_URL:-http://localhost:3000}"
export E2E_ADMIN_USERNAME="${E2E_ADMIN_USERNAME:-admin}"
export E2E_ADMIN_PASSWORD="${E2E_ADMIN_PASSWORD:-admin}"
export E2E_QUERY_RECENT_SKILL_RAW="${E2E_QUERY_RECENT_SKILL_RAW:-repo:anthropics/skills filename:SKILL.md \"name:\" \"description:\"}"
export E2E_QUERY_MARKETPLACE_RAW="${E2E_QUERY_MARKETPLACE_RAW:-repo:wshobson/agents filename:marketplace.json path:.claude-plugin}"
export E2E_QUERY_POLL_TIMEOUT_SECS="${E2E_QUERY_POLL_TIMEOUT_SECS:-900}"

# Existing test settings.
export SKILLREGISTRY_SETUP_SKIP_TEMPORAL="true"
export E2E_DISCOVERY_QUERY="${E2E_DISCOVERY_QUERY:-repo:anthropics/skills}"
export E2E_TARGET_OWNER="${E2E_TARGET_OWNER:-anthropics}"
export E2E_TARGET_REPO="${E2E_TARGET_REPO:-skills}"
export E2E_DISCOVERY_TIMEOUT_SECS="${E2E_DISCOVERY_TIMEOUT_SECS:-360}"
export E2E_SYNC_TIMEOUT_SECS="${E2E_SYNC_TIMEOUT_SECS:-600}"
export RUST_LOG="${RUST_LOG:-info}"
RUN_LEGACY_E2E="${RUN_LEGACY_E2E:-0}"
RUN_ADMIN_E2E="${RUN_ADMIN_E2E:-1}"

cd "$BACKEND_DIR"
rm -f /tmp/skillregistry-e2e.db

print_info "Running setup (migrations + S3 bucket)..."
cargo run --bin setup

print_info "Starting worker..."
cargo run --bin worker > /tmp/e2e_worker.log 2>&1 &
WORKER_PID=$!
echo "$WORKER_PID" > /tmp/e2e_worker.pid
sleep 8

print_info "Starting API..."
cargo run --bin api > /tmp/e2e_api.log 2>&1 &
API_PID=$!
echo "$API_PID" > /tmp/e2e_api.pid
sleep 5
wait_for_port "127.0.0.1" "3000" "API" 60

if [ "$RUN_LEGACY_E2E" = "1" ]; then
  run_test_or_fail "test_discovery_sync_and_upload" "discovery + upload"
else
  print_info "Skipping legacy discovery+upload e2e (RUN_LEGACY_E2E=$RUN_LEGACY_E2E)"
fi

if [ "$RUN_ADMIN_E2E" = "1" ]; then
  run_test_or_fail "test_admin_registry_trigger_and_index_two_queries" "admin registry + trigger + index"
else
  print_info "Skipping admin registry+trigger+index e2e (RUN_ADMIN_E2E=$RUN_ADMIN_E2E)"
fi

print_info "E2E tests passed"
