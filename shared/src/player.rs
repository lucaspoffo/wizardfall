use serde::{Deserialize, Serialize};
use glam::Vec2; 

use derive::NetworkState;
use crate::animation::{Animation, AnimationController};

#[derive(Debug, Clone, Serialize, Deserialize, NetworkState)]
pub struct Player {
    pub client_id: u64,
    pub direction: Vec2,
}

impl Player {
    pub fn new(client_id: u64) -> Self {
        Self {
            client_id,
            direction: Vec2::zero(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub direction: Vec2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastTarget {
    pub position: Vec2,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PlayerAction {
    CastFireball(CastTarget),
    CastTeleport(CastTarget),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub enum PlayerAnimation {
    Idle,
    Run,
}

impl Into<Animation> for PlayerAnimation {
    fn into(self) -> Animation {
        Animation::PlayerAnimation(self)
    }
}

impl PlayerAnimation {
    pub fn get_animation_controller(&self) -> AnimationController {
        match self {
            PlayerAnimation::Idle => {
                AnimationController::new(13, 6, Animation::PlayerAnimation(PlayerAnimation::Idle))
            }
            PlayerAnimation::Run => {
                AnimationController::new(13, 8, Animation::PlayerAnimation(PlayerAnimation::Run))
            }
        }
    }
}
