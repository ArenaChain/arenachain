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
    assert!(!env.exists(&vault_pda(&f.m)), "vault closed, rent to player A's address");
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
