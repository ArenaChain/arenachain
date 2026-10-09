use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    constants::*,
    errors::ArenaError,
    events::MatchCancelled,
    payout::Escrow,
    state::{Match, MatchStatus},
};

/// The creator withdraws an unjoined match. Not blocked by pause.
#[derive(Accounts)]
pub struct PlayerCancelMatch<'info> {
    #[account(mut, address = arena_match.player_a @ ArenaError::Unauthorized)]
    pub player_a: Signer<'info>,
    #[account(
        mut,
        seeds = [MATCH_SEED, arena_match.player_a.as_ref(), &arena_match.nonce.to_le_bytes()],
        bump = arena_match.bump,
        has_one = mint,
        constraint = arena_match.status == MatchStatus::Open @ ArenaError::InvalidMatchStatus,
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
        token::authority = player_a,
        token::token_program = token_program,
    )]
    pub player_a_token: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

pub fn handle_player_cancel_match(ctx: Context<PlayerCancelMatch>) -> Result<()> {
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

    ctx.accounts.vault.reload()?;
    if ctx.accounts.vault.amount == 0 {
        escrow.close(ctx.accounts.player_a.to_account_info())?;
    }

    emit!(MatchCancelled {
        arena_match: ctx.accounts.arena_match.key(),
    });
    Ok(())
}
