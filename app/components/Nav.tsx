'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';

import { WalletButton } from './WalletButton';

const LINKS = [
    { href: '/', label: 'Play' },
    { href: '/matches', label: 'Matches' },
    { href: '/leaderboard', label: 'Leaderboard' },
];

export function Nav() {
    const path = usePathname();
    return (
        <header className="nav">
            <div className="demo-banner" role="note">
                Devnet demo · test USDC · no real money
            </div>
            <div className="nav-inner">
                <Link href="/" className="brand">
                    <span className="brand-mark" aria-hidden>
                        <i className="t" />
                        <i className="ct" />
                    </span>
                    ArenaChain
                </Link>
                <nav className="nav-links">
                    {LINKS.map((l) => {
                        const active = l.href === '/' ? path === '/' : path.startsWith(l.href);
                        return (
                            <Link key={l.href} href={l.href} className={active ? 'active' : undefined}>
                                {l.label}
                            </Link>
                        );
                    })}
                </nav>
                <span className="net-pill">Devnet</span>
                <WalletButton />
            </div>
        </header>
    );
}
