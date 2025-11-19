use crate::models::{PlayerId, Player};
use redis::{AsyncCommands, aio::MultiplexedConnection};
use tokio::{io::AsyncReadExt, net::{TcpStream}, sync::mpsc::Sender};
use crate::Command;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum ProtocolMessage {
    InitialConnection(Option<PlayerId>)
}


pub async fn handle_client(mut socket: TcpStream, sender: Sender<Command>, mut redis_con: MultiplexedConnection) {
    println!("Handling client!");
    let mut buf = Vec::with_capacity(size_of::<ProtocolMessage>());
    socket.read_buf(&mut buf).await.unwrap();

    for byte in &buf {
        println!("{}", byte);
    }
    
    // Parse the message
    let message: ProtocolMessage = match serde_json::from_slice(&buf) {
        Ok(msg) => msg,
        Err(e) => {
            eprintln!("Failed to parse: {}", e);
            return;
        }
    };
    

    match message {
        ProtocolMessage::InitialConnection(pid) => {
            if pid.is_none() {
                // Create a new player and add to redis
                let player = Player::new();
                let _: () = redis_con.zadd(player.pid(), &player, player.mmr()).await.unwrap(); // ERROR HANDLE HERE!
            } else {
                // Player is online, enter into queue
            }
        }
    }
    
}