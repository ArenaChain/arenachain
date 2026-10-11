'use client';

import { createClient } from '@solana/kit';
import { solanaRpc } from '@solana/kit-plugin-rpc';
import { walletSigner } from '@solana/kit-plugin-wallet';
import { ClientProvider } from '@solana/react';
import type { ReactNode } from 'react';
import { SWRConfig } from 'swr';

import { ToastProvider } from '@/components/Toasts';
import { CHAIN, PUBLIC_RPC_URL } from '@/lib/deployment';

// One client for the app. The connected wallet (Wallet Standard) is the payer and signer.
// Version 0 transactions: every instruction here fits, and every current devnet wallet signs v0.
export const client = createClient()
    .use(walletSigner({ chain: CHAIN }))
    .use(solanaRpc({ rpcUrl: PUBLIC_RPC_URL, transactionConfig: { version: 0 } }));

export type AppClient = Awaited<typeof client>;

export async function fetchJson<T>(url: string): Promise<T> {
    const res = await fetch(url, { cache: 'no-store' });
    const body = await res.json();
    if (!res.ok) throw new Error(body.error ?? `Request failed (${res.status})`);
    return body as T;
}

export function Providers({ children }: { children: ReactNode }) {
    return (
        <ClientProvider client={client}>
            <SWRConfig value={{ fetcher: fetchJson, revalidateOnFocus: false }}>
                <ToastProvider>{children}</ToastProvider>
            </SWRConfig>
        </ClientProvider>
    );
}
