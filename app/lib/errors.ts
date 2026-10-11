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
    const hex = text.match(/custom program error: 0x([0-9a-f]+)/i)?.[1];
    const dec = text.match(/"code":(\d{4})/)?.[1];
    const n = hex ? parseInt(hex, 16) : dec ? Number(dec) : null;
    if (n != null) {
        if (PROGRAM_MESSAGES[n]) return PROGRAM_MESSAGES[n];
        if (ANCHOR_MESSAGES[n]) return ANCHOR_MESSAGES[n];
    }
    const code = n == null ? null : String(n);
    if (/already in use/i.test(text)) return 'That account already exists.';
    if (/429|Too Many Requests/i.test(text)) return 'Devnet RPC is rate-limiting. Wait a few seconds and try again.';
    if (/blockhash|not confirmed|timed? ?out/i.test(text))
        return 'The transaction expired before it landed. Try again.';
    if (/no (wallet|signer)|not connected|wallet.*disconnected/i.test(text)) return 'Connect a wallet first.';
    if (/failed to fetch|network ?error|ECONNREFUSED|fetch failed/i.test(text))
        return "Can't reach the network. Check your connection and try again.";
    if (code) return `The program rejected this transaction (error ${code}).`;
    if (/simulat/i.test(text)) return 'The transaction would fail, so it was not sent. Refresh and try again.';
    // Never show raw error dumps; keep the detail in the console for debugging.
    if (typeof console !== 'undefined') console.error('Unrecognized error', e);
    return 'Something went wrong. Refresh the page and try again.';
}
