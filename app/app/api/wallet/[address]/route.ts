import { NextResponse } from 'next/server';

import { explainError } from '@/lib/errors';
import { getWallet, invalidate, toAddress } from '@/lib/server/chain';

export const dynamic = 'force-dynamic';

export async function GET(req: Request, { params }: { params: Promise<{ address: string }> }) {
    const wallet = toAddress((await params).address);
    if (!wallet) return NextResponse.json({ error: 'Invalid wallet address.' }, { status: 400 });
    if (new URL(req.url).searchParams.has('fresh')) {
        invalidate(`wallet:${wallet}`);
        invalidate(`profile:${wallet}`);
    }
    try {
        return NextResponse.json(await getWallet(wallet));
    } catch (e) {
        return NextResponse.json({ error: explainError(e) }, { status: 502 });
    }
}
