use anchor_lang::prelude::*;

/// Player profile. PDA `["profile", authority]`.
/// Rating, record and penalty fields are written by later steps (settlement, forfeits, Steam link).
#[account]
#[derive(InitSpace)]
pub struct PlayerProfile {
    pub authority: Pubkey,
    pub elo: u32,
    pub wins: u32,
    pub losses: u32,
    pub forfeits_30d: u8,
    pub last_forfeit_at: i64,
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
