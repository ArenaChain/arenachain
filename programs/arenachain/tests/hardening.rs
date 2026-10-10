//! Regression tests for the findings in REVIEW.md.

mod common;

use {
    anchor_lang::{prelude::Pubkey, solana_program::system_instruction},
    arenachain::state::{MatchStatus, ResultKind},
    common::*,
    solana_signer::Signer,
};

// ======================= C1: creator reassigns their wallet =======================

/// Player A signs a System `Assign` on their own wallet, making it non-system-owned.
fn assign_player_a(env: &mut Env, f: &Fixture) {
    let ix = system_instruction::assign(&f.a.key(), &Pubkey::new_unique());
    env.send(&[ix], &[&f.a.kp]).expect("assign own wallet");
}

#[test]
fn c1_settlement_survives_creator_assigning_wallet() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let b0 = env.balance(&f.b.token);
    assign_player_a(&mut env, &f);
    env.submit_signed(&f, ResultKind::Win, f.b.key()).unwrap();
    let pool = ENTRY * 2;
    assert_eq!(env.balance(&f.b.token), b0 + pool - pool * 500 / 10_000);
    assert!(
        !env.exists(&vault_pda(&f.m)),
        "vault closed, rent to player A's address"
    );
}

#[test]
fn c1_expiry_refund_survives_creator_assigning_wallet() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let b0 = env.balance(&f.b.token);
    assign_player_a(&mut env, &f);
    env.advance(RESULT_WINDOW_SECS + 1);
    env.refund_expired(&f).unwrap();
    assert_eq!(env.balance(&f.b.token), b0 + ENTRY);
    assert_eq!(env.get_match(&f.m).status, MatchStatus::Refunded);
}

#[test]
fn c1_held_claim_survives_creator_assigning_wallet() {
    let mut env = Env::new();
    let f = env.fixture(HOLD_THRESHOLD, true);
    env.submit_signed(&f, ResultKind::Win, f.b.key()).unwrap();
    assign_player_a(&mut env, &f);
    env.advance(HOLD_SECS);
    let (b_token, b0) = (f.b.token, env.balance(&f.b.token));
    env.claim(&f, &b_token).unwrap();
    assert!(env.balance(&f.b.token) > b0);
    assert!(env.get_match(&f.m).paid_out);
}

// ======================= M3: result deadline counts from start =======================

#[test]
fn m3_late_start_still_gets_full_result_window() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, false);
    // Players take almost the whole window to ready up.
    env.advance(RESULT_WINDOW_SECS - 60);
    env.start_match(&f.m).unwrap();
    // Past the old join-based deadline, but inside start + window.
    env.advance(120);
    assert_code(
        env.refund_expired(&f),
        code(arenachain::errors::ArenaError::NotExpired),
    );
    let b0 = env.balance(&f.b.token);
    env.submit_signed(&f, ResultKind::Win, f.b.key()).unwrap();
    assert!(env.balance(&f.b.token) > b0);
}

#[test]
fn m3_deadlines_follow_join_then_start() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, false);
    let joined = env.get_match(&f.m);
    assert_eq!(joined.start_deadline, START_TS + RESULT_WINDOW_SECS);
    env.advance(100);
    env.start_match(&f.m).unwrap();
    let started = env.get_match(&f.m);
    assert_eq!(started.result_deadline, START_TS + 100 + RESULT_WINDOW_SECS);
    // Expiry of a started match follows the result deadline, to the second.
    env.set_time(started.result_deadline);
    assert_code(
        env.refund_expired(&f),
        code(arenachain::errors::ArenaError::NotExpired),
    );
    env.advance(1);
    env.refund_expired(&f).unwrap();
}

#[test]
fn m3_unstarted_match_expires_at_start_deadline() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, false);
    env.set_time(START_TS + RESULT_WINDOW_SECS);
    assert_code(
        env.refund_expired(&f),
        code(arenachain::errors::ArenaError::NotExpired),
    );
    env.advance(1);
    assert_code(
        env.start_match(&f.m),
        code(arenachain::errors::ArenaError::ResultDeadlinePassed),
    );
    env.refund_expired(&f).unwrap();
}

// ======================= M1: timing params are bounded =======================

#[test]
fn m1_update_config_rejects_out_of_bound_timing() {
    use arenachain::{errors::ArenaError, state::ConfigParams};
    const DAY: i64 = 24 * 3600;
    let mut env = Env::new();
    let admin = env.admin.insecure_clone();
    let with = |hold_secs: i64, result_window_secs: i64| ConfigParams {
        hold_secs,
        result_window_secs,
        ..default_params()
    };
    for bad in [
        with(14 * DAY + 1, 3600),
        with(i64::MAX, 3600),
        with(3600, 599),
        with(3600, DAY + 1),
        with(3600, i64::MAX),
    ] {
        assert_code(
            env.update_config_as(&admin, bad),
            code(ArenaError::InvalidConfigParam),
        );
    }
    for good in [with(14 * DAY, 600), with(0, DAY)] {
        env.update_config_as(&admin, good).unwrap();
    }
}

#[test]
fn m1_initialize_rejects_out_of_bound_timing() {
    use arenachain::{errors::ArenaError, state::ConfigParams};
    let mut env = Env::bare();
    let admin = env.admin.insecure_clone();
    let bad = ConfigParams {
        hold_secs: 10i64.pow(12),
        ..default_params()
    };
    assert_code(
        env.init_config(&admin, bad),
        code(ArenaError::InvalidConfigParam),
    );
    env.init_config(&admin, default_params()).unwrap();
}

// ======================= M2: config mint must be plain 6-decimal, unfreezable =======================

#[test]
fn m2_initialize_rejects_freezable_or_wrong_decimal_mint() {
    use arenachain::errors::ArenaError;
    let mut env = Env::bare();
    let admin = env.admin.insecure_clone();

    env.mint = env.create_mint_with(6, Some(Pubkey::new_unique()));
    assert_code(
        env.init_config(&admin, default_params()),
        code(ArenaError::InvalidMint),
    );

    env.mint = env.create_mint_with(9, None);
    assert_code(
        env.init_config(&admin, default_params()),
        code(ArenaError::InvalidMint),
    );

    env.mint = env.create_mint_with(6, None);
    env.init_config(&admin, default_params()).unwrap();
}
