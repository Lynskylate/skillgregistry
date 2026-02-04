#!/bin/bash

# E2E Test Runner Script
# This script sets up and runs end-to-end tests locally

set -e

echo "========================================="
echo "Setting up E2E Test Environment"
echo "========================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored messages
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Cleanup function
cleanup() {
    print_info "Cleaning up test environment..."
    docker-compose -f docker-compose.test.yml down -v 2>/dev/null || true
    
    # Kill any remaining processes
    if [ -f /tmp/e2e_api.pid ]; then
        kill $(cat /tmp/e2e_api.pid) 2>/dev/null || true
        rm /tmp/e2e_api.pid
    fi
    if [ -f /tmp/e2e_worker.pid ]; then
        kill $(cat /tmp/e2e_worker.pid) 2>/dev/null || true
        rm /tmp/e2e_worker.pid
    fi
}

# Register cleanup on exit
trap cleanup EXIT

# Check prerequisites
print_info "Checking prerequisites..."
command -v docker >/dev/null 2>&1 || { print_error "Docker is not installed"; exit 1; }
command -v docker-compose >/dev/null 2>&1 || { print_error "Docker Compose is not installed"; exit 1; }
command -v cargo >/dev/null 2>&1 || { print_error "Cargo is not installed"; exit 1; }

# Start test infrastructure
print_info "Starting test infrastructure (PostgreSQL, Temporal, S3)..."
docker-compose -f docker-compose.test.yml up -d

# Wait for services to be ready
print_info "Waiting for services to be healthy..."
sleep 10

# Check PostgreSQL
print_info "Checking PostgreSQL..."
docker-compose -f docker-compose.test.yml exec -T postgres-test pg_isready -U postgres || {
    print_error "PostgreSQL is not ready"
    exit 1
}

# Set environment variables
export DATABASE_URL="postgres://postgres:password@localhost:5433/skillregistry_test"
export API_URL="http://localhost:3000"
export TEMPORAL_SERVER_URL="http://localhost:7234"
export S3_ENDPOINT="http://localhost:9002"
export AWS_ACCESS_KEY_ID="rustfsadmin"
export AWS_SECRET_ACCESS_KEY="rustfsadmin"
export S3_BUCKET="skills"
export S3_REGION="us-east-1"
export RUST_LOG="info"

# Navigate to backend directory
cd backend

# Run migrations
print_info "Running database migrations..."
cargo run --bin setup

# Build the project
print_info "Building backend..."
cargo build --release

# Start API server
print_info "Starting API server..."
cargo run --bin api &
API_PID=$!
echo $API_PID > /tmp/e2e_api.pid
sleep 5

# Verify API is running
if ! curl -s http://localhost:3000/health > /dev/null; then
    print_error "API server failed to start"
    exit 1
fi
print_info "API server is running (PID: $API_PID)"

# Start Worker
print_info "Starting Worker..."
cargo run --bin worker &
WORKER_PID=$!
echo $WORKER_PID > /tmp/e2e_worker.pid
sleep 5

print_info "Worker is running (PID: $WORKER_PID)"

# Run E2E tests
print_info "Running E2E tests..."
echo "========================================="
cargo test -p e2e-tests --test e2e -- --ignored --nocapture

# Test results
if [ $? -eq 0 ]; then
    print_info "========================================="
    print_info "All E2E tests passed! ✓"
    print_info "========================================="
else
    print_error "========================================="
    print_error "Some E2E tests failed! ✗"
    print_error "========================================="
    exit 1
fi