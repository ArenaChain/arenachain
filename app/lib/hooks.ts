'use client';

import { useConnectedWallet } from '@solana/kit-plugin-wallet/react';
import { useClient } from '@solana/react';
import useSWR from 'swr';

import type { AppClient } from '@/app/providers';
import type { MatchDetail, MatchView, ProfileView, WalletView } from './types';

export function useWallet() {
    const client = useClient<AppClient>();
    const connected = useConnectedWallet(client);
    return { client, connected, address: connected?.account.address ?? null };
}

export function useWalletView(address: string | null) {
    return useSWR<WalletView>(address ? `/api/wallet/${address}` : null, { refreshInterval: 8000 });
}

export function useMatches() {
    return useSWR<{ matches: MatchView[]; now: number }>('/api/matches', { refreshInterval: 5000 });
}

export function useMatchDetail(address: string) {
    return useSWR<MatchDetail>(`/api/match/${address}`, { refreshInterval: 3000 });
}

export function useLeaderboard() {
    return useSWR<{ players: ProfileView[] }>('/api/leaderboard', { refreshInterval: 10000 });
}
