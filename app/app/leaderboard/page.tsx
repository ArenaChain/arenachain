'use client';

import { explorerAddress, short } from '@/lib/format';
import { useLeaderboard, useWallet } from '@/lib/hooks';

export default function LeaderboardPage() {
    const { address: me } = useWallet();
    const { data, error } = useLeaderboard();
    const players = data?.players ?? [];

    return (
        <>
            <section className="page-head">
                <h1>Leaderboard</h1>
                <p className="muted">
                    Every player profile on devnet, by rating. Ratings use integer Elo and update when a match settles.
                </p>
            </section>
            <section className="glass card board-card">
                {error && <p className="muted">{error.message}</p>}
                {data && players.length === 0 && <p className="muted">No players yet. Be the first.</p>}
                {(!data || players.length > 0) && !error && (
                    <table className="board">
                        <thead>
                            <tr>
                                <th scope="col">Rank</th>
                                <th scope="col">Player</th>
                                <th scope="col" className="num">
                                    Rating
                                </th>
                                <th scope="col" className="num">
                                    W / L
                                </th>
                                <th scope="col" className="num">
                                    Reputation
                                </th>
                            </tr>
                        </thead>
                        <tbody>
                            {!data
                                ? Array.from({ length: 6 }, (_, i) => (
                                      <tr key={i} aria-hidden="true">
                                          <td colSpan={5}>
                                              <span className="skeleton" />
                                          </td>
                                      </tr>
                                  ))
                                : players.map((p, i) => (
                                      <tr
                                          key={p.authority}
                                          className={[i < 3 ? `top top-${i + 1}` : '', p.authority === me ? 'me' : '']
                                              .filter(Boolean)
                                              .join(' ')}
                                      >
                                          <td className="rank">
                                              <span className="rank-badge num">{i + 1}</span>
                                          </td>
                                          <td>
                                              <a href={explorerAddress(p.authority)} target="_blank" rel="noreferrer">
                                                  {short(p.authority)}
                                              </a>
                                              {p.authority === me && <em className="you">You</em>}
                                          </td>
                                          <td className="num rating-cell">{p.elo}</td>
                                          <td className="num">
                                              {p.wins} / {p.losses}
                                          </td>
                                          <td className="num">{p.reputation}</td>
                                      </tr>
                                  ))}
                        </tbody>
                    </table>
                )}
            </section>
        </>
    );
}
