use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlayerError {
    #[error("Player username can't be empty")]
    EmptyPlayerUsername,
}

