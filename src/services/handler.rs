use crate::models::{PlayerId, Player};
use redis::{AsyncCommands, aio::MultiplexedConnection};
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
    Redis(#[from] redis::RedisError),
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
}


pub async fn handle_client(mut socket: TcpStream, sender: Sender<Command>, mut redis_con: MultiplexedConnection) -> Result<(), ProtocolError>{
    println!("Handling client!");

    let mut framed = Framed::new(socket, LengthDelimitedCodec::new());

    let byte_message = framed.next().await.unwrap()?;  
    
    let message: ProtocolMessage = serde_json::from_slice(&byte_message).unwrap();

    match message {
        ProtocolMessage::InitialConnection(pid) => {
            if pid.is_none() {
                // Create a new player and add to redis
                let player = Player::new();
                let _: () = redis_con.zadd(player.pid(), &player, player.mmr()).await?; // ERROR HANDLE HERE!
            }
        } 
    }

    Ok(())
}