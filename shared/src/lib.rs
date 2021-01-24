use renet::channel::{
    ChannelConfig, ReliableOrderedChannelConfig, UnreliableUnorderedChannel,
    UnreliableUnorderedChannelConfig,
};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

pub fn channels() -> HashMap<u8, Box<dyn ChannelConfig>> {
    let mut reliable_config = ReliableOrderedChannelConfig::default();
    reliable_config.message_resend_time = Duration::from_millis(100);

    let unreliable_config = UnreliableUnorderedChannelConfig::default();

    let mut channels_config: HashMap<u8, Box<dyn ChannelConfig>> = HashMap::new();
    channels_config.insert(0, Box::new(reliable_config));
    channels_config.insert(1, Box::new(unreliable_config));
    channels_config
}

#[derive(Debug)]
pub struct Player {
    pub id: u64,
    pub color: (f32, f32, f32),
    pub x: i16,
    pub y: i16,
    pub animation_manager: AnimationManager<PlayerAnimations>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerState {
    pub id: u64,
    pub color: (f32, f32, f32),
    pub x: i16,
    pub y: i16,
    pub animation_state: AnimationState<PlayerAnimations>,
}

impl Player {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            x: 100,
            y: 100,
            color: (1.0, 0.0, 0.0),
            animation_manager: AnimationManager::new()
        }
    }

    pub fn from_state(state: &PlayerState) -> Self {
        Self {
            id: state.id,
            x: state.x,
            y: state.y,
            color: state.color,
            animation_manager: AnimationManager::from_state(&state.animation_state)
        }
    }

    pub fn update_from_input(&mut self, input: &PlayerInput) {
        self.x += (input.right as i16 - input.left as i16) * 4;
        self.y += (input.down as i16 - input.up as i16) * 4;
    }

    pub fn update_from_state(&mut self, state: &PlayerState) {
        self.x = state.x;
        self.y = state.y;
        self.color = state.color;
        self.animation_manager.update_from_state(&state.animation_state);
    }

    pub fn state(&self) -> PlayerState {
        PlayerState {
            id: self.id,
            x: self.x,
            y: self.y,
            color: self.color,
            animation_state: self.animation_manager.state()
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerFrame {
    pub players: Vec<PlayerState>,
}

pub enum Messages {
    PlayerInput(PlayerInput),
    ServerFrame(ServerFrame),
}

// TODO:
// Make PlayerState that is the serializable stuff from player
// impl From<Player> for PlayerState
// impl Player fn update_from_state(&mut self, state: PlayerState)
// Pass animation to Player
// struct AnimationState (animation stuff that goes throught network)
// impl AnimationController fn update_from_state(&mut self, state: AnimationState)
#[derive(Debug)]
pub struct AnimationController {
    pub frame: u32,
    speed: Duration,
    last_updated: Instant,
    total_frames: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnimationState<T> {
    pub frame: u32,
    pub current_animation: T,
}

impl AnimationController {
    pub fn new(fps: u64, total_frames: u32) -> Self {
        let speed = Duration::from_millis(1000 / fps);
        Self {
            speed,
            total_frames,
            frame: 0,
            last_updated: Instant::now(),
        }
    }

    pub fn update(&mut self) {
        let current_time = Instant::now();
        if current_time - self.last_updated > self.speed {
            self.frame += 1;
            self.frame = self.frame % self.total_frames;
            self.last_updated = current_time;
        }
    }

    pub fn reset(&mut self) {
        self.frame = 0;
        self.last_updated = Instant::now();
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub enum PlayerAnimations {
    Idle,
    Run,
}

#[derive(Debug)]
pub struct AnimationManager<T> {
    pub current_animation: T,
    animations: HashMap<T, AnimationController>,
}

impl<T: Eq + Hash + Clone> AnimationManager<T> {
    pub fn update(&mut self) {
        let animation = self.animations.get_mut(&self.current_animation).unwrap();
        animation.update();
    }

    pub fn play(&mut self, animation: T) {
        let current_animation = self.animations.get_mut(&self.current_animation).unwrap();
        current_animation.reset();
        self.current_animation = animation;
    }

    pub fn current_animation_controller(&self) -> &AnimationController {
       self.animations.get(&self.current_animation).unwrap()
    }

    pub fn update_from_state(&mut self, state: &AnimationState<T>) {
        self.current_animation = state.current_animation.clone();
        let current_animation = self.animations.get_mut(&self.current_animation).unwrap();
        current_animation.frame = state.frame;
    }

    pub fn state(&self) -> AnimationState<T> {
        let animation = self.animations.get(&self.current_animation).unwrap();
        AnimationState {
            current_animation: self.current_animation.clone(),
            frame: animation.frame
        }
    }
}

impl AnimationManager<PlayerAnimations> {
    pub fn new() -> Self {
        let mut animations = HashMap::new();
        let idle = AnimationController::new(13, 6);
        let run = AnimationController::new(13, 8);
        animations.insert(PlayerAnimations::Idle, idle);
        animations.insert(PlayerAnimations::Run, run);
        Self {
            current_animation: PlayerAnimations::Idle,
            animations,
        }
    }

    pub fn from_state(state: &AnimationState<PlayerAnimations>) -> Self {
        let mut animation_manager = AnimationManager::new();
        animation_manager.update_from_state(state);
        animation_manager
    }

}
