use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

use crate::{
    constants::*,
    errors::ArenaError,
    events::MatchCreated,
    state::{GameServer, GlobalConfig, Match, MatchStatus, PlayerProfile},
};

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct PlayerCreateMatch<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(seeds = [PROFILE_SEED, creator.key().as_ref()], bump = creator_profile.bump)]
    pub creator_profile: Box<Account<'info, PlayerProfile>>,
    #[account(
        seeds = [SERVER_SEED, game_server.signer_key.as_ref()],
        bump = game_server.bump,
        constraint = game_server.active @ ArenaError::ServerInactive,
    )]
    pub game_server: Box<Account<'info, GameServer>>,
    #[account(
        init,
        payer = creator,
        space = 8 + Match::INIT_SPACE,
        seeds = [MATCH_SEED, creator.key().as_ref(), &nonce.to_le_bytes()],
        bump
    )]
    pub arena_match: Box<Account<'info, Match>>,
    #[account(address = config.usdc_mint)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(
        init,
        payer = creator,
        seeds = [VAULT_SEED, arena_match.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = arena_match,
        token::token_program = token_program,
    )]
    pub vault: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = creator,
        token::token_program = token_program,
    )]
    pub creator_token: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handle_player_create_match(
    ctx: Context<PlayerCreateMatch>,
    nonce: u64,
    entry_fee: u64,
) -> Result<()> {
    let clock = Clock::get()?;
    require!(!ctx.accounts.config.paused, ArenaError::Paused);
    require!(entry_fee > 0, ArenaError::ZeroEntryFee);
    require!(
        !ctx.accounts.creator_profile.is_banned(clock.unix_timestamp),
        ArenaError::PlayerBanned
    );

    let m = &mut ctx.accounts.arena_match;
    m.player_a = ctx.accounts.creator.key();
    m.player_b = Pubkey::default();
    m.game_server = ctx.accounts.game_server.key();
    m.mint = ctx.accounts.mint.key();
    m.entry_fee = entry_fee;
    // Reject pools that can't be represented, so settlement math can never fail later.
    m.pool()?;
    m.status = MatchStatus::Open;
    m.created_slot = clock.slot;
    m.created_at = clock.unix_timestamp;
    m.joined_at = 0;
    m.started_at = 0;
    m.start_deadline = 0;
    m.result_deadline = 0;
    m.result_kind = None;
    m.winner = Pubkey::default();
    m.demo_hash = [0; 32];
    m.unlock_at = 0;
    m.paid_out = false;
    m.nonce = nonce;
    m.bump = ctx.bumps.arena_match;
    m.vault_bump = ctx.bumps.vault;

    token::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.creator_token.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.creator.to_account_info(),
            },
        ),
        entry_fee,
        ctx.accounts.mint.decimals,
    )?;

    emit!(MatchCreated {
        arena_match: ctx.accounts.arena_match.key(),
        player_a: ctx.accounts.creator.key(),
        game_server: ctx.accounts.game_server.key(),
        entry_fee,
    });
    Ok(())
}
