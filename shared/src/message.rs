use serde::{Deserialize, Serialize};

use crate::{player::PlayerInput, network::ServerFrame, PlayersScore};

pub enum Messages {
    PlayerInput(PlayerInput),
    ServerFrame(Box<ServerFrame>),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessages {
    UpdateScore(PlayersScore)
}
