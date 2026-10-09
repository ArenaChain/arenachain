#!/usr/bin/env bash
# One-time Claude Code setup for ArenaChain on this computer.
# Run from the repo, inside WSL (Ubuntu) on Windows, or on Linux/macOS:
#   bash scripts/setup-claude-tools.sh
set -uo pipefail

cd "$(dirname "$0")/.."
have() { command -v "$1" >/dev/null 2>&1; }
warn() { printf '\n! %s\n' "$*"; }

if ! have claude; then
  echo "Claude Code isn't installed. Run: curl -fsSL https://claude.ai/install.sh | bash" >&2
  exit 1
fi

# security-guidance's end-of-turn and commit reviews need a git repository.
if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  git init -q && echo "Initialized a git repository."
fi

# Language servers used by the rust-analyzer-lsp and typescript-lsp plugins.
if have rustup; then
  rustup component add rust-analyzer || warn "rust-analyzer install failed."
else
  warn "Skipped rust-analyzer: install the Solana toolchain first (it installs Rust):
  curl --proto '=https' --tlsv1.2 -sSfL https://solana-install.solana.workers.dev | bash"
fi
if have npm; then
  npm install -g typescript-language-server typescript || warn "typescript-language-server install failed."
else
  warn "Skipped typescript-language-server: install Node.js 20.18+ first."
fi

# Python for the security plugins (claude-security needs 3.9+, security-guidance's commit review 3.10+).
if ! python3 -c 'import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)' 2>/dev/null; then
  warn "Install Python 3.10+ as python3 so the security plugins run fully."
fi

# Plugins at project scope: recorded in .claude/settings.json for the whole team.
claude plugin marketplace add anthropics/claude-plugins-official || warn "Couldn't add the official marketplace."
failed=()
for plugin in security-guidance claude-security rust-analyzer-lsp typescript-lsp; do
  claude plugin install "${plugin}@claude-plugins-official" --scope project || failed+=("$plugin")
done

echo
claude plugin list
if ((${#failed[@]})); then
  warn "These plugins didn't install: ${failed[*]}. Re-run this script, or use /plugin inside Claude Code."
  exit 1
fi

cat <<'EOF'

Done. Next:
  1. Run `claude` in this folder and approve solana-mcp-server when asked.
  2. On the Pro plan, turn on Dynamic workflows in /config before using /claude-security.
EOF
