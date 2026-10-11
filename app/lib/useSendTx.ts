'use client';

import { isAbortError } from '@solana/kit';
import { useAction } from '@solana/react';

import type { AppClient } from '@/app/providers';
import { useToast } from '@/components/Toasts';
import { explainError } from './errors';
import { useWallet } from './hooks';

type Plan = Parameters<AppClient['sendTransaction']>[0];

/** Sends with the connected wallet; every result shows a toast with its Explorer link. */
export function useSendTx() {
    const { client } = useWallet();
    const toast = useToast();
    const { dispatchAsync, isRunning } = useAction(async (signal: AbortSignal, plan: Plan) => {
        const result = await client.sendTransaction(plan, { abortSignal: signal });
        return result.context.signature as string;
    });
    async function send(doneLabel: string, plan: Plan): Promise<string | null> {
        try {
            const signature = await dispatchAsync(plan);
            toast({ tone: 'ok', title: doneLabel, signature });
            return signature;
        } catch (e) {
            if (!isAbortError(e)) toast({ tone: 'error', title: explainError(e) });
            return null;
        }
    }
    return { send, isRunning };
}
