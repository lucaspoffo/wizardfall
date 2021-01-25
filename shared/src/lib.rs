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

#[derive(Debug)]
pub struct Player {
    pub id: u32,
    pub position: Vec2,
    pub animation_manager: AnimationManager<PlayerAnimations>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerState {
    pub id: u32,
    pub position: Vec2,
    pub animation_state: AnimationState<PlayerAnimations>,
}

impl Player {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            position: vec2(100.0, 100.0),
            animation_manager: AnimationManager::new(),
        }
    }

    pub fn from_state(state: &PlayerState) -> Self {
        Self {
            id: state.id,
            position: state.position,
            animation_manager: AnimationManager::from_state(&state.animation_state),
        }
    }

    pub fn update_from_input(&mut self, input: &PlayerInput) {
        let x = (input.right as i8 - input.left as i8) as f32;
        let y = (input.down as i8 - input.up as i8) as f32;
        let mut direction = vec2(x, y);

        if direction.length() != 0.0 {
            direction = direction.normalize();
            self.position.x += direction.x * 4.0;
            self.position.y += direction.y * 4.0;
        }

        if input.right ^ input.left || input.down ^ input.up {
            self.animation_manager.play(PlayerAnimations::Run);
        } else {
            self.animation_manager.play(PlayerAnimations::Idle);
        }

        if input.right ^ input.left {
            self.animation_manager.h_flip = !input.right;
        }
    }

    pub fn update_from_state(&mut self, state: &PlayerState) {
        self.position.x = state.position.x;
        self.position.y = state.position.y;
        self.animation_manager
            .update_from_state(&state.animation_state);
    }

    pub fn state(&self) -> PlayerState {
        PlayerState {
            id: self.id,
            position: self.position,
            animation_state: self.animation_manager.state(),
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
    pub projectiles: Vec<ProjectileState>,
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
    h_flip: bool,
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
    pub h_flip: bool,
    animations: HashMap<T, AnimationController>,
}

impl<T: Eq + Hash + Clone> AnimationManager<T> {
    pub fn update(&mut self) {
        let animation = self.animations.get_mut(&self.current_animation).unwrap();
        animation.update();
    }

    pub fn play(&mut self, animation: T) {
        if self.current_animation == animation {
            return;
        }

        let current_animation = self.animations.get_mut(&self.current_animation).unwrap();
        current_animation.reset();
        self.current_animation = animation;
    }

    pub fn current_animation_controller(&self) -> &AnimationController {
        self.animations.get(&self.current_animation).unwrap()
    }

    pub fn update_from_state(&mut self, state: &AnimationState<T>) {
        self.current_animation = state.current_animation.clone();
        self.h_flip = state.h_flip;
        let current_animation = self.animations.get_mut(&self.current_animation).unwrap();
        current_animation.frame = state.frame;
    }

    pub fn state(&self) -> AnimationState<T> {
        let animation = self.animations.get(&self.current_animation).unwrap();
        AnimationState {
            current_animation: self.current_animation.clone(),
            frame: animation.frame,
            h_flip: self.h_flip,
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
            h_flip: false,
        }
    }

    pub fn from_state(state: &AnimationState<PlayerAnimations>) -> Self {
        let mut animation_manager = AnimationManager::new();
        animation_manager.update_from_state(state);
        animation_manager
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CastTarget {
    pub position: Vec2
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
    id: u32,
    projectile_type: ProjectileType,
    owner: u32,
    pub position: Vec2,
    pub direction: Vec2,
    pub rotation: f32,
    pub duration: Duration,
}

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

impl Projectile {
    pub fn from_cast_target(
        projectile_type: ProjectileType,
        owner: u32,
        cast_target: CastTarget,
        origin: Vec2,
    ) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let direction = (cast_target.position - origin).normalize();
        let rotation = direction.angle_between(Vec2::unit_x());
        Self {
            id,
            rotation,
            projectile_type,
            owner,
            position: origin,
            direction,
            duration: Duration::from_secs(2),
        }
    }

    pub fn from_state(state: &ProjectileState) -> Self {
        Self {
            id: state.id,
            rotation: state.rotation,
            projectile_type: state.projectile_type.clone(),
            position: state.position,
            owner: state.owner,
            direction: Vec2::unit_x(),
            duration: Duration::from_secs(0),
        }
    }

    pub fn state(&self) -> ProjectileState {
        ProjectileState {
            id: self.id,
            projectile_type: self.projectile_type.clone(),
            owner: self.owner,
            position: self.position,
            rotation: self.direction.angle_between(Vec2::unit_x()),
        }
    }

    pub fn update_from_state(&mut self, state: &ProjectileState) {
        self.position.x = state.position.x;
        self.position.y = state.position.y;
        self.rotation = state.rotation;
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectileState {
    pub id: u32,
    projectile_type: ProjectileType,
    owner: u32,
    position: Vec2,
    rotation: f32,
}
