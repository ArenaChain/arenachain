'use client';

import { address as toAddress } from '@solana/kit';
import { useRouter } from 'next/navigation';
import { useState } from 'react';

import { MatchRow } from '@/components/match';
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
            <section className="intro">
                <h1>Matches</h1>
                <p>
                    Put up an entry fee and wait for an opponent, or take an open seat. The winner takes the pool minus
                    5%.
                </p>
            </section>

            <section className="panel create">
                <h2>Create a match</h2>
                <div className="create-row">
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
                                {q}
                            </button>
                        ))}
                    </div>
                    <button className="btn btn-primary" onClick={create} disabled={!!problem || isRunning}>
                        {isRunning && !joining ? 'Confirm in wallet…' : 'Create match'}
                    </button>
                </div>
                <p className="muted small">
                    {problem ??
                        `Pool ${formatUsdc((amount ?? 0n) * 2n)} USDC. ${
                            amount != null && amount * 2n >= HOLD_THRESHOLD
                                ? 'Pools this large are held for 24 h before payout.'
                                : 'Paid out as soon as the server signs the result.'
                        }`}
                </p>
            </section>

            <section className="panel">
                <h2>Open matches</h2>
                {open.length === 0 ? (
                    <p className="muted">No open matches. Create one above and invite a friend.</p>
                ) : (
                    <ul className="match-list">
                        {open.map((m) => (
                            <MatchRow
                                key={m.address}
                                m={m}
                                me={address}
                                now={data?.now ?? 0}
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

            <section className="panel">
                <h2>Live and recent</h2>
                {rest.length === 0 ? (
                    <p className="muted">Nothing yet.</p>
                ) : (
                    <ul className="match-list">
                        {rest.map((m) => (
                            <MatchRow key={m.address} m={m} me={address} now={data?.now ?? 0} />
                        ))}
                    </ul>
                )}
            </section>
        </>
    );
}
