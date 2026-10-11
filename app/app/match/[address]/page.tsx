'use client';

import { address as toAddress } from '@solana/kit';
import Link from 'next/link';
import { use } from 'react';

import { Scoreboard, ServerPanel, SettlementPanel, StatusChip, StatusTrack } from '@/components/match';
import { cancelMatchIx, joinMatchIx, refundExpiredIx } from '@/lib/actions';
import { explorerAddress, explorerTx, formatUsdc, isExpired, timeAgo } from '@/lib/format';
import { useMatchDetail, useWallet, useWalletView } from '@/lib/hooks';
import { useSendTx } from '@/lib/useSendTx';

function Deadline({ label, at, now }: { label: string; at: number; now: number }) {
    const left = at - now;
    return (
        <p className="muted small">
            {label} {left > 0 ? `in ${Math.ceil(left / 60)} min` : 'has passed: anyone can refund both players'}
        </p>
    );
}

export default function MatchPage({ params }: { params: Promise<{ address: string }> }) {
    const { address: matchAddress } = use(params);
    const { client, address: me } = useWallet();
    const { data: w } = useWalletView(me);
    const { data, error, mutate } = useMatchDetail(matchAddress);
    const { send, isRunning } = useSendTx();

    if (error)
        return (
            <section className="panel">
                <h2>Match not available</h2>
                <p className="muted">{error.message}</p>
            </section>
        );
    if (!data) return <p className="muted">Loading match…</p>;
    const m = data.match;

    const canJoin = m.status === 'Open' && me != null && me !== m.playerA;
    const canCancel = m.status === 'Open' && me === m.playerA;
    const expired = isExpired(m, data.now);

    async function join() {
        if (await send('Joined the match', await joinMatchIx(client.payer, toAddress(m.address)))) mutate();
    }
    async function refund() {
        const ix = await refundExpiredIx(toAddress(m.address), toAddress(m.playerA), toAddress(m.playerB!));
        if (await send('Both players refunded', ix)) mutate();
    }
    async function cancel() {
        if (await send('Match cancelled, entry refunded', await cancelMatchIx(client.payer, toAddress(m.address))))
            mutate();
    }

    return (
        <>
            <div className="match-head">
                <Link href="/matches" className="back">
                    All matches
                </Link>
                <StatusChip status={m.status} expired={expired} />
                <a className="muted small" href={explorerAddress(m.address)} target="_blank" rel="noreferrer">
                    Match account on Explorer
                </a>
            </div>

            <Scoreboard detail={data} me={me} />
            <StatusTrack status={m.status} />

            {(canJoin || canCancel) && (
                <section className="panel actions">
                    {canJoin && (
                        <>
                            <p>
                                Pay {formatUsdc(m.entryFee)} test USDC to take seat B. It stays in escrow until the
                                server signs the result.
                            </p>
                            <button
                                className="btn btn-primary"
                                onClick={join}
                                disabled={isRunning || !w?.profile || BigInt(w?.usdc ?? 0) < BigInt(m.entryFee)}
                            >
                                {isRunning ? 'Confirm in wallet…' : `Join for ${formatUsdc(m.entryFee)} USDC`}
                            </button>
                            {!w?.profile && <p className="muted small">Create your profile on the Play page first.</p>}
                        </>
                    )}
                    {canCancel && (
                        <>
                            <p>Waiting for an opponent. Share this page, or open it in your second browser profile.</p>
                            <button className="btn btn-ghost" onClick={cancel} disabled={isRunning}>
                                {isRunning ? 'Confirm in wallet…' : 'Cancel and get my entry back'}
                            </button>
                        </>
                    )}
                </section>
            )}

            {expired && (
                <section className="panel actions">
                    <p>
                        Nothing settled before the deadline, so the escrow can&apos;t stay locked: anyone can send both
                        entries back. No fee is taken.
                    </p>
                    {me ? (
                        <button className="btn btn-primary" onClick={refund} disabled={isRunning}>
                            {isRunning ? 'Confirm in wallet…' : 'Refund both players'}
                        </button>
                    ) : (
                        <p className="muted small">Connect any wallet to send the refund.</p>
                    )}
                </section>
            )}

            {!expired && m.status === 'Full' && (
                <Deadline label="The server must start the match" at={m.startDeadline} now={data.now} />
            )}
            {!expired && m.status === 'InProgress' && (
                <Deadline label="The result is due" at={m.resultDeadline} now={data.now} />
            )}

            <SettlementPanel detail={data} />
            <ServerPanel detail={data} />

            <section className="panel">
                <h2>Transactions</h2>
                {data.txs.length === 0 ? (
                    <p className="muted">No transactions yet.</p>
                ) : (
                    <ol className="txs">
                        {data.txs.map((t) => (
                            <li key={t.signature} className={t.ok ? '' : 'failed'}>
                                <span className="tx-label">{t.label}</span>
                                <span className="muted small">{t.blockTime ? timeAgo(t.blockTime) : ''}</span>
                                <a href={explorerTx(t.signature)} target="_blank" rel="noreferrer">
                                    Explorer
                                </a>
                            </li>
                        ))}
                    </ol>
                )}
            </section>
        </>
    );
}
