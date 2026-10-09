use anchor_lang::prelude::*;

#[error_code]
pub enum ArenaError {
    #[msg("Signer is not authorized for this action")]
    Unauthorized,
    #[msg("Invalid config parameter")]
    InvalidConfigParam,
    #[msg("Program is paused")]
    Paused,
    #[msg("Entry fee must be greater than zero")]
    ZeroEntryFee,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Player is banned from paid matches")]
    PlayerBanned,
    #[msg("Game server is not active")]
    ServerInactive,
    #[msg("Match is not in the required status")]
    InvalidMatchStatus,
    #[msg("A player cannot join their own match")]
    CannotJoinOwnMatch,
    #[msg("Result deadline has passed")]
    ResultDeadlinePassed,
    #[msg("Winner must be one of the match's players")]
    InvalidWinner,
    #[msg("Void results must not name a winner")]
    VoidWithWinner,
    #[msg("Expected an Ed25519 instruction immediately before this one")]
    MissingEd25519Instruction,
    #[msg("Malformed Ed25519 instruction")]
    MalformedEd25519Instruction,
    #[msg("Ed25519 signer is not the match's server key")]
    WrongServerKey,
    #[msg("Signed message does not match this match's result")]
    ResultMessageMismatch,
    #[msg("Payout already made")]
    AlreadyPaid,
    #[msg("Payout is still on hold")]
    PayoutLocked,
}
