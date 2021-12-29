use std::{collections::HashMap, net::SocketAddr, time::Duration};

use glam::{vec2, Vec2};
use serde::{Deserialize, Serialize};
use shipyard::EntityId;
use renet_udp::renet::channel::{ChannelConfig, ReliableChannelConfig, UnreliableChannelConfig};

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

#[derive(Debug, Clone, Serialize, Deserialize, NetworkState)]
pub struct Transform {
    pub position: Vec2,
    pub rotation: f32,
}

#[repr(u8)]
pub enum Channel {
    Reliable = 0,
    ReliableCritical = 1,
    Unreliable = 2,
}

impl Channel {
    pub fn id(self) -> u8 {
        self as u8
    }
}

pub fn channels_config() -> Vec<ChannelConfig> {
    let reliable = ChannelConfig::Reliable(ReliableChannelConfig {
        channel_id: Channel::Reliable.id(),
        ..Default::default()
    });
    let reliable_critical = ChannelConfig::Reliable(ReliableChannelConfig {
        channel_id: Channel::ReliableCritical.id(),
        message_resend_time: Duration::ZERO,
        ..Default::default()
    });
    let unreliable = ChannelConfig::Unreliable(UnreliableChannelConfig {
        channel_id: Channel::Unreliable.id(),
        ..Default::default()
    });
    vec![reliable, reliable_critical, unreliable]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LobbyInfo {
    pub clients: HashMap<SocketAddr, ClientInfo>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct PlayersScore {
    pub score: HashMap<SocketAddr, u8>,
    pub updated: bool
}

#[derive(Debug, Clone, Serialize, Deserialize, NetworkState)]
pub struct Health {
    pub max: u8,
    pub current: u8,
    pub killer: Option<SocketAddr>,
}

impl Health {
    pub fn new(max: u8) -> Self {
        Self {
            max,
            current: max,
            killer: None,
        }
    }

    pub fn take_damage(&mut self, damage: u8, damage_dealer: Option<SocketAddr>) {
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

