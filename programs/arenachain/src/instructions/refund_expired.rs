use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    constants::*,
    errors::ArenaError,
    events::MatchExpired,
    payout::Escrow,
    state::{Match, MatchStatus},
};

/// Refunds both players of a joined or started match once its result deadline has passed
/// with nothing settled. Anyone can call it; not blocked by pause or by the server's status,
/// so escrow can never be trapped.
#[derive(Accounts)]
pub struct RefundExpired<'info> {
    #[account(
        mut,
        seeds = [MATCH_SEED, arena_match.player_a.as_ref(), &arena_match.nonce.to_le_bytes()],
        bump = arena_match.bump,
        has_one = mint,
        constraint = matches!(arena_match.status, MatchStatus::Full | MatchStatus::InProgress)
            @ ArenaError::InvalidMatchStatus,
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
        token::authority = arena_match.player_a,
        token::token_program = token_program,
    )]
    pub player_a_token: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = arena_match.player_b,
        token::token_program = token_program,
    )]
    pub player_b_token: Box<Account<'info, TokenAccount>>,
    /// Receives the vault's rent when it closes.
    #[account(mut, address = arena_match.player_a)]
    pub player_a: SystemAccount<'info>,
    pub token_program: Program<'info, Token>,
}

pub fn handle_refund_expired(ctx: Context<RefundExpired>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        now > ctx.accounts.arena_match.result_deadline,
        ArenaError::NotExpired
    );

    ctx.accounts.arena_match.status = MatchStatus::Refunded;
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
    escrow.transfer(ctx.accounts.player_a_token.to_account_info(), m.entry_fee)?;
    escrow.transfer(ctx.accounts.player_b_token.to_account_info(), m.entry_fee)?;

    ctx.accounts.vault.reload()?;
    if ctx.accounts.vault.amount == 0 {
        escrow.close(ctx.accounts.player_a.to_account_info())?;
    }

    emit!(MatchExpired {
        arena_match: ctx.accounts.arena_match.key(),
    });
    Ok(())
}
