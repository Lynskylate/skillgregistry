SHELL := /bin/bash

COMPOSE := docker compose
BUILD_ENV := DOCKER_BUILDKIT=1 COMPOSE_DOCKER_CLI_BUILD=1

.PHONY: help build build-plain up up-build down down-v ps config logs logs-app logs-backend logs-worker shell-backend shell-worker shell-setup rebuild-backend dev reset clean prune prune-all

help:
	@echo "Common commands:"
	@echo "  make build          - Build all services"
	@echo "  make build-plain    - Build with BuildKit + plain output"
	@echo "  make up             - Start all services in background"
	@echo "  make down           - Stop and remove services"
	@echo "  make down-v         - Stop and remove services and volumes"
	@echo "  make ps             - Show service status"
	@echo "  make logs-app       - Follow backend + worker logs"
	@echo "  make shell-backend  - Open shell in backend container"
	@echo "  make dev            - Build, start, and follow app logs"
	@echo "  make reset          - Recreate stack (down -v, build, up)"

build:
	$(COMPOSE) build

build-plain:
	$(BUILD_ENV) $(COMPOSE) --progress=plain build

up:
	$(COMPOSE) up -d

up-build:
	$(COMPOSE) up -d --build

down:
	$(COMPOSE) down

down-v:
	$(COMPOSE) down -v

ps:
	$(COMPOSE) ps

config:
	$(COMPOSE) config

logs:
	$(COMPOSE) logs -f

logs-app:
	$(COMPOSE) logs -f skillregistry-backend skillregistry-worker

logs-backend:
	$(COMPOSE) logs -f skillregistry-backend

logs-worker:
	$(COMPOSE) logs -f skillregistry-worker

shell-backend:
	$(COMPOSE) exec skillregistry-backend sh

shell-worker:
	$(COMPOSE) exec skillregistry-worker sh

shell-setup:
	$(COMPOSE) exec skillregistry-setup sh

rebuild-backend:
	$(COMPOSE) up -d --build skillregistry-backend

dev:
	$(BUILD_ENV) $(COMPOSE) --progress=plain build
	$(COMPOSE) up -d
	$(COMPOSE) logs -f skillregistry-backend skillregistry-worker

reset:
	$(COMPOSE) down -v
	$(BUILD_ENV) $(COMPOSE) --progress=plain build
	$(COMPOSE) up -d

clean:
	docker system df

prune:
	docker system prune -f

prune-all:
	docker system prune -a -f
