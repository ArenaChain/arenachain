import type { MatchView } from './types';

export const USDC_DECIMALS = 6;
const UNIT = 10n ** BigInt(USDC_DECIMALS);

/** "12.5" from base units, trimming trailing zeros. */
export function formatUsdc(base: bigint | string | number): string {
    const v = BigInt(base);
    const whole = v / UNIT;
    const frac = (v % UNIT).toString().padStart(USDC_DECIMALS, '0').replace(/0+$/, '');
    return frac ? `${whole}.${frac}` : `${whole}`;
}

/** Base units from a user-typed amount like "5" or "2.5". Returns null if invalid. */
export function parseUsdc(input: string): bigint | null {
    const m = input.trim().match(/^(\d+)(?:\.(\d{1,6}))?$/);
    if (!m) return null;
    return BigInt(m[1]) * UNIT + BigInt((m[2] ?? '').padEnd(USDC_DECIMALS, '0'));
}

export function formatSol(lamports: bigint | string | number): string {
    return (Number(lamports) / 1e9).toLocaleString(undefined, { maximumFractionDigits: 4 });
}

export const short = (a: string) => `${a.slice(0, 4)}…${a.slice(-4)}`;

export const explorerTx = (sig: string) => `https://explorer.solana.com/tx/${sig}?cluster=devnet`;
export const explorerAddress = (a: string) => `https://explorer.solana.com/address/${a}?cluster=devnet`;

/** True once a joined or started match passed its deadline with no result. */
export function isExpired(m: MatchView, now: number): boolean {
    if (m.status === 'Full') return now > m.startDeadline;
    if (m.status === 'InProgress') return now > m.resultDeadline;
    return false;
}

export function timeAgo(unixSeconds: number): string {
    const s = Math.max(0, Math.floor(Date.now() / 1000) - unixSeconds);
    if (s < 60) return `${s}s ago`;
    if (s < 3600) return `${Math.floor(s / 60)} min ago`;
    if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
    return `${Math.floor(s / 86400)} d ago`;
}
