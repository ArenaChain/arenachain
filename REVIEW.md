# ArenaChain pre-deploy adversarial review

Scope: `programs/arenachain` (all 15 instructions), the LiteSVM and Surfpool tests, the Codama client and `clients/js/src/result.ts`. Reviewed at `66307d5` on `feat/core-escrow`; fixes through `cbfddeb`.

How the findings were checked:
- **By reading the code** against CLAUDE.md and `.claude/claude-security-guidance.md`.
- **By a proof-of-concept test** for C1, run in a scratch copy of the repo; the repo itself was not edited.
- **By mutation testing:** 10 bugs were injected one at a time into the scratch copy, and the full suite was run to see whether any test fails. Results are in [Tests](#test-findings).

No finding lets anyone pay escrow to a non-player. The worst issue is a **permanent lock** of both entry fees (C1).

---

## Resolution (Phase 2)

All approved findings are fixed on `feat/core-escrow`, one commit each. Every fix came with a test that failed before the fix and passes after it. For tests that guard existing behavior, the matching check was removed temporarily to confirm the test fails without it.

| Finding | Status | Commit |
|---|---|---|
| C1 creator locks funds by reassigning their wallet | **Fixed** | `94b5c14` |
| H1 dispute freeze | **Deferred**: post-hackathon, required before mainnet (documented in CLAUDE.md) | — |
| M1 unbounded hold / result window | **Fixed** (hold ≤ 14 days, window 10 min–24 h) | `af0e473` |
| M2 freezable or wrong-decimal mint | **Fixed** | `40e9979` |
| M3 result deadline counted from join | **Fixed** (start deadline + result deadline) | `a09f487` |
| M4 admin can't be rotated | **Fixed** (two-step handover, dispute-authority rotation) | `4277e20` |
| L1 Void blocked by pause | **Fixed** | `ae5a2ad` |
| L2 join with an inactive server | **Fixed** | `f5063ee` |
| L3 match accounts never closed | **Skipped** (documented in CLAUDE.md) | — |
| L4 default admin or dispute authority | **Fixed** | `eeb185d` |
| L5 no cluster binding | **Documented** in CLAUDE.md | — |
| Extra: player A closes or gives away their token account | **Proven safe**, no change needed: anyone can recreate A's ATA and the refund goes through | `027281a` |
| T1, T3, T5 | **Added** (mutation-verified) | `8a67b38` |
| T6 weak assertions | **Fixed** | `59f931c` |
| T9 order-dependent e2e | **Fixed** | `78f2d6a` |
| T4, T7, T8 | **Added** (T4 mutation-verified) | `fd684ee` |
| T2 | Covered by C1's tests and T1 | — |
| Cleanup | **Done** (unused deps, `split_pool` error, Codama PDA names) | `cbfddeb` |

**Final verification:**
- `anchor build` with 0 warnings, and `npm run generate` leaves no diff.
- `npm run typecheck` passes.
- `NO_DNA=1 anchor test` runs 130 LiteSVM tests and 5 Surfpool flows, all green.
- clippy reports 0 warnings, and the Solana MCP `program_autofixer` reports no issues.

**Notes from the fixes:**
- **L1:** the automated security reviewer flagged "pause bypass". It's accepted as approved: a Void only returns each player their own entry, and deactivating a server blocks its Voids even while paused (`l1_paused_void_still_needs_an_active_server`).
- **T7:** in `submit_result` and `claim_payout`, passing the fee vault as the match vault is rejected even earlier, as a duplicate writable account (`ConstraintDuplicateMutableAccount`), because both already take the fee vault as their own account.

---

## Critical

### C1. Player A can permanently lock both entry fees by reassigning their own wallet
**Status: fixed in `94b5c14`.**
**Files:**
- `src/instructions/submit_result.rs:75-76`
- `src/instructions/claim_payout.rs:48-49`
- `src/instructions/refund_expired.rs:51-52`

All three exit paths for a joined match take the creator's wallet as `player_a: SystemAccount<'info>`, so the vault's rent can be returned to them. `SystemAccount` requires the account to be owned by the System Program.

**Exploit (proven):**
1. Player A is losing a live match.
2. Player A signs `system_instruction::assign(player_a, <any other program id>)` on their own wallet.
3. From then on, every `submit_result`, `refund_expired` and `claim_payout` fails with `AccountOwnedByWrongProgram` (3011).
4. The assignment can't be undone (only the new owner could change it), so the pool is locked forever and player B never gets paid. This also works on a held payout after settlement.

PoC output from the scratch copy:
```
SUBMIT after assign: InstructionError(1, Custom(3011))
REFUND after assign: InstructionError(0, Custom(3011))
CLAIM after assign:  InstructionError(0, Custom(3011))
```
**Fix:** turn the rent destination into `UncheckedAccount` with `address = arena_match.player_a` and a `/// CHECK:` comment. A credit to the vault's rent doesn't need the account to be system-owned. Add the three PoC cases as regression tests: settle, refund and claim must all succeed after an Assign. `player_cancel_match` is unaffected because there `player_a` is a `Signer`.

---

## High

### H1. Dispute freeze is missing, so held payouts can't be stopped (CLAUDE.md rule gap)
**Status: deferred to post-hackathon, required before mainnet.** Documented in CLAUDE.md; the devnet `hold_threshold` (250 USDC) is above demo pool sizes.
**Files:** `src/instructions/claim_payout.rs:24`, `src/state/match_account.rs:10` (`Disputed` is never set), `src/state/config.rs:9` (`dispute_authority` is never read).

**Rule (CLAUDE.md):** a held payout is paid through claim after the hold, "unless the dispute authority freezes it". As built:
- Nothing can freeze a payout.
- A VAC ban that arrives during the hold changes nothing; anyone can claim once `unlock_at` passes.
- The result-service rule "ask the dispute authority to freeze" has nothing to call.

**Fix (needs your product decision):**
- `dispute_freeze` (dispute authority only): `Settled` and `!paid_out` → `Disputed`.
- `dispute_resolve(outcome)`, which only ever pays players:
  - `Release` → back to `Settled` (claimable)
  - `AwardOpponent` → the other player gets the pool with no fee
  - `Refund` → both players are refunded
- To keep "funds can never be trapped" true, a frozen match also needs a timeout. For example, after N days anyone can trigger `Refund`.
- **To decide:** the resolution outcomes, the timeout, and whether ratings get reversed.

---

## Medium

### M1. Admin can lock escrow indefinitely through unbounded timing params
**Status: fixed in `af0e473`.**
**File:** `src/state/config.rs:35-40`. `validate()` only checks `hold_secs >= 0` and `result_window_secs > 0`, with no upper bound.

**Scenario:** a compromised or mistaken admin sets:
- `hold_secs = 10^12`: every new held result unlocks in about 30,000 years.
- `result_window_secs = 10^12`: a newly joined match whose server dies can't be refunded for the same span.

Either way funds are locked. Escrow is never *taken*, but CLAUDE.md says "funds can never be trapped". Setting `i64::MAX` instead makes `checked_add` fail, which blocks joins or results outright.

**Fix:** add upper bounds in `validate()`. Proposed: `hold_secs <= 14 days`, `result_window_secs <= 24 h`, and `result_window_secs >= 10 min`. **These numbers are your call.** Test: `update_config` / `init` above each bound must return `InvalidConfigParam`.

### M2. Config init accepts any classic mint, including one with a freeze authority
**Status: fixed in `40e9979`.**
**File:** `src/instructions/admin_initialize_config.rs:25`

**Scenario:** if the configured mint has a freeze authority, its holder can freeze any match vault or player token account. Every `transfer_checked` out of that vault then fails, which locks settlement, refund and claim. The mint is fixed forever at init.

**Fix:** require `usdc_mint.freeze_authority.is_none()` and `decimals == 6` in `admin_initialize_config`. Test: init with a freezable mint, and with a 9-decimal mint, must both be rejected. The devnet mint in the deploy plan is created without `--enable-freeze`, so it passes. Real mainnet USDC has a freeze authority, but mainnet is out of scope.

### M3. The result deadline counts from join, not from start
**Status: fixed in `a09f487`.**
**Files:** `src/instructions/player_join_match.rs:56-62`, `src/instructions/server_start_match.rs:33`

**Scenario:**
1. The window is 2 h and starts at **join**.
2. The players take 1 h 30 min to ready up, then the match takes 45 min.
3. The server's correct result arrives after the deadline and is rejected (`ResultDeadlinePassed`).
4. Anyone calls `refund_expired`, so the winner loses the prize.

A losing player can also stall the ready-up to push the match toward the deadline.

**Fix (rule nuance, needs your OK):**
- Keep the join-based deadline as a *start* deadline.
- In `server_start_match`, set `result_deadline = started_at + result_window_secs`.
- Test: a late start followed by a result just under the new deadline must settle, and a refund at the old deadline must fail with `NotExpired`.

### M4. Config `admin` can't be rotated, which blocks the move to the multisig
**Status: fixed in `4277e20`.**
**Files:** `src/instructions/admin_initialize_config.rs:46,54`. No instruction ever writes `config.admin` (or `dispute_authority`) after init.

**Scenario:** the deploy wallet becomes admin on devnet. CLAUDE.md requires the team multisig to hold control, and that's impossible without a program upgrade. A leaked admin key can't be rotated either.

**Fix:** two-step `admin_propose_admin(new)` / `admin_accept_admin` (the new admin signs), plus `admin_set_dispute_authority`. Add tests for: non-admin rejected, accept by the wrong signer rejected, and the old admin losing access after the handover.

---

## Low

### L1. The server-signed Void refund is blocked by pause
**Status: fixed in `ae5a2ad`.**
**File:** `src/instructions/submit_result.rs:92`

**Scenario:** during a pause, a server that reports a crash (Void) can't refund the players. They have to wait for the result deadline. Funds aren't trapped, but this cuts against "pause never blocks refunds".

**Fix:** skip the pause check when `kind == Void`. Test: Void while paused refunds both players.

### L2. Joining doesn't check that the match's game server is still active
**Status: fixed in `f5063ee`.**
**File:** `src/instructions/player_join_match.rs:11-46`

**Scenario:** the server is deactivated while the match is Open. Player B can still join, the match can never start, and B's entry is locked until the deadline before it can be refunded.

**Fix:** add `game_server` to the join accounts with `address = arena_match.game_server` and `constraint = game_server.active`.

### L3. Match accounts and dust-holding vaults are never closed
**Status: skipped by decision; documented in CLAUDE.md.**
**Files:** every match instruction. Match accounts have no close path.

**Scenario:** each match keeps about 0.0028 SOL of the creator's rent forever. A vault that received donated dust is never closed, so its rent stays locked too. No escrow is affected.

**Fix:** add a later `close_match` instruction for terminal, paid matches, with rent going to `player_a`. The dust stays where it is.

### L4. Config init accepts `Pubkey::default()` as admin or dispute authority
**Status: fixed in `eeb185d`.**
**File:** `src/instructions/admin_initialize_config.rs:51-55`

**Scenario:** a typo in the init script bricks the admin role permanently. Init can't run twice.

**Fix:** use `require_keys_neq!` against the default key for both, as is already done for `treasury`.

### L5. The result message doesn't bind to a cluster
**Status: documented in CLAUDE.md (Known limitations).**
**File:** `src/ed25519.rs:22-45`

**Scenario:** the same program keypair deployed on devnet and on mainnet would give the same match PDA for the same creator and nonce. A signed devnet result would replay only if `created_slot` also matched, which is practically impossible. Mainnet is also out of scope.

**Fix:** none now. Document it. If mainnet ever happens, add a cluster tag to the domain (e.g. `ARENACHAIN_RESULT_V1_MAINNET`).

---

## Test findings

The mutation runs injected these bugs into the scratch copy and checked whether any test fails. The baseline was 104 passing, which includes 3 PoC tests.

| Injected bug | Caught? | Severity of the gap |
|---|---|---|
| `submit_result` deadline `<=` → `<` | ✅ `result_at_exact_deadline_accepted` | — |
| `player_cancel_match` status check removed | ✅ `cancel_rejects_non_creator_and_joined_match` | — |
| `refund_expired` rent destination unconstrained | ✅ `refund_rejects_substituted_accounts` | — |
| `player_create_match` mint not pinned | ✅ `create_rejects_wrong_mint` | — |
| **`claim_payout` without `has_one = fee_vault`** | ❌ | **Medium (T1)** |
| `submit_result` / `claim_payout` rent destination unconstrained | ❌ | Low (T2) |
| `player_join_match` joiner token-owner check removed | ❌ | Low (T3) |
| Ed25519 `accounts.is_empty()` check removed | ❌ | Low (T4) |
| Ed25519 signature-bounds check removed | ❌ | Low (T4) |

**T1 (Medium).** `claim_payout` has no account-substitution tests at all. If the `fee_vault` check regressed, anyone could pass their own token account as `fee_vault` and steal the 5% fee on every held claim. Nothing would catch it. **Add:** claim with a wrong fee vault (`ConstraintHasOne`), a wrong vault (`ConstraintSeeds`), a wrong mint (`ConstraintHasOne`), and a wrong rent destination (`ConstraintAddress`).

**T2 (Low).** Add rent-destination substitution tests for `submit_result` and `claim_payout`. These get rewritten anyway by the C1 fix.

**T3 (Low).** Add a join test that pays from a token account owned by someone else (`ConstraintTokenOwner`) and one with the wrong mint (`ConstraintTokenMint`). The token program would block a straight theft, but not a payment from an account the joiner was only approved to spend from (a delegate).

**T4 (Low).** These two checks are defense in depth, because the precompile already enforces both. Add two small tests: an Ed25519 instruction carrying account metas, and a message with one extra trailing byte (`ResultMessageMismatch`).

**T5 (Low).** `ed25519_offsets_into_other_instruction_rejected` (`tests/results.rs:260`) points the index fields at the precompile's own instruction. It does fail if the `u16::MAX` check is removed, but it doesn't model the real Wormhole-style attack, in which the offsets point at bytes in a *different* instruction. **Add:**
1. A memo instruction carrying a rogue key, signature and message.
2. An Ed25519 instruction whose index fields point at the memo, while its own data holds the server's pubkey and the expected message.

The result must be `MalformedEd25519Instruction`.

**T6 (Low): weak assertions.** These tests only check that a call failed, not why:
- `initialize_config_only_once` (`tests/admin_and_matches.rs:38`)
- creating a profile twice (`:117`)
- creating a duplicate match (`:241`, `:417`)
- `match_vault_cannot_be_drained_as_fee_vault` (`tests/step4.rs:472`, which should assert `ConstraintHasOne`)
- `invalid_signature_bytes_rejected_by_precompile` (`tests/results.rs:302`, which should assert the failure is at instruction 0, the precompile)

The remaining `assert_failed` calls are correct: they hit a closed vault and also assert that balances didn't change.

**T7 (Low).** `only_withdraw_fees_signs_as_config` (`tests/step4.rs:513`) greps for two exact spellings of the seed array. Signer seeds built any other way, such as `bump.as_ref()`, would get past it. **Add:** a behavioral test in which every other instruction that takes the config account is run with the fee vault passed in place of a payout recipient, and must fail. Keep the grep test as a second check.

**T8 (Low).** Missing boundary tests:
- A forfeit exactly 30 days after the previous one (the window uses `<`, so it's not counted).
- `admin_update_config` with negative `hold_secs`.
- A held Forfeit claimed at exactly `unlock_at`.

**T9 (Low).** In the Surfpool e2e, `admin withdraws collected fees` (`tests/arenachain.test.ts:281`) depends on fees from the earlier tests. Run on its own it fails. **Fix:** have it settle its own match.

---

## Cleanup

**Remove (proved unused):**
- The `solana-clock` dev-dependency (`programs/arenachain/Cargo.toml:35`) is never imported.
- `@solana-program/system` (`package.json:13`) is never imported. The Kit plugins stay: they're peer dependencies of `@solana/surfpool`.

**Small fixes:**
- `split_pool` returns `InvalidWinner` for `Void` (`src/payout.rs:20`). That path is unreachable, and the error is misleading. Return `InvalidMatchStatus` instead.
- Codama PDA naming: `serverStartMatchGameServer`, `creatorProfile` and `joinerProfile` are generated duplicates of `gameServer` and `profile`. Rename them with a Codama `updatePdas` / `setPda` visitor in `clients/generate.mjs`, not by hand-editing.

**Keep and document:**
- The signer-seed + `Escrow { … }` block is repeated in 4 instruction files. A `Match::escrow(...)` helper would remove the repetition. That's style only, so skip it unless the C1 fix touches those lines anyway.
- Reserved but not yet read on-chain: `GlobalConfig.dispute_authority`, `MatchStatus::Disputed` (H1), `PlayerProfile.steam_id` (step 7), and `GameServer.operator` (metadata). `Match.created_at/joined_at/started_at` are for clients.
- GitHub: the default branch is `feat/core-escrow`, the only branch. Create `main` before opening PRs.

---

## Checked and found sound
- **Payout + fee = pool:** proven for every pool size, including `u64::MAX` (u128 math, rounds down).
- **Fee rules:** a Forfeit waives the fee, and a Void refunds exactly `entry_fee` to each player.
- **Pool overflow** is rejected at create time.
- **Recipient checks:** every recipient is constrained by mint and owner (player A, player B, the winner, or the config-fixed fee vault). Swapped vaults and wrong mints are rejected. Player A can't also be player B. Two vaults for one match are impossible because of the seeds.
- **Ed25519 verification:**
  - The sysvar has an address constraint and is read only with the `_checked` loaders.
  - Exactly one signature, with all index fields `u16::MAX`.
  - Offsets are bounds-checked.
  - The key is the match's registered, active server.
  - The 157-byte canonical message binds domain, program, match, created slot, kind, winner and demo hash. Replay across matches and programs is blocked (tested).
- **State machine:**
  - `Open→Full` (join)
  - `Open→Refunded` (cancel)
  - `Full→InProgress` (start, `now <= deadline`)
  - `Full|InProgress→Refunded` (expiry, `now > deadline`)
  - `InProgress→Settled|Refunded` (result, `now <= deadline`)
  - held `Settled` → paid (claim, `now >= unlock_at`)

  Every transition checks the status, terminal states are final, results and refunds can't overlap, and the status is written before any transfer.
- **Pause** never blocks cancel, expiry refunds or claims. L1 covers the one exception, the server-signed Void.
- **Ratings:** integer-only Elo with a floor of 100, reputation clamped to 0–100, the forfeit ladder matches CLAUDE.md, and bans never shorten.
- **Admin powers:** the admin can never move escrow. The only config-PDA signer is `admin_withdraw_fees`, which pays only the current treasury's USDC account (tested).
- **`result.ts`** matches the on-chain message layout byte for byte (exercised by the e2e).
