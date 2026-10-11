// Drives the running app the way two browser wallets would, against devnet:
// faucet and simulated-server calls go through the app's HTTP API (the same requests the
// buttons make), and player transactions use the same builders the pages use (lib/actions.ts),
// signed by two fresh keypairs standing in for the two wallets.
// Usage (app running on :3000): npx tsx scripts/verify-devnet.ts

import { mkdirSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';

import { getTransferSolInstruction } from '@solana-program/system';
import {
    createClient,
    createKeyPairSignerFromPrivateKeyBytes,
    getAddressEncoder,
    type KeyPairSigner,
} from '@solana/kit';
import { solanaRpc } from '@solana/kit-plugin-rpc';
import { signer } from '@solana/kit-plugin-signer';

import { createMatchIx, createProfileIx, joinMatchIx } from '../lib/actions';
import { TREASURY } from '../lib/deployment';
import { explorerTx } from '../lib/format';
import type { MatchDetail, ProfileView, WalletView } from '../lib/types';

const APP = process.env.APP_URL ?? 'http://localhost:3000';
const RPC = process.env.SOLANA_RPC_URL ?? 'https://api.devnet.solana.com';
const KEY_DIR = resolve(homedir(), '.config/solana/arenachain-devnet-e2e');
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const results: Record<string, boolean> = {};
const check = (name: string, ok: boolean, detail = '') => {
    results[name] = ok;
    console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail ? `  (${detail})` : ''}`);
};

async function api<T>(path: string, body?: unknown): Promise<{ status: number; body: T }> {
    for (let i = 0; ; i++) {
        const res = await fetch(
            `${APP}${path}`,
            body === undefined
                ? {}
                : {
                      method: 'POST',
                      headers: { 'content-type': 'application/json' },
                      body: JSON.stringify(body),
                  },
        );
        const json = (await res.json()) as T;
        if (res.status === 502 && i < 4) {
            await sleep(1500 * (i + 1)); // devnet read rate-limited behind the API
            continue;
        }
        return { status: res.status, body: json };
    }
}

async function newWallet(name: string): Promise<KeyPairSigner> {
    mkdirSync(KEY_DIR, { recursive: true, mode: 0o700 });
    const seed = crypto.getRandomValues(new Uint8Array(32));
    const kp = await createKeyPairSignerFromPrivateKeyBytes(seed);
    writeFileSync(
        resolve(KEY_DIR, `${Date.now()}-app-${name}.json`),
        JSON.stringify([...seed, ...getAddressEncoder().encode(kp.address)]),
        { mode: 0o600 },
    );
    return kp;
}

/** A browser wallet: pays its own fees and signs its own transactions. */
function walletClient(kp: KeyPairSigner) {
    return createClient()
        .use(signer(kp))
        .use(solanaRpc({ rpcUrl: RPC, transactionConfig: { version: 0 } }));
}

async function sendAs(
    kp: KeyPairSigner,
    label: string,
    plan: Parameters<ReturnType<typeof walletClient>['sendTransaction']>[0],
) {
    await sleep(1200);
    const sig = (await walletClient(kp).sendTransaction(plan)).context.signature;
    console.log(`${label.padEnd(30)} ${explorerTx(sig)}`);
    return sig;
}

async function server(arenaMatch: string, action: string, label: string) {
    await sleep(1200);
    const { status, body } = await api<{ signature?: string; error?: string }>('/api/server', {
        match: arenaMatch,
        action,
    });
    if (status !== 200 || !body.signature) throw new Error(`${action}: ${status} ${body.error}`);
    console.log(`${label.padEnd(30)} ${explorerTx(body.signature)}`);
    return body.signature;
}

async function waitFor<T>(what: string, read: () => Promise<T>, ok: (v: T) => boolean): Promise<T> {
    for (let i = 0; i < 20; i++) {
        const v = await read();
        if (ok(v)) return v;
        await sleep(2000);
    }
    throw new Error(`timed out waiting for ${what}`);
}

const walletView = async (a: string) => (await api<WalletView>(`/api/wallet/${a}?fresh`)).body;

// --- pages render ---
for (const page of ['/', '/matches', '/leaderboard']) {
    const res = await fetch(`${APP}${page}`);
    check(`page ${page} renders`, res.status === 200);
}

// --- two players, as two browser profiles ---
const a = await newWallet('A');
const b = await newWallet('B');
console.log(`Wallet A ${a.address}\nWallet B ${b.address}\n`);

for (const [name, kp] of [
    ['A', a],
    ['B', b],
] as const) {
    const claim = await api<{ ok: boolean; signature?: string; usdc?: string; sol?: string }>('/api/faucet', {
        wallet: kp.address,
    });
    check(
        `faucet pays wallet ${name}`,
        claim.status === 200 && claim.body.ok,
        `${claim.body.usdc} base units USDC, ${claim.body.sol} lamports SOL`,
    );
    if (claim.body.signature) console.log(`${`faucet ${name}`.padEnd(30)} ${explorerTx(claim.body.signature)}`);
}
const again = await api<{ retryAt?: number }>('/api/faucet', { wallet: a.address });
check('faucet rate-limits a second claim', again.status === 429 && !!again.body.retryAt);

await waitFor(
    'faucet funds',
    () => walletView(b.address),
    (w) => BigInt(w.usdc) === 20_000_000n && BigInt(w.sol) > 0n,
);

await sendAs(a, 'profile A (wallet A)', await createProfileIx(a));
await sendAs(b, 'profile B (wallet B)', await createProfileIx(b));

const entry = 5_000_000n;
const { ix, arenaMatch } = await createMatchIx(a, entry);
await sendAs(a, 'create match (wallet A)', ix);
const open = await waitFor(
    'open match listed',
    () => api<{ matches: { address: string; status: string }[] }>('/api/matches?fresh'),
    (r) => r.body.matches.some((m) => m.address === arenaMatch && m.status === 'Open'),
);
check('open match appears in the list', !!open);

await sendAs(b, 'join match (wallet B)', await joinMatchIx(b, arenaMatch));
const detailOf = () => api<MatchDetail>(`/api/match/${arenaMatch}`).then((r) => r.body);
await waitFor('Full', detailOf, (d) => d.match?.status === 'Full');
check('status Open -> Full', true);

await server(arenaMatch, 'start', 'server: start match');
await waitFor('Started', detailOf, (d) => d.match?.status === 'InProgress');
check('status Full -> Started', true);

const refusal = await api<{ error?: string }>('/api/server', { match: arenaMatch, action: 'start' });
check('server refuses to start twice', refusal.status === 409, refusal.body.error);

const payoutSig = await server(arenaMatch, 'win-b', 'server: player B wins (payout)');
const done = await waitFor(
    'Settled',
    detailOf,
    (d) => d.match?.status === 'Settled' && d.txs.some((t) => t.signature === payoutSig),
);
check('status Started -> Settled', done.match.status === 'Settled' && done.match.paidOut);
check(
    'settlement: pool 10, fee 0.5, payout 9.5',
    done.settlement?.pool === '10000000' && done.settlement?.fee === '500000' && done.settlement?.payout === '9500000',
);
check(
    'payout tx listed with label',
    done.txs.some((t) => t.signature === payoutSig && t.label === 'Server-signed result'),
);
check('every step has an Explorer-linked tx', done.txs.length >= 4, done.txs.map((t) => t.label).join(' | '));

const [wa, wb] = [await walletView(a.address), await walletView(b.address)];
check('wallet A: 20 - 5 = 15 USDC', wa.usdc === '15000000', wa.usdc);
check('wallet B: 20 - 5 + 9.5 = 24.5 USDC', wb.usdc === '24500000', wb.usdc);
check(
    'B rating up, A down',
    (wb.profile?.elo ?? 0) > 1000 && (wa.profile?.elo ?? 2000) < 1000,
    `A ${wa.profile?.elo}, B ${wb.profile?.elo}`,
);

const board = await waitFor(
    'leaderboard',
    () => api<{ players: ProfileView[] }>('/api/leaderboard'),
    (r) => r.body.players.some((p) => p.authority === b.address),
);
const ranks = board.body.players.map((p) => p.elo);
check(
    'leaderboard has both players, sorted by rating',
    board.body.players.some((p) => p.authority === a.address) && ranks.every((v, i) => i === 0 || ranks[i - 1] >= v),
);

const settled = await api<{ error?: string }>('/api/server', { match: arenaMatch, action: 'win-a' });
check('server refuses a second result', settled.status === 409, settled.body.error);

// --- return the throwaway wallets' SOL to the treasury ---
for (const kp of [a, b]) {
    const rpcClient = walletClient(kp);
    const { value } = await rpcClient.rpc.getBalance(kp.address).send();
    if (value > 5000n) {
        await sendAs(
            kp,
            `return SOL ${kp.address.slice(0, 6)}`,
            getTransferSolInstruction({ source: kp, destination: TREASURY, amount: value - 5000n }),
        );
    }
}

console.log(`\nMatch page: ${APP}/match/${arenaMatch}\nPayout: ${explorerTx(payoutSig)}`);
const failed = Object.entries(results).filter(([, ok]) => !ok);
console.log(
    failed.length ? `\n${failed.length} check(s) failed` : `\nAll ${Object.keys(results).length} checks passed`,
);
process.exit(failed.length ? 1 : 0);
