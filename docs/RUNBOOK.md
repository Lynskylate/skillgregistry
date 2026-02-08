# Deployment Runbook

This runbook covers deployment procedures, monitoring, alerting, common issues, and rollback procedures for the Agent Skill Registry.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Deployment Procedures](#deployment-procedures)
- [Monitoring and Alerts](#monitoring-and-alerts)
- [Health Checks](#health-checks)
- [Common Issues and Fixes](#common-issues-and-fixes)
- [Rollback Procedures](#rollback-procedures)
- [Maintenance Tasks](#maintenance-tasks)
- [Disaster Recovery](#disaster-recovery)

## Architecture Overview

### System Components

```
┌─────────────────┐     ┌─────────────────┐
│   Frontend      │────▶│      API        │
│  (React/Vite)   │     │   (Axum/Rust)   │
└─────────────────┘     └────────┬────────┘
                                 │
                    ┌────────────┼────────────┐
                    ▼            ▼            ▼
            ┌───────────┐ ┌──────────┐ ┌──────────────┐
            │PostgreSQL │ │   S3     │ │  Temporal    │
            │ Database  │ │ Storage  │ │  Server      │
            └───────────┘ └──────────┘ └──────────────┘
                                 ▲
                    ┌────────────┴────────────┐
                    ▼                         ▼
            ┌───────────┐              ┌──────────┐
            │  Worker   │◀─────────────│ Temporal │
            │ Processor │  Work Queue  │  Server  │
            └───────────┘              └──────────┘
```

### Service Dependencies

- **Frontend** → API (HTTP)
- **API** → PostgreSQL, S3, Temporal
- **Worker** → GitHub API, PostgreSQL, S3, Temporal

### Infrastructure Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 2 cores | 4+ cores |
| RAM | 4 GB | 8+ GB |
| Storage | 20 GB | 50+ GB SSD |
| Database | PostgreSQL 15+ | PostgreSQL 15+ with replication |
| S3 | Any S3-compatible | AWS S3 / MinIO cluster |

## Deployment Procedures

### Prerequisites

1. **Required Environment Variables** (see `.env.example` for complete list)
2. **GitHub Personal Access Token** with `repo` scope
3. **S3 Storage** credentials and bucket
4. **PostgreSQL** database
5. **Temporal Server** (or use managed Temporal Cloud)

### Local Development Deployment

#### Quick Start

```bash
# Clone repository
git clone https://github.com/Lynskylate/skillgregistry.git
cd skillgregistry

# Configure environment
cp .env.example backend/.env
# Edit backend/.env with your settings

# Deploy with Docker Compose
make dev
```

#### Manual Deployment

```bash
# Start infrastructure
docker compose up -d postgres rustfs temporal

# Run database migrations
docker compose run --rm skillregistry-setup

# Start application
docker compose up -d skillregistry-backend skillregistry-worker skillregistry-frontend
```

### Production Deployment

#### Docker Image Build

```bash
# Build images locally
DOCKER_BUILDKIT=1 COMPOSE_DOCKER_CLI_BUILD=1 docker compose --progress=plain build

# Or use GitHub Actions to build and push to GHCR
# Images are automatically built on push to main
```

#### Production Docker Compose

Create `docker-compose.prod.yml`:

```yaml
services:
  postgres:
    image: postgres:15-alpine
    environment:
      POSTGRES_USER: ${DB_USER}
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: skillregistry
    volumes:
      - postgres_data:/var/lib/postgresql/data
    restart: always

  skillregistry-backend:
    image: ghcr.io/Lynskylate/skillgregistry:latest
    environment:
      - SKILLREGISTRY_DATABASE__URL=${DATABASE_URL}
      - SKILLREGISTRY_S3__ENDPOINT=${S3_ENDPOINT}
      - SKILLREGISTRY_S3__BUCKET=${S3_BUCKET}
      - SKILLREGISTRY_S3__REGION=${S3_REGION}
      - SKILLREGISTRY_S3__ACCESS_KEY_ID=${S3_ACCESS_KEY}
      - SKILLREGISTRY_S3__SECRET_ACCESS_KEY=${S3_SECRET_KEY}
      - SKILLREGISTRY_TEMPORAL__SERVER_URL=${TEMPORAL_URL}
      - SKILLREGISTRY_AUTH__JWT__SIGNING_KEY=${JWT_SIGNING_KEY}
      - SKILLREGISTRY_AUTH__JWT__ISSUER=${JWT_ISSUER}
      - SKILLREGISTRY_AUTH__JWT__AUDIENCE=${JWT_AUDIENCE}
      - SKILLREGISTRY_DEBUG=false
      - SKILLREGISTRY_AUTH__FRONTEND_ORIGIN=${FRONTEND_ORIGIN}
      - SKILLREGISTRY_AUTH__COOKIE_DOMAIN=${COOKIE_DOMAIN}
      - RUST_LOG=info
    ports:
      - "3000:3000"
    restart: always
    depends_on:
      - postgres

  skillregistry-worker:
    image: ghcr.io/Lynskylate/skillgregistry:latest
    command: ["/app/worker"]
    environment:
      # Same as backend above
    restart: always
    depends_on:
      - postgres

volumes:
  postgres_data:
```

#### Deployment Steps

1. **Prepare environment file**
   ```bash
   cp .env.example .env.prod
   # Edit with production values
   ```

2. **Pull latest images**
   ```bash
   docker compose -f docker-compose.prod.yml pull
   ```

3. **Run migrations**
   ```bash
   docker compose -f docker-compose.prod.yml run --rm skillregistry-setup
   ```

4. **Start services**
   ```bash
   docker compose -f docker-compose.prod.yml up -d
   ```

5. **Verify deployment**
   ```bash
   docker compose -f docker-compose.prod.yml ps
   docker compose -f docker-compose.prod.yml logs -f
   ```

### Zero-Downtime Deployment

For production use with zero downtime:

```bash
# Deploy new version alongside old
docker compose -f docker-compose.prod.yml up -d --no-deps --scale skillregistry-backend=2 skillregistry-backend-new

# Verify new containers are healthy
docker compose -f docker-compose.prod.yml ps

# Switch traffic (requires reverse proxy)
# Update nginx/traefik configuration

# Remove old containers
docker compose -f docker-compose.prod.yml up -d --scale skillregistry-backend=1 skillregistry-backend
docker compose -f docker-compose.prod.yml rm -f skillregistry-backend-old
```

## Monitoring and Alerts

### Key Metrics to Monitor

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| API Response Time | Average API latency | > 500ms |
| Error Rate | 5xx errors | > 1% |
| Database Connections | Active DB connections | > 80% of max |
| Worker Queue Depth | Pending Temporal tasks | > 1000 |
| Disk Usage | Storage utilization | > 85% |
| Memory Usage | Container memory | > 90% |

### Log Aggregation

**Backend Logs:**
```bash
# Follow backend logs
docker compose logs -f skillregistry-backend

# View logs with specific level
RUST_LOG=error docker compose logs skillregistry-backend
```

**Worker Logs:**
```bash
# Follow worker logs
docker compose logs -f skillregistry-worker
```

### Health Checks

**API Health Check:**
```bash
curl http://localhost:3000/health
```

**Database Health:**
```bash
docker compose exec postgres pg_isready -U postgres
```

**S3 Storage Health:**
```bash
curl http://localhost:9000/health
```

**Temporal Health:**
```bash
temporal operator cluster health
```

### Setting Up Alerts

Recommended alerts:

1. **API Down**: Alert if health check fails for > 2 minutes
2. **High Error Rate**: Alert if error rate exceeds 1% for 5 minutes
3. **Worker Queue**: Alert if pending tasks exceed 1000
4. **Disk Space**: Alert if disk usage exceeds 85%
5. **Database Connections**: Alert if connections exceed 80% of pool

## Common Issues and Fixes

### Database Connection Issues

**Symptom**: API/Worker cannot connect to PostgreSQL

**Diagnosis:**
```bash
docker compose logs skillregistry-backend | grep -i "database\|connection"
docker compose exec postgres pg_isready
```

**Fixes:**
1. Verify `SKILLREGISTRY_DATABASE__URL` is correct
2. Check PostgreSQL is running: `docker compose ps postgres`
3. Verify network connectivity: `docker compose exec skillregistry-backend ping postgres`
4. Check database credentials
5. Review PostgreSQL logs: `docker compose logs postgres`

### S3 Storage Issues

**Symptom**: Failed to upload/download skill packages

**Diagnosis:**
```bash
docker compose logs skillregistry-worker | grep -i "s3\|upload"
```

**Fixes:**
1. Verify S3 credentials are correct
2. Check S3 endpoint is accessible: `curl http://localhost:9000`
3. Verify bucket exists: `docker compose exec skillregistry-backend ls /app/s3-client`
4. Check bucket permissions

### Worker Not Processing

**Symptom**: Skills not being discovered/indexed

**Diagnosis:**
```bash
# Check worker logs
docker compose logs skillregistry-worker | tail -100

# Check Temporal tasks
temporal task-queue describe --task-queue skill-registry-queue
```

**Fixes:**
1. Verify Temporal server is running
2. Check worker is registered: `docker compose exec skillregistry-worker ps aux`
3. Verify `SKILLREGISTRY_TEMPORAL__SERVER_URL`
4. Restart worker: `docker compose restart skillregistry-worker`
5. Check for stuck workflows: `temporal workflow list`

### GitHub API Rate Limiting

**Symptom**: 403 errors from GitHub API

**Diagnosis:**
```bash
docker compose logs skillregistry-worker | grep -i "rate\|limit"
```

**Fixes:**
1. Verify GitHub token has proper permissions
2. Check token rate limit: `curl -H "Authorization: token $TOKEN" https://api.github.com/rate_limit`
3. Reduce `SKILLREGISTRY_WORKER__SCAN_INTERVAL_SECONDS`
4. Consider using GitHub App authentication for higher limits

### High Memory Usage

**Symptom**: Container OOM killed

**Diagnosis:**
```bash
docker stats
docker compose logs skillregistry-backend | grep -i "oom\|memory"
```

**Fixes:**
1. Increase container memory limit
2. Reduce `RUST_LOG` level to `warn` or `error`
3. Check for memory leaks: profile with `valgrind`
4. Scale horizontally

### Frontend Build Issues

**Symptom**: Frontend container fails to build

**Diagnosis:**
```bash
docker compose logs skillregistry-frontend
```

**Fixes:**
1. Clear node modules cache
2. Rebuild without cache: `docker compose build --no-cache skillregistry-frontend`
3. Check npm dependencies in `package.json`
4. Verify Node.js version compatibility

## Rollback Procedures

### Container Rollback

```bash
# List available versions
docker images | grep skillregistry

# Stop current containers
docker compose down

# Deploy previous version
docker compose up -d --scale skillregistry-backend=0 skillregistry-worker=0
docker tag ghcr.io/Lynskylate/skillgregistry:previous ghcr.io/Lynskylate/skillgregistry:latest
docker compose up -d

# Verify rollback
docker compose ps
docker compose logs -f
```

### Database Migration Rollback

```bash
# Run migration rollback
docker compose run --rm skillregistry-setup migrate-rollback

# If manual rollback needed
docker compose exec postgres psql -U postgres -d skillregistry
# Execute SQL to revert changes
```

### Full System Rollback

```bash
# Stop all services
docker compose down -v

# Restore from backup (see Disaster Recovery)
# ...

# Restart with previous version
git checkout <previous-tag>
make up
```

## Maintenance Tasks

### Daily

- Review error logs
- Check disk space
- Verify worker is processing

### Weekly

- Review Temporal workflow history
- Check GitHub API rate limits
- Review and rotate logs

### Monthly

- Review and update dependencies
- Database maintenance (VACUUM, ANALYZE)
- Review storage costs
- Security audit

### Database Maintenance

```bash
# Connect to database
docker compose exec postgres psql -U postgres -d skillregistry

# Vacuum analyze
VACUUM ANALYZE;

# Check table sizes
SELECT schemaname, tablename,
  pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;

# Reindex
REINDEX DATABASE skillregistry;
```

### Log Rotation

```bash
# Configure Docker log rotation in docker-compose.yml
services:
  skillregistry-backend:
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
```

## Disaster Recovery

### Backup Strategy

**Database Backups:**
```bash
# Manual backup
docker compose exec postgres pg_dump -U postgres skillregistry > backup.sql

# Automated backup (cron)
0 2 * * * docker compose exec postgres pg_dump -U postgres skillregistry | gzip > /backups/skillregistry-$(date +\%Y\%m\%d).sql.gz
```

**S3 Backup:**
- Use S3 versioning
- Enable cross-region replication

**Configuration Backup:**
```bash
# Backup environment and compose files
tar czf config-backup-$(date +%Y%m%d).tar.gz .env docker-compose.yml
```

### Restore Procedure

**Database Restore:**
```bash
# Stop application
docker compose down

# Restore database
docker compose up -d postgres
docker compose exec -T postgres psql -U postgres skillregistry < backup.sql

# Start application
docker compose up -d
```

**Full System Restore:**
1. Provision new infrastructure
2. Install Docker and Docker Compose
3. Restore configuration files
4. Restore database from backup
5. Start services
6. Verify health checks

### Recovery Time Objective (RTO)

| Component | RTO Target | Actual |
|-----------|------------|--------|
| API/Worker | 15 minutes | ~10 minutes |
| Database | 30 minutes | ~20 minutes |
| Full System | 1 hour | ~45 minutes |

### Recovery Point Objective (RPO)

| Component | RPO Target | Actual |
|-----------|------------|--------|
| Database | 1 hour | 1 hour (daily backup) |
| S3 Storage | Real-time | Real-time (versioning) |
| Configuration | 1 day | On change |

## Security Considerations

### Production Checklist

- [ ] Change all default passwords
- [ ] Use strong `JWT_SIGNING_KEY`
- [ ] Enable HTTPS/TLS
- [ ] Set `SKILLREGISTRY_DEBUG=false`
- [ ] Configure proper `FRONTEND_ORIGIN`
- [ ] Set up firewall rules
- [ ] Enable database SSL
- [ ] Rotate secrets regularly
- [ ] Set up log aggregation
- [ ] Configure backup alerts

### Secret Rotation

```bash
# Rotate JWT signing key
1. Generate new key
2. Update SKILLREGISTRY_AUTH__JWT__SIGNING_KEY
3. Restart services: docker compose restart
4. Verify authentication works
5. Remove old key from secrets manager
```

## Additional Resources

- [Contributor Guide](CONTRIB.md)
- [Docker Commands Reference](DOCKER_COMMANDS.md)
- [Backend Architecture Notes](BACKEND_ARCHITECTURE_NOTE.md)
- [E2E Testing Guide](E2E_TESTING.md)
- [GitHub Repository](https://github.com/Lynskylate/skillgregistry)
