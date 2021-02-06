use std::time::Duration;

use serde::{Deserialize, Serialize};
use shipyard::EntityId;

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
}

impl Projectile {
    pub fn new(projectile_type: ProjectileType, owner: EntityId) -> Self {
        Self {
            owner,
            projectile_type,
            duration: Duration::from_secs(2),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectileState {
    projectile_type: ProjectileType,
}
