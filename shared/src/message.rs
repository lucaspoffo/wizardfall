use serde::{Deserialize, Serialize};
use crate::player::PlayerInput;
use crate::network::ServerFrame;
use crate::{PlayersScore, LobbyInfo};

pub enum Messages {
    PlayerInput(PlayerInput),
    ServerFrame(Box<ServerFrame>),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessages {
    UpdateScore(PlayersScore),
    UpdateLobby(LobbyInfo),
    StartGameplay,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientAction {
    LobbyReady,
}

