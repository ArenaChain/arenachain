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

/** Calls the app's API. Errors always come back as one readable sentence. */
export async function apiJson<T>(
    url: string,
    body?: unknown,
): Promise<{ status: number; data: T & { error?: string } }> {
    let res: Response;
    try {
        res = await fetch(url, {
            cache: 'no-store',
            ...(body === undefined
                ? {}
                : { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) }),
        });
    } catch {
        throw new Error("Can't reach the ArenaChain server. Check that it's running and try again.");
    }
    try {
        return { status: res.status, data: await res.json() };
    } catch {
        throw new Error(`The ArenaChain server had a problem (HTTP ${res.status}). Try again in a moment.`);
    }
}

export async function fetchJson<T>(url: string): Promise<T> {
    const { status, data } = await apiJson<T>(url);
    if (status >= 400) throw new Error(data.error ?? `The ArenaChain server had a problem (HTTP ${status}).`);
    return data;
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
