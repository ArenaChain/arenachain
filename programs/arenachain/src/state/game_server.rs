use anchor_lang::prelude::*;

/// A registered CS2 game server. PDA `["server", signer_key]`.
#[account]
#[derive(InitSpace)]
pub struct GameServer {
    /// Ed25519 public key the result service signs with.
    pub signer_key: Pubkey,
    /// Free-form label (region, host id), zero-padded.
    pub operator: [u8; 32],
    pub active: bool,
    pub bump: u8,
}
