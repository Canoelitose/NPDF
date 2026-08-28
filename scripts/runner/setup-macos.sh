#!/usr/bin/env bash
#
# Sets up a self-hosted GitHub Actions runner on macOS, including everything an
# NPDF build needs. This is the only self-hosted runner that covers a target
# nothing else can: the DMG and, later, iOS.
#
#   bash scripts/runner/setup-macos.sh <REGISTRIERUNGS_TOKEN> [NAME]
#
# Run it as your normal user, NOT with sudo. The token comes from Settings,
# Actions, Runners, New self-hosted runner, and is valid for one hour.
#
# WARNING, read this before running it against a public repository:
# a self-hosted runner executes whatever a workflow tells it to, including a
# workflow from a stranger's pull request. Set Settings, Actions, General,
# Fork pull request workflows from outside collaborators, to
# "Require approval for all external contributors" first.

set -euo pipefail

REPO_URL="https://github.com/Canoelitose/NPDF"
TOKEN="${1:-}"
RUNNER_NAME="${2:-npdf-mac}"
RUNNER_HOME="${HOME}/actions-runner-npdf"

if [[ -z "$TOKEN" ]]; then
  echo "Kein Token angegeben." >&2
  echo "Aufruf: bash $0 <REGISTRIERUNGS_TOKEN> [NAME]" >&2
  exit 1
fi

if [[ "$(id -u)" -eq 0 ]]; then
  echo "Bitte NICHT mit sudo starten. Der Runner laeuft als dein Benutzer." >&2
  exit 1
fi

echo "==> Xcode Kommandozeilenwerkzeuge"
# Needed for the Apple linker, and for anything iOS later on.
xcode-select -p >/dev/null 2>&1 || xcode-select --install || true

echo "==> Homebrew"
if ! command -v brew >/dev/null; then
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  eval "$(/opt/homebrew/bin/brew shellenv 2>/dev/null || /usr/local/bin/brew shellenv)"
fi

echo "==> Node und jq"
brew list node >/dev/null 2>&1 || brew install node
brew list jq   >/dev/null 2>&1 || brew install jq

echo "==> Rust"
command -v cargo >/dev/null || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
# shellcheck disable=SC1091
source "$HOME/.cargo/env" 2>/dev/null || true
# Both architectures, so one universal DMG can be produced.
rustup target add aarch64-apple-darwin x86_64-apple-darwin

echo "==> Runner herunterladen, jeweils die aktuelle Ausgabe"
mkdir -p "$RUNNER_HOME"
cd "$RUNNER_HOME"
VERSION="$(curl -fsSL https://api.github.com/repos/actions/runner/releases/latest | jq -r .tag_name | sed 's/^v//')"
ARCH="$( [[ "$(uname -m)" == "arm64" ]] && echo arm64 || echo x64 )"
ARCHIVE="actions-runner-osx-${ARCH}-${VERSION}.tar.gz"
echo "    Version ${VERSION}, ${ARCH}"
if [[ ! -f ./config.sh ]]; then
  curl -fsSL -o "$ARCHIVE" "https://github.com/actions/runner/releases/download/v${VERSION}/${ARCHIVE}"
  tar xzf "$ARCHIVE"
  rm -f "$ARCHIVE"
fi

echo "==> Anmelden"
./config.sh --url "$REPO_URL" --token "$TOKEN" --name "$RUNNER_NAME" \
  --labels self-hosted,macOS,"$ARCH",npdf,npdf-mac \
  --work _work --unattended --replace

echo "==> Ruhezustand abschalten"
# A build machine that falls asleep is not a build machine. Two problems, two
# answers: asleep between jobs means the runner drops off and jobs queue
# forever; asleep during a job takes the runner down mid step and the job dies
# without a log. The workflow keeps the Mac awake while it builds, this keeps it
# awake the rest of the time. Needs administrator rights, so it asks once.
if sudo -n true 2>/dev/null || sudo -v; then
  sudo pmset -a sleep 0 disksleep 0
  sudo pmset -a womp 1 2>/dev/null || true
  echo "    Ruhezustand aus. Rueckgaengig mit: sudo pmset -a sleep 1"
else
  echo "    Uebersprungen, keine Administratorrechte." >&2
  echo "    Ohne das schlaeft der Mac ein und Jobs bleiben haengen. Von Hand:" >&2
  echo "      sudo pmset -a sleep 0 disksleep 0" >&2
fi

echo "==> Als Dienst einrichten"
./svc.sh install
./svc.sh start
./svc.sh status

echo
echo "Fertig. Der Runner sollte unter ${REPO_URL}/settings/actions/runners als Idle stehen."
echo "Zum Einschalten die Repository-Variable MACOS_RUNNER auf ${RUNNER_NAME} setzen."
