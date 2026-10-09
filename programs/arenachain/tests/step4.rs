mod common;

use {
    anchor_lang::{error::ErrorCode as AnchorError, prelude::Pubkey},
    arenachain::{
        errors::ArenaError,
        state::{GlobalConfig, MatchStatus, ResultKind},
        MAX_REPUTATION, MIN_ELO, SECOND_FORFEIT_BAN_SECS, STARTING_ELO, THIRD_FORFEIT_BAN_SECS,
    },
    common::*,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

const DAY: i64 = 24 * 3600;

fn clone(p: &Player) -> Player {
    Player {
        kp: p.kp.insecure_clone(),
        token: p.token,
    }
}

/// Creates (nonce), joins, starts and settles a match between `a` and `b`.
fn play(
    env: &mut Env,
    a: &Player,
    b: &Player,
    nonce: u64,
    kind: ResultKind,
    winner: Pubkey,
) -> Fixture {
    env.create_match(a, nonce, ENTRY).expect("create");
    let m = match_pda(&a.key(), nonce);
    env.join_match(b, &m).expect("join");
    env.start_match(&m).expect("start");
    let f = Fixture {
        a: clone(a),
        b: clone(b),
        m,
    };
    env.submit_signed(&f, kind, winner).expect("submit");
    f
}

// ======================= expiry refund =======================

#[test]
fn expired_in_progress_match_refunds_both() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let (a0, b0) = (env.balance(&f.a.token), env.balance(&f.b.token));

    // At the deadline a result is still allowed, so refunds are not.
    env.advance(RESULT_WINDOW_SECS);
    assert_code(env.refund_expired(&f), code(ArenaError::NotExpired));
    env.advance(1);
    env.refund_expired(&f).unwrap();

    assert_eq!(env.balance(&f.a.token), a0 + ENTRY);
    assert_eq!(env.balance(&f.b.token), b0 + ENTRY);
    let m = env.get_match(&f.m);
    assert_eq!(m.status, MatchStatus::Refunded);
    assert!(m.paid_out);
    assert!(!env.exists(&vault_pda(&f.m)));
    // No stats change on expiry.
    assert_eq!(env.profile(&f.a.key()).elo, STARTING_ELO);
    assert_eq!(env.profile(&f.b.key()).reputation, MAX_REPUTATION);
}

#[test]
fn expired_full_match_refunds_both() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, false);
    env.advance(RESULT_WINDOW_SECS + 1);
    env.refund_expired(&f).unwrap();
    assert_eq!(env.get_match(&f.m).status, MatchStatus::Refunded);
    assert_eq!(env.balance(&f.a.token), ENTRY * 3);
    assert_eq!(env.balance(&f.b.token), ENTRY * 3);
}

#[test]
fn refund_before_deadline_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    assert_code(env.refund_expired(&f), code(ArenaError::NotExpired));
}

#[test]
fn refund_open_match_rejected() {
    let mut env = Env::new();
    let a = env.player(ENTRY * 3);
    let b = env.player(ENTRY * 3);
    env.create_match(&a, 1, ENTRY).unwrap();
    let m = match_pda(&a.key(), 1);
    let f = Fixture { a, b, m };
    env.advance(RESULT_WINDOW_SECS * 10);
    assert_code(env.refund_expired(&f), code(ArenaError::InvalidMatchStatus));
}

#[test]
fn refund_after_settlement_or_twice_rejected() {
    let mut env = Env::new();
    // Held settlement keeps the vault open, so the status check is what stops it.
    let f = env.fixture(HOLD_THRESHOLD, true);
    env.submit_signed(&f, ResultKind::Win, f.a.key()).unwrap();
    env.advance(RESULT_WINDOW_SECS + 1);
    assert_code(env.refund_expired(&f), code(ArenaError::InvalidMatchStatus));

    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    env.advance(RESULT_WINDOW_SECS + 1);
    env.refund_expired(&f).unwrap();
    let a_after = env.balance(&f.a.token);
    assert_failed(env.refund_expired(&f));
    assert_eq!(env.balance(&f.a.token), a_after);
}

#[test]
fn result_after_expiry_rejected_and_refund_still_works() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    env.advance(RESULT_WINDOW_SECS + 1);
    assert_code(
        env.submit_signed(&f, ResultKind::Win, f.a.key()),
        code(ArenaError::ResultDeadlinePassed),
    );
    env.refund_expired(&f).unwrap();
}

#[test]
fn refund_works_while_paused_and_server_inactive() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    env.set_paused(true).unwrap();
    let key = env.server.pubkey();
    env.set_server_active(&key, false).unwrap();
    env.advance(RESULT_WINDOW_SECS + 1);
    env.refund_expired(&f).unwrap();
    assert_eq!(env.get_match(&f.m).status, MatchStatus::Refunded);
}

#[test]
fn refund_rejects_substituted_accounts() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    env.advance(RESULT_WINDOW_SECS + 1);

    // Recipient owned by an attacker.
    let attacker = env.wallet();
    let attacker_token = env.usdc(&attacker.pubkey(), 0);
    let mut i = env.refund_expired_ix(&f);
    i.accounts[4].pubkey = attacker_token; // player_b_token
    assert_code(
        env.send(&[i], &[&attacker]),
        code(AnchorError::ConstraintTokenOwner),
    );

    // Another token account as the vault.
    let other = env.usdc(&f.a.key(), ENTRY);
    let mut i = env.refund_expired_ix(&f);
    i.accounts[2].pubkey = other;
    assert_code(
        env.send(&[i], &[&attacker]),
        code(AnchorError::ConstraintSeeds),
    );

    // Wrong mint.
    let fake_mint = env.create_mint();
    let mut i = env.refund_expired_ix(&f);
    i.accounts[1].pubkey = fake_mint;
    assert_code(
        env.send(&[i], &[&attacker]),
        code(AnchorError::ConstraintHasOne),
    );

    // Rent destination must be player A.
    let mut i = env.refund_expired_ix(&f);
    i.accounts[5].pubkey = attacker.pubkey();
    assert_code(
        env.send(&[i], &[&attacker]),
        code(AnchorError::ConstraintAddress),
    );

    env.refund_expired(&f).unwrap();
}

// ======================= ratings at settlement =======================

#[test]
fn win_updates_elo_record_and_reputation() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    // Start A below the cap so the +1 is visible.
    let mut pa = env.profile(&f.a.key());
    pa.reputation = 50;
    env.write(&profile_pda(&f.a.key()), &pa);

    env.submit_signed(&f, ResultKind::Win, f.b.key()).unwrap();
    let (pa, pb) = (env.profile(&f.a.key()), env.profile(&f.b.key()));
    assert_eq!((pb.elo, pa.elo), (STARTING_ELO + 16, STARTING_ELO - 16));
    assert_eq!((pb.wins, pb.losses, pa.wins, pa.losses), (1, 0, 0, 1));
    assert_eq!((pb.reputation, pa.reputation), (MAX_REPUTATION, 51));
    assert_eq!((pa.banned_until, pa.total_forfeits), (0, 0));
}

#[test]
fn void_leaves_stats_untouched() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    env.submit_signed(&f, ResultKind::Void, Pubkey::default())
        .unwrap();
    for p in [env.profile(&f.a.key()), env.profile(&f.b.key())] {
        assert_eq!(
            (p.elo, p.wins, p.losses, p.reputation),
            (STARTING_ELO, 0, 0, MAX_REPUTATION)
        );
    }
}

#[test]
fn held_win_updates_stats_once() {
    let mut env = Env::new();
    let f = env.fixture(HOLD_THRESHOLD, true);
    env.submit_signed(&f, ResultKind::Win, f.a.key()).unwrap();
    assert_eq!(env.profile(&f.a.key()).wins, 1);
    env.advance(HOLD_SECS);
    let a_token = f.a.token;
    env.claim(&f, &a_token).unwrap();
    let pa = env.profile(&f.a.key());
    assert_eq!((pa.wins, pa.elo), (1, STARTING_ELO + 16));
}

#[test]
fn elo_floor_holds_on_chain() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    for (who, elo) in [(f.a.key(), MIN_ELO + 5), (f.b.key(), MIN_ELO + 10)] {
        let mut p = env.profile(&who);
        p.elo = elo;
        env.write(&profile_pda(&who), &p);
    }
    env.submit_signed(&f, ResultKind::Win, f.b.key()).unwrap();
    assert_eq!(env.profile(&f.a.key()).elo, MIN_ELO);
}

#[test]
fn swapped_or_foreign_profile_rejected() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let outsider = env.player(0);
    let server = env.server.insecure_clone();
    let demo = [9u8; 32];
    let msg = env.result_message(&f.m, ResultKind::Forfeit, &f.a.key(), &demo);

    // B's penalty redirected to an outsider's profile.
    let mut s = env.submit_accounts(&f);
    s.player_b_profile = profile_pda(&outsider.key());
    let ixs = [
        ed25519_ix(&server, &msg),
        env.submit_ix(&s, ResultKind::Forfeit, f.a.key(), demo),
    ];
    assert_code(
        env.send(&ixs, &[&f.b.kp]),
        code(AnchorError::ConstraintSeeds),
    );

    // A and B profiles swapped.
    let mut s = env.submit_accounts(&f);
    std::mem::swap(&mut s.player_a_profile, &mut s.player_b_profile);
    let ixs = [
        ed25519_ix(&server, &msg),
        env.submit_ix(&s, ResultKind::Forfeit, f.a.key(), demo),
    ];
    assert_code(
        env.send(&ixs, &[&f.b.kp]),
        code(AnchorError::ConstraintSeeds),
    );
    assert_eq!(env.profile(&outsider.key()).total_forfeits, 0);
}

// ======================= forfeit penalties =======================

#[test]
fn forfeit_ladder_on_chain() {
    let mut env = Env::new();
    let a = env.player(ENTRY * 20);
    let quitter = env.player(ENTRY * 20);

    // 1st forfeit: reputation hit only.
    play(&mut env, &a, &quitter, 1, ResultKind::Forfeit, a.key());
    let p = env.profile(&quitter.key());
    assert_eq!((p.reputation, p.banned_until, p.total_forfeits), (90, 0, 1));
    assert_eq!(p.losses, 1);
    assert_eq!(env.profile(&a.key()).elo, STARTING_ELO + 16);

    // 2nd within 30 days: 24-hour ban, which blocks new paid matches.
    env.advance(DAY);
    play(&mut env, &a, &quitter, 2, ResultKind::Forfeit, a.key());
    let now = env.now();
    let p = env.profile(&quitter.key());
    assert_eq!(
        (p.reputation, p.banned_until),
        (80, now + SECOND_FORFEIT_BAN_SECS)
    );
    assert_code(
        env.create_match(&quitter, 99, ENTRY),
        code(ArenaError::PlayerBanned),
    );
    env.create_match(&a, 3, ENTRY).unwrap();
    assert_code(
        env.join_match(&quitter, &match_pda(&a.key(), 3)),
        code(ArenaError::PlayerBanned),
    );

    // Ban lifts on time; 3rd forfeit in the window: 7-day ban.
    env.advance(SECOND_FORFEIT_BAN_SECS);
    env.join_match(&quitter, &match_pda(&a.key(), 3)).unwrap();
    let m3 = match_pda(&a.key(), 3);
    env.start_match(&m3).unwrap();
    let f3 = Fixture {
        a: clone(&a),
        b: clone(&quitter),
        m: m3,
    };
    env.submit_signed(&f3, ResultKind::Forfeit, a.key())
        .unwrap();
    let now = env.now();
    let p = env.profile(&quitter.key());
    assert_eq!(
        (p.reputation, p.banned_until, p.total_forfeits),
        (70, now + THIRD_FORFEIT_BAN_SECS, 3)
    );
    env.advance(THIRD_FORFEIT_BAN_SECS - 1);
    assert_code(
        env.create_match(&quitter, 98, ENTRY),
        code(ArenaError::PlayerBanned),
    );
    env.advance(1);
    env.create_match(&quitter, 98, ENTRY).unwrap();
}

#[test]
fn forfeit_outside_window_is_first_again() {
    let mut env = Env::new();
    let a = env.player(ENTRY * 10);
    let q = env.player(ENTRY * 10);
    play(&mut env, &a, &q, 1, ResultKind::Forfeit, a.key());
    env.advance(31 * DAY);
    play(&mut env, &a, &q, 2, ResultKind::Forfeit, a.key());
    let p = env.profile(&q.key());
    assert_eq!((p.banned_until, p.reputation, p.total_forfeits), (0, 80, 2));
}

#[test]
fn creator_can_be_the_forfeiter() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    env.submit_signed(&f, ResultKind::Forfeit, f.b.key())
        .unwrap();
    let (pa, pb) = (env.profile(&f.a.key()), env.profile(&f.b.key()));
    assert_eq!((pa.total_forfeits, pa.reputation, pa.losses), (1, 90, 1));
    assert_eq!((pb.total_forfeits, pb.wins), (0, 1));
}

#[test]
fn reputation_never_below_zero() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, true);
    let mut p = env.profile(&f.b.key());
    p.reputation = 3;
    env.write(&profile_pda(&f.b.key()), &p);
    env.submit_signed(&f, ResultKind::Forfeit, f.a.key())
        .unwrap();
    assert_eq!(env.profile(&f.b.key()).reputation, 0);
}

// ======================= fees =======================

fn collect_fees(env: &mut Env) -> u64 {
    let f = env.fixture(ENTRY, true);
    env.submit_signed(&f, ResultKind::Win, f.a.key()).unwrap();
    env.balance(&fee_vault_pda())
}

#[test]
fn admin_withdraws_fees_to_treasury() {
    let mut env = Env::new();
    let fees = collect_fees(&mut env);
    assert_eq!(fees, ENTRY * 2 * 500 / 10_000);
    let treasury = env.treasury.pubkey();
    let t_token = env.usdc(&treasury, 0);
    let admin = env.admin.insecure_clone();

    let i = env.withdraw_fees_ix(&admin.pubkey(), &t_token, fees - 1);
    env.send(&[i], &[&admin]).unwrap();
    assert_eq!(env.balance(&t_token), fees - 1);
    assert_eq!(env.balance(&fee_vault_pda()), 1);

    let i = env.withdraw_fees_ix(&admin.pubkey(), &t_token, 1);
    env.send(&[i], &[&admin]).unwrap();
    assert_eq!(env.balance(&fee_vault_pda()), 0);
}

#[test]
fn withdraw_rejects_bad_amounts() {
    let mut env = Env::new();
    let fees = collect_fees(&mut env);
    let treasury = env.treasury.pubkey();
    let t_token = env.usdc(&treasury, 0);
    let admin = env.admin.insecure_clone();
    for amount in [0, fees + 1, u64::MAX] {
        let i = env.withdraw_fees_ix(&admin.pubkey(), &t_token, amount);
        assert_code(env.send(&[i], &[&admin]), code(ArenaError::InvalidAmount));
    }
}

#[test]
fn withdraw_rejects_non_admin_and_wrong_destination() {
    let mut env = Env::new();
    let fees = collect_fees(&mut env);
    let attacker = env.wallet();
    let attacker_token = env.usdc(&attacker.pubkey(), 0);
    let treasury = env.treasury.pubkey();
    let t_token = env.usdc(&treasury, 0);

    // Non-admin, even paying to the real treasury.
    let i = env.withdraw_fees_ix(&attacker.pubkey(), &t_token, fees);
    assert_code(env.send(&[i], &[&attacker]), code(ArenaError::Unauthorized));

    // Admin, but to an account the treasury doesn't own.
    let admin = env.admin.insecure_clone();
    let i = env.withdraw_fees_ix(&admin.pubkey(), &attacker_token, fees);
    assert_code(
        env.send(&[i], &[&admin]),
        code(AnchorError::ConstraintTokenOwner),
    );

    // Treasury-owned account of another mint.
    let other_mint = env.create_mint();
    let t_other = env.token_account(&other_mint, &treasury, 0);
    let i = env.withdraw_fees_ix(&admin.pubkey(), &t_other, fees);
    assert_code(
        env.send(&[i], &[&admin]),
        code(AnchorError::ConstraintTokenMint),
    );

    // A different token account standing in for the fee vault.
    let fake_vault = env.usdc(&config_pda(), fees);
    let mut i = env.withdraw_fees_ix(&admin.pubkey(), &t_token, fees);
    i.accounts[3].pubkey = fake_vault;
    assert_code(
        env.send(&[i], &[&admin]),
        code(AnchorError::ConstraintHasOne),
    );

    assert_eq!(env.balance(&fee_vault_pda()), fees);
}

#[test]
fn match_vault_cannot_be_drained_as_fee_vault() {
    // The withdrawal signs only as the config PDA; a match vault (owned by the match PDA)
    // can't be passed in its place.
    let mut env = Env::new();
    collect_fees(&mut env);
    let f = env.fixture(HOLD_THRESHOLD, true);
    let treasury = env.treasury.pubkey();
    let t_token = env.usdc(&treasury, 0);
    let admin = env.admin.insecure_clone();
    let mut i = env.withdraw_fees_ix(&admin.pubkey(), &t_token, 1);
    i.accounts[3].pubkey = vault_pda(&f.m);
    assert_failed(env.send(&[i], &[&admin]));
    assert_eq!(env.balance(&vault_pda(&f.m)), HOLD_THRESHOLD * 2);
}

#[test]
fn treasury_rotation_admin_only() {
    let mut env = Env::new();
    let fees = collect_fees(&mut env);
    let attacker = env.wallet();
    assert_code(
        env.set_treasury_as(&attacker, attacker.pubkey()),
        code(ArenaError::Unauthorized),
    );
    let admin = env.admin.insecure_clone();
    assert_code(
        env.set_treasury_as(&admin, Pubkey::default()),
        code(ArenaError::InvalidConfigParam),
    );

    let new_treasury = Keypair::new();
    env.set_treasury_as(&admin, new_treasury.pubkey()).unwrap();
    let c: GlobalConfig = env.config();
    assert_eq!(c.treasury, new_treasury.pubkey());

    // Old treasury no longer accepted; new one is.
    let old_token = {
        let t = env.treasury.pubkey();
        env.usdc(&t, 0)
    };
    let i = env.withdraw_fees_ix(&admin.pubkey(), &old_token, fees);
    assert_code(
        env.send(&[i], &[&admin]),
        code(AnchorError::ConstraintTokenOwner),
    );
    let new_token = env.usdc(&new_treasury.pubkey(), 0);
    let i = env.withdraw_fees_ix(&admin.pubkey(), &new_token, fees);
    env.send(&[i], &[&admin]).unwrap();
    assert_eq!(env.balance(&new_token), fees);
}

#[test]
fn only_withdraw_fees_signs_as_config() {
    // The fee vault's authority is the config PDA; only one instruction may sign with it.
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut signers = Vec::new();
    for entry in walk(std::path::Path::new(src)) {
        let text = std::fs::read_to_string(&entry).unwrap();
        if text.contains("[CONFIG_SEED, &bump]") || text.contains("CONFIG_SEED, &[") {
            signers.push(entry.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert_eq!(signers, vec!["admin_withdraw_fees.rs".to_string()]);
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out
}

#[test]
fn initialize_requires_treasury() {
    let mut env = Env::bare();
    let admin = env.admin.insecure_clone();
    let mut i = env.init_config_ix(&admin.pubkey(), default_params());
    // Instruction data: 8-byte discriminator, admin(32), dispute(32), treasury(32), params.
    i.data[8 + 64..8 + 96].copy_from_slice(&[0u8; 32]);
    assert_code(
        env.send(&[i], &[&admin]),
        code(ArenaError::InvalidConfigParam),
    );
}
