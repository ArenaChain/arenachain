use anchor_lang::prelude::*;

#[constant]
pub const CONFIG_SEED: &[u8] = b"config";
#[constant]
pub const FEE_VAULT_SEED: &[u8] = b"fee_vault";
#[constant]
pub const SERVER_SEED: &[u8] = b"server";
#[constant]
pub const PROFILE_SEED: &[u8] = b"profile";
#[constant]
pub const MATCH_SEED: &[u8] = b"match";
#[constant]
pub const VAULT_SEED: &[u8] = b"vault";

/// Platform fee on `Win` results, in basis points. Fixed in code so no admin can change it.
#[constant]
pub const FEE_BPS: u64 = 500;
pub const BPS_DENOMINATOR: u64 = 10_000;

#[constant]
pub const STARTING_ELO: u32 = 1000;
#[constant]
pub const MIN_ELO: u32 = 100;
pub const ELO_K: i64 = 32;

#[constant]
pub const MAX_REPUTATION: i32 = 100;
pub const FORFEIT_REPUTATION_PENALTY: i32 = 10;
pub const COMPLETED_MATCH_REPUTATION: i32 = 1;

/// Bounds on admin-set timing, so no config change can lock escrow for long.
#[constant]
pub const MAX_HOLD_SECS: i64 = 14 * 24 * 3600;
#[constant]
pub const MIN_RESULT_WINDOW_SECS: i64 = 10 * 60;
#[constant]
pub const MAX_RESULT_WINDOW_SECS: i64 = 24 * 3600;

pub const FORFEIT_WINDOW_SECS: i64 = 30 * 24 * 3600;
pub const SECOND_FORFEIT_BAN_SECS: i64 = 24 * 3600;
pub const THIRD_FORFEIT_BAN_SECS: i64 = 7 * 24 * 3600;

/// Domain tag that prefixes every server-signed result message.
pub const RESULT_DOMAIN: &[u8; 20] = b"ARENACHAIN_RESULT_V1";
/// domain(20) + program(32) + match(32) + created_slot(8) + kind(1) + winner(32) + demo_hash(32)
pub const RESULT_MESSAGE_LEN: usize = 20 + 32 + 32 + 8 + 1 + 32 + 32;
