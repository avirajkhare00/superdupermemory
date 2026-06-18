#!/usr/bin/env bash
set -euo pipefail

REPO="avirajkhare00/superdupermemory"
INSTALL_DIR="/usr/local/bin"
SERVICE_NAME="superdupermemory"
DATA_DIR="/var/lib/superdupermemory"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
info()  { echo -e "${GREEN}[sdm]${NC} $*"; }
warn()  { echo -e "${YELLOW}[sdm]${NC} $*"; }
error() { echo -e "${RED}[sdm] ERROR:${NC} $*" >&2; exit 1; }

# ── detect arch ────────────────────────────────────────────────────────────
ARCH=$(uname -m)
case "$ARCH" in
  x86_64)  ARCH_TAG="x86_64-unknown-linux-gnu" ;;
  aarch64) ARCH_TAG="aarch64-unknown-linux-gnu" ;;
  *) error "Unsupported architecture: $ARCH" ;;
esac

# ── check deps ─────────────────────────────────────────────────────────────
for cmd in curl tar systemctl; do
  command -v "$cmd" &>/dev/null || error "Required command not found: $cmd"
done

# ── fetch latest release ───────────────────────────────────────────────────
info "Fetching latest release..."
VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/')
[ -z "$VERSION" ] && error "Could not determine latest version"
info "Installing superdupermemory $VERSION ($ARCH_TAG)"

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/superdupermemory-${VERSION}-${ARCH_TAG}.tar.gz"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

curl -fsSL "$DOWNLOAD_URL" -o "$TMP/sdm.tar.gz"
tar -xzf "$TMP/sdm.tar.gz" -C "$TMP"
install -m 755 "$TMP/superdupermemory" "$INSTALL_DIR/superdupermemory"

# ── create data dir ────────────────────────────────────────────────────────
mkdir -p "$DATA_DIR"

# ── write systemd unit ─────────────────────────────────────────────────────
info "Installing systemd service..."
cat > /etc/systemd/system/${SERVICE_NAME}.service <<EOF
[Unit]
Description=Superdupermemory — local-first memory layer for AI agents
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/superdupermemory serve-web
Restart=on-failure
RestartSec=5
Environment=SDM_DB_PATH=${DATA_DIR}/memory.db
Environment=SDM_HTTP_PORT=3000
EnvironmentFile=-/etc/superdupermemory/env

[Install]
WantedBy=multi-user.target
EOF

# ── env file ───────────────────────────────────────────────────────────────
mkdir -p /etc/superdupermemory
if [ ! -f /etc/superdupermemory/env ]; then
  cat > /etc/superdupermemory/env <<EOF
# Required: set your LLM API key
# ANTHROPIC_API_KEY=sk-ant-...
# Or use OpenAI:
# SDM_EXTRACTOR=openai
# SDM_EMBEDDER=openai
# OPENAI_API_KEY=sk-...
EOF
  warn "Edit /etc/superdupermemory/env to add your API key before starting."
fi

# ── enable + start ─────────────────────────────────────────────────────────
systemctl daemon-reload
systemctl enable "$SERVICE_NAME"
systemctl restart "$SERVICE_NAME"

info "Done! Superdupermemory is running at http://localhost:3000"
info "Logs: journalctl -u superdupermemory -f"
