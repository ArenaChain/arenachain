// Every error the UI can show must be a readable sentence, never a raw dump.
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { explainError } from './errors';

const err = (message: string, cause?: unknown) => Object.assign(new Error(message), { cause });

test('wallet rejection', () => {
    assert.equal(
        explainError(err('WalletSignTransactionError: User rejected the request.')),
        'You rejected the request in your wallet.',
    );
});

test('known program errors, hex and decimal', () => {
    assert.match(
        explainError(err('Transaction simulation failed', err('custom program error: 0x1778'))),
        /can't join your own match/,
    );
    assert.match(explainError(err('x', { message: 'y', context: { code: 6005 } })), /banned/);
    assert.match(explainError(err('custom program error: 0xbc4')), /Create your profile/); // 3012
});

test('unknown program error names the code, not the dump', () => {
    const msg = explainError(
        err('Simulation failed. Logs: [..."Program log: AnchorError ... 0x177d"] custom program error: 0x177d'),
    );
    assert.equal(msg, 'The program rejected this transaction (error 6013).');
});

test('token, SOL, rate limit, expiry, network, no wallet', () => {
    assert.match(explainError(err('custom program error: 0x1')), /test USDC/);
    assert.match(explainError(err('Attempt to debit an account but found no record of a prior credit.')), /devnet SOL/);
    assert.match(explainError(err('HTTP error (429): Too Many Requests')), /rate-limiting/);
    assert.match(explainError(err('Blockhash not found')), /expired/);
    assert.match(explainError(new TypeError('Failed to fetch')), /Can't reach the network/);
    assert.match(explainError(err('No wallet connected')), /Connect a wallet/);
});

test('anything unrecognized becomes a plain sentence', () => {
    const raw = err('SolanaError: {"__code":7050011,"index":3,"accounts":["AbC123..."],"data":"0xdeadbeef"}');
    const orig = console.error;
    console.error = () => {};
    try {
        assert.equal(explainError(raw), 'Something went wrong. Refresh the page and try again.');
    } finally {
        console.error = orig;
    }
});
