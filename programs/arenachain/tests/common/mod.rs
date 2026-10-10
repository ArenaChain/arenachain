#![allow(dead_code, clippy::result_large_err)]

use {
    anchor_lang::{
        prelude::{Clock, Pubkey},
        solana_program::{bpf_loader_upgradeable, instruction::Instruction, system_program},
        AccountDeserialize, AccountSerialize, InstructionData, ToAccountMetas,
    },
    arenachain::{
        ed25519::build_result_message,
        state::{ConfigParams, GlobalConfig, Match, PlayerProfile, ResultKind},
        CONFIG_SEED, FEE_VAULT_SEED, MATCH_SEED, PROFILE_SEED, SERVER_SEED, VAULT_SEED,
    },
    litesvm::{
        types::{FailedTransactionMetadata, TransactionResult},
        LiteSVM,
    },
    solana_account::Account,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_program_option::COption,
    solana_program_pack::Pack,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    spl_token_interface::state::{Account as SplAccount, AccountState, Mint as SplMint},
};

pub const DECIMALS: u8 = 6;
pub const ENTRY: u64 = 10_000_000; // 10 USDC
pub const HOLD_THRESHOLD: u64 = 100_000_000; // pools of 100 USDC or more are held
pub const HOLD_SECS: i64 = 72 * 3600;
pub const RESULT_WINDOW_SECS: i64 = 3 * 3600;
pub const START_TS: i64 = 1_800_000_000;

pub fn token_program() -> Pubkey {
    spl_token_interface::ID
}

pub fn pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &arenachain::id()).0
}

pub fn config_pda() -> Pubkey {
    pda(&[CONFIG_SEED])
}
pub fn fee_vault_pda() -> Pubkey {
    pda(&[FEE_VAULT_SEED])
}
pub fn server_pda(signer: &Pubkey) -> Pubkey {
    pda(&[SERVER_SEED, signer.as_ref()])
}
pub fn profile_pda(wallet: &Pubkey) -> Pubkey {
    pda(&[PROFILE_SEED, wallet.as_ref()])
}
pub fn match_pda(creator: &Pubkey, nonce: u64) -> Pubkey {
    pda(&[MATCH_SEED, creator.as_ref(), &nonce.to_le_bytes()])
}
pub fn vault_pda(arena_match: &Pubkey) -> Pubkey {
    pda(&[VAULT_SEED, arena_match.as_ref()])
}
pub fn program_data_address() -> Pubkey {
    Pubkey::find_program_address(&[arenachain::id().as_ref()], &bpf_loader_upgradeable::ID).0
}

pub fn ix<D: InstructionData, A: ToAccountMetas>(data: D, accounts: A) -> Instruction {
    Instruction::new_with_bytes(
        arenachain::id(),
        &data.data(),
        accounts.to_account_metas(None),
    )
}

pub fn default_params() -> ConfigParams {
    ConfigParams {
        hold_threshold: HOLD_THRESHOLD,
        hold_secs: HOLD_SECS,
        result_window_secs: RESULT_WINDOW_SECS,
    }
}

/// Asserts a transaction failed with the given custom error code
/// (`ArenaError::X.into()` or `anchor_lang::error::ErrorCode::X.into()`).
pub fn assert_code(res: TransactionResult, code: u32) {
    let err = res.expect_err("transaction should have failed");
    let rendered = format!("{:?}", err.err);
    assert!(
        rendered.contains(&format!("Custom({code})")),
        "expected Custom({code}), got {rendered}\nlogs: {:#?}",
        err.meta.logs
    );
}

pub fn assert_failed(res: TransactionResult) -> FailedTransactionMetadata {
    res.expect_err("transaction should have failed")
}

pub fn code<E: Into<u32>>(e: E) -> u32 {
    e.into()
}

/// Ed25519 precompile instruction with one signature and every offset pointing into its
/// own data. `index_override` replaces the three instruction-index fields when set.
pub fn ed25519_ix_raw(sigs: &[(&Keypair, &[u8])], index_override: Option<u16>) -> Instruction {
    let n = sigs.len();
    let header = 2 + 14 * n;
    let mut data = vec![n as u8, 0];
    let mut body = Vec::new();
    for (signer, msg) in sigs {
        let pk_off = header + body.len();
        body.extend_from_slice(signer.pubkey().as_ref());
        let sig_off = header + body.len();
        body.extend_from_slice(signer.sign_message(msg).as_ref());
        let msg_off = header + body.len();
        body.extend_from_slice(msg);
        let idx = index_override.unwrap_or(u16::MAX);
        for v in [
            sig_off as u16,
            idx,
            pk_off as u16,
            idx,
            msg_off as u16,
            msg.len() as u16,
            idx,
        ] {
            data.extend_from_slice(&v.to_le_bytes());
        }
    }
    data.extend_from_slice(&body);
    Instruction::new_with_bytes(solana_sdk_ids::ed25519_program::ID, &data, vec![])
}

pub fn ed25519_ix(signer: &Keypair, msg: &[u8]) -> Instruction {
    ed25519_ix_raw(&[(signer, msg)], None)
}

pub struct Env {
    pub svm: LiteSVM,
    pub admin: Keypair,
    pub mint: Pubkey,
    pub server: Keypair,
    pub treasury: Keypair,
}

impl Env {
    /// Program loaded, config initialized, one active server registered.
    pub fn new() -> Self {
        let mut env = Self::bare();
        env.init_config(&env.admin.insecure_clone(), default_params())
            .expect("init config");
        let server = env.server.insecure_clone();
        env.register_server(&server).expect("register server");
        env
    }

    /// Program loaded with `admin` as upgrade authority; nothing initialized.
    pub fn bare() -> Self {
        let mut svm = LiteSVM::new();
        let bytes = include_bytes!(concat!(
            env!("CARGO_TARGET_TMPDIR"),
            "/../deploy/arenachain.so"
        ));
        svm.add_program(arenachain::id(), bytes).unwrap();
        let admin = Keypair::new();
        svm.airdrop(&admin.pubkey(), 100_000_000_000).unwrap();

        // LiteSVM loads programs with no upgrade authority; make `admin` the authority.
        let pd = program_data_address();
        let mut acc = svm.get_account(&pd).unwrap();
        acc.data[0..4].copy_from_slice(&3u32.to_le_bytes());
        acc.data[12] = 1;
        acc.data[13..45].copy_from_slice(admin.pubkey().as_ref());
        svm.set_account(pd, acc).unwrap();

        let mut env = Self {
            svm,
            admin,
            mint: Pubkey::default(),
            server: Keypair::new(),
            treasury: Keypair::new(),
        };
        env.set_time(START_TS);
        env.mint = env.create_mint();
        env
    }

    // ---------- clock ----------

    pub fn now(&self) -> i64 {
        self.svm.get_sysvar::<Clock>().unix_timestamp
    }

    pub fn set_time(&mut self, ts: i64) {
        let mut clock = self.svm.get_sysvar::<Clock>();
        clock.unix_timestamp = ts;
        clock.slot += 10;
        self.svm.set_sysvar(&clock);
    }

    pub fn advance(&mut self, secs: i64) {
        let now = self.now();
        self.set_time(now + secs);
    }

    pub fn slot(&self) -> u64 {
        self.svm.get_sysvar::<Clock>().slot
    }

    // ---------- tokens ----------

    pub fn create_mint(&mut self) -> Pubkey {
        self.create_mint_with(DECIMALS, None)
    }

    pub fn create_mint_with(&mut self, decimals: u8, freeze_authority: Option<Pubkey>) -> Pubkey {
        let key = Keypair::new().pubkey();
        let mut data = vec![0u8; SplMint::LEN];
        SplMint::pack(
            SplMint {
                mint_authority: COption::Some(Pubkey::new_unique()),
                supply: u64::MAX,
                decimals,
                is_initialized: true,
                freeze_authority: freeze_authority.map_or(COption::None, COption::Some),
            },
            &mut data,
        )
        .unwrap();
        self.put(key, data, token_program());
        key
    }

    pub fn token_account(&mut self, mint: &Pubkey, owner: &Pubkey, amount: u64) -> Pubkey {
        let key = Keypair::new().pubkey();
        let mut data = vec![0u8; SplAccount::LEN];
        SplAccount::pack(
            SplAccount {
                mint: *mint,
                owner: *owner,
                amount,
                delegate: COption::None,
                state: AccountState::Initialized,
                is_native: COption::None,
                delegated_amount: 0,
                close_authority: COption::None,
            },
            &mut data,
        )
        .unwrap();
        self.put(key, data, token_program());
        key
    }

    pub fn usdc(&mut self, owner: &Pubkey, amount: u64) -> Pubkey {
        let mint = self.mint;
        self.token_account(&mint, owner, amount)
    }

    pub fn balance(&self, token_account: &Pubkey) -> u64 {
        let acc = self.svm.get_account(token_account).expect("token account");
        SplAccount::unpack(&acc.data).unwrap().amount
    }

    pub fn exists(&self, key: &Pubkey) -> bool {
        self.svm.get_account(key).is_some_and(|a| a.lamports > 0)
    }

    fn put(&mut self, key: Pubkey, data: Vec<u8>, owner: Pubkey) {
        let lamports = self.svm.minimum_balance_for_rent_exemption(data.len());
        self.svm
            .set_account(
                key,
                Account {
                    lamports,
                    data,
                    owner,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .unwrap();
    }

    // ---------- accounts ----------

    pub fn read<T: AccountDeserialize>(&self, key: &Pubkey) -> T {
        let acc = self.svm.get_account(key).expect("account");
        T::try_deserialize(&mut acc.data.as_slice()).unwrap()
    }

    pub fn write<T: AccountSerialize>(&mut self, key: &Pubkey, value: &T) {
        let mut acc = self.svm.get_account(key).expect("account");
        let mut buf = Vec::new();
        value.try_serialize(&mut buf).unwrap();
        acc.data[..buf.len()].copy_from_slice(&buf);
        self.svm.set_account(*key, acc).unwrap();
    }

    pub fn get_match(&self, key: &Pubkey) -> Match {
        self.read(key)
    }

    pub fn config(&self) -> GlobalConfig {
        self.read(&config_pda())
    }

    pub fn wallet(&mut self) -> Keypair {
        let kp = Keypair::new();
        self.svm.airdrop(&kp.pubkey(), 10_000_000_000).unwrap();
        kp
    }

    // ---------- send ----------

    pub fn send(&mut self, ixs: &[Instruction], signers: &[&Keypair]) -> TransactionResult {
        self.svm.expire_blockhash();
        let blockhash = self.svm.latest_blockhash();
        let msg = Message::new_with_blockhash(ixs, Some(&signers[0].pubkey()), &blockhash);
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
        self.svm.send_transaction(tx)
    }

    // ---------- instructions ----------

    pub fn init_config_ix(&self, authority: &Pubkey, params: ConfigParams) -> Instruction {
        ix(
            arenachain::instruction::AdminInitializeConfig {
                admin: self.admin.pubkey(),
                dispute_authority: self.admin.pubkey(),
                treasury: self.treasury.pubkey(),
                params,
            },
            arenachain::accounts::AdminInitializeConfig {
                authority: *authority,
                config: config_pda(),
                usdc_mint: self.mint,
                fee_vault: fee_vault_pda(),
                program: arenachain::id(),
                program_data: program_data_address(),
                token_program: token_program(),
                system_program: system_program::ID,
            },
        )
    }

    pub fn init_config(&mut self, authority: &Keypair, params: ConfigParams) -> TransactionResult {
        let i = self.init_config_ix(&authority.pubkey(), params);
        self.send(&[i], &[authority])
    }

    pub fn register_server_as(&mut self, admin: &Keypair, server: &Keypair) -> TransactionResult {
        let i = ix(
            arenachain::instruction::AdminRegisterServer {
                signer_key: server.pubkey(),
                operator: [7; 32],
            },
            arenachain::accounts::AdminRegisterServer {
                admin: admin.pubkey(),
                config: config_pda(),
                game_server: server_pda(&server.pubkey()),
                system_program: system_program::ID,
            },
        );
        self.send(&[i], &[admin])
    }

    pub fn register_server(&mut self, server: &Keypair) -> TransactionResult {
        let admin = self.admin.insecure_clone();
        self.register_server_as(&admin, server)
    }

    pub fn set_server_active(&mut self, server: &Pubkey, active: bool) -> TransactionResult {
        let admin = self.admin.insecure_clone();
        let i = ix(
            arenachain::instruction::AdminSetServerActive { active },
            arenachain::accounts::AdminSetServerActive {
                admin: admin.pubkey(),
                config: config_pda(),
                game_server: server_pda(server),
            },
        );
        self.send(&[i], &[&admin])
    }

    pub fn set_paused_as(&mut self, admin: &Keypair, paused: bool) -> TransactionResult {
        let i = ix(
            arenachain::instruction::AdminSetPaused { paused },
            arenachain::accounts::AdminUpdateConfig {
                admin: admin.pubkey(),
                config: config_pda(),
            },
        );
        self.send(&[i], &[admin])
    }

    pub fn set_paused(&mut self, paused: bool) -> TransactionResult {
        let admin = self.admin.insecure_clone();
        self.set_paused_as(&admin, paused)
    }

    pub fn update_config_as(&mut self, admin: &Keypair, params: ConfigParams) -> TransactionResult {
        let i = ix(
            arenachain::instruction::AdminUpdateConfig { params },
            arenachain::accounts::AdminUpdateConfig {
                admin: admin.pubkey(),
                config: config_pda(),
            },
        );
        self.send(&[i], &[admin])
    }

    pub fn create_profile(&mut self, wallet: &Keypair) -> TransactionResult {
        let i = ix(
            arenachain::instruction::PlayerCreateProfile {},
            arenachain::accounts::PlayerCreateProfile {
                authority: wallet.pubkey(),
                profile: profile_pda(&wallet.pubkey()),
                system_program: system_program::ID,
            },
        );
        self.send(&[i], &[wallet])
    }

    pub fn ban(&mut self, wallet: &Pubkey, until: i64) {
        let key = profile_pda(wallet);
        let mut p: PlayerProfile = self.read(&key);
        p.banned_until = until;
        self.write(&key, &p);
    }

    /// A funded player with a profile and a USDC token account.
    pub fn player(&mut self, usdc: u64) -> Player {
        let kp = self.wallet();
        self.create_profile(&kp).expect("create profile");
        let token = self.usdc(&kp.pubkey(), usdc);
        Player { kp, token }
    }

    pub fn create_match_ix(
        &self,
        creator: &Pubkey,
        creator_token: &Pubkey,
        nonce: u64,
        entry_fee: u64,
        server: &Pubkey,
        mint: &Pubkey,
    ) -> Instruction {
        let m = match_pda(creator, nonce);
        ix(
            arenachain::instruction::PlayerCreateMatch { nonce, entry_fee },
            arenachain::accounts::PlayerCreateMatch {
                creator: *creator,
                config: config_pda(),
                creator_profile: profile_pda(creator),
                game_server: server_pda(server),
                arena_match: m,
                mint: *mint,
                vault: vault_pda(&m),
                creator_token: *creator_token,
                token_program: token_program(),
                system_program: system_program::ID,
            },
        )
    }

    pub fn create_match(&mut self, p: &Player, nonce: u64, entry_fee: u64) -> TransactionResult {
        let (server, mint) = (self.server.pubkey(), self.mint);
        let i = self.create_match_ix(&p.kp.pubkey(), &p.token, nonce, entry_fee, &server, &mint);
        self.send(&[i], &[&p.kp])
    }

    pub fn join_match_ix(&self, joiner: &Pubkey, joiner_token: &Pubkey, m: &Pubkey) -> Instruction {
        ix(
            arenachain::instruction::PlayerJoinMatch {},
            arenachain::accounts::PlayerJoinMatch {
                joiner: *joiner,
                config: config_pda(),
                joiner_profile: profile_pda(joiner),
                arena_match: *m,
                mint: self.mint,
                vault: vault_pda(m),
                joiner_token: *joiner_token,
                token_program: token_program(),
            },
        )
    }

    pub fn join_match(&mut self, p: &Player, m: &Pubkey) -> TransactionResult {
        let i = self.join_match_ix(&p.kp.pubkey(), &p.token, m);
        self.send(&[i], &[&p.kp])
    }

    pub fn cancel_match_ix(
        &self,
        player_a: &Pubkey,
        player_a_token: &Pubkey,
        m: &Pubkey,
    ) -> Instruction {
        ix(
            arenachain::instruction::PlayerCancelMatch {},
            arenachain::accounts::PlayerCancelMatch {
                player_a: *player_a,
                arena_match: *m,
                mint: self.mint,
                vault: vault_pda(m),
                player_a_token: *player_a_token,
                token_program: token_program(),
            },
        )
    }

    pub fn start_match_as(&mut self, server: &Keypair, m: &Pubkey) -> TransactionResult {
        let i = ix(
            arenachain::instruction::ServerStartMatch {},
            arenachain::accounts::ServerStartMatch {
                server_signer: server.pubkey(),
                config: config_pda(),
                game_server: server_pda(&server.pubkey()),
                arena_match: *m,
            },
        );
        let payer = self.wallet();
        self.send(&[i], &[&payer, server])
    }

    pub fn start_match(&mut self, m: &Pubkey) -> TransactionResult {
        let server = self.server.insecure_clone();
        self.start_match_as(&server, m)
    }

    pub fn result_message(
        &self,
        m: &Pubkey,
        kind: ResultKind,
        winner: &Pubkey,
        demo: &[u8; 32],
    ) -> Vec<u8> {
        let created_slot = self.get_match(m).created_slot;
        build_result_message(&arenachain::id(), m, created_slot, kind, winner, demo).to_vec()
    }

    pub fn submit_ix(
        &self,
        s: &SubmitAccounts,
        kind: ResultKind,
        winner: Pubkey,
        demo: [u8; 32],
    ) -> Instruction {
        ix(
            arenachain::instruction::SubmitResult {
                kind,
                winner,
                demo_hash: demo,
            },
            arenachain::accounts::SubmitResult {
                config: config_pda(),
                arena_match: s.arena_match,
                game_server: s.game_server,
                mint: s.mint,
                vault: s.vault,
                player_a_token: s.player_a_token,
                player_b_token: s.player_b_token,
                player_a_profile: s.player_a_profile,
                player_b_profile: s.player_b_profile,
                fee_vault: s.fee_vault,
                player_a: s.player_a,
                instructions_sysvar: s.instructions_sysvar,
                token_program: token_program(),
            },
        )
    }

    pub fn submit_accounts(&self, f: &Fixture) -> SubmitAccounts {
        SubmitAccounts {
            arena_match: f.m,
            game_server: server_pda(&self.server.pubkey()),
            mint: self.mint,
            vault: vault_pda(&f.m),
            player_a_token: f.a.token,
            player_b_token: f.b.token,
            fee_vault: fee_vault_pda(),
            player_a: f.a.kp.pubkey(),
            player_a_profile: profile_pda(&f.a.kp.pubkey()),
            player_b_profile: profile_pda(&f.b.kp.pubkey()),
            instructions_sysvar: solana_sdk_ids::sysvar::instructions::ID,
        }
    }

    /// Server signs the canonical message; a third party relays it.
    pub fn submit_signed(
        &mut self,
        f: &Fixture,
        kind: ResultKind,
        winner: Pubkey,
    ) -> TransactionResult {
        let demo = [9u8; 32];
        let msg = self.result_message(&f.m, kind, &winner, &demo);
        let server = self.server.insecure_clone();
        let accounts = self.submit_accounts(f);
        let relayer = self.wallet();
        let ixs = [
            ed25519_ix(&server, &msg),
            self.submit_ix(&accounts, kind, winner, demo),
        ];
        self.send(&ixs, &[&relayer])
    }

    pub fn refund_expired_ix(&self, f: &Fixture) -> Instruction {
        ix(
            arenachain::instruction::RefundExpired {},
            arenachain::accounts::RefundExpired {
                arena_match: f.m,
                mint: self.mint,
                vault: vault_pda(&f.m),
                player_a_token: f.a.token,
                player_b_token: f.b.token,
                player_a: f.a.kp.pubkey(),
                token_program: token_program(),
            },
        )
    }

    pub fn refund_expired(&mut self, f: &Fixture) -> TransactionResult {
        let i = self.refund_expired_ix(f);
        let relayer = self.wallet();
        self.send(&[i], &[&relayer])
    }

    pub fn withdraw_fees_ix(
        &self,
        admin: &Pubkey,
        treasury_token: &Pubkey,
        amount: u64,
    ) -> Instruction {
        ix(
            arenachain::instruction::AdminWithdrawFees { amount },
            arenachain::accounts::AdminWithdrawFees {
                admin: *admin,
                config: config_pda(),
                mint: self.mint,
                fee_vault: fee_vault_pda(),
                treasury_token: *treasury_token,
                token_program: token_program(),
            },
        )
    }

    pub fn set_treasury_as(&mut self, admin: &Keypair, new_treasury: Pubkey) -> TransactionResult {
        let i = ix(
            arenachain::instruction::AdminSetTreasury { new_treasury },
            arenachain::accounts::AdminUpdateConfig {
                admin: admin.pubkey(),
                config: config_pda(),
            },
        );
        self.send(&[i], &[admin])
    }

    pub fn profile(&self, wallet: &Pubkey) -> PlayerProfile {
        self.read(&profile_pda(wallet))
    }

    pub fn claim_ix(&self, m: &Pubkey, winner_token: &Pubkey, player_a: &Pubkey) -> Instruction {
        ix(
            arenachain::instruction::ClaimPayout {},
            arenachain::accounts::ClaimPayout {
                config: config_pda(),
                arena_match: *m,
                mint: self.mint,
                vault: vault_pda(m),
                winner_token: *winner_token,
                fee_vault: fee_vault_pda(),
                player_a: *player_a,
                token_program: token_program(),
            },
        )
    }

    pub fn claim(&mut self, f: &Fixture, winner_token: &Pubkey) -> TransactionResult {
        let i = self.claim_ix(&f.m, winner_token, &f.a.kp.pubkey());
        let relayer = self.wallet();
        self.send(&[i], &[&relayer])
    }

    /// Two players, a created + joined match with `entry` each. Started when `start`.
    pub fn fixture(&mut self, entry: u64, start: bool) -> Fixture {
        let a = self.player(entry.saturating_mul(3));
        let b = self.player(entry.saturating_mul(3));
        self.create_match(&a, 1, entry).expect("create");
        let m = match_pda(&a.kp.pubkey(), 1);
        self.join_match(&b, &m).expect("join");
        if start {
            self.start_match(&m).expect("start");
        }
        Fixture { a, b, m }
    }
}

pub struct Player {
    pub kp: Keypair,
    pub token: Pubkey,
}

impl Player {
    pub fn key(&self) -> Pubkey {
        self.kp.pubkey()
    }
}

pub struct Fixture {
    pub a: Player,
    pub b: Player,
    pub m: Pubkey,
}

#[derive(Clone)]
pub struct SubmitAccounts {
    pub arena_match: Pubkey,
    pub game_server: Pubkey,
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub player_a_token: Pubkey,
    pub player_b_token: Pubkey,
    pub fee_vault: Pubkey,
    pub player_a: Pubkey,
    pub instructions_sysvar: Pubkey,
    pub player_a_profile: Pubkey,
    pub player_b_profile: Pubkey,
}
