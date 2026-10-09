// Server-signed result helpers shared by the result service, the frontend and tests.
// The byte layout must match `build_result_message` in programs/arenachain/src/ed25519.rs.

import {
    type Address,
    getAddressEncoder,
    getU64Encoder,
    type Instruction,
    address,
} from '@solana/kit';

import { ResultKind } from './generated';

export const ED25519_PROGRAM_ADDRESS = address('Ed25519SigVerify111111111111111111111111111');
export const RESULT_DOMAIN = new TextEncoder().encode('ARENACHAIN_RESULT_V1');
export const RESULT_MESSAGE_LENGTH = 20 + 32 + 32 + 8 + 1 + 32 + 32;
const DEFAULT_ADDRESS = address('11111111111111111111111111111111');

export type ResultMessageInput = {
    programAddress: Address;
    arenaMatch: Address;
    /** `Match.createdSlot`, read from the match account. */
    createdSlot: bigint;
    kind: ResultKind;
    /** Omit (or pass the default address) for `Void`. */
    winner?: Address;
    /** 32-byte hash of the demo file. */
    demoHash: Uint8Array;
};

/** The exact bytes the game server signs for a result. */
export function getResultMessage(input: ResultMessageInput): Uint8Array {
    if (input.demoHash.length !== 32) throw new Error('demoHash must be 32 bytes');
    const addr = getAddressEncoder();
    const parts = [
        RESULT_DOMAIN,
        addr.encode(input.programAddress),
        addr.encode(input.arenaMatch),
        getU64Encoder().encode(input.createdSlot),
        Uint8Array.of(input.kind),
        addr.encode(input.winner ?? DEFAULT_ADDRESS),
        input.demoHash,
    ];
    const out = new Uint8Array(RESULT_MESSAGE_LENGTH);
    let at = 0;
    for (const p of parts) {
        out.set(p, at);
        at += p.length;
    }
    return out;
}

/**
 * Ed25519 precompile instruction carrying one signature, with the pubkey, signature and
 * message inside its own data (every instruction-index field is u16::MAX). It must be placed
 * immediately before `submit_result` in the same transaction.
 */
export function getEd25519VerifyInstruction(
    signer: Address,
    signature: Uint8Array,
    message: Uint8Array,
): Instruction {
    if (signature.length !== 64) throw new Error('signature must be 64 bytes');
    const headerLength = 2 + 14;
    const pubkeyOffset = headerLength;
    const signatureOffset = pubkeyOffset + 32;
    const messageOffset = signatureOffset + 64;
    const data = new Uint8Array(messageOffset + message.length);
    const view = new DataView(data.buffer);
    data[0] = 1; // one signature
    const fields = [signatureOffset, 0xffff, pubkeyOffset, 0xffff, messageOffset, message.length, 0xffff];
    fields.forEach((v, i) => view.setUint16(2 + i * 2, v, true));
    data.set(getAddressEncoder().encode(signer), pubkeyOffset);
    data.set(signature, signatureOffset);
    data.set(message, messageOffset);
    return { programAddress: ED25519_PROGRAM_ADDRESS, accounts: [], data };
}
