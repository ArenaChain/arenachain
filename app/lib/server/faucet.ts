import 'server-only';

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { getTransferSolInstruction } from '@solana-program/system';
import {
    fetchToken,
    getCreateAssociatedTokenIdempotentInstructionAsync,
    getTransferCheckedInstruction,
} from '@solana-program/token';
import { type Address, lamports } from '@solana/kit';

import { usdcAccountOf } from '../actions';
import { USDC_MINT } from '../deployment';
import { invalidate, retry, rpc, treasuryClient } from './chain';

export const FAUCET_USDC = 20_000_000n; // 20 test USDC
const COOLDOWN_MS = Number(process.env.FAUCET_COOLDOWN_HOURS ?? 24) * 3600_000;
/** New devnet wallets usually have no SOL; top them up just enough for rent and fees. */
const SOL_TOPUP = BigInt(process.env.FAUCET_SOL_TOPUP_LAMPORTS ?? 20_000_000); // 0.02 SOL
const SOL_TOPUP_BELOW = 10_000_000n; // only if the wallet holds < 0.01 SOL

const STORE = resolve(process.cwd(), '.data/faucet.json');
type Claims = Record<string, number>; // wallet -> last claim (ms)

function readClaims(): Claims {
    try {
        return JSON.parse(readFileSync(STORE, 'utf8'));
    } catch {
        return {};
    }
}
function writeClaims(c: Claims) {
    mkdirSync(resolve(STORE, '..'), { recursive: true });
    writeFileSync(STORE, JSON.stringify(c, null, 2));
}

export function nextClaimAt(wallet: Address): number | null {
    const last = readClaims()[wallet];
    return last && Date.now() - last < COOLDOWN_MS ? last + COOLDOWN_MS : null;
}

// One claim at a time, so two quick clicks can't both pass the rate-limit check.
let queue: Promise<unknown> = Promise.resolve();

export type FaucetResult =
    | { ok: true; signature: string; usdc: string; sol: string }
    | { ok: false; retryAt: number }
    | { ok: false; error: string };

export function claim(wallet: Address): Promise<FaucetResult> {
    const run = queue.then(async (): Promise<FaucetResult> => {
        const retryAt = nextClaimAt(wallet);
        if (retryAt) return { ok: false, retryAt };

        const { client, treasury } = await treasuryClient();
        const source = await usdcAccountOf(treasury.address);
        const destination = await usdcAccountOf(wallet);
        const stock = await retry(() => fetchToken(rpc, source));
        if (stock.data.amount < FAUCET_USDC) return { ok: false, error: 'The faucet is out of test USDC.' };

        const { value: walletLamports } = await retry(() => rpc.getBalance(wallet).send());
        const sol = walletLamports < SOL_TOPUP_BELOW ? SOL_TOPUP : 0n;

        const result = await client.sendTransaction([
            await getCreateAssociatedTokenIdempotentInstructionAsync({
                payer: treasury,
                owner: wallet,
                mint: USDC_MINT,
            }),
            getTransferCheckedInstruction({
                source,
                mint: USDC_MINT,
                destination,
                authority: treasury,
                amount: FAUCET_USDC,
                decimals: 6,
            }),
            ...(sol > 0n
                ? [getTransferSolInstruction({ source: treasury, destination: wallet, amount: lamports(sol) })]
                : []),
        ]);
        writeClaims({ ...readClaims(), [wallet]: Date.now() });
        invalidate(`wallet:${wallet}`);
        return { ok: true, signature: result.context.signature, usdc: FAUCET_USDC.toString(), sol: sol.toString() };
    });
    queue = run.catch(() => undefined);
    return run;
}
