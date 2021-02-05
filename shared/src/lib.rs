use std::collections::HashMap;
use std::time::Duration;

use renet::channel::{
    ChannelConfig, ReliableOrderedChannelConfig, UnreliableUnorderedChannelConfig,
};
use serde::{Deserialize, Serialize};
use glam::{vec2, Vec2};
use shipyard::EntityId;

use derive::NetworkState;

pub mod ldtk;
pub mod network;
pub mod player;
pub mod animation;
pub mod message;
pub mod projectile;

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
