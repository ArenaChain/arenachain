use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum MatchStatus {
    Open,
    Full,
    InProgress,
    Settled,
    Refunded,
    Disputed,
}

/// Wire values are part of the signed result message: never reorder.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum ResultKind {
    Win,
    Forfeit,
    Void,
}

impl ResultKind {
    pub fn as_u8(self) -> u8 {
        match self {
            ResultKind::Win => 0,
            ResultKind::Forfeit => 1,
            ResultKind::Void => 2,
        }
    }
}

/// A 1v1 match. PDA `["match", player_a, nonce_le]`. Its escrow is the token account
/// PDA `["vault", match]`, whose authority is this account.
#[account]
#[derive(InitSpace)]
pub struct Match {
    pub player_a: Pubkey,
    /// `Pubkey::default()` until someone joins.
    pub player_b: Pubkey,
    pub game_server: Pubkey,
    pub mint: Pubkey,
    pub entry_fee: u64,
    pub status: MatchStatus,
    pub created_slot: u64,
    pub created_at: i64,
    pub joined_at: i64,
    pub started_at: i64,
    /// Set at join: the server must start the match by then (`joined_at + window`).
    pub start_deadline: i64,
    /// Set at start: the result must arrive by then (`started_at + window`). 0 until started.
    pub result_deadline: i64,
    pub result_kind: Option<ResultKind>,
    /// `Pubkey::default()` until a Win/Forfeit result settles.
    pub winner: Pubkey,
    pub demo_hash: [u8; 32],
    /// Earliest time a held payout can be claimed; 0 when not held.
    pub unlock_at: i64,
    pub paid_out: bool,
    pub nonce: u64,
    pub bump: u8,
    pub vault_bump: u8,
}

impl Match {
    pub fn pool(&self) -> Result<u64> {
        self.entry_fee
            .checked_mul(2)
            .ok_or_else(|| error!(crate::errors::ArenaError::MathOverflow))
    }

    /// After this time, with nothing settled, anyone can refund both players.
    pub fn expiry_deadline(&self) -> Option<i64> {
        match self.status {
            MatchStatus::Full => Some(self.start_deadline),
            MatchStatus::InProgress => Some(self.result_deadline),
            _ => None,
        }
    }

    pub fn is_player(&self, key: &Pubkey) -> bool {
        *key == self.player_a || *key == self.player_b
    }
}
