# Claude Code setup for ArenaChain

Copy everything in this folder into the root of the ArenaChain repo. It gives Claude Code:

| File | What it does |
| --- | --- |
| `.claude/skills/solana-dev/` | Solana Foundation's development skill: Anchor, `@solana/kit`, testing, security. Loads automatically in this repo. |
| `.mcp.json` | Solana Developer MCP: live Solana docs and an Anchor expert. |
| `.claude/settings.json` | Turns on the security-guidance, claude-security, rust-analyzer-lsp and typescript-lsp plugins for everyone, pre-approves the Solana MCP, and sets `NO_DNA=1` so Anchor and Surfpool know an agent is running them. |
| `.claude/claude-security-guidance.md`, `.claude/security-patterns.json` | ArenaChain's own security rules, used by security-guidance's reviews. |
| `CLAUDE.md` | Project rules Claude Code reads every session. |
| `scripts/setup-claude-tools.sh` | One-time install of the plugins and language servers on each computer. |

## One-time setup on each computer

1. **Windows only:** in PowerShell run `wsl --install`, restart, and open Ubuntu. Anchor 1.1 needs GLIBC 2.39+ (Ubuntu 24.04 or newer); check with `ldd --version`. Keep the repo in the Linux filesystem (for example `~/arenachain`), not under `/mnt/c`, or builds will be slow.
2. **Solana toolchain** (Rust, Solana CLI, Anchor, Node.js):
   `curl --proto '=https' --tlsv1.2 -sSfL https://solana-install.solana.workers.dev | bash`
   Then Surfpool, which `anchor test` uses: `curl -sL https://run.surfpool.run/ | bash`
3. **Claude Code:** `curl -fsSL https://claude.ai/install.sh | bash`, then run `claude` once to log in.
4. **Plugins and language servers:** from the repo root, `bash scripts/setup-claude-tools.sh`
5. Start `claude` in the repo. If it asks about `solana-mcp-server`, approve it.

`/claude-security` scans use a lot of plan usage: run it once on the finished program before submitting. On Pro, turn on Dynamic workflows in `/config` first.

The skill is vendored from [solana-foundation/solana-dev-skill](https://github.com/solana-foundation/solana-dev-skill) at commit `c614f82` (2026-10-07), MIT license. To update it, replace `.claude/skills/solana-dev` with `skills/solana-dev` from a fresh clone.
