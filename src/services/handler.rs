use crate::models::{PlayerId, Player};
use redis::{AsyncCommands, aio::MultiplexedConnection};
use tokio::{io::AsyncReadExt, net::{TcpStream}, sync::mpsc::Sender};
use crate::Command;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum ProtocolMessage {
    InitialConnection(Option<PlayerId>)
}


pub async fn handle_client(mut socket: TcpStream, sender: Sender<Command>, redis_con: MultiplexedConnection) {
    let (mut reader, writer) = tokio::io::split(socket);
    let mut buf = Vec::with_capacity(size_of::<ProtocolMessage>());
    reader.read_buf(&mut buf);
    let message: ProtocolMessage = serde_json::from_slice(&buf).unwrap();

    match message {
        ProtocolMessage::InitialConnection(pid) => {
            if pid.is_none() {
                // Create a new player and add to redis
                let player = Player::new();
                redis_con.zadd(player.pid(), player, player.mmr());
            } else {
                // Player is online, enter into queue
            }
        }
    }
    
}