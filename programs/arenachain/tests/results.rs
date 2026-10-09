mod common;

use {
    anchor_lang::{
        error::ErrorCode as AnchorError, prelude::Pubkey, solana_program::system_instruction,
    },
    arenachain::{
        errors::ArenaError,
        state::{MatchStatus, ResultKind},
    },
    common::*,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

const DEMO: [u8; 32] = [9u8; 32];

fn win_fee(pool: u64) -> u64 {
    pool * 500 / 10_000
}

// ---------- happy paths ----------

#[test]
fn win_pays_winner_minus_fee_and_closes_vault() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let (a_before, b_before) = (env.balance(&f.a.token), env.balance(&f.b.token));
    let a_lamports = env.svm.get_account(&f.a.key()).unwrap().lamports;

    env.submit_signed(&f, ResultKind::Win, f.b.key()).unwrap();

    let pool = ENTRY * 2;
    let fee = win_fee(pool);
    assert_eq!(env.balance(&f.b.token), b_before + pool - fee);
    assert_eq!(env.balance(&f.a.token), a_before);
    assert_eq!(env.balance(&fee_vault_pda()), fee);
    assert!(!env.exists(&vault_pda(&f.m)), "vault closed when empty");
    assert!(
        env.svm.get_account(&f.a.key()).unwrap().lamports > a_lamports,
        "rent to creator"
    );

    let m = env.get_match(&f.m);
    assert_eq!(m.status, MatchStatus::Settled);
    assert_eq!(m.result_kind, Some(ResultKind::Win));
    assert_eq!(m.winner, f.b.key());
    assert_eq!(m.demo_hash, DEMO);
    assert!(m.paid_out);
    assert_eq!(m.unlock_at, 0);
}

#[test]
fn forfeit_pays_whole_pool_no_fee() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let a_before = env.balance(&f.a.token);
    env.submit_signed(&f, ResultKind::Forfeit, f.a.key())
        .unwrap();
    assert_eq!(env.balance(&f.a.token), a_before + ENTRY * 2);
    assert_eq!(env.balance(&fee_vault_pda()), 0);
    assert_eq!(env.get_match(&f.m).status, MatchStatus::Settled);
}

#[test]
fn void_refunds_both_players() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let (a_before, b_before) = (env.balance(&f.a.token), env.balance(&f.b.token));
    env.submit_signed(&f, ResultKind::Void, Pubkey::default())
        .unwrap();
    assert_eq!(env.balance(&f.a.token), a_before + ENTRY);
    assert_eq!(env.balance(&f.b.token), b_before + ENTRY);
    assert_eq!(env.balance(&fee_vault_pda()), 0);
    let m = env.get_match(&f.m);
    assert_eq!(m.status, MatchStatus::Refunded);
    assert!(m.paid_out);
    assert!(!env.exists(&vault_pda(&f.m)));
}

#[test]
fn void_with_winner_and_win_with_outsider_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    assert_code(
        env.submit_signed(&f, ResultKind::Void, f.a.key()),
        code(ArenaError::VoidWithWinner),
    );
    let outsider = Pubkey::new_unique();
    assert_code(
        env.submit_signed(&f, ResultKind::Win, outsider),
        code(ArenaError::InvalidWinner),
    );
    assert_code(
        env.submit_signed(&f, ResultKind::Forfeit, Pubkey::default()),
        code(ArenaError::InvalidWinner),
    );
}

// ---------- state checks ----------

#[test]
fn result_requires_in_progress() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, false); // Full, not started
    assert_code(
        env.submit_signed(&f, ResultKind::Win, f.a.key()),
        code(ArenaError::InvalidMatchStatus),
    );
}

#[test]
fn double_settle_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    env.submit_signed(&f, ResultKind::Win, f.a.key()).unwrap();
    // Same signed result again, and a conflicting one.
    assert_failed(env.submit_signed(&f, ResultKind::Win, f.a.key()));
    assert_failed(env.submit_signed(&f, ResultKind::Void, Pubkey::default()));
}

#[test]
fn double_settle_rejected_with_vault_still_open() {
    // A held result keeps the vault open, so the status check alone must stop a second settle.
    let mut env = Env::new();
    let f = env.fixture(HOLD_THRESHOLD / 2, true);
    env.submit_signed(&f, ResultKind::Win, f.a.key()).unwrap();
    assert_code(
        env.submit_signed(&f, ResultKind::Win, f.b.key()),
        code(ArenaError::InvalidMatchStatus),
    );
}

#[test]
fn refund_after_payout_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    env.submit_signed(&f, ResultKind::Win, f.a.key()).unwrap();
    let a_after = env.balance(&f.a.token);
    // Vault is closed after payout; the cancel can't load it, and the status check backs that up.
    let i = env.cancel_match_ix(&f.a.key(), &f.a.token, &f.m);
    assert_failed(env.send(&[i], &[&f.a.kp]));
    assert_eq!(env.balance(&f.a.token), a_after);
    assert_eq!(env.get_match(&f.m).status, MatchStatus::Settled);
}

#[test]
fn result_blocked_when_paused_or_after_deadline() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    env.set_paused(true).unwrap();
    assert_code(
        env.submit_signed(&f, ResultKind::Win, f.a.key()),
        code(ArenaError::Paused),
    );
    env.set_paused(false).unwrap();
    env.advance(RESULT_WINDOW_SECS + 1);
    assert_code(
        env.submit_signed(&f, ResultKind::Win, f.a.key()),
        code(ArenaError::ResultDeadlinePassed),
    );
}

#[test]
fn result_at_exact_deadline_accepted() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    env.advance(RESULT_WINDOW_SECS);
    env.submit_signed(&f, ResultKind::Win, f.a.key()).unwrap();
}

// ---------- server key ----------

#[test]
fn inactive_server_cannot_settle() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let key = env.server.pubkey();
    env.set_server_active(&key, false).unwrap();
    assert_code(
        env.submit_signed(&f, ResultKind::Win, f.a.key()),
        code(ArenaError::ServerInactive),
    );
}

#[test]
fn wrong_server_key_signature_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let rogue = Keypair::new();
    let msg = env.result_message(&f.m, ResultKind::Win, &f.b.key(), &DEMO);
    let s = env.submit_accounts(&f);
    let ixs = [
        ed25519_ix(&rogue, &msg),
        env.submit_ix(&s, ResultKind::Win, f.b.key(), DEMO),
    ];
    assert_code(env.send(&ixs, &[&f.b.kp]), code(ArenaError::WrongServerKey));
}

#[test]
fn another_registered_server_cannot_settle() {
    let mut env = Env::new();
    let other = Keypair::new();
    env.register_server(&other).unwrap();
    let f = env.fixture(ENTRY, true);
    let msg = env.result_message(&f.m, ResultKind::Win, &f.b.key(), &DEMO);
    let mut s = env.submit_accounts(&f);
    s.game_server = server_pda(&other.pubkey());
    let ixs = [
        ed25519_ix(&other, &msg),
        env.submit_ix(&s, ResultKind::Win, f.b.key(), DEMO),
    ];
    assert_code(env.send(&ixs, &[&f.b.kp]), code(ArenaError::WrongServerKey));
}

// ---------- Ed25519 instruction attacks ----------

fn signed_ixs(
    env: &mut Env,
    f: &Fixture,
    kind: ResultKind,
    winner: Pubkey,
) -> (
    anchor_lang::solana_program::instruction::Instruction,
    anchor_lang::solana_program::instruction::Instruction,
) {
    let msg = env.result_message(&f.m, kind, &winner, &DEMO);
    let server = env.server.insecure_clone();
    let s = env.submit_accounts(f);
    (
        ed25519_ix(&server, &msg),
        env.submit_ix(&s, kind, winner, DEMO),
    )
}

#[test]
fn missing_ed25519_instruction_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let (_, submit) = signed_ixs(&mut env, &f, ResultKind::Win, f.b.key());
    assert_code(
        env.send(&[submit], &[&f.b.kp]),
        code(ArenaError::MissingEd25519Instruction),
    );
}

#[test]
fn ed25519_not_immediately_before_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let (sig, submit) = signed_ixs(&mut env, &f, ResultKind::Win, f.b.key());
    let filler = system_instruction::transfer(&f.b.key(), &f.b.key(), 0);
    assert_code(
        env.send(&[sig, filler, submit], &[&f.b.kp]),
        code(ArenaError::MissingEd25519Instruction),
    );
}

#[test]
fn ed25519_offsets_into_other_instruction_rejected() {
    // Index fields point at instruction 0 (the precompile itself), which the precompile
    // accepts, but the program requires u16::MAX.
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let msg = env.result_message(&f.m, ResultKind::Win, &f.b.key(), &DEMO);
    let server = env.server.insecure_clone();
    let s = env.submit_accounts(&f);
    let ixs = [
        ed25519_ix_raw(&[(&server, &msg)], Some(0)),
        env.submit_ix(&s, ResultKind::Win, f.b.key(), DEMO),
    ];
    assert_code(
        env.send(&ixs, &[&f.b.kp]),
        code(ArenaError::MalformedEd25519Instruction),
    );
}

#[test]
fn ed25519_with_two_signatures_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let msg = env.result_message(&f.m, ResultKind::Win, &f.b.key(), &DEMO);
    let server = env.server.insecure_clone();
    let s = env.submit_accounts(&f);
    let ixs = [
        ed25519_ix_raw(&[(&server, &msg), (&server, &msg)], None),
        env.submit_ix(&s, ResultKind::Win, f.b.key(), DEMO),
    ];
    assert_code(
        env.send(&ixs, &[&f.b.kp]),
        code(ArenaError::MalformedEd25519Instruction),
    );
}

#[test]
fn invalid_signature_bytes_rejected_by_precompile() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let (mut sig, submit) = signed_ixs(&mut env, &f, ResultKind::Win, f.b.key());
    // Flip a byte of the signature (pubkey is at 16..48, signature at 48..112).
    sig.data[60] ^= 0xff;
    assert_failed(env.send(&[sig, submit], &[&f.b.kp]));
    assert_eq!(env.get_match(&f.m).status, MatchStatus::InProgress);
}

#[test]
fn tampered_result_fields_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let server = env.server.insecure_clone();
    let s = env.submit_accounts(&f);
    let signed = env.result_message(&f.m, ResultKind::Win, &f.a.key(), &DEMO);

    // Server said A won; relayer claims B won.
    let ixs = [
        ed25519_ix(&server, &signed),
        env.submit_ix(&s, ResultKind::Win, f.b.key(), DEMO),
    ];
    assert_code(
        env.send(&ixs, &[&f.b.kp]),
        code(ArenaError::ResultMessageMismatch),
    );

    // Win upgraded to Forfeit (fee waived).
    let ixs = [
        ed25519_ix(&server, &signed),
        env.submit_ix(&s, ResultKind::Forfeit, f.a.key(), DEMO),
    ];
    assert_code(
        env.send(&ixs, &[&f.a.kp]),
        code(ArenaError::ResultMessageMismatch),
    );

    // Different demo hash.
    let ixs = [
        ed25519_ix(&server, &signed),
        env.submit_ix(&s, ResultKind::Win, f.a.key(), [1u8; 32]),
    ];
    assert_code(
        env.send(&ixs, &[&f.a.kp]),
        code(ArenaError::ResultMessageMismatch),
    );

    // Wrong created slot.
    let created = env.get_match(&f.m).created_slot;
    let wrong_slot = arenachain::ed25519::build_result_message(
        &arenachain::id(),
        &f.m,
        created + 1,
        ResultKind::Win,
        &f.a.key(),
        &DEMO,
    );
    let ixs = [
        ed25519_ix(&server, &wrong_slot),
        env.submit_ix(&s, ResultKind::Win, f.a.key(), DEMO),
    ];
    assert_code(
        env.send(&ixs, &[&f.a.kp]),
        code(ArenaError::ResultMessageMismatch),
    );

    // Wrong program id (cross-program / cross-deployment replay).
    let other_program = arenachain::ed25519::build_result_message(
        &Pubkey::new_unique(),
        &f.m,
        created,
        ResultKind::Win,
        &f.a.key(),
        &DEMO,
    );
    let ixs = [
        ed25519_ix(&server, &other_program),
        env.submit_ix(&s, ResultKind::Win, f.a.key(), DEMO),
    ];
    assert_code(
        env.send(&ixs, &[&f.a.kp]),
        code(ArenaError::ResultMessageMismatch),
    );

    assert_eq!(env.get_match(&f.m).status, MatchStatus::InProgress);
}

#[test]
fn result_replayed_on_another_match_rejected() {
    let mut env = Env::new();
    let f1 = env.fixture(ENTRY, true);
    // Second match between the same players.
    env.create_match(&f1.a, 2, ENTRY).unwrap();
    let m2 = match_pda(&f1.a.key(), 2);
    env.join_match(&f1.b, &m2).unwrap();
    env.start_match(&m2).unwrap();

    let server = env.server.insecure_clone();
    let msg1 = env.result_message(&f1.m, ResultKind::Win, &f1.a.key(), &DEMO);
    let mut s2 = env.submit_accounts(&f1);
    s2.arena_match = m2;
    s2.vault = vault_pda(&m2);
    let ixs = [
        ed25519_ix(&server, &msg1),
        env.submit_ix(&s2, ResultKind::Win, f1.a.key(), DEMO),
    ];
    assert_code(
        env.send(&ixs, &[&f1.a.kp]),
        code(ArenaError::ResultMessageMismatch),
    );
}

#[test]
fn fake_instructions_sysvar_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let msg = env.result_message(&f.m, ResultKind::Win, &f.b.key(), &DEMO);
    let server = env.server.insecure_clone();
    let mut s = env.submit_accounts(&f);
    // Attacker-controlled account standing in for the sysvar.
    let fake = env.usdc(&f.b.key(), 0);
    s.instructions_sysvar = fake;
    let ixs = [
        ed25519_ix(&server, &msg),
        env.submit_ix(&s, ResultKind::Win, f.b.key(), DEMO),
    ];
    assert_code(
        env.send(&ixs, &[&f.b.kp]),
        code(AnchorError::ConstraintAddress),
    );
}

// ---------- recipient / vault substitution ----------

#[test]
fn payout_to_non_player_token_account_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let attacker = env.wallet();
    let attacker_token = env.usdc(&attacker.pubkey(), 0);
    let msg = env.result_message(&f.m, ResultKind::Win, &f.b.key(), &DEMO);
    let server = env.server.insecure_clone();
    let mut s = env.submit_accounts(&f);
    s.player_b_token = attacker_token;
    let ixs = [
        ed25519_ix(&server, &msg),
        env.submit_ix(&s, ResultKind::Win, f.b.key(), DEMO),
    ];
    assert_code(
        env.send(&ixs, &[&attacker]),
        code(AnchorError::ConstraintTokenOwner),
    );
}

#[test]
fn wrong_fee_vault_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let attacker = env.wallet();
    let attacker_token = env.usdc(&attacker.pubkey(), 0);
    let msg = env.result_message(&f.m, ResultKind::Win, &f.b.key(), &DEMO);
    let server = env.server.insecure_clone();
    let mut s = env.submit_accounts(&f);
    s.fee_vault = attacker_token;
    let ixs = [
        ed25519_ix(&server, &msg),
        env.submit_ix(&s, ResultKind::Win, f.b.key(), DEMO),
    ];
    assert_code(
        env.send(&ixs, &[&attacker]),
        code(AnchorError::ConstraintHasOne),
    );
}

#[test]
fn wrong_mint_token_account_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let other_mint = env.create_mint();
    let b_other = env.token_account(&other_mint, &f.b.key(), 0);
    let msg = env.result_message(&f.m, ResultKind::Win, &f.b.key(), &DEMO);
    let server = env.server.insecure_clone();
    let mut s = env.submit_accounts(&f);
    s.player_b_token = b_other;
    let ixs = [
        ed25519_ix(&server, &msg),
        env.submit_ix(&s, ResultKind::Win, f.b.key(), DEMO),
    ];
    assert_code(
        env.send(&ixs, &[&f.b.kp]),
        code(AnchorError::ConstraintTokenMint),
    );
}

#[test]
fn wrong_vault_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let other = env.usdc(&f.a.key(), ENTRY * 10);
    let msg = env.result_message(&f.m, ResultKind::Win, &f.b.key(), &DEMO);
    let server = env.server.insecure_clone();
    let mut s = env.submit_accounts(&f);
    s.vault = other;
    let ixs = [
        ed25519_ix(&server, &msg),
        env.submit_ix(&s, ResultKind::Win, f.b.key(), DEMO),
    ];
    assert_code(
        env.send(&ixs, &[&f.b.kp]),
        code(AnchorError::ConstraintSeeds),
    );
}

// ---------- hold / claim ----------

#[test]
fn pool_at_threshold_is_held_then_claimed() {
    let mut env = Env::new();
    let entry = HOLD_THRESHOLD / 2; // pool == threshold
    let f = env.fixture(entry, true);
    let b_before = env.balance(&f.b.token);
    env.submit_signed(&f, ResultKind::Win, f.b.key()).unwrap();

    let m = env.get_match(&f.m);
    assert_eq!(m.status, MatchStatus::Settled);
    assert!(!m.paid_out);
    assert_eq!(m.unlock_at, START_TS + HOLD_SECS);
    assert_eq!(env.balance(&vault_pda(&f.m)), entry * 2);
    assert_eq!(env.balance(&f.b.token), b_before);

    assert_code(
        env.claim(&f, &f.b.token.clone()),
        code(ArenaError::PayoutLocked),
    );
    env.advance(HOLD_SECS - 1);
    assert_code(
        env.claim(&f, &f.b.token.clone()),
        code(ArenaError::PayoutLocked),
    );
    env.advance(1);

    // Paying the loser is impossible.
    assert_code(
        env.claim(&f, &f.a.token.clone()),
        code(AnchorError::ConstraintTokenOwner),
    );

    let b_token = f.b.token;
    env.claim(&f, &b_token).unwrap();
    let pool = entry * 2;
    assert_eq!(env.balance(&f.b.token), b_before + pool - win_fee(pool));
    assert_eq!(env.balance(&fee_vault_pda()), win_fee(pool));
    assert!(!env.exists(&vault_pda(&f.m)));
    assert!(env.get_match(&f.m).paid_out);

    let b_after = env.balance(&b_token);
    assert_failed(env.claim(&f, &b_token));
    assert_eq!(env.balance(&b_token), b_after);
}

#[test]
fn pool_below_threshold_paid_immediately() {
    let mut env = Env::new();
    let entry = (HOLD_THRESHOLD - 2) / 2; // pool == threshold - 2 (pools are even)
    let f = env.fixture(entry, true);
    env.submit_signed(&f, ResultKind::Win, f.a.key()).unwrap();
    let m = env.get_match(&f.m);
    assert!(m.paid_out);
    assert_eq!(m.unlock_at, 0);
    let a_token = f.a.token;
    let a_after = env.balance(&a_token);
    assert_failed(env.claim(&f, &a_token));
    assert_eq!(env.balance(&a_token), a_after);
}

#[test]
fn claim_twice_with_dust_in_vault_hits_already_paid() {
    // Dust keeps the vault open after payout, so the paid_out flag is what stops a second claim.
    let mut env = Env::new();
    let f = env.fixture(HOLD_THRESHOLD, true);
    env.submit_signed(&f, ResultKind::Win, f.a.key()).unwrap();
    let vault = vault_pda(&f.m);
    let mut acc = env.svm.get_account(&vault).unwrap();
    let mut st =
        <spl_token_interface::state::Account as solana_program_pack::Pack>::unpack(&acc.data)
            .unwrap();
    st.amount += 1;
    solana_program_pack::Pack::pack(st, &mut acc.data).unwrap();
    env.svm.set_account(vault, acc).unwrap();

    env.advance(HOLD_SECS);
    let a_token = f.a.token;
    env.claim(&f, &a_token).unwrap();
    assert_eq!(env.balance(&vault), 1);
    assert_code(env.claim(&f, &a_token), code(ArenaError::AlreadyPaid));
}

#[test]
fn threshold_minus_one_pool_paid_immediately() {
    let mut env = Env::new();
    let admin = env.admin.insecure_clone();
    // Odd threshold so that pool == threshold - 1 is reachable.
    env.update_config_as(
        &admin,
        arenachain::state::ConfigParams {
            hold_threshold: 2 * ENTRY + 1,
            ..default_params()
        },
    )
    .unwrap();
    let f = env.fixture(ENTRY, true);
    env.submit_signed(&f, ResultKind::Win, f.a.key()).unwrap();
    assert!(env.get_match(&f.m).paid_out);
}

#[test]
fn held_forfeit_claims_full_pool_and_void_never_held() {
    let mut env = Env::new();
    let entry = HOLD_THRESHOLD;
    let f = env.fixture(entry, true);
    env.submit_signed(&f, ResultKind::Forfeit, f.a.key())
        .unwrap();
    assert!(!env.get_match(&f.m).paid_out);
    env.advance(HOLD_SECS);
    let a_before = env.balance(&f.a.token);
    let a_token = f.a.token;
    env.claim(&f, &a_token).unwrap();
    assert_eq!(env.balance(&f.a.token), a_before + entry * 2);
    assert_eq!(env.balance(&fee_vault_pda()), 0);

    let mut env = Env::new();
    let f = env.fixture(entry, true);
    env.submit_signed(&f, ResultKind::Void, Pubkey::default())
        .unwrap();
    let m = env.get_match(&f.m);
    assert!(m.paid_out);
    assert_eq!(m.status, MatchStatus::Refunded);
}

#[test]
fn claim_not_blocked_by_pause() {
    let mut env = Env::new();
    let f = env.fixture(HOLD_THRESHOLD, true);
    env.submit_signed(&f, ResultKind::Win, f.a.key()).unwrap();
    env.set_paused(true).unwrap();
    env.advance(HOLD_SECS);
    let a_token = f.a.token;
    env.claim(&f, &a_token).unwrap();
}

#[test]
fn claim_on_unsettled_match_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let a_token = f.a.token;
    assert_code(
        env.claim(&f, &a_token),
        code(ArenaError::InvalidMatchStatus),
    );
}

// ---------- arithmetic edges ----------

#[test]
fn smallest_entry_fee_rounds_fee_to_zero() {
    let mut env = Env::new();
    let f = env.fixture(1, true);
    let b_before = env.balance(&f.b.token);
    env.submit_signed(&f, ResultKind::Win, f.b.key()).unwrap();
    assert_eq!(env.balance(&f.b.token), b_before + 2);
    assert_eq!(env.balance(&fee_vault_pda()), 0);
    assert!(!env.exists(&vault_pda(&f.m)));
}

#[test]
fn fee_rounds_down_on_odd_pool() {
    let mut env = Env::new();
    let entry = 10_019; // pool 20_038 -> fee 1_001.9 -> 1_001
    let f = env.fixture(entry, true);
    let b_before = env.balance(&f.b.token);
    env.submit_signed(&f, ResultKind::Win, f.b.key()).unwrap();
    assert_eq!(env.balance(&fee_vault_pda()), 1_001);
    assert_eq!(env.balance(&f.b.token), b_before + 20_038 - 1_001);
}

#[test]
fn largest_pool_settles_without_overflow() {
    let mut env = Env::new();
    let entry = u64::MAX / 2;
    let a = env.player(entry);
    let b = env.player(entry);
    env.create_match(&a, 1, entry).unwrap();
    let m = match_pda(&a.key(), 1);
    env.join_match(&b, &m).unwrap();
    env.start_match(&m).unwrap();
    let f = Fixture { a, b, m };
    env.submit_signed(&f, ResultKind::Win, f.a.key()).unwrap();
    env.advance(HOLD_SECS);
    let a_token = f.a.token;
    env.claim(&f, &a_token).unwrap();
    let pool = entry as u128 * 2;
    let fee = (pool * 500 / 10_000) as u64;
    assert_eq!(env.balance(&fee_vault_pda()), fee);
    assert_eq!(env.balance(&f.a.token), (pool as u64) - fee);
}
