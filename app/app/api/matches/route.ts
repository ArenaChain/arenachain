import { NextResponse } from 'next/server';

import { explainError } from '@/lib/errors';
import { allMatches, invalidate } from '@/lib/server/chain';

export const dynamic = 'force-dynamic';

export async function GET(req: Request) {
    if (new URL(req.url).searchParams.has('fresh')) invalidate('matches');
    try {
        return NextResponse.json({ matches: await allMatches(), now: Math.floor(Date.now() / 1000) });
    } catch (e) {
        return NextResponse.json({ error: explainError(e) }, { status: 502 });
    }
}
