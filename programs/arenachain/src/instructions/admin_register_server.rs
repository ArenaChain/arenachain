use anchor_lang::prelude::*;

use crate::{
    constants::*,
    errors::ArenaError,
    state::{GameServer, GlobalConfig},
};

#[derive(Accounts)]
#[instruction(signer_key: Pubkey)]
pub struct AdminRegisterServer<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ ArenaError::Unauthorized,
    )]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        init,
        payer = admin,
        space = 8 + GameServer::INIT_SPACE,
        seeds = [SERVER_SEED, signer_key.as_ref()],
        bump
    )]
    pub game_server: Account<'info, GameServer>,
    pub system_program: Program<'info, System>,
}

pub fn handle_admin_register_server(
    ctx: Context<AdminRegisterServer>,
    signer_key: Pubkey,
    operator: [u8; 32],
) -> Result<()> {
    let server = &mut ctx.accounts.game_server;
    server.signer_key = signer_key;
    server.operator = operator;
    server.active = true;
    server.bump = ctx.bumps.game_server;
    Ok(())
}

#[derive(Accounts)]
pub struct AdminSetServerActive<'info> {
    pub admin: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ ArenaError::Unauthorized,
    )]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SERVER_SEED, game_server.signer_key.as_ref()],
        bump = game_server.bump,
    )]
    pub game_server: Account<'info, GameServer>,
}

/// Deactivating a server (e.g. a leaked key) stops it from starting matches or signing
/// results. Its in-flight matches then refund through expiry.
pub fn handle_admin_set_server_active(
    ctx: Context<AdminSetServerActive>,
    active: bool,
) -> Result<()> {
    ctx.accounts.game_server.active = active;
    Ok(())
}
