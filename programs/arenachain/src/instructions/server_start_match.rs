use anchor_lang::prelude::*;

use crate::{
    constants::*,
    errors::ArenaError,
    events::MatchStarted,
    state::{GameServer, Match, MatchStatus},
};

/// The match's assigned game server marks a full match as live.
#[derive(Accounts)]
pub struct ServerStartMatch<'info> {
    pub server_signer: Signer<'info>,
    #[account(
        seeds = [SERVER_SEED, server_signer.key().as_ref()],
        bump = game_server.bump,
        constraint = game_server.active @ ArenaError::ServerInactive,
    )]
    pub game_server: Account<'info, GameServer>,
    #[account(
        mut,
        seeds = [MATCH_SEED, arena_match.player_a.as_ref(), &arena_match.nonce.to_le_bytes()],
        bump = arena_match.bump,
        has_one = game_server @ ArenaError::Unauthorized,
        constraint = arena_match.status == MatchStatus::Full @ ArenaError::InvalidMatchStatus,
    )]
    pub arena_match: Account<'info, Match>,
}

pub fn handle_server_start_match(ctx: Context<ServerStartMatch>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let m = &mut ctx.accounts.arena_match;
    require!(now <= m.result_deadline, ArenaError::ResultDeadlinePassed);
    m.status = MatchStatus::InProgress;
    m.started_at = now;

    emit!(MatchStarted {
        arena_match: m.key(),
        game_server: ctx.accounts.game_server.key(),
    });
    Ok(())
}
