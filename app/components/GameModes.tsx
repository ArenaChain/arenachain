// Purely visual: CS2 1v1 is the only live mode. The others are announced, not playable.

const SOON = [
    { name: 'CS2 2v2 Wingman', line: 'Two-player teams on Wingman maps, with the pool split between the winners.' },
    { name: 'CS2 5v5', line: 'Full-team matches, with every player’s entry in one escrowed pool.' },
    { name: 'Tournaments', line: 'Bracket play with a prize pool paid out by final placement.' },
    { name: 'Dota 2', line: '1v1 mid matches settled by a signed result from the game server.' },
];

function Lock() {
    return (
        <svg className="mode-lock" width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
            <rect x="3" y="7" width="10" height="7" rx="1.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
            <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" fill="none" stroke="currentColor" strokeWidth="1.5" />
        </svg>
    );
}

export function GameModes() {
    return (
        <section aria-labelledby="modes-title">
            <h2 id="modes-title" className="section-title">
                Game modes
            </h2>
            <ul className="modes">
                <li className="mode mode-live">
                    <span className="mode-tag">Live</span>
                    <strong className="mode-name">CS2 1v1</strong>
                    <span className="mode-line">Head-to-head on a CS2 server. Playable now.</span>
                </li>
                {SOON.map((m) => (
                    <li key={m.name} className="mode mode-soon">
                        <span className="mode-tag">
                            <Lock />
                            Coming soon
                        </span>
                        <strong className="mode-name">{m.name}</strong>
                        <span className="mode-line">{m.line}</span>
                        <span className="visually-hidden">Not available yet.</span>
                    </li>
                ))}
            </ul>
        </section>
    );
}
