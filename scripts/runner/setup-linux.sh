#!/usr/bin/env bash
#
# Sets up a self-hosted GitHub Actions runner on a Debian or Ubuntu machine,
# including everything an NPDF build needs.
#
#   sudo bash scripts/runner/setup-linux.sh <REGISTRIERUNGS_TOKEN> [NAME]
#
# The token comes from Settings, Actions, Runners, New self-hosted runner.
# It is valid for one hour.
#
# WARNING, read this before running it on a public repository:
# a self-hosted runner executes whatever a workflow tells it to, including
# workflows from a stranger's pull request. Set
#   Settings, Actions, General, Fork pull request workflows from outside
#   collaborators, to "Require approval for all external contributors"
# first, or do not use a self-hosted runner at all.

set -euo pipefail

REPO_URL="https://github.com/Canoelitose/NPDF"
TOKEN="${1:-}"
RUNNER_NAME="${2:-npdf-linux}"
RUNNER_USER="actions"
RUNNER_HOME="/home/${RUNNER_USER}/actions-runner-npdf"

if [[ -z "$TOKEN" ]]; then
  echo "Kein Token angegeben." >&2
  echo "Aufruf: sudo bash $0 <REGISTRIERUNGS_TOKEN> [NAME]" >&2
  exit 1
fi

if [[ "$(id -u)" -ne 0 ]]; then
  echo "Bitte mit sudo starten, das Skript legt einen Benutzer an." >&2
  exit 1
fi

echo "==> Systempakete"
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq \
  curl git jq tar build-essential pkg-config \
  libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libssl-dev \
  libappindicator3-dev patchelf xdg-utils file desktop-file-utils
apt-get install -y -qq libfuse2 || apt-get install -y -qq libfuse2t64

echo "==> Node 22"
if ! command -v node >/dev/null || [[ "$(node -v | cut -c2-3)" -lt 22 ]]; then
  curl -fsSL https://deb.nodesource.com/setup_22.x | bash -
  apt-get install -y -qq nodejs
fi
node -v

echo "==> Benutzer ${RUNNER_USER}"
# The runner refuses to configure itself as root, on purpose: it runs foreign
# code from workflows and must not do that with system privileges.
id -u "$RUNNER_USER" >/dev/null 2>&1 || adduser --disabled-password --gecos "" "$RUNNER_USER"
mkdir -p "$RUNNER_HOME"
chown -R "${RUNNER_USER}:${RUNNER_USER}" "/home/${RUNNER_USER}"

echo "==> Rust fuer ${RUNNER_USER}"
su - "$RUNNER_USER" -c 'command -v cargo >/dev/null || curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path'
su - "$RUNNER_USER" -c 'echo ". \$HOME/.cargo/env" >> ~/.bashrc' || true

echo "==> Runner herunterladen, jeweils die aktuelle Ausgabe"
VERSION="$(curl -fsSL https://api.github.com/repos/actions/runner/releases/latest | jq -r .tag_name | sed 's/^v//')"
ARCHIVE="actions-runner-linux-x64-${VERSION}.tar.gz"
echo "    Version ${VERSION}"
su - "$RUNNER_USER" -c "cd ~/actions-runner-npdf && \
  curl -fsSL -o '${ARCHIVE}' 'https://github.com/actions/runner/releases/download/v${VERSION}/${ARCHIVE}' && \
  tar xzf '${ARCHIVE}' && rm -f '${ARCHIVE}'"

echo "==> Anmelden"
su - "$RUNNER_USER" -c "cd ~/actions-runner-npdf && ./config.sh \
  --url '${REPO_URL}' \
  --token '${TOKEN}' \
  --name '${RUNNER_NAME}' \
  --labels self-hosted,Linux,X64,npdf,npdf-linux \
  --work _work --unattended --replace"

echo "==> Als Dienst einrichten"
# svc.sh does want root, that is not a contradiction: it writes a systemd unit.
cd "$RUNNER_HOME"
./svc.sh install "$RUNNER_USER"
./svc.sh start
./svc.sh status

echo
echo "Fertig. Der Runner sollte unter ${REPO_URL}/settings/actions/runners als Idle stehen."
echo "Zum Einschalten die Repository-Variable LINUX_RUNNER auf ${RUNNER_NAME} setzen."
