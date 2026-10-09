use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    constants::*,
    errors::ArenaError,
    events::PayoutMade,
    payout::{split_pool, Escrow},
    state::{GlobalConfig, Match, MatchStatus},
};

/// Pays a held result once its hold has passed. Anyone can call it; funds only go to the
/// recorded winner and the fee vault. Not blocked by pause. A frozen (Disputed) match fails
/// the status check.
#[derive(Accounts)]
pub struct ClaimPayout<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = fee_vault)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(
        mut,
        seeds = [MATCH_SEED, arena_match.player_a.as_ref(), &arena_match.nonce.to_le_bytes()],
        bump = arena_match.bump,
        has_one = mint,
        constraint = arena_match.status == MatchStatus::Settled @ ArenaError::InvalidMatchStatus,
        constraint = !arena_match.paid_out @ ArenaError::AlreadyPaid,
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
        token::authority = arena_match.winner,
        token::token_program = token_program,
    )]
    pub winner_token: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub fee_vault: Box<Account<'info, TokenAccount>>,
    /// Receives the vault's rent when it closes.
    #[account(mut, address = arena_match.player_a)]
    pub player_a: SystemAccount<'info>,
    pub token_program: Program<'info, Token>,
}

pub fn handle_claim_payout(ctx: Context<ClaimPayout>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let m = &ctx.accounts.arena_match;
    require!(now >= m.unlock_at, ArenaError::PayoutLocked);
    let kind = m.result_kind.ok_or(ArenaError::InvalidMatchStatus)?;
    let (payout, fee) = split_pool(m.pool()?, kind)?;

    ctx.accounts.arena_match.paid_out = true;

    let m = &ctx.accounts.arena_match;
    let nonce = m.nonce.to_le_bytes();
    let bump = [m.bump];
    let seeds: &[&[u8]] = &[MATCH_SEED, m.player_a.as_ref(), &nonce, &bump];
    let signer_seeds = &[seeds];
    let escrow = Escrow {
        token_program: ctx.accounts.token_program.key(),
        mint: ctx.accounts.mint.to_account_info(),
        decimals: ctx.accounts.mint.decimals,
        vault: ctx.accounts.vault.to_account_info(),
        authority: m.to_account_info(),
        signer_seeds,
    };
    escrow.transfer(ctx.accounts.winner_token.to_account_info(), payout)?;
    escrow.transfer(ctx.accounts.fee_vault.to_account_info(), fee)?;
    emit!(PayoutMade {
        arena_match: m.key(),
        winner: m.winner,
        payout,
        fee,
    });

    ctx.accounts.vault.reload()?;
    if ctx.accounts.vault.amount == 0 {
        escrow.close(ctx.accounts.player_a.to_account_info())?;
    }
    Ok(())
}
