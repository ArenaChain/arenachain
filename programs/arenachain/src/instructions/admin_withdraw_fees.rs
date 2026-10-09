use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

use crate::{constants::*, errors::ArenaError, events::FeesWithdrawn, state::GlobalConfig};

/// Moves collected fees from the fee vault to the treasury's USDC account. This is the only
/// instruction that signs as the config PDA, i.e. the only way out of the fee vault.
#[derive(Accounts)]
pub struct AdminWithdrawFees<'info> {
    pub admin: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ ArenaError::Unauthorized,
        has_one = fee_vault,
    )]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(address = config.usdc_mint)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        seeds = [FEE_VAULT_SEED],
        bump = config.fee_vault_bump,
        token::mint = mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub fee_vault: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = config.treasury,
        token::token_program = token_program,
    )]
    pub treasury_token: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

pub fn handle_admin_withdraw_fees(ctx: Context<AdminWithdrawFees>, amount: u64) -> Result<()> {
    require!(
        amount > 0 && amount <= ctx.accounts.fee_vault.amount,
        ArenaError::InvalidAmount
    );
    let bump = [ctx.accounts.config.bump];
    let seeds: &[&[u8]] = &[CONFIG_SEED, &bump];
    let signer_seeds = &[seeds];
    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.fee_vault.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.treasury_token.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;
    emit!(FeesWithdrawn {
        treasury: ctx.accounts.config.treasury,
        amount,
    });
    Ok(())
}
