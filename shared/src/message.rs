use crate::{player::PlayerInput, network::ServerFrame};

pub enum Messages {
    PlayerInput(PlayerInput),
    ServerFrame(Box<ServerFrame>),
}
