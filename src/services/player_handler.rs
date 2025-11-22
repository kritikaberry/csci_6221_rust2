use crate::{DatabaseHandler, models::{Duel, Mmr, Player, PlayerId, duel::DuelPacket, player}};
use redis::{AsyncCommands, aio::MultiplexedConnection, io::tcp::socket2::Protocol};
use tokio::{io::AsyncReadExt, net::TcpStream, sync::mpsc::{self, Sender, Receiver, error::SendError}};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use futures::{SinkExt, StreamExt};
use crate::Command;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize)]
enum ProtocolMessage<T> {
    InitialConnection(Option<PlayerId>),
    ReadyToQueue,
    Ack,
    Disconnect,
    GamePacket(DuelPacket<T>),
}

#[derive(Error, Debug)]
pub enum ProtocolError<T> {
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("serde error: {0}")]
    ParseError(#[from] serde_json::Error),
    #[error("send to matchmaker error: {0}")]
    SendToMatchMakerError(#[from] SendError<Command<T>>),
    #[error("player shouldn't be null")]
    NullPlayer,
    #[error("did not receive sender back from matchmaker")]
    NullResponseFromMatchMaker,
}

struct PlayerHandler<T> {
    framed: Framed<TcpStream, LengthDelimitedCodec>,
    db: DatabaseHandler,
    pid: Option<PlayerId>,
    send_to_matchmaker: Sender<Command<T>>,
}

impl<T> PlayerHandler<T> {
    pub fn new(socket: TcpStream, send_to_matchmaker: Sender<Command<T>>, db: DatabaseHandler) -> PlayerHandler<T>{
        let framed = Framed::new(socket, LengthDelimitedCodec::new());
        let pid: Option<PlayerId> = None;
        PlayerHandler {
            framed,
            db,
            pid,
            send_to_matchmaker
        }
    }

    fn get_pid(&self) -> Result<PlayerId, ProtocolError<T>> {
        match &self.pid {
            Some(pid) => Ok(*pid),
            None => Err(ProtocolError::NullPlayer)
        }
    }

    async fn get_player_message(&mut self) -> Result<ProtocolMessage<T>, ProtocolError<T>> {
        let byte_message = self.framed.next().await.unwrap()?;  
        let message: ProtocolMessage<T> = serde_json::from_slice(&byte_message)?;
        Ok(message)
    }

    async fn send_player_message(&mut self, message: ProtocolMessage<T>) -> Result<(), ProtocolError<T>> {
        let byte_message = serde_json::to_vec(&message)?;
        self.framed.send(byte_message.into()).await?;
        Ok(())
    }

    async fn send_matchmaker_command(&mut self, command: Command<T>) -> Result<(), SendError> {
        self.send_to_matchmaker.send(command).await?;
        Ok(())
    }

    async fn add_player(&mut self) -> Result<PlayerId, ProtocolError<T>>{
        let player= Player::new();
        self.pid = Some(*player.pid());
        self.db.add_player(self.pid.unwrap());
        Ok(self.pid.unwrap())
    }

    async fn play_match(&mut self, player_sender: Sender<DuelPacket<T>>, player_receiver: Receiver<DuelPacket<T>>) {

        player_sender.send(DuelPacket::StartDuel).await;
        
        loop {
            player_receiver.recv().await
            if player_receiver.recv().await == DuelPacket::StartDuel{
                break;
            }
        }
    }
}


pub async fn handle_client<T>(socket: TcpStream, sender: Sender<Command<T>>, redis_con: MultiplexedConnection) -> Result<(), ProtocolError<T>>{
    let mut player_handler = PlayerHandler::new(socket, sender, redis_con);

    loop {
        let message = player_handler.get_player_message().await?; 

        match message {
            ProtocolMessage::InitialConnection(pid) => {
                if pid.is_none() {
                    let _ = player_handler.add_player().await?;
                    player_handler.send_player_message(ProtocolMessage::Ack).await?;
                    break
                }
            } 
            _ => { continue }  
        }
    }

    loop {
        let message = player_handler.get_player_message().await?;

        match message {
            ProtocolMessage::ReadyToQueue => {
                let (sender, mut receiver) = tokio::sync::mpsc::channel(100);
                let command = Command::AddToQueue{
                    pid: player_handler.get_pid()?,
                    response_sender: sender,
                };

                player_handler.send_matchmaker_command(command).await?;

                let (player_sender, player_receiver) =  match receiver.recv().await {
                    Some((player_sender, player_receiver)) => (player_sender, player_receiver),
                    None => return Err(ProtocolError::NullResponseFromMatchMaker) 
                };
            },

            ProtocolMessage::Disconnect => return Ok(()),

            _ => continue,

        }
    }

    Ok(())
}