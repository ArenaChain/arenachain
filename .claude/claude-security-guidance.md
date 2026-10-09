# Security guidance: ArenaChain

Threat model: real money sits in program-owned escrow. Attackers include players (cheaters, quitters, anyone forging or replaying a result), anyone calling the program directly, and anyone who steals a server signing key. The program must only ever pay a match's own players (plus the fee to the fee vault), and must never pay twice.

## Program (Anchor)

- Every privileged action needs a `Signer` checked against the stored authority (`has_one` or `address` constraints). Never take an authority from instruction data alone.
- Use `Account<'info, T>` or `InterfaceAccount` so Anchor checks owner and discriminator. Any `UncheckedAccount` or `AccountInfo` needs a `/// CHECK:` comment plus explicit owner or address checks.
- PDAs: fixed seeds, store the canonical bump, re-derive in constraints. A match vault is a token account owned by the match PDA; check its mint equals the configured USDC mint.
- Tokens: `transfer_checked` only; check the token program id; recipient token accounts must have the expected owner (the player) and mint.
- Player A and player B must be different wallets; reject duplicate mutable accounts.

### Ed25519 result verification

Wormhole lost about $320M in Feb 2022 through this exact pattern: an unchecked sysvar loader let an attacker pass a fake instructions sysvar.

- Take the instructions sysvar with `address = sysvar::instructions::ID` and read it only with `load_current_index_checked` / `load_instruction_at_checked`.
- Require the Ed25519 instruction's program id to be the Ed25519 program, with exactly one signature, and all three instruction-index fields set to `u16::MAX` (data inside that same instruction). Parse offsets with bounds checks.
- The public key must equal the match's registered, active server key.
- The signed message must equal, byte for byte, the canonical message the program rebuilds from the match account: domain tag `ARENACHAIN_RESULT_V1`, program id, match address, match creation slot, result kind, winner, demo hash. This blocks replay across matches, programs and clusters.

### Money movement

- Check the current status at the start of every instruction. Set the final status before any transfer. Settle, refund and claim each run at most once per match.
- `checked_*` arithmetic on every amount, u64 token amounts, no truncating `as` casts. The fee rounds down, and payout + fee must equal the pool exactly, leaving nothing in the vault.
- No instruction may move escrow to anyone except the match's players and the fee vault. That includes admin and dispute paths.
- Deadlines come from `Clock::get()?.unix_timestamp`. Pause blocks new matches and new results, but never blocks refunds after expiry, so funds can't be trapped.
- Close accounts with `close =` to a known recipient. Prefer `init` over `init_if_needed`.

## Result service (off-chain signer)

- Load signing keys from env or secret storage. Never log, print or commit them. Use one key per game server.
- Authenticate incoming webhooks (shared-secret header or IP allowlist) and drop anything else.
- MatchZy can deliver the same event twice: deduplicate by match id and event, and make signing idempotent.
- Only sign results for matches that are `InProgress` on-chain and assigned to this server.
- Validate every field taken from game logs (Steam IDs, scores) before use. Never pass them to a shell or into SQL unescaped.

## Frontend and backend

- Never ask for, store or handle seed phrases or private keys. All signing goes through the wallet.
- Show the amount, recipient and match before the user signs.
- No `dangerouslySetInnerHTML` with user data; player names come from Steam.
- Steam linking: verify the Steam OpenID response server-side before the link attester signs.
- Secrets live in `.env` (gitignored). `.env.example` holds placeholders only.
