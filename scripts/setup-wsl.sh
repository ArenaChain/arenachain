#!/usr/bin/env bash
# Runs inside WSL. Copies the project to ~/arenachain, installs the Solana
# toolchain, Surfpool and Claude Code if missing, then the Claude plugins.
# Usage: bash setup-wsl.sh /mnt/c/Users/<you>/Documents/arenachain
set -uo pipefail

SRC="${1:?pass the Windows project folder (as a /mnt/c path)}"
DEST="$HOME/arenachain"
have() { command -v "$1" >/dev/null 2>&1; }
step() { printf '\n== %s\n' "$*"; }
reload_path() {
  export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$HOME/.local/share/solana/install/active_release/bin:$HOME/.avm/bin:$PATH"
  # shellcheck disable=SC1091
  [ -s "$HOME/.nvm/nvm.sh" ] && . "$HOME/.nvm/nvm.sh"
  [ -s "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
  return 0
}

step "Copying the project to $DEST (existing files are kept)"
mkdir -p "$DEST"
cp -rn "$SRC"/. "$DEST"/
rm -f "$DEST"/setup-windows.ps1 "$DEST"/setup-windows.cmd
cd "$DEST" || exit 1
reload_path

if ! have anchor || ! have solana; then
  step "Installing the Solana toolchain (Rust, Solana CLI, Anchor, Node.js)"
  curl --proto '=https' --tlsv1.2 -sSfL https://solana-install.solana.workers.dev | bash || exit 1
  reload_path
fi

if ! have surfpool; then
  step "Installing Surfpool (used by anchor test)"
  curl -sL https://run.surfpool.run/ | bash || echo "! Surfpool install failed; install it later."
  reload_path
fi

if ! have claude; then
  step "Installing Claude Code"
  curl -fsSL https://claude.ai/install.sh | bash || exit 1
  reload_path
fi

step "Installing Claude plugins and language servers"
bash scripts/setup-claude-tools.sh
