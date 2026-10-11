// Shared helpers for the devnet scripts. Devnet only: every client checks the genesis hash.

import { chmodSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';

import {
    createClient,
    createKeyPairSignerFromBytes,
    createKeyPairSignerFromPrivateKeyBytes,
    getAddressEncoder,
    type KeyPairSigner,
} from '@solana/kit';
import { solanaRpc } from '@solana/kit-plugin-rpc';
import { signer } from '@solana/kit-plugin-signer';

const ROOT = resolve(import.meta.dirname, '../..');
if (existsSync(resolve(ROOT, '.env'))) process.loadEnvFile(resolve(ROOT, '.env'));

export const DEVNET_GENESIS_HASH = 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG';
export const RPC_URL = process.env.SOLANA_RPC_URL ?? 'https://api.devnet.solana.com';
const DEPLOYMENT_FILE = resolve(ROOT, 'deployments/devnet.json');

export function expandHome(path: string): string {
    return path.startsWith('~/') ? resolve(homedir(), path.slice(2)) : path;
}

/** Loads a Solana CLI keypair file. Never prints or returns the secret. */
export async function loadKeypair(path: string): Promise<KeyPairSigner> {
    const bytes = Uint8Array.from(JSON.parse(readFileSync(expandHome(path), 'utf8')));
    return createKeyPairSignerFromBytes(bytes);
}

export const walletPath = () => process.env.DEVNET_WALLET ?? '~/.config/solana/arenachain-devnet.json';

/** Kit client paying with `payer`, refusing anything that isn't devnet. */
export async function devnetClient(payer: KeyPairSigner) {
    const client = createClient().use(signer(payer)).use(solanaRpc({ rpcUrl: RPC_URL }));
    const genesis = await client.rpc.getGenesisHash().send();
    if (genesis !== DEVNET_GENESIS_HASH) {
        throw new Error(`Refusing to run: ${RPC_URL} is not devnet (genesis ${genesis}).`);
    }
    return client;
}

export type Deployment = Record<string, unknown> & { programId: string; usdcMint: string };

export function readDeployment(): Deployment {
    return JSON.parse(readFileSync(DEPLOYMENT_FILE, 'utf8'));
}

export function updateDeployment(patch: Record<string, unknown>): void {
    writeFileSync(DEPLOYMENT_FILE, JSON.stringify({ ...readDeployment(), ...patch }, null, 2) + '\n');
}

export const explorerTx = (sig: string) => `https://explorer.solana.com/tx/${sig}?cluster=devnet`;
export const explorerAddress = (a: string) => `https://explorer.solana.com/address/${a}?cluster=devnet`;

export const USDC = 1_000_000n;

/** True if the public RPC rate-limited (HTTP 429) anywhere in the error's cause chain. */
export function isRateLimited(e: unknown): boolean {
    for (let cur = e as { message?: string; cause?: unknown } | undefined; cur; cur = cur.cause as typeof cur) {
        if (String(cur.message ?? '').includes('429')) return true;
    }
    return false;
}

/** True if the failure happened while simulating, i.e. before anything was submitted. */
export function failedBeforeSubmit(e: unknown): boolean {
    for (let cur = e as { message?: string; cause?: unknown } | undefined; cur; cur = cur.cause as typeof cur) {
        if (/estimate the compute unit|simulat/i.test(String(cur.message ?? ''))) return true;
    }
    return false;
}

export const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Retries an RPC call the public devnet RPC rate-limited, with backoff. `onlyIf` narrows which
 *  errors are safe to retry (for sends: only failures before the transaction was submitted). */
export async function retry<T>(
    fn: () => Promise<T>,
    onlyIf: (e: unknown) => boolean = () => true,
    attempts = 6,
): Promise<T> {
    for (let i = 0; ; i++) {
        try {
            return await fn();
        } catch (e) {
            if (!isRateLimited(e) || !onlyIf(e) || i + 1 >= attempts) throw e;
            await sleep(1500 * 2 ** i);
        }
    }
}

/** Throwaway e2e keys live here (outside the repo, mode 600) so their SOL can always be swept back. */
export const E2E_KEY_DIR = expandHome('~/.config/solana/arenachain-devnet-e2e');

/** New keypair, saved in Solana CLI format before it's ever funded. */
export async function newSavedKeypair(name: string): Promise<KeyPairSigner> {
    mkdirSync(E2E_KEY_DIR, { recursive: true, mode: 0o700 });
    const seed = crypto.getRandomValues(new Uint8Array(32));
    const kp = await createKeyPairSignerFromPrivateKeyBytes(seed);
    const file = resolve(E2E_KEY_DIR, `${Date.now()}-${name}.json`);
    writeFileSync(file, JSON.stringify([...seed, ...getAddressEncoder().encode(kp.address)]), { mode: 0o600 });
    chmodSync(file, 0o600);
    return kp;
}

export async function savedE2eKeypairs(): Promise<KeyPairSigner[]> {
    if (!existsSync(E2E_KEY_DIR)) return [];
    const files = readdirSync(E2E_KEY_DIR).filter((f) => f.endsWith('.json'));
    return Promise.all(files.map((f) => loadKeypair(resolve(E2E_KEY_DIR, f))));
}
