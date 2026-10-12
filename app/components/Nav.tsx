'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { useEffect, useState } from 'react';

import { WalletButton } from './WalletButton';

const LINKS = [
    { href: '/', label: 'Play' },
    { href: '/matches', label: 'Matches' },
    { href: '/leaderboard', label: 'Leaderboard' },
];

const STRIP_KEY = 'arenachain:demo-strip-dismissed';

/** Slim strip above the header. Visible by default; dismissing it lasts for this browser tab. */
function DemoStrip() {
    const [hidden, setHidden] = useState(false);
    useEffect(() => {
        try {
            if (sessionStorage.getItem(STRIP_KEY) === '1') setHidden(true);
        } catch {
            /* storage unavailable: keep the strip visible */
        }
    }, []);
    if (hidden) return null;
    return (
        <div className="demo-strip" role="note">
            <span>Devnet demo · test USDC · no real money</span>
            <button
                className="demo-strip-close"
                aria-label="Dismiss"
                onClick={() => {
                    setHidden(true);
                    try {
                        sessionStorage.setItem(STRIP_KEY, '1');
                    } catch {
                        /* ignore */
                    }
                }}
            >
                ×
            </button>
        </div>
    );
}

export function Nav() {
    const path = usePathname();
    const [menuOpen, setMenuOpen] = useState(false);
    useEffect(() => setMenuOpen(false), [path]);

    const isActive = (href: string) => (href === '/' ? path === '/' : path.startsWith(href));

    return (
        <div className="topbar">
            <DemoStrip />
            <header className="header">
                <div className="header-inner">
                    <Link href="/" className="brand" aria-label="ArenaChain home">
                        <img src="/logo-mark.png" alt="" width={30} height={30} className="brand-mark" />
                        <span className="brand-word">ArenaChain</span>
                    </Link>
                    <nav className={`nav-links ${menuOpen ? 'is-open' : ''}`} aria-label="Main">
                        {LINKS.map((l) => (
                            <Link
                                key={l.href}
                                href={l.href}
                                className={isActive(l.href) ? 'nav-link active' : 'nav-link'}
                                aria-current={isActive(l.href) ? 'page' : undefined}
                            >
                                {l.label}
                            </Link>
                        ))}
                    </nav>
                    <div className="header-right">
                        <span className="net-pill">
                            <i aria-hidden="true" />
                            Devnet
                        </span>
                        <WalletButton />
                        <button
                            className="menu-toggle"
                            aria-label={menuOpen ? 'Close menu' : 'Open menu'}
                            aria-expanded={menuOpen}
                            onClick={() => setMenuOpen((o) => !o)}
                        >
                            <span />
                            <span />
                            <span />
                        </button>
                    </div>
                </div>
            </header>
        </div>
    );
}
