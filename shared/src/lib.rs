use std::collections::HashMap;
use std::time::Duration;

use glam::{vec2, Vec2};
use renet::channel::{
    ChannelConfig, ReliableOrderedChannelConfig, UnreliableUnorderedChannelConfig,
};
use serde::{Deserialize, Serialize};
use shipyard::EntityId;

use derive::NetworkState;

pub mod animation;
pub mod ldtk;
pub mod message;
pub mod network;
pub mod player;
pub mod projectile;
pub mod timer;
pub mod physics;
pub mod math;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub ready: bool,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self { ready: false }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LobbyInfo {
    pub clients: HashMap<u64, ClientInfo>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct PlayersScore {
    pub score: HashMap<u64, u8>,
    pub updated: bool
}

#[derive(Debug, Clone, Serialize, Deserialize, NetworkState)]
pub struct Health {
    pub max: u8,
    pub current: u8,
    pub killer: Option<u64>,
}

impl Health {
    pub fn new(max: u8) -> Self {
        Self {
            max,
            current: max,
            killer: None,
        }
    }

    pub fn take_damage(&mut self, damage: u8, damage_dealer: Option<u64>) {
        if self.is_dead() {
            return;
        }

        if let Some(current) = self.current.checked_sub(damage) {
            self.current = current;
        } else {
            self.current = 0;
        }

        if self.is_dead() {
            self.killer = damage_dealer;
        }
    }

    pub fn is_dead(&self) -> bool {
        self.current == 0
    }
}

