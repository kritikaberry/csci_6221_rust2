use tokio::net::TcpListener;
use std::io;
use tokio::sync::mpsc;

mod models;
mod services;
mod error;

use services::{handler::handle_client, matchmaker::{matchmaker, Command}};

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    let (sender, receiver) = mpsc::channel(100);
    tokio::spawn(async move {
        matchmaker(receiver)
    });

    loop {
        let (socket, _) = listener.accept().await?;
        let sender_clone = sender.clone();
        tokio::spawn(async move {
            handle_client(socket, sender_clone).await
        });
    }
}