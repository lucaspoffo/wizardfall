use derive::NetworkState;

use glam::{vec2, Vec2};
use renet::channel::{
    ChannelConfig, ReliableOrderedChannelConfig, UnreliableUnorderedChannelConfig,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use shipyard::{EntitiesView, EntityId, IntoIter, IntoWithId, View, World};

use std::collections::HashMap;
use std::hash::Hash;
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
    type State: Clone;

    fn from_state(state: Self::State) -> Self;
    fn update_from_state(&mut self, state: Self::State);
    fn state(&self) -> Self::State;
}

#[derive(Debug, Clone, Serialize, Deserialize, NetworkState)]
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

#[derive(Debug, Clone, Serialize, Deserialize, NetworkState)]
pub struct Player {
    pub client_id: u64,
}

impl Player {
    pub fn new(client_id: u64) -> Self {
        Self { client_id }
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
    entities: Vec<EntityId>,
    players: NetworkComponent<Player>,
    projectiles: NetworkComponent<Projectile>,
    transforms: NetworkComponent<Transform>,
}

impl ServerFrame {
    pub fn from_world(world: &World) -> Self {
        let entities: Vec<EntityId> = world
            .run(|entities: EntitiesView| entities.iter().collect())
            .unwrap();


        Self {
            players: NetworkComponent::<Player>::from_world(&entities, world),
            projectiles: NetworkComponent::<Projectile>::from_world(&entities, world),
            transforms: NetworkComponent::<Transform>::from_world(&entities, world),
            entities,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct NetworkComponent<T>
    {
    bitmask: Vec<bool>,
    values: Vec<T>,
}

impl<T: 'static + Sync + Send + Clone + NetworkState> NetworkComponent<T> {
    fn from_world(entities_id: &[EntityId], world: &World) -> NetworkComponent<T::State> {
        let mut bitmask: Vec<bool> = vec![false; entities_id.len()];
        let mut values: Vec<Option<T::State>> = vec![None; entities_id.len()];
        world
            .run(|components: View<T>| {
                for (entity_id, component) in components.iter().with_id() {
                    let id_pos = entities_id
                        .iter()
                        .position(|&x| x == entity_id)
                        .expect("Network component EntityID not found.");

                    bitmask[id_pos] = true;
                    values[id_pos] = Some(component.state());
                }
            })
            .unwrap();

        let values = values.iter_mut().filter_map(|v| v.take()).collect();

        NetworkComponent { bitmask, values }
    }
}

pub enum Messages {
    PlayerInput(PlayerInput),
    ServerFrame(ServerFrame),
}

#[derive(Debug, Clone)]
pub struct AnimationController {
    animation: u8,
    pub frame: u8,
    speed: Duration,
    last_updated: Instant,
    total_frames: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationState {
    pub frame: u8,
    pub current_animation: u8,
}

impl AnimationController {
    pub fn new(fps: u64, total_frames: u8) -> Self {
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

#[derive(Debug, Clone, Serialize, Deserialize, NetworkState)]
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
    projectile_type: ProjectileType,
}
