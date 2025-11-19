use std::collections::HashMap;
use crate::models::{Player, PlayerId};
use tokio::sync::mpsc::{Receiver, Sender};

pub struct MatchMaker {
    players: HashMap<PlayerId, Player>,
    players_by_mmr: Vec<PlayerId>,
}

impl MatchMaker {
   pub fn new() -> MatchMaker {
        let players: HashMap<PlayerId, Player> = HashMap::new();
        let players_by_mmr: Vec<PlayerId> = Vec::new();

        MatchMaker {
            players,
            players_by_mmr,
        }
    } 

    fn add_player(&mut self) -> PlayerId{
        let player = Player::new();
        let pid = *player.pid();
        self.players.insert(pid, player);
        pid
    }

    fn get_player(&self, pid: PlayerId) -> Option<&Player> {
        self.players.get(&pid)
    }
}

pub enum Command {
    Insert{
        response_sender: Sender<PlayerId>
    },
    /*
    Get{
        pid: PlayerId,
        response_sender: Sender<Option<&Player>>,
    }
    */
}

pub async fn matchmaker(mut receiver: Receiver<Command>) {
    let mut matchmaker = MatchMaker::new(); 
    while let Some(command) = receiver.recv().await {
        match command {
            Command::Insert { 
                response_sender
            } => {
                let pid = matchmaker.add_player();
                response_sender.send(pid);
                }
            /* 
            Command::Get {
                pid,
                response_sender
            } => {
               let player = matchmaker.get_player(pid); 
               response_sender.send(player);
            }
            */
        }
    }
}

