//! Server-signed result verification via the Ed25519 precompile.
//!
//! The transaction must contain an Ed25519 program instruction immediately before
//! `submit_result`. That instruction carries exactly one signature whose pubkey,
//! signature and message all live inside its own data (every instruction-index field is
//! `u16::MAX`). We read it through the address-checked instructions sysvar and compare the
//! pubkey and message byte for byte with what the program expects.

use anchor_lang::prelude::*;
use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};
use solana_sdk_ids::ed25519_program;

use crate::{constants::*, errors::ArenaError, state::ResultKind};

const HEADER_LEN: usize = 2; // num_signatures (u8) + padding (u8)
const OFFSETS_LEN: usize = 14; // 7 x u16
const PUBKEY_LEN: usize = 32;
const SIGNATURE_LEN: usize = 64;
const THIS_INSTRUCTION: u16 = u16::MAX;

/// Canonical message the game server signs for a result.
pub fn build_result_message(
    program_id: &Pubkey,
    match_key: &Pubkey,
    created_slot: u64,
    kind: ResultKind,
    winner: &Pubkey,
    demo_hash: &[u8; 32],
) -> [u8; RESULT_MESSAGE_LEN] {
    let mut msg = [0u8; RESULT_MESSAGE_LEN];
    let mut at = 0;
    for part in [
        RESULT_DOMAIN.as_slice(),
        program_id.as_ref(),
        match_key.as_ref(),
        &created_slot.to_le_bytes(),
        &[kind.as_u8()],
        winner.as_ref(),
        demo_hash.as_slice(),
    ] {
        msg[at..at + part.len()].copy_from_slice(part);
        at += part.len();
    }
    msg
}

/// Checks that the instruction before the current one is an Ed25519 precompile call
/// verifying `expected_message` under `expected_signer`.
pub fn verify_preceding_ed25519(
    instructions_sysvar: &AccountInfo,
    expected_signer: &Pubkey,
    expected_message: &[u8],
) -> Result<()> {
    let current = load_current_index_checked(instructions_sysvar)? as usize;
    require!(current > 0, ArenaError::MissingEd25519Instruction);
    let ix = load_instruction_at_checked(current - 1, instructions_sysvar)?;
    require_keys_eq!(
        ix.program_id,
        ed25519_program::ID,
        ArenaError::MissingEd25519Instruction
    );
    require!(
        ix.accounts.is_empty(),
        ArenaError::MalformedEd25519Instruction
    );

    let data = ix.data.as_slice();
    require!(
        data.len() >= HEADER_LEN + OFFSETS_LEN,
        ArenaError::MalformedEd25519Instruction
    );
    require!(data[0] == 1, ArenaError::MalformedEd25519Instruction);

    let read_u16 = |at: usize| u16::from_le_bytes([data[at], data[at + 1]]);
    let o = HEADER_LEN;
    let signature_offset = read_u16(o);
    let signature_ix_index = read_u16(o + 2);
    let pubkey_offset = read_u16(o + 4);
    let pubkey_ix_index = read_u16(o + 6);
    let message_offset = read_u16(o + 8);
    let message_size = read_u16(o + 10);
    let message_ix_index = read_u16(o + 12);

    require!(
        signature_ix_index == THIS_INSTRUCTION
            && pubkey_ix_index == THIS_INSTRUCTION
            && message_ix_index == THIS_INSTRUCTION,
        ArenaError::MalformedEd25519Instruction
    );

    // The precompile already verified the signature; we only need it to be in bounds.
    slice_at(data, signature_offset, SIGNATURE_LEN)?;
    let pubkey = slice_at(data, pubkey_offset, PUBKEY_LEN)?;
    let message = slice_at(data, message_offset, message_size as usize)?;

    require!(
        pubkey == expected_signer.as_ref(),
        ArenaError::WrongServerKey
    );
    require!(
        message == expected_message,
        ArenaError::ResultMessageMismatch
    );
    Ok(())
}

fn slice_at(data: &[u8], offset: u16, len: usize) -> Result<&[u8]> {
    let start = offset as usize;
    let end = start
        .checked_add(len)
        .ok_or_else(|| error!(ArenaError::MalformedEd25519Instruction))?;
    data.get(start..end)
        .ok_or_else(|| error!(ArenaError::MalformedEd25519Instruction))
}
