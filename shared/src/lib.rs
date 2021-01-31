use derive::NetworkState;

use renet::channel::{
    ChannelConfig, ReliableOrderedChannelConfig, UnreliableUnorderedChannelConfig,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use shipyard::{
    AllStoragesViewMut, EntitiesView, EntitiesViewMut, EntityId, Get, IntoIter, IntoWithId,
    UniqueViewMut, View, ViewMut, World,
};

pub use macroquad::math::{vec2, Rect, Vec2};
use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

pub mod physics;
use physics::CollisionShape;

// Server EntityId -> Client EntityId
pub type EntityMapping = HashMap<EntityId, EntityId>;

pub fn channels() -> HashMap<u8, Box<dyn ChannelConfig>> {
    let reliable_config = ReliableOrderedChannelConfig {
        message_resend_time: Duration::from_millis(100),
        ..Default::default()
    };

    let player_action_channel = ReliableOrderedChannelConfig {
        message_resend_time: Duration::from_millis(0),
        ..Default::default()
    };

    let unreliable_config = UnreliableUnorderedChannelConfig::default();

    let mut channels_config: HashMap<u8, Box<dyn ChannelConfig>> = HashMap::new();
    channels_config.insert(0, Box::new(reliable_config));
    channels_config.insert(1, Box::new(unreliable_config));
    channels_config.insert(2, Box::new(player_action_channel));
    channels_config
}

pub trait NetworkState {
    type State: Clone + std::fmt::Debug + Serialize + DeserializeOwned;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerFrame {
    entities: Vec<EntityId>,
    players: NetworkComponent<Player>,
    projectiles: NetworkComponent<Projectile>,
    transforms: NetworkComponent<Transform>,
    animations: NetworkComponent<AnimationController>,
    collisions_shape: NetworkComponent<CollisionShape>,
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
            animations: NetworkComponent::<AnimationController>::from_world(&entities, world),
            collisions_shape: NetworkComponent::<CollisionShape>::from_world(&entities, world),
            entities,
        }
    }

    pub fn apply_in_world(&self, world: &World) {
        self.players.apply_in_world(&self.entities, world);
        self.projectiles.apply_in_world(&self.entities, world);
        self.transforms.apply_in_world(&self.entities, world);
        self.animations.apply_in_world(&self.entities, world);
        self.collisions_shape.apply_in_world(&self.entities, world);

        // Remove entities that are not in the network frame
        world
            .run(|mut all_storages: AllStoragesViewMut| {
                let removed_entities: Vec<EntityId> = {
                    let mut mapping = all_storages
                        .borrow::<UniqueViewMut<EntityMapping>>()
                        .unwrap();
                    let mut removed_entities: Vec<EntityId> = vec![];
                    for (server_id, client_id) in mapping.clone().iter() {
                        if !self.entities.contains(server_id) {
                            removed_entities.push(*client_id);
                            mapping.remove(server_id);
                        }
                    }

                    removed_entities
                };

                for id in removed_entities.iter() {
                    all_storages.delete_entity(*id);
                }
            })
            .unwrap();
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct NetworkComponent<T: NetworkState> {
    bitmask: Vec<bool>,
    values: Vec<T::State>,
}

impl<T: 'static + Sync + Send + Clone + NetworkState> NetworkComponent<T> {
    fn from_world(entities_id: &[EntityId], world: &World) -> NetworkComponent<T> {
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

    fn apply_in_world(&self, entities_id: &[EntityId], world: &World) {
        let entities_state = entities_id
            .iter()
            .zip(self.bitmask.iter())
            .filter_map(|(id, &presence)| if presence { Some(id) } else { None })
            .zip(self.values.clone().into_iter());

        // TODO: instead of filter map we could remove component when is None
        world
            .run(
                |mut entities: EntitiesViewMut,
                 mut components: ViewMut<T>,
                 mut mapping: UniqueViewMut<EntityMapping>| {
                    for (entity_id, state) in entities_state {
                        if let Some(mapped_id) = mapping.get(entity_id) {
                            if let Ok(mut component) = (&mut components).get(*mapped_id) {
                                component.update_from_state(state);
                            } else {
                                let component = T::from_state(state);
                                entities.add_component(*mapped_id, &mut components, component);
                            }
                        } else {
                            let component = T::from_state(state);
                            let client_entity_id = entities.add_entity(&mut components, component);
                            mapping.insert(*entity_id, client_entity_id);
                        }
                    }
                },
            )
            .unwrap();
    }
}

pub enum Messages {
    PlayerInput(PlayerInput),
    ServerFrame(Box<ServerFrame>),
}

#[derive(Debug, Clone)]
pub struct AnimationController {
    pub animation: Animation,
    pub frame: u8,
    speed: Duration,
    last_updated: Instant,
    total_frames: u8,
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
