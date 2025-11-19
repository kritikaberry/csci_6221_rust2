use tokio::net::TcpListener;
use std::io;
use tokio::sync::mpsc;

mod models;
mod services;
mod error;

use services::{handler::handle_client, matchmaker::{matchmaker, Command}};

#[tokio::main]
async fn main() -> redis::RedisResult<()> {
    // Open connection to redis database of players
    let client = redis::Client::open("redis://127.0.0.1:6379").unwrap(); // ERROR HANDLE HERE
    let mut redis_con = client.get_multiplexed_async_connection().await?;
    // Create matchmaker service will use to communicate with client handlers
    let (sender, receiver) = mpsc::channel(100);
    // Start matchmaker service 
    let mut redis_con_clone = redis_con.clone();
    tokio::spawn(async move {
        matchmaker(receiver, redis_con_clone)
    });
    // Start listening for incoming player connections
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    loop {
        let (socket, _) = listener.accept().await?;
        let sender_clone = sender.clone();
        // Spawn client handler for every new player connection
        let mut redis_con_clone = redis_con.clone();
        tokio::spawn(async move {
            handle_client(socket, sender_clone, redis_con_clone).await
        });
    }
}