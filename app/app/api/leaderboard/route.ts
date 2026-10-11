import { NextResponse } from 'next/server';

import { explainError } from '@/lib/errors';
import { allProfiles } from '@/lib/server/chain';

export const dynamic = 'force-dynamic';

/** All player profiles, highest rating first (ties: more wins, then fewer losses). */
export async function GET() {
    try {
        const players = (await allProfiles()).sort((a, b) => b.elo - a.elo || b.wins - a.wins || a.losses - b.losses);
        return NextResponse.json({ players });
    } catch (e) {
        return NextResponse.json({ error: explainError(e) }, { status: 502 });
    }
}
