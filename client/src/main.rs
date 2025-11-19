// client/src/main.rs
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, net::{TcpStream}};
use uuid::Uuid;
use std::io;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use futures::{SinkExt, StreamExt};

#[derive(Serialize, Deserialize, Debug)]
struct PlayerId(Uuid);

#[derive(Serialize, Deserialize, Debug)]
enum ProtocolMessage {
    InitialConnection(Option<PlayerId>),
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let stream = TcpStream::connect("127.0.0.1:8080").await?;
    println!("Connected to server!");

    let message = ProtocolMessage::InitialConnection(None);

    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());
    
    let byte_message = serde_json::to_vec(&message).unwrap(); // ERROR HANDLE HERE

    framed.send(byte_message.into()).await?;

    Ok(())
}  
