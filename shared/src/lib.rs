use glam::{vec2, Vec2};
use renet::channel::{
    ChannelConfig, ReliableOrderedChannelConfig, UnreliableUnorderedChannelConfig,
};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

pub fn channels() -> HashMap<u8, Box<dyn ChannelConfig>> {
    let mut reliable_config = ReliableOrderedChannelConfig::default();
    reliable_config.message_resend_time = Duration::from_millis(100);

    let mut player_action_channel = ReliableOrderedChannelConfig::default();
    player_action_channel.message_resend_time = Duration::from_millis(0);

    let unreliable_config = UnreliableUnorderedChannelConfig::default();

    let mut channels_config: HashMap<u8, Box<dyn ChannelConfig>> = HashMap::new();
    channels_config.insert(0, Box::new(reliable_config));
    channels_config.insert(1, Box::new(unreliable_config));
    channels_config.insert(2, Box::new(player_action_channel));
    channels_config
}

pub trait NetworkId {
    fn id(&self) -> u32;
}

pub trait NetworkState {
    type State: NetworkId;

    fn from_state(state: &Self::State) -> Self;
    fn update_from_state(&mut self, state: &Self::State);
    fn state(&self) -> Self::State;
    // TODO: Refactor when StateFrame is using EntityId
    fn id(&self) -> u32;
}

#[derive(Debug)]
pub struct Transform {
    pub position: Vec2,
    pub rotation: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: vec2(0.0, 0.0),
            rotation: 0.0,
        }
    }
}

impl Transform {
    pub fn new(position: Vec2, rotation: f32) -> Self {
        Self { position, rotation }
    }
}

pub struct AnimationComponent {}

#[derive(Debug)]
pub struct Player {
    pub client_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub id: u32,
    pub position: Vec2,
    pub animation_state: AnimationState<PlayerAnimations>,
}

impl NetworkId for PlayerState {
    fn id(&self) -> u32 {
        self.id
    }
}

impl Player {
    pub fn new(client_id: u64) -> Self {
        Self { client_id }
    }
}

impl Player {
    fn id(&self) -> u64 {
        self.client_id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerFrame {
    pub players: Vec<PlayerState>,
    pub projectiles: Vec<ProjectileState>,
}

pub enum Messages {
    PlayerInput(PlayerInput),
    ServerFrame(ServerFrame),
}

#[derive(Debug, Clone)]
pub struct AnimationController {
    animation: u8,
    pub frame: u32,
    speed: Duration,
    last_updated: Instant,
    total_frames: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationState<T> {
    pub frame: u32,
    pub current_animation: T,
}

impl AnimationController {
    pub fn new(fps: u64, total_frames: u32) -> Self {
        let speed = Duration::from_millis(1000 / fps);
        Self {
            animation: 0,
            speed,
            total_frames,
            frame: 0,
            last_updated: Instant::now(),
        }
    }

    pub fn change_animation(&mut self, animation: AnimationController) {
        if self.animation == animation.animation {
            return;
        }
        self.animation = animation.animation;
        self.frame = 0;
        self.speed = animation.speed;
        self.last_updated = Instant::now();
        self.total_frames = animation.total_frames;
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
    animations: HashMap<T, AnimationController>,
}

impl<T: Eq + Hash + Clone> AnimationManager<T> {
    pub fn get_animation_controller(&self, animation: &T) -> AnimationController {
        let animation = self.animations.get(animation).unwrap();
        (*animation).clone()
    }
}

impl AnimationManager<PlayerAnimations> {
    pub fn new() -> Self {
        let mut animations = HashMap::new();
        let idle = AnimationController::new(13, 6);
        let run = AnimationController::new(13, 8);
        animations.insert(PlayerAnimations::Idle, idle);
        animations.insert(PlayerAnimations::Run, run);
        Self { animations }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectileType {
    Fireball,
}

#[derive(Debug)]
pub struct Projectile {
    projectile_type: ProjectileType,
    pub duration: Duration,
}

impl Projectile {
    pub fn new(projectile_type: ProjectileType) -> Self {
        Self {
            projectile_type,
            duration: Duration::from_secs(2),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectileState {
    pub id: u32,
    projectile_type: ProjectileType,
    owner: u32,
    position: Vec2,
    rotation: f32,
}

impl NetworkId for ProjectileState {
    fn id(&self) -> u32 {
        self.id
    }
}
