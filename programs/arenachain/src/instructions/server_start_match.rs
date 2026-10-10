use anchor_lang::prelude::*;

use crate::{
    constants::*,
    errors::ArenaError,
    events::MatchStarted,
    state::{GameServer, GlobalConfig, Match, MatchStatus},
};

/// The match's assigned game server marks a full match as live.
#[derive(Accounts)]
pub struct ServerStartMatch<'info> {
    pub server_signer: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, GlobalConfig>,
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
    let window = ctx.accounts.config.result_window_secs;
    let m = &mut ctx.accounts.arena_match;
    require!(now <= m.start_deadline, ArenaError::ResultDeadlinePassed);
    m.status = MatchStatus::InProgress;
    m.started_at = now;
    // The result window counts from the real start, so a slow ready-up can't eat into it.
    m.result_deadline = now.checked_add(window).ok_or(ArenaError::MathOverflow)?;

    emit!(MatchStarted {
        arena_match: m.key(),
        game_server: ctx.accounts.game_server.key(),
        result_deadline: m.result_deadline,
    });
    Ok(())
}
