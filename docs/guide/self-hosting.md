# Self-hosting

## Run the binary directly

The simplest way — download a single binary and run it. No Docker, no package manager.

**macOS (Apple Silicon)**
```bash
curl -L https://github.com/avirajkhare00/superdupermemory/releases/latest/download/superdupermemory-macos-arm64.tar.gz | tar xz
chmod +x superdupermemory
```

**macOS (Intel)**
```bash
curl -L https://github.com/avirajkhare00/superdupermemory/releases/latest/download/superdupermemory-macos-x86_64.tar.gz | tar xz
chmod +x superdupermemory
```

**Linux (x86_64)**
```bash
curl -L https://github.com/avirajkhare00/superdupermemory/releases/latest/download/superdupermemory-linux-x86_64.tar.gz | tar xz
chmod +x superdupermemory
```

**Linux (ARM64 / Graviton)**
```bash
curl -L https://github.com/avirajkhare00/superdupermemory/releases/latest/download/superdupermemory-linux-arm64.tar.gz | tar xz
chmod +x superdupermemory
```

Once you have the binary, start the web server:

```bash
ANTHROPIC_API_KEY=sk-ant-... ./superdupermemory serve-web
# Listening on http://0.0.0.0:3000
```

Open [http://localhost:3000](http://localhost:3000) to access the dashboard.

Optionally move it to your PATH:

```bash
sudo mv superdupermemory /usr/local/bin/
superdupermemory serve-web
```

### Custom port and data path

```bash
SDM_DB_PATH=./memory.db SDM_HTTP_PORT=8080 superdupermemory serve-web
```

---

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
