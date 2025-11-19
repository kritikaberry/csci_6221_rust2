// client/src/main.rs
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, net::{TcpStream}};
use uuid::Uuid;
use std::io;

#[derive(Serialize, Deserialize, Debug)]
struct PlayerId(Uuid);

#[derive(Serialize, Deserialize, Debug)]
enum ProtocolMessage {
    InitialConnection(Option<PlayerId>),
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
    println!("Connected to server!");

    let message = ProtocolMessage::InitialConnection(None);

    let mut buf = serde_json::to_vec(&message).unwrap();
    
    for byte in &buf {
        println!("{}", byte);
    }

    stream.write_all(&mut buf).await?;
    
    Ok(())
}  
