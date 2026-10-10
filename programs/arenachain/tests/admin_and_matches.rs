mod common;

use {
    anchor_lang::error::ErrorCode as AnchorError,
    arenachain::{
        errors::ArenaError,
        state::{ConfigParams, GameServer, MatchStatus, PlayerProfile},
        STARTING_ELO,
    },
    common::*,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

// ---------- config ----------

#[test]
fn initialize_config_happy_path() {
    let env = Env::new();
    let c = env.config();
    assert_eq!(c.admin, env.admin.pubkey());
    assert_eq!(c.usdc_mint, env.mint);
    assert_eq!(c.fee_vault, fee_vault_pda());
    assert_eq!(c.hold_threshold, HOLD_THRESHOLD);
    assert!(!c.paused);
    assert_eq!(env.balance(&fee_vault_pda()), 0);
}

#[test]
fn initialize_config_rejects_non_upgrade_authority() {
    let mut env = Env::bare();
    let attacker = env.wallet();
    let res = env.init_config(&attacker, default_params());
    assert_code(res, code(ArenaError::Unauthorized));
}

#[test]
fn initialize_config_only_once() {
    let mut env = Env::new();
    let admin = env.admin.insecure_clone();
    assert_failed(env.init_config(&admin, default_params()));
}

#[test]
fn initialize_config_validates_params() {
    let mut env = Env::bare();
    let admin = env.admin.insecure_clone();
    let bad = ConfigParams {
        result_window_secs: 0,
        ..default_params()
    };
    assert_code(
        env.init_config(&admin, bad),
        code(ArenaError::InvalidConfigParam),
    );
}

#[test]
fn admin_instructions_reject_wrong_signer() {
    let mut env = Env::new();
    let attacker = env.wallet();
    assert_code(
        env.set_paused_as(&attacker, true),
        code(ArenaError::Unauthorized),
    );
    assert_code(
        env.update_config_as(&attacker, default_params()),
        code(ArenaError::Unauthorized),
    );
    let rogue = Keypair::new();
    assert_code(
        env.register_server_as(&attacker, &rogue),
        code(ArenaError::Unauthorized),
    );
}

#[test]
fn admin_update_config_and_pause() {
    let mut env = Env::new();
    let admin = env.admin.insecure_clone();
    let p = ConfigParams {
        hold_threshold: 5,
        hold_secs: 10,
        result_window_secs: 600,
    };
    env.update_config_as(&admin, p).unwrap();
    assert_eq!(env.config().hold_threshold, 5);
    env.set_paused(true).unwrap();
    assert!(env.config().paused);
}

#[test]
fn register_and_deactivate_server() {
    let mut env = Env::new();
    let s: GameServer = env.read(&server_pda(&env.server.pubkey()));
    assert!(s.active);
    assert_eq!(s.signer_key, env.server.pubkey());
    let key = env.server.pubkey();
    env.set_server_active(&key, false).unwrap();
    let s: GameServer = env.read(&server_pda(&key));
    assert!(!s.active);
}

// ---------- profiles ----------

#[test]
fn create_profile_defaults_and_once() {
    let mut env = Env::new();
    let w = env.wallet();
    env.create_profile(&w).unwrap();
    let p: PlayerProfile = env.read(&profile_pda(&w.pubkey()));
    assert_eq!(p.authority, w.pubkey());
    assert_eq!(p.elo, STARTING_ELO);
    assert_eq!((p.wins, p.losses, p.reputation, p.steam_id), (0, 0, 100, 0));
    assert_eq!((p.total_forfeits, p.recent_forfeits), (0, [0, 0]));
    assert_eq!(p.created_at, START_TS);
    assert_failed(env.create_profile(&w));
}

// ---------- create / join ----------

#[test]
fn create_and_join_happy_path() {
    let mut env = Env::new();
    let a = env.player(ENTRY * 3);
    let b = env.player(ENTRY * 3);
    env.create_match(&a, 1, ENTRY).unwrap();
    let m = match_pda(&a.key(), 1);
    let created = env.get_match(&m);
    assert_eq!(created.status, MatchStatus::Open);
    assert_eq!(created.player_a, a.key());
    assert_eq!(created.game_server, server_pda(&env.server.pubkey()));
    assert_eq!(created.created_slot, env.slot());
    assert_eq!(env.balance(&vault_pda(&m)), ENTRY);
    assert_eq!(env.balance(&a.token), ENTRY * 2);

    env.join_match(&b, &m).unwrap();
    let joined = env.get_match(&m);
    assert_eq!(joined.status, MatchStatus::Full);
    assert_eq!(joined.player_b, b.key());
    assert_eq!(joined.start_deadline, START_TS + RESULT_WINDOW_SECS);
    assert_eq!(joined.result_deadline, 0);
    assert_eq!(env.balance(&vault_pda(&m)), ENTRY * 2);
    assert_eq!(env.balance(&b.token), ENTRY * 2);
}

#[test]
fn create_requires_profile() {
    let mut env = Env::new();
    let w = env.wallet();
    let t = env.usdc(&w.pubkey(), ENTRY);
    let (server, mint) = (env.server.pubkey(), env.mint);
    let i = env.create_match_ix(&w.pubkey(), &t, 1, ENTRY, &server, &mint);
    assert_code(
        env.send(&[i], &[&w]),
        code(AnchorError::AccountNotInitialized),
    );
}

#[test]
fn create_rejects_zero_fee_paused_banned_inactive_server() {
    let mut env = Env::new();
    let a = env.player(ENTRY * 3);
    assert_code(env.create_match(&a, 1, 0), code(ArenaError::ZeroEntryFee));

    env.set_paused(true).unwrap();
    assert_code(env.create_match(&a, 1, ENTRY), code(ArenaError::Paused));
    env.set_paused(false).unwrap();

    env.ban(&a.key(), START_TS + 1);
    assert_code(
        env.create_match(&a, 1, ENTRY),
        code(ArenaError::PlayerBanned),
    );
    env.advance(1); // ban ends exactly now
    env.create_match(&a, 1, ENTRY).unwrap();

    let key = env.server.pubkey();
    env.set_server_active(&key, false).unwrap();
    assert_code(
        env.create_match(&a, 2, ENTRY),
        code(ArenaError::ServerInactive),
    );
}

#[test]
fn create_rejects_unregistered_server() {
    let mut env = Env::new();
    let a = env.player(ENTRY);
    let rogue = Keypair::new();
    let mint = env.mint;
    let i = env.create_match_ix(&a.key(), &a.token, 1, ENTRY, &rogue.pubkey(), &mint);
    assert_code(
        env.send(&[i], &[&a.kp]),
        code(AnchorError::AccountNotInitialized),
    );
}

#[test]
fn create_rejects_wrong_mint() {
    let mut env = Env::new();
    let a = env.player(ENTRY);
    let fake_mint = env.create_mint();
    let fake_token = env.token_account(&fake_mint, &a.key(), ENTRY);
    let server = env.server.pubkey();
    let i = env.create_match_ix(&a.key(), &fake_token, 1, ENTRY, &server, &fake_mint);
    assert_code(
        env.send(&[i], &[&a.kp]),
        code(AnchorError::ConstraintAddress),
    );
}

#[test]
fn create_rejects_someone_elses_token_account() {
    let mut env = Env::new();
    let a = env.player(ENTRY);
    let victim = env.player(ENTRY);
    let (server, mint) = (env.server.pubkey(), env.mint);
    let i = env.create_match_ix(&a.key(), &victim.token, 1, ENTRY, &server, &mint);
    assert_code(
        env.send(&[i], &[&a.kp]),
        code(AnchorError::ConstraintTokenOwner),
    );
}

#[test]
fn create_rejects_pool_overflow() {
    let mut env = Env::new();
    let a = env.player(u64::MAX);
    assert_code(
        env.create_match(&a, 1, u64::MAX / 2 + 1),
        code(ArenaError::MathOverflow),
    );
    // Largest representable pool is accepted.
    env.create_match(&a, 2, u64::MAX / 2).unwrap();
}

#[test]
fn create_insufficient_funds_fails() {
    let mut env = Env::new();
    let a = env.player(ENTRY - 1);
    assert_failed(env.create_match(&a, 1, ENTRY));
}

#[test]
fn join_rejects_own_match_full_match_paused_banned() {
    let mut env = Env::new();
    let a = env.player(ENTRY * 3);
    let b = env.player(ENTRY * 3);
    let c = env.player(ENTRY * 3);
    env.create_match(&a, 1, ENTRY).unwrap();
    let m = match_pda(&a.key(), 1);

    assert_code(env.join_match(&a, &m), code(ArenaError::CannotJoinOwnMatch));

    env.set_paused(true).unwrap();
    assert_code(env.join_match(&b, &m), code(ArenaError::Paused));
    env.set_paused(false).unwrap();

    env.ban(&b.key(), START_TS + 100);
    assert_code(env.join_match(&b, &m), code(ArenaError::PlayerBanned));

    env.join_match(&c, &m).unwrap();
    assert_code(env.join_match(&b, &m), code(ArenaError::InvalidMatchStatus));
}

#[test]
fn join_rejects_wrong_vault() {
    let mut env = Env::new();
    let a = env.player(ENTRY * 3);
    let b = env.player(ENTRY * 3);
    env.create_match(&a, 1, ENTRY).unwrap();
    env.create_match(&a, 2, ENTRY).unwrap();
    let m1 = match_pda(&a.key(), 1);
    let m2 = match_pda(&a.key(), 2);
    let mut i = env.join_match_ix(&b.key(), &b.token, &m1);
    // Point the vault at match 2's vault.
    i.accounts[6].pubkey = vault_pda(&m2);
    assert_code(env.send(&[i], &[&b.kp]), code(AnchorError::ConstraintSeeds));
}

// ---------- cancel ----------

#[test]
fn cancel_open_match_refunds_and_closes_vault() {
    let mut env = Env::new();
    let a = env.player(ENTRY);
    env.create_match(&a, 1, ENTRY).unwrap();
    let m = match_pda(&a.key(), 1);
    let i = env.cancel_match_ix(&a.key(), &a.token, &m);
    env.send(&[i], &[&a.kp]).unwrap();
    assert_eq!(env.balance(&a.token), ENTRY);
    assert!(!env.exists(&vault_pda(&m)));
    assert_eq!(env.get_match(&m).status, MatchStatus::Refunded);

    // Refund exactly once (the closed vault or the status check stops a second one).
    let i = env.cancel_match_ix(&a.key(), &a.token, &m);
    assert_failed(env.send(&[i], &[&a.kp]));
    assert_eq!(env.balance(&a.token), ENTRY);
}

#[test]
fn cancel_works_while_paused() {
    let mut env = Env::new();
    let a = env.player(ENTRY);
    env.create_match(&a, 1, ENTRY).unwrap();
    env.set_paused(true).unwrap();
    let m = match_pda(&a.key(), 1);
    let i = env.cancel_match_ix(&a.key(), &a.token, &m);
    env.send(&[i], &[&a.kp]).unwrap();
    assert_eq!(env.balance(&a.token), ENTRY);
}

#[test]
fn cancel_rejects_non_creator_and_joined_match() {
    let mut env = Env::new();
    let a = env.player(ENTRY);
    let b = env.player(ENTRY);
    env.create_match(&a, 1, ENTRY).unwrap();
    let m = match_pda(&a.key(), 1);

    let i = env.cancel_match_ix(&b.key(), &b.token, &m);
    assert_code(env.send(&[i], &[&b.kp]), code(ArenaError::Unauthorized));

    env.join_match(&b, &m).unwrap();
    let i = env.cancel_match_ix(&a.key(), &a.token, &m);
    assert_code(
        env.send(&[i], &[&a.kp]),
        code(ArenaError::InvalidMatchStatus),
    );
}

#[test]
fn cancel_with_donated_dust_still_refunds() {
    let mut env = Env::new();
    let a = env.player(ENTRY);
    env.create_match(&a, 1, ENTRY).unwrap();
    let m = match_pda(&a.key(), 1);
    // Griefer sends 1 unit to the vault: the refund must still go through.
    let vault = vault_pda(&m);
    let mut acc = env.svm.get_account(&vault).unwrap();
    let mut state =
        <spl_token_interface::state::Account as solana_program_pack::Pack>::unpack(&acc.data)
            .unwrap();
    state.amount += 1;
    solana_program_pack::Pack::pack(state, &mut acc.data).unwrap();
    env.svm.set_account(vault, acc).unwrap();

    let i = env.cancel_match_ix(&a.key(), &a.token, &m);
    env.send(&[i], &[&a.kp]).unwrap();
    assert_eq!(env.balance(&a.token), ENTRY);
    assert_eq!(env.balance(&vault), 1);
}

// ---------- start ----------

#[test]
fn start_happy_path() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, false);
    env.start_match(&f.m).unwrap();
    let m = env.get_match(&f.m);
    assert_eq!(m.status, MatchStatus::InProgress);
    assert_eq!(m.started_at, START_TS);
}

#[test]
fn start_rejects_wrong_server_open_match_and_restart() {
    let mut env = Env::new();
    let other = Keypair::new();
    env.register_server(&other).unwrap();
    let f = env.fixture(ENTRY, false);
    assert_code(
        env.start_match_as(&other, &f.m),
        code(ArenaError::Unauthorized),
    );

    // Unregistered key.
    let rogue = Keypair::new();
    assert_code(
        env.start_match_as(&rogue, &f.m),
        code(AnchorError::AccountNotInitialized),
    );

    env.start_match(&f.m).unwrap();
    assert_code(env.start_match(&f.m), code(ArenaError::InvalidMatchStatus));

    let a = env.player(ENTRY);
    env.create_match(&a, 9, ENTRY).unwrap();
    let open = match_pda(&a.key(), 9);
    assert_code(env.start_match(&open), code(ArenaError::InvalidMatchStatus));
}

#[test]
fn start_rejects_inactive_server_and_after_deadline() {
    let mut env = Env::new();
    let f = env.fixture(ENTRY, false);
    let key = env.server.pubkey();
    env.set_server_active(&key, false).unwrap();
    assert_code(env.start_match(&f.m), code(ArenaError::ServerInactive));
    env.set_server_active(&key, true).unwrap();
    env.advance(RESULT_WINDOW_SECS + 1);
    assert_code(
        env.start_match(&f.m),
        code(ArenaError::ResultDeadlinePassed),
    );
}

#[test]
fn match_seeds_bind_creator() {
    // Matches are namespaced per creator: the same nonce for two creators gives two matches.
    let mut env = Env::new();
    let a = env.player(ENTRY);
    let b = env.player(ENTRY);
    env.create_match(&a, 1, ENTRY).unwrap();
    env.create_match(&b, 1, ENTRY).unwrap();
    assert_ne!(match_pda(&a.key(), 1), match_pda(&b.key(), 1));
    assert_failed(env.create_match(&a, 1, ENTRY));
}
