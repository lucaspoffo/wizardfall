use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::{
    animation::{Animation, AnimationController},
    timer::TimerSimple,
};

use derive::NetworkState;

#[derive(Debug, Clone, Serialize, Deserialize, NetworkState)]
pub struct Player {
    pub client_id: u64,
    pub direction: Vec2,
    pub fireball_cooldown: TimerSimple,
    pub fireball_charge: f32,
    pub fireball_max_charge: f32,
    pub dash_cooldown: TimerSimple,
    pub dash_duration: f32,
    pub current_dash_duration: f32,
    pub speed: Vec2,
}

impl Player {
    pub fn new(client_id: u64) -> Self {
        let mut fireball_cooldown = TimerSimple::new(1.5);
        fireball_cooldown.finish();

        let mut dash_cooldown = TimerSimple::new(1.);
        dash_cooldown.finish();

        Self {
            client_id,
            direction: Vec2::zero(),
            fireball_cooldown,
            dash_cooldown,
            dash_duration: 0.2,
            fireball_max_charge: 0.7,
            fireball_charge: 0.0,
            current_dash_duration: 0.0,
            speed: Vec2::zero(),
        }
    }
}

// TODO: add firing bool so we can keep mouse left pressed
// to keep casting fireball.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub dash: bool,
    pub fire: bool,
    pub direction: Vec2,
}

impl Default for PlayerInput {
    fn default() -> Self {
        Self {
            up: false,
            down: false,
            left: false,
            right: false,
            dash: false,
            fire: false,
            jump: false,
            direction: Vec2::zero(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastTarget {
    pub position: Vec2,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PlayerAction {
    CastFireball(CastTarget),
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
