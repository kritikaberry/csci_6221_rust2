use std::collections::HashMap;
use crate::models::{PlayerId, duel::DuelPacket};
use redis::{AsyncCommands, aio::MultiplexedConnection};
use tokio::sync::mpsc::{Receiver, Sender};
use thiserror::Error;

type ClientConnection<T> = (Sender<DuelPacket<T>>, Receiver<DuelPacket<T>>);

#[derive(Error, Debug)]
pub enum MatchMakerError<T> {
    #[error("Error receiving command from client handler")]
    NoCommandRecieved,
    #[error("Player already in ready queue")]
    PlayerExistsInReadyQueue,
    #[error("Player already in ready queue")]
    PlayerNotInReadyQueue,
    #[error("Could not send client connection to handler: {0}")]
    ClientConnectionSendError(#[from] tokio::sync::mpsc::error::SendError<ClientConnection<T>>),
    #[error("Error accessing redis database: {0}")]
    RedisError(#[from] redis::RedisError),
}

pub struct MatchMaker<T> {
    redis_con: MultiplexedConnection,
    client_handler_receiver: Receiver<Command<T>>,
    ready_queue: HashMap<PlayerId, Sender<ClientConnection<T>>>,
}

impl<T> MatchMaker<T>  {
   pub fn new(redis_con: MultiplexedConnection, client_handler_receiver: Receiver<Command<T>>) -> MatchMaker<T> {
        let ready_queue: HashMap<PlayerId, Sender<ClientConnection<T>>> = HashMap::new();

        MatchMaker {
            redis_con,
            client_handler_receiver,
            ready_queue,
        }
    }

    async fn receive_messages(&mut self) -> Option<Command<T>> {
        let command= self.client_handler_receiver.recv().await;
        
        match command {
            Some(command) => Some(command),
            None => None,
        }
    }    

    fn add_player_to_ready_queue(&mut self, pid: PlayerId, response_sender: Sender<ClientConnection<T>>) {
        self.ready_queue.insert(pid, response_sender);
    }

    async fn make_matches(&mut self) -> Result<(), MatchMakerError<T>>{
        // make sorted_players a shared const value
        let sorted_players: Vec<PlayerId> = self.redis_con.zrangebyscore("sorted_players", 0, -1).await?;
       
        let sorted_players: Vec<&PlayerId> = sorted_players.iter()
                                                            .filter(|pid| self.ready_queue.contains_key(*pid))
                                                            .collect();

        for pair in sorted_players.chunks_exact(2) {
            if let [player1, player2] = pair {
                self.connect_clients((**player1, **player2)).await?;
            }
        }
                    
       Ok(())
    }

    async fn connect_clients(&mut self, clients: (PlayerId, PlayerId)) -> Result<(), MatchMakerError<T>> {
        let response_sender_one = match self.ready_queue.remove(&clients.0) {
            Some(sender) => sender,
            None => return Err(MatchMakerError::PlayerNotInReadyQueue),
        };

        let response_sender_two = match self.ready_queue.remove(&clients.1) {
            Some(sender) => sender,
            None => return Err(MatchMakerError::PlayerNotInReadyQueue),
        };

        let (sender_one, receiver_one) = tokio::sync::mpsc::channel::<DuelPacket<T>>(255);
        let (sender_two, receiver_two) = tokio::sync::mpsc::channel::<DuelPacket<T>>(255);

        // Maybe I should spawn tasks for these?
        response_sender_one.send((sender_two, receiver_one)).await?;
        response_sender_two.send((sender_one, receiver_two)).await?;
 
        Ok(())
    }
}

pub enum Command<T> {
    AddToQueue{
        pid: PlayerId,
        response_sender: Sender<ClientConnection<T>>,
    },
}

pub async fn matchmaker<T>(receiver: Receiver<Command<T>>, redis_con: MultiplexedConnection) -> Result<(), MatchMakerError<T>> {
    let mut matchmaker: MatchMaker<T> = MatchMaker::new(redis_con, receiver); 
    while let Some(command) = matchmaker.receive_messages().await {
        match command {
            Command::AddToQueue{ pid, response_sender} => {
                matchmaker.add_player_to_ready_queue(pid, response_sender);
            }
        }
    }

    Ok(())
}

