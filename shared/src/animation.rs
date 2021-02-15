use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

use crate::network::NetworkState;

#[derive(Clone, Debug)]
pub struct Animation {
    pub name: String,
    pub row: u32,
    pub frames: u32,
    speed: Duration,
}

impl Animation {
    pub fn new(name: String, row: u32, frames: u32, fps: u64) -> Self {
        let speed = Duration::from_millis(1000 / fps);

        Self {
            name,
            row,
            frames,
            speed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnimationController {
    pub animation_entity: AnimationEntity,
    pub animations: Vec<Animation>,
    pub frame: u32,
    pub current_animation: usize,
    last_updated: Instant,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AnimationEntity {
    Player,
}

impl AnimationEntity {
    pub fn new_animation_controller(self) -> AnimationController {
        match self {
            AnimationEntity::Player => {
                let mut animation_controller = AnimationController::new(self);
                let idle = Animation::new("idle".to_string(), 0, 2, 13);
                let run = Animation::new("run".to_string(), 0, 2, 13);
                animation_controller.add_animation(idle);
                animation_controller.add_animation(run);
                animation_controller
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationState {
    pub animation_entity: AnimationEntity,
    pub frame: u8,
    pub current_animation: u8,
}

impl NetworkState for AnimationController {
    type State = AnimationState;

    fn from_state(state: Self::State) -> Self {
        let mut animation_controller = state.animation_entity.new_animation_controller();
        animation_controller.frame = state.frame as u32;
        animation_controller
    }

    fn update_from_state(&mut self, state: Self::State) {
        self.change_animation(state.current_animation as usize);
        self.frame = state.frame as u32;
    }

    fn state(&self) -> AnimationState {
        AnimationState {
            animation_entity: self.animation_entity,
            current_animation: self.current_animation as u8,
            frame: self.frame as u8,
        }
    }
}

impl AnimationController {
    pub fn new(animation_entity: AnimationEntity) -> Self {
        Self {
            animation_entity,
            animations: vec![],
            current_animation: 0,
            frame: 0,
            last_updated: Instant::now(),
        }
    }

    pub fn add_animation(&mut self, animation: Animation) {
        self.animations.push(animation);
    }

    pub fn play_animation(&mut self, animation: &str) {
        if let Some(animation) = self.animations.iter().position(|a| a.name == animation) {
            self.change_animation(animation);
        }
    }

    pub fn change_animation(&mut self, animation: usize) {
        if self.current_animation == animation || animation > self.animations.len() {
            return;
        }

        self.current_animation = animation;
        self.frame = 0;
        self.last_updated = Instant::now();
    }

    pub fn update(&mut self) {
        let animation = &self.animations[self.current_animation];
        let current_time = Instant::now();
        if current_time - self.last_updated > animation.speed {
            self.frame += 1;
            self.frame %= animation.frames;
            self.last_updated = current_time;
        }
    }

    pub fn reset(&mut self) {
        self.frame = 0;
        self.last_updated = Instant::now();
    }
}
