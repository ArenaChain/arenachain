use anchor_lang::prelude::*;

use crate::{constants::*, state::PlayerProfile};

#[derive(Accounts)]
pub struct PlayerCreateProfile<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + PlayerProfile::INIT_SPACE,
        seeds = [PROFILE_SEED, authority.key().as_ref()],
        bump
    )]
    pub profile: Account<'info, PlayerProfile>,
    pub system_program: Program<'info, System>,
}

pub fn handle_player_create_profile(ctx: Context<PlayerCreateProfile>) -> Result<()> {
    let profile = &mut ctx.accounts.profile;
    profile.authority = ctx.accounts.authority.key();
    profile.elo = STARTING_ELO;
    profile.wins = 0;
    profile.losses = 0;
    profile.total_forfeits = 0;
    profile.recent_forfeits = [0; 2];
    profile.reputation = MAX_REPUTATION;
    profile.banned_until = 0;
    profile.steam_id = 0;
    profile.created_at = Clock::get()?.unix_timestamp;
    profile.bump = ctx.bumps.profile;
    Ok(())
}
