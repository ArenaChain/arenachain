use anchor_lang::prelude::*;

use crate::{
    constants::*,
    errors::ArenaError,
    state::{ConfigParams, GlobalConfig},
};

#[derive(Accounts)]
pub struct AdminUpdateConfig<'info> {
    pub admin: Signer<'info>,
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ ArenaError::Unauthorized,
    )]
    pub config: Account<'info, GlobalConfig>,
}

/// Changes hold and result-window timing only. The mint, fee vault and fee rate are fixed.
pub fn handle_admin_update_config(
    ctx: Context<AdminUpdateConfig>,
    params: ConfigParams,
) -> Result<()> {
    params.validate()?;
    ctx.accounts.config.apply(&params);
    Ok(())
}

pub fn handle_admin_set_paused(ctx: Context<AdminUpdateConfig>, paused: bool) -> Result<()> {
    ctx.accounts.config.paused = paused;
    Ok(())
}
