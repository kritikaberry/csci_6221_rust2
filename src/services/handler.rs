use crate::models::{PlayerId, Player};
use redis::{AsyncCommands, aio::MultiplexedConnection, io::tcp::socket2::Protocol};
use tokio::{io::AsyncReadExt, net::{TcpStream}, sync::mpsc::Sender};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use futures::{SinkExt, StreamExt};
use crate::Command;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize)]
enum ProtocolMessage {
    InitialConnection(Option<PlayerId>)
}

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("serde error: {0}")]
    ParseError(#[from] serde_json::Error),
}

struct PlayerHandler {
    framed: Framed<TcpStream, LengthDelimitedCodec>,
    redis_con: MultiplexedConnection,
    player: Option<Player>,
    send_to_matchmaker: Sender<Command>,
}

impl PlayerHandler {
    pub fn new(socket: TcpStream, send_to_matchmaker: Sender<Command>, redis_con: MultiplexedConnection) -> PlayerHandler{
        let framed = Framed::new(socket, LengthDelimitedCodec::new());
        let player: Option<Player> = None;
        PlayerHandler {
            framed,
            redis_con,
            player,
            send_to_matchmaker
        }
    }

    fn get_pid(&self) -> PlayerId {
        self.player.pid()
    }

    async fn get_player_message(&mut self) -> Result<ProtocolMessage, ProtocolError> {
        let byte_message = self.framed.next().await.unwrap()?;  
        let message: ProtocolMessage = serde_json::from_slice(&byte_message)?;
        Ok(message)
    }

    async fn add_player(&mut self) -> Result<PlayerId, ProtocolError>{
        self.player = Player::new();
        let pid = player.pid();
        let _: () = self.redis_con.zadd(pid, &player, player.mmr()).await?;
        Ok(*pid)
    }
}


pub async fn handle_client(socket: TcpStream, sender: Sender<Command>, redis_con: MultiplexedConnection) -> Result<(), ProtocolError>{
    let mut player_handler = PlayerHandler::new(socket, sender, redis_con);

    let message = player_handler.get_player_message().await?; 

    match message {
        ProtocolMessage::InitialConnection(pid) => {
            if pid.is_none() {
                let pid = player_handler.add_player().await?;
            }
        } 
    }

    Ok(())
}