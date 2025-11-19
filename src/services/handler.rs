use tokio::{net::{TcpListener, TcpStream}, sync::mpsc::{Receiver, Sender}};
use crate::Command;

enum ProtocolMessage {
    InitialConnection()
}

pub async fn handle_client(mut socket: TcpStream, sender: Sender<Command>) {
    let (mut reader, mut writer) = socket.split();
}