//! Fee math and escrow transfers signed by the match PDA.

use anchor_lang::prelude::*;
use anchor_spl::token::{self, CloseAccount, TransferChecked};

use crate::{constants::*, errors::ArenaError, state::ResultKind};

/// Returns `(winner_payout, fee)` for a pool. `Win` pays the 5% fee (rounded down);
/// `Forfeit` waives it. `payout + fee == pool` always.
pub fn split_pool(pool: u64, kind: ResultKind) -> Result<(u64, u64)> {
    let fee = match kind {
        ResultKind::Win => {
            let fee = (pool as u128)
                .checked_mul(FEE_BPS as u128)
                .ok_or_else(|| error!(ArenaError::MathOverflow))?
                / BPS_DENOMINATOR as u128;
            u64::try_from(fee).map_err(|_| error!(ArenaError::MathOverflow))?
        }
        ResultKind::Forfeit => 0,
        // Void refunds both players; it never reaches a winner split.
        ResultKind::Void => return err!(ArenaError::InvalidMatchStatus),
    };
    let payout = pool
        .checked_sub(fee)
        .ok_or_else(|| error!(ArenaError::MathOverflow))?;
    Ok((payout, fee))
}

/// Accounts needed to move tokens out of a match vault.
pub struct Escrow<'a, 'info> {
    pub token_program: Pubkey,
    pub mint: AccountInfo<'info>,
    pub decimals: u8,
    pub vault: AccountInfo<'info>,
    /// The match PDA (vault authority).
    pub authority: AccountInfo<'info>,
    pub signer_seeds: &'a [&'a [&'a [u8]]],
}

impl<'a, 'info> Escrow<'a, 'info> {
    pub fn transfer(&self, to: AccountInfo<'info>, amount: u64) -> Result<()> {
        if amount == 0 {
            return Ok(());
        }
        token::transfer_checked(
            CpiContext::new_with_signer(
                self.token_program,
                TransferChecked {
                    from: self.vault.clone(),
                    mint: self.mint.clone(),
                    to,
                    authority: self.authority.clone(),
                },
                self.signer_seeds,
            ),
            amount,
            self.decimals,
        )
    }

    /// Closes the vault and returns its rent. Callers only do this once the vault is empty;
    /// a vault holding stray donated tokens is left open so nobody can block a payout.
    pub fn close(&self, rent_destination: AccountInfo<'info>) -> Result<()> {
        token::close_account(CpiContext::new_with_signer(
            self.token_program,
            CloseAccount {
                account: self.vault.clone(),
                destination: rent_destination,
                authority: self.authority.clone(),
            },
            self.signer_seeds,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win_fee_rounds_down_and_sums_to_pool() {
        for pool in [0u64, 1, 2, 19, 20, 21, 199, 200, 1_000_000, u64::MAX] {
            let (payout, fee) = split_pool(pool, ResultKind::Win).unwrap();
            assert_eq!(fee, ((pool as u128) * 500 / 10_000) as u64);
            assert_eq!(payout + fee, pool);
        }
    }

    #[test]
    fn forfeit_waives_fee() {
        assert_eq!(split_pool(1_000, ResultKind::Forfeit).unwrap(), (1_000, 0));
    }

    #[test]
    fn void_has_no_winner_split() {
        let err = split_pool(1_000, ResultKind::Void).unwrap_err();
        assert_eq!(err, ArenaError::InvalidMatchStatus.into());
    }
}
