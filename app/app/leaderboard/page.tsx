'use client';

import { explorerAddress, short } from '@/lib/format';
import { useLeaderboard, useWallet } from '@/lib/hooks';

export default function LeaderboardPage() {
    const { address: me } = useWallet();
    const { data, error } = useLeaderboard();
    const players = data?.players ?? [];

    return (
        <>
            <section className="intro">
                <h1>Leaderboard</h1>
                <p>
                    Every player profile on devnet, by rating. Ratings use integer Elo and update when a match settles.
                </p>
            </section>
            <section className="panel">
                {error && <p className="muted">{error.message}</p>}
                {!data && !error && <p className="muted">Loading players…</p>}
                {data && players.length === 0 && <p className="muted">No players yet. Be the first.</p>}
                {players.length > 0 && (
                    <table className="board">
                        <thead>
                            <tr>
                                <th scope="col">Rank</th>
                                <th scope="col">Player</th>
                                <th scope="col" className="num">
                                    Rating
                                </th>
                                <th scope="col" className="num">
                                    Wins
                                </th>
                                <th scope="col" className="num">
                                    Losses
                                </th>
                                <th scope="col" className="num">
                                    Reputation
                                </th>
                                <th scope="col" className="num">
                                    Forfeits
                                </th>
                            </tr>
                        </thead>
                        <tbody>
                            {players.map((p, i) => (
                                <tr key={p.authority} className={p.authority === me ? 'me' : undefined}>
                                    <td className="rank">{i + 1}</td>
                                    <td>
                                        <a href={explorerAddress(p.authority)} target="_blank" rel="noreferrer">
                                            {short(p.authority)}
                                        </a>
                                        {p.authority === me && <em className="you">You</em>}
                                    </td>
                                    <td className="num rating-cell">{p.elo}</td>
                                    <td className="num">{p.wins}</td>
                                    <td className="num">{p.losses}</td>
                                    <td className="num">{p.reputation}</td>
                                    <td className="num">{p.totalForfeits}</td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                )}
            </section>
        </>
    );
}
