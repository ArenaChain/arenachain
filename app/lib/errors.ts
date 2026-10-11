// Turns wallet and program errors into plain, actionable messages.

const PROGRAM_MESSAGES: Record<number, string> = {
    6002: 'The arena is paused. New matches and results are on hold.',
    6003: 'Entry must be more than 0 USDC.',
    6005: 'This wallet is banned from paid matches for now (recent forfeits).',
    6006: 'The game server for this match is offline.',
    6007: 'The match already moved on. Refresh to see its current state.',
    6008: "You can't join your own match. Switch to the other wallet.",
    6009: 'The deadline passed. Anyone can refund this match now.',
};
const ANCHOR_MESSAGES: Record<number, string> = {
    3012: 'Create your profile and get test USDC first.',
    2015: 'Use your own USDC account.',
};

function walk(e: unknown): string[] {
    const out: string[] = [];
    for (
        let cur = e as { message?: string; cause?: unknown; context?: unknown } | undefined;
        cur;
        cur = cur.cause as typeof cur
    ) {
        out.push(String(cur.message ?? ''));
        if (cur.context) out.push(JSON.stringify(cur.context, (_, v) => (typeof v === 'bigint' ? v.toString() : v)));
    }
    return out;
}

export function explainError(e: unknown): string {
    const text = walk(e).join(' | ');
    if (/reject|declin|denied|cancel/i.test(text) && /user|wallet/i.test(text))
        return 'You rejected the request in your wallet.';
    if (
        /insufficient lamports|insufficient funds for fee|0x1\b.*lamports|AccountNotFound|debit an account but found no record/i.test(
            text,
        )
    )
        return 'Not enough devnet SOL for fees. Use "Get test USDC", which also tops up a little SOL.';
    if (/custom program error: 0x1\b|"code":1\b|InsufficientFunds|insufficient funds/i.test(text))
        return 'Not enough test USDC. Use "Get test USDC" first.';
    const code = text.match(/custom program error: 0x([0-9a-f]+)/i)?.[1] ?? text.match(/"code":(\d{4})/)?.[1];
    if (code) {
        const n = text.includes('0x') && /custom program error/.test(text) ? parseInt(code, 16) : Number(code);
        if (PROGRAM_MESSAGES[n]) return PROGRAM_MESSAGES[n];
        if (ANCHOR_MESSAGES[n]) return ANCHOR_MESSAGES[n];
    }
    if (/already in use/i.test(text)) return 'That account already exists.';
    if (/429|Too Many Requests/i.test(text)) return 'Devnet RPC is rate-limiting. Wait a few seconds and try again.';
    if (/blockhash/i.test(text)) return 'The transaction expired before it landed. Try again.';
    return walk(e).find(Boolean)?.slice(0, 200) ?? 'Something went wrong.';
}
