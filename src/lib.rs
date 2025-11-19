pub mod models;
pub mod services;
pub mod error;

// Re-export commonly used types
pub use models::player::{Player, PlayerId, Mmr};
pub use services::matchmaker::{MatchMaker, Command};
pub use error::PlayerError;