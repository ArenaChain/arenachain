import { NextResponse } from 'next/server';

import { GAME_SERVER_KEY } from '@/lib/deployment';
import { explainError } from '@/lib/errors';
import { getMatch, getProfile, settlementOf, toAddress } from '@/lib/server/chain';
import { matchHistory } from '@/lib/server/history';
import type { MatchDetail } from '@/lib/types';

export const dynamic = 'force-dynamic';

export async function GET(_req: Request, { params }: { params: Promise<{ address: string }> }) {
    const arenaMatch = toAddress((await params).address);
    if (!arenaMatch) return NextResponse.json({ error: 'Invalid match address.' }, { status: 400 });
    try {
        const match = await getMatch(arenaMatch);
        if (!match) return NextResponse.json({ error: 'Match not found.' }, { status: 404 });
        const players = [match.playerA, match.playerB].filter(Boolean) as string[];
        const profiles = Object.fromEntries(
            await Promise.all(players.map(async (p) => [p, await getProfile(toAddress(p)!)] as const)),
        );
        const detail: MatchDetail = {
            match,
            profiles,
            settlement: settlementOf(match),
            txs: await matchHistory(arenaMatch),
            serverKey: GAME_SERVER_KEY,
            now: Math.floor(Date.now() / 1000),
        };
        return NextResponse.json(detail);
    } catch (e) {
        return NextResponse.json({ error: explainError(e) }, { status: 502 });
    }
}
