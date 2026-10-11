import 'server-only';

import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';

import {
    fetchMaybeMatch,
    fetchMaybePlayerProfile,
    findProfilePda,
    getMatchDecoder,
    getPlayerProfileDecoder,
    type Match,
    MATCH_DISCRIMINATOR,
    MatchStatus,
    PLAYER_PROFILE_DISCRIMINATOR,
    type PlayerProfile,
    ResultKind,
} from '@arenachain/client';
import { fetchMaybeToken } from '@solana-program/token';
import {
    type Address,
    address,
    type Base58EncodedBytes,
    createClient,
    createKeyPairSignerFromBytes,
    createSolanaRpc,
    getBase58Decoder,
    getBase64Encoder,
    isSome,
    type KeyPairSigner,
    type ReadonlyUint8Array,
} from '@solana/kit';
import { solanaRpc } from '@solana/kit-plugin-rpc';
import { signer } from '@solana/kit-plugin-signer';

import { usdcAccountOf } from '../actions';
import { GAME_SERVER_KEY, PROGRAM_ID, TREASURY } from '../deployment';
import type { MatchStatusName, MatchView, ProfileView, ResultKindName, Settlement, WalletView } from '../types';

const DEVNET_GENESIS_HASH = 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG';
export const RPC_URL = process.env.SOLANA_RPC_URL ?? 'https://api.devnet.solana.com';
export const rpc = createSolanaRpc(RPC_URL);

// ---------- rate-limit friendly reads ----------

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function isRateLimited(e: unknown): boolean {
    for (let cur = e as { message?: string; cause?: unknown } | undefined; cur; cur = cur.cause as typeof cur) {
        if (String(cur.message ?? '').includes('429')) return true;
    }
    return false;
}

/** Retries RPC reads the public devnet endpoint rate-limited. */
export async function retry<T>(fn: () => Promise<T>, attempts = 5): Promise<T> {
    for (let i = 0; ; i++) {
        try {
            return await fn();
        } catch (e) {
            if (!isRateLimited(e) || i + 1 >= attempts) throw e;
            await sleep(800 * 2 ** i);
        }
    }
}

const cache = new Map<string, { at: number; value: Promise<unknown> }>();
/** Short-lived shared cache so many viewers don't multiply RPC calls. */
export function cached<T>(key: string, ttlMs: number, fn: () => Promise<T>): Promise<T> {
    const hit = cache.get(key);
    if (hit && Date.now() - hit.at < ttlMs) return hit.value as Promise<T>;
    const value = fn().catch((e) => {
        cache.delete(key);
        throw e;
    });
    cache.set(key, { at: Date.now(), value });
    return value;
}
export const invalidate = (prefix: string) => {
    for (const k of cache.keys()) if (k.startsWith(prefix)) cache.delete(k);
};

// ---------- keys (server only, never sent to the browser) ----------

const expandHome = (p: string) => (p.startsWith('~/') ? resolve(homedir(), p.slice(2)) : p);

async function loadKey(envVar: string, fallback: string, expected: Address): Promise<KeyPairSigner> {
    const path = expandHome(process.env[envVar] ?? fallback);
    const kp = await createKeyPairSignerFromBytes(Uint8Array.from(JSON.parse(readFileSync(path, 'utf8'))));
    if (kp.address !== expected) throw new Error(`${envVar} is ${kp.address}, expected ${expected}`);
    return kp;
}

let treasuryClientPromise: ReturnType<typeof makeTreasuryClient> | undefined;
async function makeTreasuryClient() {
    const treasury = await loadKey('TREASURY_KEYPAIR', '~/.config/solana/arenachain-devnet.json', TREASURY);
    const client = createClient()
        .use(signer(treasury))
        .use(solanaRpc({ rpcUrl: RPC_URL }));
    const genesis = await retry(() => client.rpc.getGenesisHash().send());
    if (genesis !== DEVNET_GENESIS_HASH) throw new Error(`${RPC_URL} is not devnet`);
    return { client, treasury };
}
/** Client that pays fees from the treasury wallet (faucet, simulated server relays). */
export function treasuryClient() {
    return (treasuryClientPromise ??= makeTreasuryClient().catch((e) => {
        treasuryClientPromise = undefined;
        throw e;
    }));
}

let serverKeyPromise: Promise<KeyPairSigner> | undefined;
/** The simulated CS2 server's signing key. */
export function gameServerKey() {
    return (serverKeyPromise ??= loadKey(
        'GAME_SERVER_KEYPAIR',
        '~/.config/solana/arenachain-devnet-server.json',
        GAME_SERVER_KEY,
    ));
}

// ---------- views ----------

const DEFAULT = '11111111111111111111111111111111';
const STATUS: MatchStatusName[] = ['Open', 'Full', 'InProgress', 'Settled', 'Refunded', 'Disputed'];
const KIND: ResultKindName[] = ['Win', 'Forfeit', 'Void'];

export function matchView(addr: string, m: Match): MatchView {
    return {
        address: addr,
        playerA: m.playerA,
        playerB: m.playerB === DEFAULT ? null : m.playerB,
        gameServer: m.gameServer,
        entryFee: m.entryFee.toString(),
        status: STATUS[m.status as MatchStatus],
        createdAt: Number(m.createdAt),
        joinedAt: Number(m.joinedAt),
        startedAt: Number(m.startedAt),
        startDeadline: Number(m.startDeadline),
        resultDeadline: Number(m.resultDeadline),
        resultKind: isSome(m.resultKind) ? KIND[m.resultKind.value as ResultKind] : null,
        winner: m.winner === DEFAULT ? null : m.winner,
        unlockAt: Number(m.unlockAt),
        paidOut: m.paidOut,
    };
}

export function profileView(p: PlayerProfile): ProfileView {
    return {
        authority: p.authority,
        elo: p.elo,
        wins: p.wins,
        losses: p.losses,
        totalForfeits: p.totalForfeits,
        reputation: p.reputation,
        bannedUntil: Number(p.bannedUntil),
    };
}

/** Pool, fee and payout exactly as the program computes them. */
export function settlementOf(m: MatchView): Settlement | null {
    if (!m.resultKind) return null;
    const entry = BigInt(m.entryFee);
    const pool = entry * 2n;
    if (m.resultKind === 'Void') {
        return { pool: pool.toString(), fee: '0', payout: '0', refundEach: entry.toString(), held: false };
    }
    const fee = m.resultKind === 'Win' ? (pool * 500n) / 10_000n : 0n;
    return {
        pool: pool.toString(),
        fee: fee.toString(),
        payout: (pool - fee).toString(),
        refundEach: null,
        held: !m.paidOut,
    };
}

const b58 = getBase58Decoder();
async function scan<T>(discriminator: ReadonlyUint8Array, decode: (bytes: ReadonlyUint8Array) => T) {
    const accounts = await retry(() =>
        rpc
            .getProgramAccounts(PROGRAM_ID, {
                encoding: 'base64',
                filters: [
                    {
                        memcmp: {
                            offset: 0n,
                            bytes: b58.decode(discriminator) as Base58EncodedBytes,
                            encoding: 'base58',
                        },
                    },
                ],
            })
            .send(),
    );
    const b64 = getBase64Encoder();
    return accounts.map((a) => ({ address: a.pubkey as string, data: decode(b64.encode(a.account.data[0])) }));
}

export function allMatches(): Promise<MatchView[]> {
    return cached('matches', 4000, async () => {
        const rows = await scan(MATCH_DISCRIMINATOR, (b) => getMatchDecoder().decode(b));
        return rows.map((r) => matchView(r.address, r.data)).sort((x, y) => y.createdAt - x.createdAt);
    });
}

export function allProfiles(): Promise<ProfileView[]> {
    return cached('profiles', 8000, async () => {
        const rows = await scan(PLAYER_PROFILE_DISCRIMINATOR, (b) => getPlayerProfileDecoder().decode(b));
        return rows.map((r) => profileView(r.data));
    });
}

export function getMatch(addr: Address): Promise<MatchView | null> {
    return cached(`match:${addr}`, 2000, async () => {
        const acc = await retry(() => fetchMaybeMatch(rpc, addr));
        return acc.exists ? matchView(addr, acc.data) : null;
    });
}

export function getProfile(wallet: Address): Promise<ProfileView | null> {
    return cached(`profile:${wallet}`, 2000, async () => {
        const [pda] = await findProfilePda({ authority: wallet });
        const acc = await retry(() => fetchMaybePlayerProfile(rpc, pda));
        return acc.exists ? profileView(acc.data) : null;
    });
}

export function getWallet(wallet: Address): Promise<WalletView> {
    return cached(`wallet:${wallet}`, 2000, async () => {
        const ata = await usdcAccountOf(wallet);
        const [{ value: lamports }, token, profile] = await Promise.all([
            retry(() => rpc.getBalance(wallet).send()),
            retry(() => fetchMaybeToken(rpc, ata)),
            getProfile(wallet),
        ]);
        return {
            wallet,
            sol: lamports.toString(),
            usdc: token.exists ? token.data.amount.toString() : '0',
            hasUsdcAccount: token.exists,
            profile,
        };
    });
}

export const toAddress = (s: string): Address | null => {
    try {
        return address(s);
    } catch {
        return null;
    }
};
