pub mod constants;
pub mod ed25519;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod payout;
pub mod rating;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("HzNHCFVGQu9SKEeAo1UPLV5itD7TSYGoVtmYz4MJeKsw");

#[program]
pub mod arenachain {
    use super::*;

    pub fn admin_initialize_config(
        ctx: Context<AdminInitializeConfig>,
        admin: Pubkey,
        dispute_authority: Pubkey,
        treasury: Pubkey,
        params: ConfigParams,
    ) -> Result<()> {
        instructions::admin_initialize_config::handle_admin_initialize_config(
            ctx,
            admin,
            dispute_authority,
            treasury,
            params,
        )
    }

    pub fn admin_propose_admin(ctx: Context<AdminUpdateConfig>, new_admin: Pubkey) -> Result<()> {
        instructions::admin_handover::handle_admin_propose_admin(ctx, new_admin)
    }

    pub fn admin_accept_admin(ctx: Context<AdminAcceptAdmin>) -> Result<()> {
        instructions::admin_handover::handle_admin_accept_admin(ctx)
    }

    pub fn admin_set_dispute_authority(
        ctx: Context<AdminUpdateConfig>,
        new_authority: Pubkey,
    ) -> Result<()> {
        instructions::admin_handover::handle_admin_set_dispute_authority(ctx, new_authority)
    }

    pub fn admin_set_treasury(ctx: Context<AdminUpdateConfig>, new_treasury: Pubkey) -> Result<()> {
        instructions::admin_update_config::handle_admin_set_treasury(ctx, new_treasury)
    }

    pub fn admin_withdraw_fees(ctx: Context<AdminWithdrawFees>, amount: u64) -> Result<()> {
        instructions::admin_withdraw_fees::handle_admin_withdraw_fees(ctx, amount)
    }

    pub fn admin_update_config(
        ctx: Context<AdminUpdateConfig>,
        params: ConfigParams,
    ) -> Result<()> {
        instructions::admin_update_config::handle_admin_update_config(ctx, params)
    }

    pub fn admin_set_paused(ctx: Context<AdminUpdateConfig>, paused: bool) -> Result<()> {
        instructions::admin_update_config::handle_admin_set_paused(ctx, paused)
    }

    pub fn admin_register_server(
        ctx: Context<AdminRegisterServer>,
        signer_key: Pubkey,
        operator: [u8; 32],
    ) -> Result<()> {
        instructions::admin_register_server::handle_admin_register_server(ctx, signer_key, operator)
    }

    pub fn admin_set_server_active(ctx: Context<AdminSetServerActive>, active: bool) -> Result<()> {
        instructions::admin_register_server::handle_admin_set_server_active(ctx, active)
    }

    pub fn player_create_profile(ctx: Context<PlayerCreateProfile>) -> Result<()> {
        instructions::player_create_profile::handle_player_create_profile(ctx)
    }

    pub fn player_create_match(
        ctx: Context<PlayerCreateMatch>,
        nonce: u64,
        entry_fee: u64,
    ) -> Result<()> {
        instructions::player_create_match::handle_player_create_match(ctx, nonce, entry_fee)
    }

    pub fn player_join_match(ctx: Context<PlayerJoinMatch>) -> Result<()> {
        instructions::player_join_match::handle_player_join_match(ctx)
    }

    pub fn player_cancel_match(ctx: Context<PlayerCancelMatch>) -> Result<()> {
        instructions::player_cancel_match::handle_player_cancel_match(ctx)
    }

    pub fn server_start_match(ctx: Context<ServerStartMatch>) -> Result<()> {
        instructions::server_start_match::handle_server_start_match(ctx)
    }

    pub fn submit_result(
        ctx: Context<SubmitResult>,
        kind: ResultKind,
        winner: Pubkey,
        demo_hash: [u8; 32],
    ) -> Result<()> {
        instructions::submit_result::handle_submit_result(ctx, kind, winner, demo_hash)
    }

    pub fn claim_payout(ctx: Context<ClaimPayout>) -> Result<()> {
        instructions::claim_payout::handle_claim_payout(ctx)
    }

    pub fn refund_expired(ctx: Context<RefundExpired>) -> Result<()> {
        instructions::refund_expired::handle_refund_expired(ctx)
    }
}
