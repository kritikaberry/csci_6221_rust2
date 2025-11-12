use redis::AsyncCommands;
use tokio::{net::{TcpListener, TcpStream}, sync::mpsc::{Receiver, Sender}};
use std::{collections::{BTreeMap, HashMap}, io};
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Eq, Hash, PartialEq, Clone, Copy)]
pub struct PlayerId(Uuid);

impl PlayerId {
    pub fn new() -> PlayerId {
        let pid = Uuid::new_v4();
        PlayerId(pid)
    }
}
pub struct Mmr(i32);

impl Mmr {
    pub fn new() -> Mmr {
        Mmr(0)
    }
}
pub struct Player {
    pid: PlayerId,
    mmr: Mmr,
}

pub struct MatchMaker {
    players: HashMap<PlayerId, Player>,
}

impl MatchMaker {
   pub fn new() -> MatchMaker {
        let players: HashMap<PlayerId, Player> = HashMap::new();
        MatchMaker {
            players,
        }
   } 

   pub fn add_player(&mut self) -> PlayerId{
        let pid = PlayerId::new();
        let mmr = Mmr::new();

        let player = Player {
            pid,
            mmr
        };
        self.players.insert(pid, player);
        pid
   }

   pub fn get_player(&self, pid: PlayerId) -> Option<&Player> {
        self.players.get(&pid)
   }
}

pub enum Command {
    Insert{
        response_sender: Sender<PlayerId>
    },
    Get{
        pid: PlayerId,
        response_sender: Sender<Option<&Player>>,
    }
}

async fn matchmaker(mut receiver: Receiver<Command>) {
    let mut matchmaker = MatchMaker::new(); 
    while let Some(command) = receiver.recv().await {
        match command {
            Command::Insert { 
                response_sender
            } => {
                let pid = matchmaker.add_player();
                response_sender.send(pid);
                }

            Command::Get {
                pid,
                response_sender
            } => {
               let player = matchmaker.get_player(pid); 
               response_sender.send(player);
            }
        }
    }
}

async fn handle_client(mut socket: TcpStream, sender: Sender<Command>) {
    let (mut reader, mut writer) = socket.split();
    let (sender, receiver) = mpsc::channel(100);
}

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
/* 
#[tokio::main]
async fn main() -> redis::RedisResult<()> {
    // Connect to local Redis (must be running)
    let client = redis::Client::open("redis://127.0.0.1/")?;
    let mut con = client.get_multiplexed_async_connection().await?;

    println!("✅ Connected to Redis");

    // Set and get a test key
    let _: () = con.set("greeting", "Hello Redis!").await?;
    let msg: String = con.get("greeting").await?;

    println!("📦 Retrieved from Redis: {}", msg);

    Ok(())
}
*/


