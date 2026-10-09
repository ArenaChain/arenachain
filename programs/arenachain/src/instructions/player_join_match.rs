use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

use crate::{
    constants::*,
    errors::ArenaError,
    events::MatchJoined,
    state::{GlobalConfig, Match, MatchStatus, PlayerProfile},
};

#[derive(Accounts)]
pub struct PlayerJoinMatch<'info> {
    #[account(mut)]
    pub joiner: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(seeds = [PROFILE_SEED, joiner.key().as_ref()], bump = joiner_profile.bump)]
    pub joiner_profile: Box<Account<'info, PlayerProfile>>,
    #[account(
        mut,
        seeds = [MATCH_SEED, arena_match.player_a.as_ref(), &arena_match.nonce.to_le_bytes()],
        bump = arena_match.bump,
        has_one = mint,
        constraint = arena_match.status == MatchStatus::Open @ ArenaError::InvalidMatchStatus,
        constraint = arena_match.player_a != joiner.key() @ ArenaError::CannotJoinOwnMatch,
    )]
    pub arena_match: Box<Account<'info, Match>>,
    pub mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        seeds = [VAULT_SEED, arena_match.key().as_ref()],
        bump = arena_match.vault_bump,
        token::mint = mint,
        token::authority = arena_match,
        token::token_program = token_program,
    )]
    pub vault: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = joiner,
        token::token_program = token_program,
    )]
    pub joiner_token: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

pub fn handle_player_join_match(ctx: Context<PlayerJoinMatch>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(!ctx.accounts.config.paused, ArenaError::Paused);
    require!(
        !ctx.accounts.joiner_profile.is_banned(now),
        ArenaError::PlayerBanned
    );

    let result_deadline = now
        .checked_add(ctx.accounts.config.result_window_secs)
        .ok_or(ArenaError::MathOverflow)?;
    let m = &mut ctx.accounts.arena_match;
    m.player_b = ctx.accounts.joiner.key();
    m.joined_at = now;
    m.result_deadline = result_deadline;
    m.status = MatchStatus::Full;
    let entry_fee = m.entry_fee;

    token::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.joiner_token.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.joiner.to_account_info(),
            },
        ),
        entry_fee,
        ctx.accounts.mint.decimals,
    )?;

    emit!(MatchJoined {
        arena_match: ctx.accounts.arena_match.key(),
        player_b: ctx.accounts.joiner.key(),
        result_deadline,
    });
    Ok(())
}
