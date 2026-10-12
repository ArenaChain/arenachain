import type { Metadata } from 'next';
import { Barlow_Condensed, Poppins } from 'next/font/google';
import type { ReactNode } from 'react';

import { Background } from '@/components/Background';
import { Nav } from '@/components/Nav';
import './globals.css';
import { Providers } from './providers';

// Brand book: Poppins Bold for display, Medium for headings, Regular for body. Barlow Condensed is
// kept only for the scoreboard numbers, because Poppins has no tabular figures.
const poppins = Poppins({ subsets: ['latin'], weight: ['400', '500', '700'], variable: '--font-poppins' });
const score = Barlow_Condensed({ subsets: ['latin'], weight: ['700'], variable: '--font-score' });

// Icons come from Next's file conventions in this folder: favicon.ico, icon1.png (32px),
// icon2.png (192px) and apple-icon.png (180px).

export const viewport = { themeColor: '#040813' };

export const metadata: Metadata = {
    title: 'ArenaChain',
    description: 'CS2 1v1s with escrowed test-USDC prize pools on Solana devnet.',
};

export default function RootLayout({ children }: { children: ReactNode }) {
    return (
        <html lang="en" className={`${poppins.variable} ${score.variable}`}>
            <body>
                <Background />
                <Providers>
                    <Nav />
                    <main className="page">{children}</main>
                </Providers>
            </body>
        </html>
    );
}
