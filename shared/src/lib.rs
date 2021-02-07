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

#[derive(Debug, Eq, PartialEq)]
pub enum EntityType {
    Unknown,
    Player,
    Fireball,
    Wall,
}

impl From<u8> for EntityType {
    fn from(value: u8) -> Self {
        use crate::EntityType::*;

        match value {
            1 => Player,
            2 => Fireball,
            3 => Wall,
            _ => Unknown,
        }
    }
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

// Used adding aditional info for the colliders.
// Making it easier to handle collisions.
#[derive(Debug)]
pub struct EntityUserData {
    pub entity_id: EntityId,
    pub entity_type: EntityType,
}

impl EntityUserData {
    pub fn new(id: EntityId, entity_type: EntityType) -> Self {
        Self {
            entity_id: id,
            entity_type,
        }
    }

    pub fn from_user_data(user_data: u128) -> Self {
        let entity_id = user_data as u64;
        let entity_id = EntityId::from_inner(entity_id).unwrap();
        let entity_type: EntityType = ((user_data >> 64) as u8).into();

        Self {
            entity_id,
            entity_type,
        }
    }
}

impl Into<u128> for EntityUserData {
    fn into(self) -> u128 {
        let entity_id = self.entity_id.inner() as u128;
        let entity_type = (self.entity_type as u128) << 64;
        entity_type | entity_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_user_data() {
        let entity_id = EntityId::from_inner(127).unwrap();
        let entity_user_data = EntityUserData::new(entity_id, EntityType::Wall);

        let user_data: u128 = entity_user_data.into();

        let e = EntityUserData::from_user_data(user_data);
        assert_eq!(e.entity_type, EntityType::Wall);
        assert_eq!(e.entity_id, entity_id);
    }
}
