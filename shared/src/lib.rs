use renet::channel::ChannelConfig;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::time::Duration;

pub fn channels() -> HashMap<u8, ChannelConfig> {
    let mut channel_config = ChannelConfig::default();
    channel_config.message_resend_time = Duration::from_millis(100);

    let mut channels_config = HashMap::new();
    channels_config.insert(0, channel_config);
    channels_config
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: u64,
    pub color: (f32, f32, f32),
    pub x: i16,
    pub y: i16,
}

impl Player {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            x: 100,
            y: 100,
            color: (1.0, 0.0, 0.0),
        }
    }

    pub fn update_from_input(&mut self, input: &PlayerInput) {
        self.x += (input.right as i16 - input.left as i16) * 4;
        self.y += (input.down as i16 - input.up as i16) * 4;
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
    pub players: Vec<Player>,
}

pub enum Messages {
    PlayerInput(PlayerInput),
    ServerFrame(ServerFrame),
}
