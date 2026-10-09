# ArenaChain

Skill-based CS2 matches with prize money on Solana. Players' entry fees sit in an escrow the program owns; the match server signs the result; the program pays the winner. Built for the Colosseum hackathon, submission due 2026-10-11. **Devnet only.**

## Hackathon scope (build in this order)

1. Wallet sign-in and a player profile account
2. Create and join a 1v1 match; entry in test USDC held in escrow
3. Server-signed result → payout (Ed25519 check against the server's registered key)
4. Forfeits for quitting and no-shows; refunds only when the server fails
5. Elo, wins, losses and reputation updated at settlement, plus a leaderboard page
6. Dual-confirmation fallback (both players sign the same winner)
7. Steam link (a mock attester is fine)

Not before submission: KYC, governance, tournaments, Blinks, SOL→USDC swaps, mainnet.

## Stack

- Program: Anchor 1.1.x with Solana CLI 3.1.x (see the solana-dev skill's compatibility matrix). Pin versions in `Anchor.toml`.
- Frontend: Next.js + `@solana/kit` + `@solana/kit-plugin-wallet` + `@solana/react`; program clients generated with Codama.
- Tests: LiteSVM for unit tests, Surfpool for integration (`anchor test` runs on Surfpool).
- Result service: TypeScript service that receives MatchZy events (or CS2 HTTP logs) and signs results with ed25519.
- Game servers: CS2 dedicated server with MatchZy on DatHost. VAC stays on: never launch with `-insecure`.
- RPC: `SOLANA_RPC_URL` from `.env` (public devnet by default).
- On Windows, everything runs inside WSL (Ubuntu 24.04+).

Follow the `solana-dev` skill (`.claude/skills/solana-dev`) for Solana patterns. Where it differs from this file, this file wins.

## Commands

- Build: `anchor build`
- Test: `anchor test` (LiteSVM tests via `cargo test -p arenachain`, then the Surfpool e2e via `npm run test:e2e`; both need a fresh `anchor build`)
- Regenerate the Codama client: `npm run generate` (writes `clients/js/src/generated`; never hand-edit it)
- Typecheck TS: `npm run typecheck`
- Add frontend and result-service commands here as those packages are scaffolded.

## Rules the program must enforce

- Match lifecycle: `Open → Full → InProgress → Settled | Refunded | Disputed`. Every instruction checks the current state; a match settles or refunds exactly once.
- Result kinds, signed by the match's registered server key:
  - `Win`: winner takes the pool minus the fee.
  - `Forfeit`: a player left a live match and didn't reconnect within 5 minutes (any stage, even when ahead), or didn't join within 15 minutes. The opponent takes the whole pool and the fee is waived.
  - `Void`: server failure that can't be restored. Both players are refunded.
- Fee: 500 bps. `fee = pool * 500 / 10_000`, rounded down; the winner gets `pool - fee`; nothing stays in the vault.
- Payout hold: pools at or above `hold_threshold` record the winner and an unlock time. Payout happens through `claim` after the hold unless the dispute authority freezes it, because VAC bans arrive late.
- Expiry: if nothing settles by the result deadline, anyone can trigger refunds to both players, so funds can never be trapped.
- Dual confirmation: if both players sign the same winner, settle without the server.
- Escrow only ever goes to the match's players, plus the fee to the fee vault. No admin path withdraws escrow.
- Forfeit penalties within 30 days: reputation hit, then a 24-hour ban from paid matches, then a 7-day ban.
- Ratings: Elo with integer-only math.

Security details for the program, the result service and the frontend are in `.claude/claude-security-guidance.md`. Follow them.

## Result service rules

- Only sign results for matches that are `InProgress` on-chain and assigned to that server.
- Deduplicate events (MatchZy may send duplicates); signing must be idempotent.
- Before a held payout unlocks, check the winners' Steam bans; if one is banned, ask the dispute authority to freeze the match.

## How we work

- Plan first (plan mode) for any new instruction or account change.
- Every instruction ships with tests: the happy path plus wrong signer, wrong server key, replayed result, double settle, refund after payout, wrong mint or vault, and arithmetic edge cases.
- Regenerate Codama clients after every program change.
- Never commit keypairs, `.env` files or private keys. Keep the program keypair (`target/deploy/*-keypair.json`) out of git and back it up privately.
- Upgrade authority belongs to the team multisig. Never deploy to mainnet.
- Before submission, run `/claude-security` on the whole repo and fix or document every finding.
