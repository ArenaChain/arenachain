import 'server-only';

import type { Address, Signature } from '@solana/kit';

import type { TxView } from '../types';
import { cached, retry, rpc } from './chain';

const LABELS: Record<string, string> = {
    PlayerCreateMatch: 'Match created, A paid entry',
    PlayerJoinMatch: 'B joined, paid entry',
    PlayerCancelMatch: 'Cancelled, entry refunded',
    ServerStartMatch: 'Server started the match',
    SubmitResult: 'Server-signed result',
    ClaimPayout: 'Held payout claimed',
    RefundExpired: 'Expired, both refunded',
};

// Confirmed transactions never change, so their labels are cached for the process lifetime.
const txLabels = new Map<string, { label: string; ok: boolean }>();

async function labelOf(sig: string) {
    const hit = txLabels.get(sig);
    if (hit) return hit;
    const tx = await retry(() =>
        rpc
            .getTransaction(sig as Signature, {
                maxSupportedTransactionVersion: 0,
                encoding: 'json',
                commitment: 'confirmed',
            })
            .send(),
    );
    const logs = tx?.meta?.logMessages ?? [];
    const ix = logs.map((l) => l.match(/^Program log: Instruction: (\w+)/)?.[1]).find((n) => n && LABELS[n]);
    const value = { label: ix ? LABELS[ix] : 'Transaction', ok: tx?.meta?.err == null };
    if (tx) txLabels.set(sig, value);
    return value;
}

/** Every transaction that touched the match account, oldest first. */
export function matchHistory(arenaMatch: Address): Promise<TxView[]> {
    return cached(`history:${arenaMatch}`, 3000, async () => {
        const sigs = await retry(() =>
            rpc.getSignaturesForAddress(arenaMatch, { limit: 25, commitment: 'confirmed' }).send(),
        );
        const out: TxView[] = [];
        for (const s of [...sigs].reverse()) {
            const { label, ok } = await labelOf(s.signature);
            out.push({
                signature: s.signature,
                label,
                blockTime: s.blockTime == null ? null : Number(s.blockTime),
                ok: ok && s.err == null,
            });
        }
        return out;
    });
}
