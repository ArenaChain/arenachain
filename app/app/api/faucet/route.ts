import { NextResponse } from 'next/server';

import { explainError } from '@/lib/errors';
import { toAddress } from '@/lib/server/chain';
import { claim, nextClaimAt } from '@/lib/server/faucet';

export const dynamic = 'force-dynamic';

/** When this wallet can claim again (null = now). */
export async function GET(req: Request) {
    const wallet = toAddress(new URL(req.url).searchParams.get('wallet') ?? '');
    if (!wallet) return NextResponse.json({ error: 'Invalid wallet address.' }, { status: 400 });
    return NextResponse.json({ retryAt: nextClaimAt(wallet) });
}

export async function POST(req: Request) {
    const body = await req.json().catch(() => ({}));
    const wallet = toAddress(String(body.wallet ?? ''));
    if (!wallet) return NextResponse.json({ error: 'Invalid wallet address.' }, { status: 400 });
    try {
        const result = await claim(wallet);
        if (result.ok) return NextResponse.json(result);
        if ('retryAt' in result) return NextResponse.json(result, { status: 429 });
        return NextResponse.json(result, { status: 503 });
    } catch (e) {
        console.error('faucet', e);
        return NextResponse.json({ ok: false, error: explainError(e) }, { status: 500 });
    }
}
