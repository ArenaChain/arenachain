use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct GlobalConfig {
    /// Team multisig.
    pub admin: Pubkey,
    /// Proposed next admin; `Pubkey::default()` when no handover is pending.
    pub pending_admin: Pubkey,
    /// Can freeze held payouts (used from a later step; never moves funds).
    pub dispute_authority: Pubkey,
    /// Test-USDC mint (classic SPL Token). Fixed at initialization.
    pub usdc_mint: Pubkey,
    /// Fee vault token account, PDA `["fee_vault"]`, owned by this config.
    pub fee_vault: Pubkey,
    /// Wallet whose USDC token account receives withdrawn fees.
    pub treasury: Pubkey,
    /// Pools at or above this amount wait `hold_secs` before payout.
    pub hold_threshold: u64,
    pub hold_secs: i64,
    /// Seconds after a match fills by which a result must arrive.
    pub result_window_secs: i64,
    /// Blocks new matches and new results. Never blocks refunds or claims.
    pub paused: bool,
    pub bump: u8,
    pub fee_vault_bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct ConfigParams {
    pub hold_threshold: u64,
    pub hold_secs: i64,
    pub result_window_secs: i64,
}

impl ConfigParams {
    pub fn validate(&self) -> Result<()> {
        use crate::constants::*;
        require!(
            self.hold_threshold > 0
                && (0..=MAX_HOLD_SECS).contains(&self.hold_secs)
                && (MIN_RESULT_WINDOW_SECS..=MAX_RESULT_WINDOW_SECS)
                    .contains(&self.result_window_secs),
            crate::errors::ArenaError::InvalidConfigParam
        );
        Ok(())
    }
}

impl GlobalConfig {
    pub fn apply(&mut self, params: &ConfigParams) {
        self.hold_threshold = params.hold_threshold;
        self.hold_secs = params.hold_secs;
        self.result_window_secs = params.result_window_secs;
    }
}
