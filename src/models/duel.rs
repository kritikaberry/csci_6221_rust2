use serde::{Deserialize, Serialize};

use crate::models::PlayerId;
pub struct Duel {
    player1: PlayerId,
    player2: PlayerId,
}

pub struct DuelResult {
    winner: Option<PlayerId>,
    loser: Option<PlayerId>,
}

#[derive(Serialize, Deserialize)]
pub enum DuelPacket<T> {
    StartDuel,
    DuelState(T),
    EndDuel,
}