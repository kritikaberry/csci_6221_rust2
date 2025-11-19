#[derive(Clone)]
pub struct Player {
    pub id: u64,
    pub rating: i32,
}

#[derive(Clone)]
pub struct MatchInfo {
    pub match_id: u64,
    pub p1: u64,
    pub p2: u64,
}
