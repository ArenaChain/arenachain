import type { Metadata } from 'next';
import { Barlow, Barlow_Condensed } from 'next/font/google';
import type { ReactNode } from 'react';

import { Nav } from '@/components/Nav';
import './globals.css';
import { Providers } from './providers';

const body = Barlow({ subsets: ['latin'], weight: ['400', '500', '600'], variable: '--font-body' });
const display = Barlow_Condensed({ subsets: ['latin'], weight: ['500', '600', '700'], variable: '--font-display' });

export const metadata: Metadata = {
    title: 'ArenaChain',
    description: 'CS2 1v1s with escrowed test-USDC prize pools on Solana devnet.',
};

export default function RootLayout({ children }: { children: ReactNode }) {
    return (
        <html lang="en" className={`${body.variable} ${display.variable}`}>
            <body>
                <Providers>
                    <Nav />
                    <main className="page">{children}</main>
                </Providers>
            </body>
        </html>
    );
}
