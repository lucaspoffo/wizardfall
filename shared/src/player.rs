use glam::Vec2;
use serde::{Deserialize, Serialize};

use derive::NetworkState;

use crate::timer::TimerSimple;

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
