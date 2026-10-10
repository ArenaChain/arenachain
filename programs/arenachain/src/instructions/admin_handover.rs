use anchor_lang::prelude::*;

use crate::{
    constants::*, errors::ArenaError, instructions::AdminUpdateConfig, state::GlobalConfig,
};

/// Step 1 of moving the admin role (e.g. to the team multisig). Proposing
/// `Pubkey::default()` cancels a pending handover.
pub fn handle_admin_propose_admin(
    ctx: Context<AdminUpdateConfig>,
    new_admin: Pubkey,
) -> Result<()> {
    ctx.accounts.config.pending_admin = new_admin;
    Ok(())
}

/// Step 2: the proposed admin signs to take over, so the role can't land on a key nobody
/// controls.
#[derive(Accounts)]
pub struct AdminAcceptAdmin<'info> {
    pub new_admin: Signer<'info>,
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        constraint = config.pending_admin != Pubkey::default()
            && config.pending_admin == new_admin.key() @ ArenaError::Unauthorized,
    )]
    pub config: Account<'info, GlobalConfig>,
}

pub fn handle_admin_accept_admin(ctx: Context<AdminAcceptAdmin>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.admin = ctx.accounts.new_admin.key();
    config.pending_admin = Pubkey::default();
    Ok(())
}

/// Rotates the key allowed to freeze held payouts (the freeze itself is post-hackathon).
pub fn handle_admin_set_dispute_authority(
    ctx: Context<AdminUpdateConfig>,
    new_authority: Pubkey,
) -> Result<()> {
    require_keys_neq!(
        new_authority,
        Pubkey::default(),
        ArenaError::InvalidConfigParam
    );
    ctx.accounts.config.dispute_authority = new_authority;
    Ok(())
}
