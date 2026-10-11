# ArenaChain demo app

Next.js app for the devnet deployment in `../deployments/devnet.json`. Players connect a Wallet Standard wallet (Phantom, Solflare, Backpack), get test USDC, create a profile, and play 1v1 matches whose entry fees sit in the program's escrow. A **simulated CS2 server** panel stands in for the MatchZy result service: it holds the game-server key on the backend and signs results with Ed25519, exactly like the real service would (`clients/js/src/result.ts`).

## Run it

```bash
npm install                        # from the repo root (npm workspaces)
cp app/.env.example app/.env.local # key *paths* for the treasury and the game server
# For local demos only: uncomment SIMULATED_SERVER=enabled in app/.env.local to turn on the server buttons.
npm run app:build && npm run start -w app   # http://localhost:3000
```

`app/.env.local` points at two keypair files that stay on the server:

| Variable | What it is |
|---|---|
| `TREASURY_KEYPAIR` | Devnet treasury wallet: funds the faucet and pays fees for the simulated server |
| `GAME_SERVER_KEYPAIR` | The registered game-server key that signs results |

Both are checked against the public addresses in `deployments/devnet.json` at startup and never reach the browser (`server-only` modules; the build fails if one is imported client-side).

`SIMULATED_SERVER=enabled` turns on the simulated server's signing route. While it's on, **anyone who can reach the app can sign a result for any live match assigned to the demo server key**, which is what the demo buttons are for. Keep it to local demos and devnet, and never use a real result-service key with it.

## What's where

| Page | |
|---|---|
| `/` Play | Connect, get test USDC, create profile; rating, wins/losses, reputation |
| `/matches` | Create a match, open matches to join, live and recent matches |
| `/match/[address]` | Scoreboard, Open → Full → Started → Settled, simulated server panel, payout breakdown, every transaction with an Explorer link |
| `/leaderboard` | Every player profile on devnet, by rating |

The faucet sends 20 test USDC per wallet per 24 hours (`FAUCET_COOLDOWN_HOURS`), plus 0.02 SOL if the wallet has under 0.01 SOL, so a brand-new devnet wallet can pay rent and fees. Claims are recorded in `app/.data/faucet.json`.

## Verify against devnet

With the app running, this plays a full match through the same API calls and transaction builders the pages use, with two fresh keypairs standing in for the two wallets:

```bash
cd app && npx tsx scripts/verify-devnet.ts
```

## Recording a demo

**Before recording**
1. Two Chrome profiles, e.g. "Player A" and "Player B", each with Phantom installed and a **new** wallet (fresh wallets haven't used their daily faucet claim).
2. In each Phantom: Settings → Developer settings → Testnet mode on → Solana Devnet.
3. In `app/.env.local`, uncomment `SIMULATED_SERVER=enabled` (it's off by default; local demos only, never a public deployment). Without it the simulated server's buttons return "turned off".
4. Start the app, open `http://localhost:3000` in both profiles, and place the windows side by side.

**The take**
1. **A**: Connect wallet → Phantom → Connect. Click **Get 20 test USDC**, then click **View on Explorer** in the toast to show the transfer.
2. **A**: **Create profile** → approve. The profile card shows rating 1000 and reputation 100.
3. **B**: repeat steps 1–2.
4. **A**: **Matches** → entry **5** → **Create match** → approve. The match page shows A against an open seat, status **Open**.
5. **B**: **Matches** → under Open matches, **Join for 5** → approve. Both windows show **Full** within a few seconds.
6. Scroll to **Simulated CS2 server** → **Start match**. Status **Started**.
7. Click **Player B wins**. Status **Settled**: pool 10, fee 0.5, payout 9.5 USDC. Click **View payout on Explorer** to show the escrow paying B and the fee vault.
8. Show the **Transactions** list (one Explorer link per step), then **Leaderboard**: B at 1016, A at 984.

Optional extras: a second match ended with **A forfeits (left)** (fee waived, A loses 10 reputation), or **Void (server crash)** (both refunded).
