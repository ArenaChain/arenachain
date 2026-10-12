'use client';

import Link from 'next/link';
import { useState } from 'react';
import useSWR from 'swr';

import { apiJson } from '@/app/providers';
import { useToast } from '@/components/Toasts';
import { ConnectButton } from '@/components/WalletButton';
import { createProfileIx } from '@/lib/actions';
import { formatSol, formatUsdc, short } from '@/lib/format';
import { useMatches, useWallet, useWalletView } from '@/lib/hooks';
import { useSendTx } from '@/lib/useSendTx';

const MIN_SOL = 5_000_000n; // enough for profile rent + fees

const HOW_IT_WORKS = [
    {
        title: 'Pay entry into escrow',
        body: 'Both players lock the same test-USDC entry in an account only the program controls.',
    },
    { title: 'Play', body: 'You play the 1v1 on the CS2 server. Nobody can touch the pool while you play.' },
    {
        title: 'Signed result, instant payout',
        body: 'The server signs the result, Solana checks the signature and pays the winner in the same transaction.',
    },
];

/** The most recent real payout on devnet, as proof the loop works end to end. */
function LatestPayout() {
    const { data } = useMatches();
    if (!data) return <div className="glass latest skeleton-card" aria-hidden="true" />;
    const m = data.matches.find((x) => x.status === 'Settled' && x.resultKind !== 'Void' && x.winner);
    if (!m || !m.winner) return null;
    const pool = BigInt(m.entryFee) * 2n;
    const payout = m.resultKind === 'Win' ? pool - (pool * 500n) / 10_000n : pool;
    const winnerIsA = m.winner === m.playerA;
    return (
        <Link href={`/match/${m.address}`} className="glass latest">
            <span className="latest-label">Latest payout</span>
            <span className={`latest-winner ${winnerIsA ? 'pa' : 'pb'}`}>{short(m.winner)}</span>
            <span className="latest-amount num">+{formatUsdc(payout)} USDC</span>
            <span className="muted small">
                from a {formatUsdc(pool)} USDC pool{m.resultKind === 'Forfeit' ? ', opponent forfeited' : ''}
            </span>
        </Link>
    );
}

function FaucetButton({ wallet, onDone }: { wallet: string; onDone: () => void }) {
    const toast = useToast();
    const [busy, setBusy] = useState(false);
    const { data, mutate } = useSWR<{ retryAt: number | null }>(`/api/faucet?wallet=${wallet}`);
    const retryAt = data?.retryAt ?? null;

    async function claim() {
        setBusy(true);
        try {
            const { status, data: body } = await apiJson<{
                retryAt?: number;
                usdc: string;
                sol: string;
                signature: string;
            }>('/api/faucet', { wallet });
            if (status === 429)
                throw new Error(`Already claimed. Try again ${new Date(body.retryAt!).toLocaleString()}.`);
            if (status >= 400) throw new Error(body.error ?? "The faucet couldn't send test USDC. Try again.");
            const sol = BigInt(body.sol) > 0n ? ` and ${formatSol(body.sol)} SOL for fees` : '';
            toast({
                tone: 'ok',
                title: `Received ${formatUsdc(body.usdc)} test USDC${sol}`,
                signature: body.signature,
            });
            onDone();
        } catch (e) {
            toast({ tone: 'error', title: (e as Error).message });
        } finally {
            setBusy(false);
            mutate();
        }
    }

    if (retryAt) {
        return <p className="muted small">Claimed. Next claim {new Date(retryAt).toLocaleString()}.</p>;
    }
    return (
        <button className="btn btn-primary" onClick={claim} disabled={busy}>
            {busy ? 'Sending…' : 'Get 20 test USDC'}
        </button>
    );
}

export default function PlayPage() {
    const { address } = useWallet();
    const { data: w, mutate } = useWalletView(address);
    const { send, isRunning } = useSendTx();
    const { client } = useWallet();
    const refresh = () => mutate(fetch(`/api/wallet/${address}?fresh`).then((r) => r.json()));

    async function createProfile() {
        const sig = await send('Profile created', await createProfileIx(client.payer));
        if (sig) refresh();
    }

    const hasFunds = w ? BigInt(w.usdc) > 0n : false;
    const hasSol = w ? BigInt(w.sol) >= MIN_SOL : false;
    const p = w?.profile ?? null;

    return (
        <>
            <section className="hero">
                <div className="hero-copy">
                    <h1>
                        <span className="hero-line">Win. Get paid.</span>{' '}
                        <span className="hero-line">No middleman.</span>
                    </h1>
                    <p className="hero-sub">
                        Skill-based CS2 1v1s. The server signs the result, Solana pays the winner instantly.
                    </p>
                    <div className="hero-ctas">
                        {!address && <ConnectButton />}
                        <Link className={`btn btn-lg ${address ? 'btn-primary' : 'btn-glass'}`} href="/matches">
                            Find a match
                        </Link>
                    </div>
                </div>
                <LatestPayout />
            </section>

            <section aria-labelledby="how-title">
                <h2 id="how-title" className="section-title">
                    How it works
                </h2>
                <ol className="how">
                    {HOW_IT_WORKS.map((s, i) => (
                        <li key={s.title} className="glass how-step">
                            <span className="how-num" aria-hidden="true">
                                {i + 1}
                            </span>
                            <h3>{s.title}</h3>
                            <p className="muted">{s.body}</p>
                        </li>
                    ))}
                </ol>
            </section>

            {address && (
                <div className="grid-2">
                    <section className="glass card">
                        <h2>Get started</h2>
                        {!w ? (
                            <div className="skeleton-lines" aria-hidden="true">
                                <span className="skeleton" />
                                <span className="skeleton" />
                                <span className="skeleton" />
                            </div>
                        ) : (
                            <>
                                <ol className="steps">
                                    <li className={hasFunds ? 'done' : ''}>
                                        <div>
                                            <strong>Get test USDC</strong>
                                            <p className="muted small">
                                                20 test USDC from the arena treasury, once a day per wallet.
                                            </p>
                                        </div>
                                        {hasFunds ? (
                                            <span className="tick">Done</span>
                                        ) : (
                                            <FaucetButton wallet={address} onDone={refresh} />
                                        )}
                                    </li>
                                    <li className={p ? 'done' : ''}>
                                        <div>
                                            <strong>Create your player profile</strong>
                                            <p className="muted small">
                                                Holds your rating, record and reputation on-chain.
                                            </p>
                                        </div>
                                        {p ? (
                                            <span className="tick">Done</span>
                                        ) : (
                                            <button
                                                className="btn btn-primary"
                                                onClick={createProfile}
                                                disabled={isRunning || !hasSol}
                                                title={
                                                    hasSol
                                                        ? undefined
                                                        : 'Get test USDC first: it also tops up SOL for fees'
                                                }
                                            >
                                                {isRunning ? 'Confirm in wallet…' : 'Create profile'}
                                            </button>
                                        )}
                                    </li>
                                    <li>
                                        <div>
                                            <strong>Play a match</strong>
                                            <p className="muted small">
                                                Create one with an entry fee, or join an open one.
                                            </p>
                                        </div>
                                        <Link className="btn btn-glass" href="/matches">
                                            Go to matches
                                        </Link>
                                    </li>
                                </ol>
                                {hasFunds && (
                                    <div className="faucet-again">
                                        <FaucetButton wallet={address} onDone={refresh} />
                                    </div>
                                )}
                            </>
                        )}
                    </section>

                    <section className="glass card profile">
                        <h2>Your profile</h2>
                        {!w ? (
                            <div className="skeleton-lines" aria-hidden="true">
                                <span className="skeleton skeleton-xl" />
                                <span className="skeleton" />
                                <span className="skeleton" />
                            </div>
                        ) : p ? (
                            <>
                                <div className="rating">
                                    <span className="rating-value num">{p.elo}</span>
                                    <span className="rating-label">rating</span>
                                </div>
                                <dl className="stats">
                                    <div>
                                        <dt>Wins</dt>
                                        <dd className="num">{p.wins}</dd>
                                    </div>
                                    <div>
                                        <dt>Losses</dt>
                                        <dd className="num">{p.losses}</dd>
                                    </div>
                                    <div>
                                        <dt>Forfeits</dt>
                                        <dd className="num">{p.totalForfeits}</dd>
                                    </div>
                                </dl>
                                <div className="rep">
                                    <div className="rep-head">
                                        <span>Reputation</span>
                                        <span className="num">{p.reputation} / 100</span>
                                    </div>
                                    <div
                                        className="rep-bar"
                                        role="meter"
                                        aria-valuemin={0}
                                        aria-valuemax={100}
                                        aria-valuenow={p.reputation}
                                    >
                                        <span style={{ width: `${p.reputation}%` }} />
                                    </div>
                                </div>
                                {p.bannedUntil * 1000 > Date.now() && (
                                    <p className="note">
                                        Banned from paid matches until {new Date(p.bannedUntil * 1000).toLocaleString()}{' '}
                                        after repeated forfeits.
                                    </p>
                                )}
                            </>
                        ) : (
                            <p className="muted">No profile yet. Create one to start playing.</p>
                        )}
                        <dl className="balances">
                            <div>
                                <dt>Test USDC</dt>
                                <dd className="num">{w ? formatUsdc(w.usdc) : '–'}</dd>
                            </div>
                            <div>
                                <dt>SOL for fees</dt>
                                <dd className="num">{w ? formatSol(w.sol) : '–'}</dd>
                            </div>
                        </dl>
                    </section>
                </div>
            )}
        </>
    );
}
