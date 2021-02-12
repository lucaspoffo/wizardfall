use std::time::Duration;

use serde::{Deserialize, Serialize};
use shipyard::EntityId;

use glam::Vec2;

use derive::NetworkState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectileType {
    Fireball,
}

#[derive(Debug, Clone, Serialize, Deserialize, NetworkState)]
pub struct Projectile {
    pub projectile_type: ProjectileType,
    pub owner: EntityId,
    pub duration: Duration,
    pub speed: Vec2,
}

impl Projectile {
    pub fn new(projectile_type: ProjectileType, speed: Vec2, owner: EntityId) -> Self {
        Self {
            owner,
            speed,
            projectile_type,
            duration: Duration::from_secs(2),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectileState {
    projectile_type: ProjectileType,
}
