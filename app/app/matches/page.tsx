'use client';

import { address as toAddress } from '@solana/kit';
import { useRouter } from 'next/navigation';
import { useState } from 'react';

import { GameModes } from '@/components/GameModes';
import { MatchCard, MatchListSkeleton } from '@/components/match';
import { createMatchIx, joinMatchIx } from '@/lib/actions';
import { HOLD_THRESHOLD } from '@/lib/deployment';
import { formatUsdc, parseUsdc } from '@/lib/format';
import { useMatches, useWallet, useWalletView } from '@/lib/hooks';
import { useSendTx } from '@/lib/useSendTx';

const QUICK = ['1', '5', '10'];

export default function MatchesPage() {
    const router = useRouter();
    const { client, address } = useWallet();
    const { data: w } = useWalletView(address);
    const { data, mutate } = useMatches();
    const { send, isRunning } = useSendTx();
    const [entry, setEntry] = useState('5');
    const [joining, setJoining] = useState<string | null>(null);

    const amount = parseUsdc(entry);
    const balance = w ? BigInt(w.usdc) : 0n;
    const problem = !address
        ? 'Connect a wallet first.'
        : !w?.profile
          ? 'Create your profile on the Play page first.'
          : amount == null || amount <= 0n
            ? 'Enter an amount like 5 or 2.5.'
            : amount > balance
              ? `You have ${formatUsdc(balance)} test USDC.`
              : null;

    async function create() {
        if (problem || amount == null) return;
        const { ix, arenaMatch } = await createMatchIx(client.payer, amount);
        const sig = await send(`Match created: ${formatUsdc(amount)} USDC entry`, ix);
        if (sig) router.push(`/match/${arenaMatch}`);
    }

    async function join(arenaMatch: string) {
        setJoining(arenaMatch);
        const sig = await send('Joined the match', await joinMatchIx(client.payer, toAddress(arenaMatch)));
        setJoining(null);
        if (sig) router.push(`/match/${arenaMatch}`);
        else mutate();
    }

    const matches = data?.matches ?? [];
    const open = matches.filter((m) => m.status === 'Open');
    const rest = matches.filter((m) => m.status !== 'Open').slice(0, 12);

    return (
        <>
            <section className="page-head">
                <h1>Matches</h1>
                <p className="muted">
                    Put up an entry fee and wait for an opponent, or take an open seat. The winner takes the pool minus
                    5%.
                </p>
            </section>

            <div className="matches-grid">
                <div className="matches-left">
                    <section className="glass card create">
                        <h2>Create a CS2 1v1 match</h2>
                        <label className="field">
                            <span>Entry fee (test USDC)</span>
                            <input
                                inputMode="decimal"
                                value={entry}
                                onChange={(e) => setEntry(e.target.value)}
                                aria-invalid={amount == null}
                            />
                        </label>
                        <div className="quick" role="group" aria-label="Quick amounts">
                            {QUICK.map((q) => (
                                <button
                                    key={q}
                                    className={`btn btn-sm ${entry === q ? 'btn-selected' : 'btn-ghost'}`}
                                    onClick={() => setEntry(q)}
                                >
                                    {q} USDC
                                </button>
                            ))}
                        </div>
                        <dl className="create-summary">
                            <div>
                                <dt>Pool</dt>
                                <dd className="num">{formatUsdc((amount ?? 0n) * 2n)} USDC</dd>
                            </div>
                            <div>
                                <dt>Winner gets</dt>
                                <dd className="num">
                                    {formatUsdc((amount ?? 0n) * 2n - ((amount ?? 0n) * 2n * 500n) / 10_000n)} USDC
                                </dd>
                            </div>
                        </dl>
                        <button
                            className="btn btn-primary btn-block"
                            onClick={create}
                            disabled={!!problem || isRunning}
                        >
                            {isRunning && !joining ? 'Confirm in wallet…' : 'Create match'}
                        </button>
                        <p className="muted small create-note">
                            {problem ??
                                (amount != null && amount * 2n >= HOLD_THRESHOLD
                                    ? 'Pools this large are held for 24 h before payout.'
                                    : 'Paid out as soon as the server signs the result.')}
                        </p>
                    </section>
                    <GameModes />
                </div>

                <div className="matches-right">
                    <section className="glass card">
                        <h2>Open matches</h2>
                        {!data ? (
                            <MatchListSkeleton rows={2} />
                        ) : open.length === 0 ? (
                            <p className="muted">No open matches. Create one and invite a friend.</p>
                        ) : (
                            <ul className="match-list">
                                {open.map((m) => (
                                    <MatchCard
                                        key={m.address}
                                        m={m}
                                        me={address}
                                        now={data.now}
                                        action={
                                            address && m.playerA !== address ? (
                                                <button
                                                    className="btn btn-primary btn-sm"
                                                    disabled={isRunning || !w?.profile || balance < BigInt(m.entryFee)}
                                                    onClick={() => join(m.address)}
                                                >
                                                    {joining === m.address
                                                        ? 'Confirm in wallet…'
                                                        : `Join for ${formatUsdc(m.entryFee)}`}
                                                </button>
                                            ) : null
                                        }
                                    />
                                ))}
                            </ul>
                        )}
                    </section>

                    <section className="glass card">
                        <h2>Live and recent</h2>
                        {!data ? (
                            <MatchListSkeleton rows={4} />
                        ) : rest.length === 0 ? (
                            <p className="muted">Nothing yet.</p>
                        ) : (
                            <ul className="match-list">
                                {rest.map((m) => (
                                    <MatchCard key={m.address} m={m} me={address} now={data.now} />
                                ))}
                            </ul>
                        )}
                    </section>
                </div>
            </div>
        </>
    );
}
