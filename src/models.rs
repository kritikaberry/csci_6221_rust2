use std::collections::{HashMap, VecDeque};

pub type PlayerId = u64;

#[derive(Clone, Debug)]
pub struct MatchInfo {
    pub match_id: u64,
    pub game_id: String,
    pub a: PlayerId,
    pub b: PlayerId,
}

#[derive(Default)]
pub struct State {
    pub queues: HashMap<String, VecDeque<(PlayerId, f64)>>,
    pub matches: HashMap<u64, MatchInfo>,
    pub next_match_id: u64,
    pub active_matches: u64,
    pub total_matches: u64,
}
