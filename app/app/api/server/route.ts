import { NextResponse } from 'next/server';

import { explainError } from '@/lib/errors';
import { toAddress } from '@/lib/server/chain';
import { runServerAction, SERVER_ACTIONS, type ServerAction, ServerRefusal } from '@/lib/server/gameServer';

export const dynamic = 'force-dynamic';

/** Simulated CS2 server: { match, action } -> transaction signature. */
export async function POST(req: Request) {
    const body = await req.json().catch(() => ({}));
    const arenaMatch = toAddress(String(body.match ?? ''));
    const action = String(body.action ?? '') as ServerAction;
    if (!arenaMatch || !SERVER_ACTIONS.includes(action)) {
        return NextResponse.json({ error: 'Expected { match, action }.' }, { status: 400 });
    }
    try {
        const signature = await runServerAction(arenaMatch, action);
        return NextResponse.json({ signature });
    } catch (e) {
        if (e instanceof ServerRefusal) return NextResponse.json({ error: e.message }, { status: 409 });
        console.error('game server', e);
        return NextResponse.json({ error: explainError(e) }, { status: 500 });
    }
}
