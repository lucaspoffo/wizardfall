use std::time::Duration;

use serde::{Deserialize, Serialize};

use derive::NetworkState;

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
