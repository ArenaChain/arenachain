'use client';

import Link from 'next/link';
import { type ReactNode, useState } from 'react';
import { useSWRConfig } from 'swr';

import { useToast } from '@/components/Toasts';
import { explorerAddress, explorerTx, formatUsdc, isExpired, short } from '@/lib/format';
import type { MatchDetail, MatchStatusName, MatchView, ProfileView } from '@/lib/types';

// ---------- status ----------

const STEPS = ['Open', 'Full', 'Started', 'Settled'] as const;
const STEP_INDEX: Record<MatchStatusName, number> = {
    Open: 0,
    Full: 1,
    InProgress: 2,
    Settled: 3,
    Refunded: 3,
    Disputed: 3,
};
export const statusLabel = (s: MatchStatusName) => (s === 'InProgress' ? 'Started' : s);

export function StatusChip({ status, expired = false }: { status: MatchStatusName; expired?: boolean }) {
    if (expired) return <span className="chip chip-expired">Expired, refund due</span>;
    return <span className={`chip chip-${status.toLowerCase()}`}>{statusLabel(status)}</span>;
}

/** Open → Full → Started → Settled, with the last step reading "Refunded" when money went back. */
export function StatusTrack({ status }: { status: MatchStatusName }) {
    const at = STEP_INDEX[status];
    return (
        <ol className="track" aria-label={`Match status: ${statusLabel(status)}`}>
            {STEPS.map((step, i) => {
                const name = i === 3 && status === 'Refunded' ? 'Refunded' : step;
                const state = i < at ? 'done' : i === at ? 'current' : 'todo';
                return (
                    <li
                        key={step}
                        className={`track-step ${state} ${i === 3 && status === 'Refunded' ? 'refunded' : ''}`}
                    >
                        <span className="track-bar" />
                        <span className="track-name">{name}</span>
                    </li>
                );
            })}
        </ol>
    );
}

// ---------- scoreboard ----------

function Side({
    side,
    wallet,
    profile,
    you,
    winner,
    payout,
}: {
    side: 'a' | 'b';
    wallet: string | null;
    profile: ProfileView | null | undefined;
    you: boolean;
    winner: boolean;
    payout: string | null;
}) {
    return (
        <div className={`side side-${side} ${winner ? 'is-winner' : ''}`}>
            <span className="side-role">{side === 'a' ? 'Player A' : 'Player B'}</span>
            {wallet ? (
                <>
                    <a
                        className="side-addr"
                        href={explorerAddress(wallet)}
                        target="_blank"
                        rel="noreferrer"
                        title={wallet}
                    >
                        {short(wallet)}
                    </a>
                    <span className="side-meta">
                        {profile ? `${profile.elo} rating` : 'No profile'}
                        {you && <em className="you">You</em>}
                    </span>
                    {winner && <span className="side-win">Winner{payout ? ` +${formatUsdc(payout)} USDC` : ''}</span>}
                </>
            ) : (
                <span className="side-waiting">Waiting for an opponent</span>
            )}
        </div>
    );
}

export function Scoreboard({ detail, me }: { detail: MatchDetail; me: string | null }) {
    const { match: m, profiles, settlement } = detail;
    const pool = BigInt(m.entryFee) * 2n;
    const winnerPayout = settlement && settlement.refundEach == null ? settlement.payout : null;
    return (
        <section className="scoreboard">
            <Side
                side="a"
                wallet={m.playerA}
                profile={profiles[m.playerA]}
                you={me === m.playerA}
                winner={m.winner === m.playerA}
                payout={winnerPayout}
            />
            <div className="pot">
                <span className="pot-amount">{formatUsdc(pool)}</span>
                <span className="pot-label">USDC pool</span>
                <span className="pot-entry">{formatUsdc(m.entryFee)} entry each</span>
            </div>
            <Side
                side="b"
                wallet={m.playerB}
                profile={m.playerB ? profiles[m.playerB] : null}
                you={me != null && me === m.playerB}
                winner={m.winner != null && m.winner === m.playerB}
                payout={winnerPayout}
            />
        </section>
    );
}

// ---------- settlement ----------

export function SettlementPanel({ detail }: { detail: MatchDetail }) {
    const { match: m, settlement: s, txs } = detail;
    if (!s) return null;
    const payoutTx = [...txs]
        .reverse()
        .find((t) => t.label === 'Server-signed result' || t.label === 'Held payout claimed');
    const winnerSide = m.winner === m.playerA ? 'A' : 'B';
    return (
        <section className="panel settlement">
            <h2>{m.resultKind === 'Void' ? 'Refunded' : `Player ${winnerSide} won`}</h2>
            <p className="muted">
                {m.resultKind === 'Win' && 'Server-signed result. The winner takes the pool minus the 5% fee.'}
                {m.resultKind === 'Forfeit' &&
                    `Player ${winnerSide === 'A' ? 'B' : 'A'} left the match. The opponent takes the whole pool; the fee is waived.`}
                {m.resultKind === 'Void' &&
                    'The server reported a failure it could not recover from. Both entries went back.'}
            </p>
            <dl className="money">
                <div>
                    <dt>Pool</dt>
                    <dd>{formatUsdc(s.pool)} USDC</dd>
                </div>
                {s.refundEach ? (
                    <div>
                        <dt>Refunded to each player</dt>
                        <dd className="paid">{formatUsdc(s.refundEach)} USDC</dd>
                    </div>
                ) : (
                    <>
                        <div>
                            <dt>{m.resultKind === 'Forfeit' ? 'Fee (waived on forfeit)' : 'Fee (5%)'}</dt>
                            <dd>{formatUsdc(s.fee)} USDC</dd>
                        </div>
                        <div>
                            <dt>Payout to player {winnerSide}</dt>
                            <dd className="paid">{formatUsdc(s.payout)} USDC</dd>
                        </div>
                    </>
                )}
            </dl>
            {s.held && (
                <p className="note">
                    Large pool: the payout is held until {new Date(m.unlockAt * 1000).toLocaleString()} in case a VAC
                    ban arrives late.
                </p>
            )}
            {payoutTx && (
                <a className="btn btn-outline" href={explorerTx(payoutTx.signature)} target="_blank" rel="noreferrer">
                    View payout on Explorer
                </a>
            )}
        </section>
    );
}

// ---------- simulated server ----------

type Action = 'start' | 'win-a' | 'win-b' | 'forfeit-a' | 'forfeit-b' | 'void';

export function ServerPanel({ detail }: { detail: MatchDetail }) {
    const toast = useToast();
    const { mutate } = useSWRConfig();
    const [busy, setBusy] = useState<Action | null>(null);
    const s = detail.match.status;

    async function run(action: Action, done: string) {
        setBusy(action);
        try {
            const res = await fetch('/api/server', {
                method: 'POST',
                headers: { 'content-type': 'application/json' },
                body: JSON.stringify({ match: detail.match.address, action }),
            });
            const body = await res.json();
            if (!res.ok) throw new Error(body.error ?? 'The server refused.');
            toast({ tone: 'ok', title: done, signature: body.signature });
            await Promise.all([mutate(`/api/match/${detail.match.address}`), mutate('/api/matches')]);
        } catch (e) {
            toast({ tone: 'error', title: (e as Error).message });
        } finally {
            setBusy(null);
        }
    }

    const live = s === 'InProgress';
    const finished = s === 'Settled' || s === 'Refunded' || s === 'Disputed';
    const expired = isExpired(detail.match, detail.now);
    const btn = (action: Action, label: string, done: string, enabled: boolean, cls = 'btn-server') => (
        <button className={`btn ${cls}`} disabled={!enabled || busy != null} onClick={() => run(action, done)}>
            {busy === action ? 'Signing…' : label}
        </button>
    );

    return (
        <section className="panel server" aria-labelledby="server-title">
            <h2 id="server-title">Simulated CS2 server</h2>
            <p className="muted">
                Stands in for the MatchZy result service on a real CS2 server. It holds the game-server key on the
                backend and signs each result with Ed25519; the program pays out only if that signature checks out.
            </p>
            <p className="server-key">
                Server key{' '}
                <a href={explorerAddress(detail.serverKey)} target="_blank" rel="noreferrer">
                    {short(detail.serverKey)}
                </a>
            </p>
            {!finished && !expired && (
                <div className="server-actions">
                    {btn('start', 'Start match', 'Match started', s === 'Full', 'btn-primary')}
                    {btn('win-a', 'Player A wins', 'Result signed: A wins', live, 'btn-server btn-a')}
                    {btn('win-b', 'Player B wins', 'Result signed: B wins', live, 'btn-server btn-b')}
                    {btn('forfeit-a', 'A forfeits (left)', 'Forfeit signed: A left', live)}
                    {btn('forfeit-b', 'B forfeits (left)', 'Forfeit signed: B left', live)}
                    {btn('void', 'Void (server crash)', 'Void signed: both refunded', live)}
                </div>
            )}
            {s === 'Open' && <p className="hint">Waiting for player B to join before the server can start.</p>}
            {finished && <p className="hint">This match is finished. Nothing left to sign.</p>}
            {expired && (
                <p className="hint">
                    The deadline passed, so the server can no longer act. Anyone can refund both players.
                </p>
            )}
        </section>
    );
}

// ---------- lists ----------

export function MatchRow({ m, me, now, action }: { m: MatchView; me: string | null; now: number; action?: ReactNode }) {
    const mine = me != null && (me === m.playerA || me === m.playerB);
    return (
        <li className="match-row">
            <Link href={`/match/${m.address}`} className="match-row-main">
                <span className="match-entry">{formatUsdc(m.entryFee)} USDC</span>
                <span className="match-players">
                    <span className="pa">{short(m.playerA)}</span>
                    <span className="vs">vs</span>
                    <span className="pb">{m.playerB ? short(m.playerB) : 'open slot'}</span>
                </span>
                {mine && <em className="you">You</em>}
                <StatusChip status={m.status} expired={isExpired(m, now)} />
            </Link>
            {action}
        </li>
    );
}
