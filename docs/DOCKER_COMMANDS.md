# Docker Command Reference

This document summarizes frequently used Docker and Docker Compose commands for local development and troubleshooting.

## Prerequisites

```bash
docker --version
docker compose version
docker info
```

## Image Commands

```bash
# Build image from current directory Dockerfile
docker build -t my-image:latest .

# List local images
docker images

# Remove one image
docker rmi my-image:latest

# Remove dangling images
docker image prune
```

## Container Commands

```bash
# Run container in foreground
docker run --rm my-image:latest

# Run container in background and publish a port
docker run -d --name my-app -p 8080:80 my-image:latest

# List running containers
docker ps

# List all containers (including exited)
docker ps -a

# Show container logs
docker logs -f my-app

# Execute shell inside a running container
docker exec -it my-app sh

# Stop and remove container
docker stop my-app && docker rm my-app
```

## Docker Compose Commands

```bash
# Validate and render effective compose config
docker compose config

# Build all services
docker compose build

# Build with BuildKit and plain progress output
DOCKER_BUILDKIT=1 COMPOSE_DOCKER_CLI_BUILD=1 docker compose --progress=plain build

# Start all services in background
docker compose up -d

# Stop and remove services, networks, and anonymous volumes
docker compose down

# Stop and remove services, networks, and all volumes
docker compose down -v

# Follow logs from specific services
docker compose logs -f skillregistry-backend skillregistry-worker

# Rebuild and restart one service
docker compose up -d --build skillregistry-backend

# Open a shell in a running service container
docker compose exec skillregistry-backend sh
```

## Project-Specific Quick Commands

Run from repository root:

```bash
# Build all project images
DOCKER_BUILDKIT=1 COMPOSE_DOCKER_CLI_BUILD=1 docker compose --progress=plain build

# Start infra and app services
docker compose up -d

# Check service status
docker compose ps

# Tail backend and worker logs
docker compose logs -f skillregistry-backend skillregistry-worker

# Tear down everything including volumes
docker compose down -v
```

## Makefile Shortcuts

If you prefer short commands, use the project `Makefile`:

```bash
# List available targets
make help

# Build with BuildKit and plain output
make build-plain

# Start services
make up

# Build, start, and follow backend + worker logs
make dev

# Recreate stack (down -v, build, up)
make reset
```

## Cleanup and Disk Usage

```bash
# Show Docker disk usage summary
docker system df

# Remove stopped containers
docker container prune

# Remove unused networks
docker network prune

# Remove unused volumes
docker volume prune

# Remove all unused images, containers, networks, build cache
docker system prune -a
```

## Debugging Tips

- Use `docker compose config` first when compose fields appear invalid.
- Use `--progress=plain` for verbose build logs.
- Use `docker compose logs -f <service>` to identify startup/runtime errors.
- Use `docker exec -it <container> sh` or `docker compose exec <service> sh` for live inspection.
- If build cache behaves unexpectedly, try `docker builder prune` before rebuilding.
