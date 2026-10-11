// Instructions a player signs with their own wallet. Used by the UI (wallet signer) and by the
// devnet verification script (keypair signer), so both exercise exactly the same transactions.

import {
    findArenaMatchPda,
    getPlayerCancelMatchInstructionAsync,
    getPlayerCreateMatchInstructionAsync,
    getPlayerCreateProfileInstructionAsync,
    getPlayerJoinMatchInstructionAsync,
    getRefundExpiredInstructionAsync,
} from '@arenachain/client';
import { findAssociatedTokenPda, TOKEN_PROGRAM_ADDRESS } from '@solana-program/token';
import type { Address, TransactionSigner } from '@solana/kit';

import { GAME_SERVER, USDC_MINT } from './deployment';

export async function usdcAccountOf(owner: Address): Promise<Address> {
    const [ata] = await findAssociatedTokenPda({ owner, mint: USDC_MINT, tokenProgram: TOKEN_PROGRAM_ADDRESS });
    return ata;
}

export function createProfileIx(player: TransactionSigner) {
    return getPlayerCreateProfileInstructionAsync({ authority: player });
}

/** Returns the instruction and the new match's address. */
export async function createMatchIx(player: TransactionSigner, entryFee: bigint) {
    const nonce = BigInt(Date.now());
    const [arenaMatch] = await findArenaMatchPda({ creator: player.address, nonce });
    const ix = await getPlayerCreateMatchInstructionAsync({
        creator: player,
        gameServer: GAME_SERVER,
        mint: USDC_MINT,
        creatorToken: await usdcAccountOf(player.address),
        nonce,
        entryFee,
    });
    return { ix, arenaMatch };
}

export async function joinMatchIx(player: TransactionSigner, arenaMatch: Address) {
    return getPlayerJoinMatchInstructionAsync({
        joiner: player,
        arenaMatch,
        gameServer: GAME_SERVER,
        mint: USDC_MINT,
        joinerToken: await usdcAccountOf(player.address),
    });
}

export async function cancelMatchIx(player: TransactionSigner, arenaMatch: Address) {
    return getPlayerCancelMatchInstructionAsync({
        playerA: player,
        arenaMatch,
        mint: USDC_MINT,
        playerAToken: await usdcAccountOf(player.address),
    });
}

/** Anyone can refund both players once a match's deadline passed with no result. */
export async function refundExpiredIx(arenaMatch: Address, playerA: Address, playerB: Address) {
    return getRefundExpiredInstructionAsync({
        arenaMatch,
        mint: USDC_MINT,
        playerAToken: await usdcAccountOf(playerA),
        playerBToken: await usdcAccountOf(playerB),
        playerA,
    });
}
