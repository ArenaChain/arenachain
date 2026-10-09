use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    constants::*,
    ed25519::{build_result_message, verify_preceding_ed25519},
    errors::ArenaError,
    events::{PayoutMade, ResultSubmitted},
    payout::{split_pool, Escrow},
    state::{GameServer, GlobalConfig, Match, MatchStatus, ResultKind},
};

/// Settles a live match with a result signed by its game server. Anyone can relay it;
/// the Ed25519 instruction immediately before this one must carry the server's signature
/// over the canonical result message.
#[derive(Accounts)]
pub struct SubmitResult<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = fee_vault)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(
        mut,
        seeds = [MATCH_SEED, arena_match.player_a.as_ref(), &arena_match.nonce.to_le_bytes()],
        bump = arena_match.bump,
        has_one = game_server @ ArenaError::WrongServerKey,
        has_one = mint,
        constraint = arena_match.status == MatchStatus::InProgress @ ArenaError::InvalidMatchStatus,
    )]
    pub arena_match: Box<Account<'info, Match>>,
    #[account(
        seeds = [SERVER_SEED, game_server.signer_key.as_ref()],
        bump = game_server.bump,
        constraint = game_server.active @ ArenaError::ServerInactive,
    )]
    pub game_server: Box<Account<'info, GameServer>>,
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
    #[account(mut)]
    pub fee_vault: Box<Account<'info, TokenAccount>>,
    /// Receives the vault's rent when it closes.
    #[account(mut, address = arena_match.player_a)]
    pub player_a: SystemAccount<'info>,
    /// CHECK: address-checked; read only through the `_checked` sysvar loaders.
    #[account(address = solana_sdk_ids::sysvar::instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

pub fn handle_submit_result(
    ctx: Context<SubmitResult>,
    kind: ResultKind,
    winner: Pubkey,
    demo_hash: [u8; 32],
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let config = &ctx.accounts.config;
    let m = &ctx.accounts.arena_match;
    require!(!config.paused, ArenaError::Paused);
    require!(now <= m.result_deadline, ArenaError::ResultDeadlinePassed);
    match kind {
        ResultKind::Void => require_keys_eq!(winner, Pubkey::default(), ArenaError::VoidWithWinner),
        ResultKind::Win | ResultKind::Forfeit => {
            require!(m.is_player(&winner), ArenaError::InvalidWinner)
        }
    }

    let message = build_result_message(
        ctx.program_id,
        &m.key(),
        m.created_slot,
        kind,
        &winner,
        &demo_hash,
    );
    verify_preceding_ed25519(
        &ctx.accounts.instructions_sysvar.to_account_info(),
        &ctx.accounts.game_server.signer_key,
        &message,
    )?;

    let pool = m.pool()?;
    let entry_fee = m.entry_fee;
    let hold = kind != ResultKind::Void && pool >= config.hold_threshold;
    let unlock_at = if hold {
        now.checked_add(config.hold_secs)
            .ok_or(ArenaError::MathOverflow)?
    } else {
        0
    };

    // Final state is written before any tokens move.
    let m = &mut ctx.accounts.arena_match;
    m.status = if kind == ResultKind::Void {
        MatchStatus::Refunded
    } else {
        MatchStatus::Settled
    };
    m.result_kind = Some(kind);
    m.winner = winner;
    m.demo_hash = demo_hash;
    m.unlock_at = unlock_at;
    m.paid_out = !hold;

    emit!(ResultSubmitted {
        arena_match: m.key(),
        kind,
        winner,
        demo_hash,
        unlock_at,
    });
    if hold {
        return Ok(());
    }

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

    if kind == ResultKind::Void {
        escrow.transfer(ctx.accounts.player_a_token.to_account_info(), entry_fee)?;
        escrow.transfer(ctx.accounts.player_b_token.to_account_info(), entry_fee)?;
    } else {
        let (payout, fee) = split_pool(pool, kind)?;
        let winner_token = if winner == m.player_a {
            ctx.accounts.player_a_token.to_account_info()
        } else {
            ctx.accounts.player_b_token.to_account_info()
        };
        escrow.transfer(winner_token, payout)?;
        escrow.transfer(ctx.accounts.fee_vault.to_account_info(), fee)?;
        emit!(PayoutMade {
            arena_match: m.key(),
            winner,
            payout,
            fee,
        });
    }

    ctx.accounts.vault.reload()?;
    if ctx.accounts.vault.amount == 0 {
        escrow.close(ctx.accounts.player_a.to_account_info())?;
    }
    Ok(())
}
