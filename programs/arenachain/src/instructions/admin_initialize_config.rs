use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    constants::*,
    errors::ArenaError,
    program::Arenachain,
    state::{ConfigParams, GlobalConfig},
};

/// One-time setup. Only the program's upgrade authority can call it, so nobody can
/// front-run initialization after deploy.
#[derive(Accounts)]
pub struct AdminInitializeConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + GlobalConfig::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Box<Account<'info, GlobalConfig>>,
    pub usdc_mint: Box<Account<'info, Mint>>,
    #[account(
        init,
        payer = authority,
        seeds = [FEE_VAULT_SEED],
        bump,
        token::mint = usdc_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub fee_vault: Box<Account<'info, TokenAccount>>,
    #[account(constraint = program.programdata_address()? == Some(program_data.key()) @ ArenaError::Unauthorized)]
    pub program: Program<'info, Arenachain>,
    #[account(constraint = program_data.upgrade_authority_address == Some(authority.key()) @ ArenaError::Unauthorized)]
    pub program_data: Account<'info, ProgramData>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handle_admin_initialize_config(
    ctx: Context<AdminInitializeConfig>,
    admin: Pubkey,
    dispute_authority: Pubkey,
    treasury: Pubkey,
    params: ConfigParams,
) -> Result<()> {
    params.validate()?;
    require_keys_neq!(treasury, Pubkey::default(), ArenaError::InvalidConfigParam);
    let config = &mut ctx.accounts.config;
    config.admin = admin;
    config.dispute_authority = dispute_authority;
    config.usdc_mint = ctx.accounts.usdc_mint.key();
    config.fee_vault = ctx.accounts.fee_vault.key();
    config.treasury = treasury;
    config.apply(&params);
    config.paused = false;
    config.bump = ctx.bumps.config;
    config.fee_vault_bump = ctx.bumps.fee_vault;
    Ok(())
}
