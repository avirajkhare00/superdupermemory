# Self-hosting

## Docker Compose

The simplest production setup. One container, SQLite on a mounted volume.

```yaml
# docker-compose.yml
services:
  superdupermemory:
    image: ghcr.io/avirajkhare00/superdupermemory:latest
    ports:
      - "3000:3000"
    volumes:
      - ./data:/data
    environment:
      - SDM_DB_PATH=/data/memory.db
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
    restart: unless-stopped
```

```bash
ANTHROPIC_API_KEY=sk-ant-... docker compose up -d
```

## Bare metal (Debian / Ubuntu)

The install script downloads the binary, creates a systemd service, and sets up `/etc/superdupermemory/env`:

```bash
curl -fsSL https://raw.githubusercontent.com/avirajkhare00/superdupermemory/master/install.sh | sudo bash
```

Then add your API key:

```bash
sudo nano /etc/superdupermemory/env
# Add: ANTHROPIC_API_KEY=sk-ant-...
sudo systemctl restart superdupermemory
```

## Reverse proxy (nginx)

Put nginx in front to handle TLS:

```nginx
server {
    listen 443 ssl;
    server_name memory.yourdomain.com;

    ssl_certificate     /etc/letsencrypt/live/memory.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/memory.yourdomain.com/privkey.pem;

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
    }
}
```

## Backups

The database is a single SQLite file. Back it up live (safe while running):

```bash
superdupermemory backup /path/to/backup.db
```

Or automate with cron:

```bash
0 3 * * * superdupermemory backup /backups/memory-$(date +\%F).db
```
