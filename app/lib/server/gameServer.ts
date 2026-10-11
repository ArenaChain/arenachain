import 'server-only';

// Simulated CS2 server: stands in for the MatchZy result service. It holds the game-server key
// (server-side only) and signs results exactly like the real service would, via result.ts.

import {
    ARENACHAIN_PROGRAM_ADDRESS,
    fetchMatch,
    findProfilePda,
    getEd25519VerifyInstruction,
    getResultMessage,
    getServerStartMatchInstructionAsync,
    getSubmitResultInstructionAsync,
    ResultKind,
} from '@arenachain/client';
import { type Address, nonDivisibleSequentialInstructionPlan, signBytes } from '@solana/kit';

import { usdcAccountOf } from '../actions';
import { FEE_VAULT, GAME_SERVER, USDC_MINT } from '../deployment';
import { getMatch, invalidate, retry, rpc, treasuryClient, gameServerKey } from './chain';

export type ServerAction = 'start' | 'win-a' | 'win-b' | 'forfeit-a' | 'forfeit-b' | 'void';
export const SERVER_ACTIONS: ServerAction[] = ['start', 'win-a', 'win-b', 'forfeit-a', 'forfeit-b', 'void'];

export class ServerRefusal extends Error {}

// A click that's already being processed for a match returns the same promise (idempotent).
const inFlight = new Map<string, Promise<string>>();

export function runServerAction(arenaMatch: Address, action: ServerAction): Promise<string> {
    const key = `${arenaMatch}:${action === 'start' ? 'start' : 'result'}`;
    const existing = inFlight.get(key);
    if (existing) return existing;
    const p = act(arenaMatch, action).finally(() => inFlight.delete(key));
    inFlight.set(key, p);
    return p;
}

async function act(arenaMatch: Address, action: ServerAction): Promise<string> {
    invalidate(`match:${arenaMatch}`);
    const m = await getMatch(arenaMatch);
    if (!m) throw new ServerRefusal('No such match.');
    // The real result service only acts on matches assigned to it.
    if (m.gameServer !== GAME_SERVER) throw new ServerRefusal('This match is assigned to a different game server.');

    const { client } = await treasuryClient();
    const server = await gameServerKey();

    if (action === 'start') {
        if (m.status !== 'Full')
            throw new ServerRefusal(`Can only start a full match (this one is ${label(m.status)}).`);
        const ix = await getServerStartMatchInstructionAsync({ serverSigner: server, arenaMatch });
        const sig = (await client.sendTransaction(ix)).context.signature;
        invalidate(`match:${arenaMatch}`);
        return sig;
    }

    // Results: only for matches that are live on-chain.
    if (m.status !== 'InProgress')
        throw new ServerRefusal(`Results need a started match (this one is ${label(m.status)}).`);
    const playerA = m.playerA as Address;
    const playerB = m.playerB as Address;
    const { kind, winner } = {
        'win-a': { kind: ResultKind.Win, winner: playerA },
        'win-b': { kind: ResultKind.Win, winner: playerB },
        'forfeit-a': { kind: ResultKind.Forfeit, winner: playerB }, // A left: B takes the pool
        'forfeit-b': { kind: ResultKind.Forfeit, winner: playerA },
        void: { kind: ResultKind.Void, winner: undefined },
    }[action];

    const onChain = await retry(() => fetchMatch(rpc, arenaMatch));
    const demoHash = new Uint8Array(
        await crypto.subtle.digest('SHA-256', new TextEncoder().encode(`demo:${arenaMatch}`)),
    );
    const message = getResultMessage({
        programAddress: ARENACHAIN_PROGRAM_ADDRESS,
        arenaMatch,
        createdSlot: onChain.data.createdSlot,
        kind,
        winner,
        demoHash,
    });
    const signature = await signBytes(server.keyPair.privateKey, message);
    const submit = await getSubmitResultInstructionAsync({
        arenaMatch,
        gameServer: GAME_SERVER,
        mint: USDC_MINT,
        playerAToken: await usdcAccountOf(playerA),
        playerBToken: await usdcAccountOf(playerB),
        playerAProfile: (await findProfilePda({ authority: playerA }))[0],
        playerBProfile: (await findProfilePda({ authority: playerB }))[0],
        feeVault: FEE_VAULT,
        playerA,
        kind,
        winner: winner ?? ('11111111111111111111111111111111' as Address),
        demoHash,
    });
    const sig = (
        await client.sendTransaction(
            nonDivisibleSequentialInstructionPlan([
                getEd25519VerifyInstruction(server.address, signature, message),
                submit,
            ]),
        )
    ).context.signature;
    invalidate(`match:${arenaMatch}`);
    invalidate('matches');
    invalidate('profiles');
    invalidate(`profile:${playerA}`);
    invalidate(`profile:${playerB}`);
    invalidate(`wallet:${playerA}`);
    invalidate(`wallet:${playerB}`);
    return sig;
}

const label = (s: string) => (s === 'InProgress' ? 'started' : s.toLowerCase());
