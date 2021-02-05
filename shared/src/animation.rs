use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

use crate::player::PlayerAnimation;
use crate::network::NetworkState;

#[derive(Debug, Clone)]
pub struct AnimationController {
    pub animation: Animation,
    pub frame: u8,
    speed: Duration,
    last_updated: Instant,
    total_frames: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Animation {
    PlayerAnimation(PlayerAnimation),
}

impl Animation {
    pub fn get_animation_controller(&self) -> AnimationController {
        match self {
            Animation::PlayerAnimation(player_animation) => {
                player_animation.get_animation_controller()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationState {
    pub frame: u8,
    pub current_animation: Animation,
}

impl NetworkState for AnimationController {
    type State = AnimationState;

    fn from_state(state: Self::State) -> Self {
        let mut animation_controller = state.current_animation.get_animation_controller();
        animation_controller.frame = state.frame;
        animation_controller
    }

    fn update_from_state(&mut self, state: Self::State) {
        self.change_animation(state.current_animation);
        self.frame = state.frame;
    }

    fn state(&self) -> AnimationState {
        AnimationState {
            current_animation: self.animation.clone(),
            frame: self.frame,
        }
    }
}

impl AnimationController {
    pub fn new(fps: u64, total_frames: u8, animation: Animation) -> Self {
        let speed = Duration::from_millis(1000 / fps);
        Self {
            animation,
            speed,
            total_frames,
            frame: 0,
            last_updated: Instant::now(),
        }
    }

    pub fn change_animation(&mut self, animation: Animation) {
        if self.animation == animation {
            return;
        }
        let animation_controller = animation.get_animation_controller();
        self.animation = animation_controller.animation;
        self.frame = 0;
        self.speed = animation_controller.speed;
        self.last_updated = Instant::now();
        self.total_frames = animation_controller.total_frames;
    }

    pub fn update(&mut self) {
        let current_time = Instant::now();
        if current_time - self.last_updated > self.speed {
            self.frame += 1;
            self.frame %= self.total_frames;
            self.last_updated = current_time;
        }
    }

    pub fn reset(&mut self) {
        self.frame = 0;
        self.last_updated = Instant::now();
    }
}
