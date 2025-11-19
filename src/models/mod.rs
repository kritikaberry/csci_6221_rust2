pub mod player;
pub mod duel;

// Re-export for easier access
pub use player::{Player, PlayerId, Mmr, PlayerUsername};
pub use duel::Duel;