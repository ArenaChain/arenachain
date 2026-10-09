use anchor_lang::prelude::*;

use crate::state::ResultKind;

#[event]
pub struct MatchCreated {
    pub arena_match: Pubkey,
    pub player_a: Pubkey,
    pub game_server: Pubkey,
    pub entry_fee: u64,
}

#[event]
pub struct MatchJoined {
    pub arena_match: Pubkey,
    pub player_b: Pubkey,
    pub result_deadline: i64,
}

#[event]
pub struct MatchCancelled {
    pub arena_match: Pubkey,
}

#[event]
pub struct MatchStarted {
    pub arena_match: Pubkey,
    pub game_server: Pubkey,
}

#[event]
pub struct ResultSubmitted {
    pub arena_match: Pubkey,
    pub kind: ResultKind,
    pub winner: Pubkey,
    pub demo_hash: [u8; 32],
    /// 0 when paid immediately.
    pub unlock_at: i64,
}

#[event]
pub struct PayoutMade {
    pub arena_match: Pubkey,
    pub winner: Pubkey,
    pub payout: u64,
    pub fee: u64,
}
