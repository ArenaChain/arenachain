use anchor_lang::prelude::*;

/// Player profile. PDA `["profile", authority]`.
/// Rating, record, reputation and penalties are updated at settlement (`rating.rs`).
#[account]
#[derive(InitSpace)]
pub struct PlayerProfile {
    pub authority: Pubkey,
    pub elo: u32,
    pub wins: u32,
    pub losses: u32,
    pub total_forfeits: u32,
    /// The two most recent forfeit times, newest first (0 = none). Enough to count forfeits
    /// in the last 30 days exactly for the three-step penalty ladder.
    pub recent_forfeits: [i64; 2],
    /// 0..=100, starts at 100.
    pub reputation: i32,
    pub banned_until: i64,
    /// SteamID64; 0 means unlinked.
    pub steam_id: u64,
    pub created_at: i64,
    pub bump: u8,
}

impl PlayerProfile {
    pub fn is_banned(&self, now: i64) -> bool {
        self.banned_until > now
    }
}
