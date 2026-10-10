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

// ======================= M4: two-step admin handover, dispute authority rotation =======================

fn propose_admin_ix(
    admin: &Pubkey,
    new_admin: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    ix(
        arenachain::instruction::AdminProposeAdmin { new_admin },
        arenachain::accounts::AdminUpdateConfig {
            admin: *admin,
            config: config_pda(),
        },
    )
}

fn accept_admin_ix(new_admin: &Pubkey) -> anchor_lang::solana_program::instruction::Instruction {
    ix(
        arenachain::instruction::AdminAcceptAdmin {},
        arenachain::accounts::AdminAcceptAdmin {
            new_admin: *new_admin,
            config: config_pda(),
        },
    )
}

fn set_dispute_ix(
    admin: &Pubkey,
    new_authority: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    ix(
        arenachain::instruction::AdminSetDisputeAuthority { new_authority },
        arenachain::accounts::AdminUpdateConfig {
            admin: *admin,
            config: config_pda(),
        },
    )
}

#[test]
fn m4_admin_handover_two_step() {
    use arenachain::errors::ArenaError;
    let mut env = Env::new();
    let old = env.admin.insecure_clone();
    let new = env.wallet();
    let attacker = env.wallet();

    // Only the admin can propose.
    let i = propose_admin_ix(&attacker.pubkey(), attacker.pubkey());
    assert_code(env.send(&[i], &[&attacker]), code(ArenaError::Unauthorized));

    // Nothing pending: nobody can accept.
    let i = accept_admin_ix(&new.pubkey());
    assert_code(env.send(&[i], &[&new]), code(ArenaError::Unauthorized));

    let i = propose_admin_ix(&old.pubkey(), new.pubkey());
    env.send(&[i], &[&old]).unwrap();
    assert_eq!(env.config().pending_admin, new.pubkey());
    // Still the old admin until accepted.
    env.set_paused_as(&old, true).unwrap();
    assert_code(
        env.set_paused_as(&new, false),
        code(ArenaError::Unauthorized),
    );

    // Wrong signer can't accept.
    let i = accept_admin_ix(&attacker.pubkey());
    assert_code(env.send(&[i], &[&attacker]), code(ArenaError::Unauthorized));

    let i = accept_admin_ix(&new.pubkey());
    env.send(&[i], &[&new]).unwrap();
    let c = env.config();
    assert_eq!(
        (c.admin, c.pending_admin),
        (new.pubkey(), Pubkey::default())
    );
    assert_code(
        env.set_paused_as(&old, false),
        code(ArenaError::Unauthorized),
    );
    env.set_paused_as(&new, false).unwrap();

    // Accepting again does nothing useful: pending is cleared.
    let i = accept_admin_ix(&new.pubkey());
    assert_code(env.send(&[i], &[&new]), code(ArenaError::Unauthorized));
}

#[test]
fn m4_proposing_default_cancels_handover() {
    use arenachain::errors::ArenaError;
    let mut env = Env::new();
    let admin = env.admin.insecure_clone();
    let new = env.wallet();
    env.send(
        &[propose_admin_ix(&admin.pubkey(), new.pubkey())],
        &[&admin],
    )
    .unwrap();
    env.send(
        &[propose_admin_ix(&admin.pubkey(), Pubkey::default())],
        &[&admin],
    )
    .unwrap();
    let i = accept_admin_ix(&new.pubkey());
    assert_code(env.send(&[i], &[&new]), code(ArenaError::Unauthorized));
    assert_eq!(env.config().admin, admin.pubkey());
}

#[test]
fn m4_set_dispute_authority_admin_only() {
    use arenachain::errors::ArenaError;
    let mut env = Env::new();
    let admin = env.admin.insecure_clone();
    let attacker = env.wallet();
    let i = set_dispute_ix(&attacker.pubkey(), attacker.pubkey());
    assert_code(env.send(&[i], &[&attacker]), code(ArenaError::Unauthorized));
    let i = set_dispute_ix(&admin.pubkey(), Pubkey::default());
    assert_code(
        env.send(&[i], &[&admin]),
        code(ArenaError::InvalidConfigParam),
    );
    let next = Pubkey::new_unique();
    env.send(&[set_dispute_ix(&admin.pubkey(), next)], &[&admin])
        .unwrap();
    assert_eq!(env.config().dispute_authority, next);
}

// ======================= L1: server-signed Void refunds even while paused =======================

#[test]
fn l1_void_refund_allowed_while_paused() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let (a0, b0) = (env.balance(&f.a.token), env.balance(&f.b.token));
    env.set_paused(true).unwrap();
    env.submit_signed(&f, ResultKind::Void, Pubkey::default())
        .unwrap();
    assert_eq!(env.balance(&f.a.token), a0 + ENTRY);
    assert_eq!(env.balance(&f.b.token), b0 + ENTRY);
    // Win/Forfeit stay blocked while paused.
    let g = {
        let a = env.player(ENTRY * 3);
        let b = env.player(ENTRY * 3);
        env.set_paused(false).unwrap();
        env.create_match(&a, 7, ENTRY).unwrap();
        let m = match_pda(&a.key(), 7);
        env.join_match(&b, &m).unwrap();
        env.start_match(&m).unwrap();
        env.set_paused(true).unwrap();
        Fixture { a, b, m }
    };
    assert_code(
        env.submit_signed(&g, ResultKind::Win, g.a.key()),
        code(arenachain::errors::ArenaError::Paused),
    );
}

#[test]
fn l1_paused_void_still_needs_an_active_server() {
    // Mitigation for a stolen server key during a pause: deactivating the server stops Voids too.
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    env.set_paused(true).unwrap();
    let key = env.server.pubkey();
    env.set_server_active(&key, false).unwrap();
    assert_code(
        env.submit_signed(&f, ResultKind::Void, Pubkey::default()),
        code(arenachain::errors::ArenaError::ServerInactive),
    );
    assert_eq!(env.get_match(&f.m).status, MatchStatus::InProgress);
}

// ======================= L2: join requires the match's server to be active =======================

#[test]
fn l2_join_rejected_when_server_inactive() {
    let mut env = Env::new();
    let a = env.player(ENTRY);
    let b = env.player(ENTRY);
    env.create_match(&a, 1, ENTRY).unwrap();
    let m = match_pda(&a.key(), 1);
    let key = env.server.pubkey();
    env.set_server_active(&key, false).unwrap();
    assert_code(
        env.join_match(&b, &m),
        code(arenachain::errors::ArenaError::ServerInactive),
    );
    assert_eq!(env.balance(&b.token), ENTRY);
    env.set_server_active(&key, true).unwrap();
    env.join_match(&b, &m).unwrap();
}

#[test]
fn l2_join_rejects_another_servers_account() {
    // An attacker can't satisfy the check by passing some other (active) server.
    let mut env = Env::new();
    let other = solana_keypair::Keypair::new();
    env.register_server(&other).unwrap();
    let a = env.player(ENTRY);
    let b = env.player(ENTRY);
    env.create_match(&a, 1, ENTRY).unwrap();
    let m = match_pda(&a.key(), 1);
    let mut i = env.join_match_ix(&b.key(), &b.token, &m);
    i.accounts[4].pubkey = server_pda(&other.pubkey());
    assert_code(
        env.send(&[i], &[&b.kp]),
        code(anchor_lang::error::ErrorCode::ConstraintAddress),
    );
}

// ======================= L4: init rejects a default admin or dispute authority =======================

#[test]
fn l4_initialize_rejects_default_admin_or_dispute_authority() {
    use arenachain::errors::ArenaError;
    let mut env = Env::bare();
    let admin = env.admin.insecure_clone();
    // Instruction data: discriminator(8), admin(32), dispute_authority(32), treasury(32), params.
    for range in [8..40, 40..72] {
        let mut i = env.init_config_ix(&admin.pubkey(), default_params());
        i.data[range].copy_from_slice(&[0u8; 32]);
        assert_code(
            env.send(&[i], &[&admin]),
            code(ArenaError::InvalidConfigParam),
        );
    }
    env.init_config(&admin, default_params()).unwrap();
}

// ======================= Extra: player A abandons their token account =======================
// Same class as C1: can player A block player B's refund by closing or giving away the token
// account a refund would be paid into? No: anyone can recreate A's associated token account
// (create-idempotent) in the same transaction, and the recipient check only requires
// owner == player A and the right mint.

use spl_associated_token_account_interface::{
    address::get_associated_token_address, instruction::create_associated_token_account_idempotent,
};

enum Abandon {
    Close,
    GiveAway,
}

/// Player A empties and closes their token account, or hands its ownership to someone else.
fn abandon_token_account(env: &mut Env, f: &Fixture, how: Abandon) {
    let owner = f.a.key();
    let tp = token_program();
    let ixs = match how {
        Abandon::Close => {
            let sink = env.usdc(&Pubkey::new_unique(), 0);
            let left = env.balance(&f.a.token);
            vec![
                spl_token_interface::instruction::transfer(
                    &tp,
                    &f.a.token,
                    &sink,
                    &owner,
                    &[],
                    left,
                )
                .unwrap(),
                spl_token_interface::instruction::close_account(
                    &tp,
                    &f.a.token,
                    &owner,
                    &owner,
                    &[],
                )
                .unwrap(),
            ]
        }
        Abandon::GiveAway => vec![spl_token_interface::instruction::set_authority(
            &tp,
            &f.a.token,
            Some(&Pubkey::new_unique()),
            spl_token_interface::instruction::AuthorityType::AccountOwner,
            &owner,
            &[],
        )
        .unwrap()],
    };
    env.send(&ixs, &[&f.a.kp])
        .expect("player A abandons their token account");
}

/// A third party recreates player A's ATA and the refund in one transaction.
fn relayer_recreates_ata_then(
    env: &mut Env,
    f: &Fixture,
    then: anchor_lang::solana_program::instruction::Instruction,
) -> Pubkey {
    let relayer = env.wallet();
    let ata = get_associated_token_address(&f.a.key(), &env.mint);
    let create = create_associated_token_account_idempotent(
        &relayer.pubkey(),
        &f.a.key(),
        &env.mint,
        &token_program(),
    );
    env.send(&[create, then], &[&relayer])
        .expect("refund after ATA recreate");
    ata
}

fn refund_with_a_token(
    env: &Env,
    f: &Fixture,
    a_token: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    let mut i = env.refund_expired_ix(f);
    i.accounts[3].pubkey = a_token; // player_a_token
    i
}

#[test]
fn extra_expiry_refund_survives_closed_or_given_away_token_account() {
    for how in [Abandon::Close, Abandon::GiveAway] {
        let mut env = Env::new();
        let f = env.fixture(ENTRY, true);
        let b0 = env.balance(&f.b.token);
        abandon_token_account(&mut env, &f, how);
        env.advance(RESULT_WINDOW_SECS + 1);

        // The original account no longer qualifies.
        assert_failed(env.refund_expired(&f));

        let ata = get_associated_token_address(&f.a.key(), &env.mint);
        let i = refund_with_a_token(&env, &f, ata);
        relayer_recreates_ata_then(&mut env, &f, i);
        assert_eq!(env.balance(&ata), ENTRY);
        assert_eq!(env.balance(&f.b.token), b0 + ENTRY);
        assert_eq!(env.get_match(&f.m).status, MatchStatus::Refunded);
    }
}

#[test]
fn extra_void_refund_survives_closed_or_given_away_token_account() {
    for how in [Abandon::Close, Abandon::GiveAway] {
        let mut env = Env::new();
        let f = env.fixture(ENTRY, true);
        let b0 = env.balance(&f.b.token);
        abandon_token_account(&mut env, &f, how);

        let ata = get_associated_token_address(&f.a.key(), &env.mint);
        let demo = [9u8; 32];
        let msg = env.result_message(&f.m, ResultKind::Void, &Pubkey::default(), &demo);
        let server = env.server.insecure_clone();
        let mut s = env.submit_accounts(&f);
        s.player_a_token = ata;
        let relayer = env.wallet();
        let create = create_associated_token_account_idempotent(
            &relayer.pubkey(),
            &f.a.key(),
            &env.mint,
            &token_program(),
        );
        // ATA create first; the Ed25519 instruction must stay directly before submit_result.
        let ixs = [
            create,
            ed25519_ix(&server, &msg),
            env.submit_ix(&s, ResultKind::Void, Pubkey::default(), demo),
        ];
        env.send(&ixs, &[&relayer])
            .expect("void after ATA recreate");
        assert_eq!(env.balance(&ata), ENTRY);
        assert_eq!(env.balance(&f.b.token), b0 + ENTRY);
    }
}

#[test]
fn extra_refund_survives_assigned_wallet_and_closed_token_account() {
    // C1 and the token-account abandonment combined.
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let b0 = env.balance(&f.b.token);
    abandon_token_account(&mut env, &f, Abandon::Close);
    assign_player_a(&mut env, &f);
    env.advance(RESULT_WINDOW_SECS + 1);
    let ata = get_associated_token_address(&f.a.key(), &env.mint);
    let i = refund_with_a_token(&env, &f, ata);
    relayer_recreates_ata_then(&mut env, &f, i);
    assert_eq!(env.balance(&ata), ENTRY);
    assert_eq!(env.balance(&f.b.token), b0 + ENTRY);
}

// ======================= T1: claim_payout account substitution =======================

#[test]
fn t1_claim_rejects_substituted_accounts() {
    use anchor_lang::error::ErrorCode as AnchorError;
    let mut env = Env::new();
    let f = env.fixture(HOLD_THRESHOLD, true);
    env.submit_signed(&f, ResultKind::Win, f.b.key()).unwrap();
    env.advance(HOLD_SECS);
    let attacker = env.wallet();
    let attacker_token = env.usdc(&attacker.pubkey(), 0);
    let fee0 = env.balance(&fee_vault_pda());
    // ClaimPayout accounts: config, arena_match, mint, vault, winner_token, fee_vault, player_a, token_program.
    let cases: [(usize, Pubkey, u32); 5] = [
        (5, attacker_token, AnchorError::ConstraintHasOne.into()), // steal the fee
        (4, attacker_token, AnchorError::ConstraintTokenOwner.into()), // steal the payout
        (
            3,
            env.usdc(&f.a.key(), HOLD_THRESHOLD * 2),
            AnchorError::ConstraintSeeds.into(),
        ),
        (2, env.create_mint(), AnchorError::ConstraintHasOne.into()),
        (6, attacker.pubkey(), AnchorError::ConstraintAddress.into()), // vault rent
    ];
    for (idx, key, err) in cases {
        let mut i = env.claim_ix(&f.m, &f.b.token, &f.a.key());
        i.accounts[idx].pubkey = key;
        assert_code(env.send(&[i], &[&attacker]), err);
    }
    assert_eq!(env.balance(&attacker_token), 0);
    assert_eq!(env.balance(&fee_vault_pda()), fee0);
    let b_token = f.b.token;
    env.claim(&f, &b_token).unwrap();
}

// ======================= T3: join pays only from the joiner's own account =======================

#[test]
fn t3_join_rejects_foreign_or_wrong_mint_token_account() {
    use anchor_lang::error::ErrorCode as AnchorError;
    let mut env = Env::new();
    let a = env.player(ENTRY);
    let b = env.player(ENTRY);
    let victim = env.player(ENTRY * 5);
    env.create_match(&a, 1, ENTRY).unwrap();
    let m = match_pda(&a.key(), 1);

    // The victim approved B as a delegate, so the token program alone would allow the transfer.
    let approve = spl_token_interface::instruction::approve(
        &token_program(),
        &victim.token,
        &b.key(),
        &victim.key(),
        &[],
        ENTRY * 5,
    )
    .unwrap();
    env.send(&[approve], &[&victim.kp]).unwrap();
    // PlayerJoinMatch accounts: joiner, config, profile, match, game_server, mint, vault, joiner_token, ...
    let mut i = env.join_match_ix(&b.key(), &b.token, &m);
    i.accounts[7].pubkey = victim.token;
    assert_code(
        env.send(&[i], &[&b.kp]),
        AnchorError::ConstraintTokenOwner.into(),
    );
    assert_eq!(env.balance(&victim.token), ENTRY * 5);

    let other_mint = env.create_mint();
    let wrong = env.token_account(&other_mint, &b.key(), ENTRY);
    let mut i = env.join_match_ix(&b.key(), &b.token, &m);
    i.accounts[7].pubkey = wrong;
    assert_code(
        env.send(&[i], &[&b.kp]),
        AnchorError::ConstraintTokenMint.into(),
    );
}

// ======================= T5: Ed25519 offsets into another instruction (Wormhole-style) =======================

#[test]
fn t5_signature_borrowed_from_another_instruction_rejected() {
    // ix0: a genuine, valid Ed25519 check of a ROGUE key over a rogue message.
    // ix1: an Ed25519 instruction whose index fields point at ix0 (so the precompile verifies
    //      ix0's bytes and passes), while its own data holds the SERVER key and the expected
    //      message at the same offsets. A program that read ix1's bytes without checking the
    //      index fields would accept a result the server never signed.
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let rogue = solana_keypair::Keypair::new();
    let demo = [9u8; 32];
    let expected = env.result_message(&f.m, ResultKind::Win, &f.b.key(), &demo);
    let genuine = ed25519_ix(&rogue, &expected);

    let mut forged = genuine.clone();
    // Index fields -> instruction 0.
    for at in [4usize, 8, 14] {
        forged.data[at..at + 2].copy_from_slice(&0u16.to_le_bytes());
    }
    // Same offsets (pubkey at 16, signature at 48, message at 112), but the server's key.
    forged.data[16..48].copy_from_slice(env.server.pubkey().as_ref());
    forged.data[48..112].fill(0);

    let s = env.submit_accounts(&f);
    let ixs = [
        genuine,
        forged,
        env.submit_ix(&s, ResultKind::Win, f.b.key(), demo),
    ];
    let relayer = env.wallet();
    assert_code(
        env.send(&ixs, &[&relayer]),
        code(arenachain::errors::ArenaError::MalformedEd25519Instruction),
    );
    assert_eq!(env.get_match(&f.m).status, MatchStatus::InProgress);
}
