#!/bin/sh
# Installs or updates swofty-triage on Debian or Ubuntu. Run as root: install.sh [git ref, default main]
set -eu

REF="${1:-main}"
REPO="${TRIAGE_SOURCE:-https://github.com/Swofty-Developments/SwoftyLauncher.git}"
PREFIX=/opt/swofty-triage

apt-get update -qq
DEBIAN_FRONTEND=noninteractive apt-get install -y -qq python3 python3-venv git ripgrep openssh-client ca-certificates >/dev/null
id triage >/dev/null 2>&1 || useradd --system --home-dir /var/lib/swofty-triage --shell /usr/sbin/nologin triage

if [ ! -d "$PREFIX/src/.git" ]; then
    mkdir -p "$PREFIX"
    git clone --quiet --filter=blob:none --no-checkout "$REPO" "$PREFIX/src"
    git -C "$PREFIX/src" sparse-checkout set triage
fi
git -C "$PREFIX/src" fetch --quiet --depth 1 origin "$REF"
git -C "$PREFIX/src" checkout --quiet --force FETCH_HEAD
echo "swofty-triage source: $(git -C "$PREFIX/src" log -1 --format='%h %s')"

[ -x "$PREFIX/uv/bin/uv" ] || { python3 -m venv "$PREFIX/uv" && "$PREFIX/uv/bin/pip" install --quiet uv; }
UV_PROJECT_ENVIRONMENT="$PREFIX/venv" "$PREFIX/uv/bin/uv" sync --quiet --frozen --no-dev --no-editable \
    --reinstall-package swofty-triage --refresh-package swofty-triage --project "$PREFIX/src/triage"

install -m 0644 "$PREFIX/src/triage/deploy/swofty-triage.service" /etc/systemd/system/swofty-triage.service
install -m 0644 "$PREFIX/src/triage/deploy/swofty-usage.service" /etc/systemd/system/swofty-usage.service
systemctl daemon-reload
systemctl enable --quiet swofty-usage
systemctl restart swofty-usage
if [ ! -f /etc/swofty-triage.env ]; then
    install -m 0640 -g triage "$PREFIX/src/triage/deploy/swofty-triage.env.example" /etc/swofty-triage.env
    echo "Fill in /etc/swofty-triage.env, then run: systemctl enable --now swofty-triage"
    exit 0
fi
systemctl enable --quiet swofty-triage
systemctl restart swofty-triage
systemctl --no-pager --lines=5 status swofty-triage
