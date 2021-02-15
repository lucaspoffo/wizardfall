use ldtk_rust::Project;
use macroquad::prelude::*;
use shipyard::World;

use crate::physics::Physics;

pub const BASE_DIR: &str = "../levels/";
pub const PROJECT_FILE: &str = "Typical_TopDown_example.ldtk";

pub fn load_project() -> Project {
    Project::new(BASE_DIR.to_owned() + PROJECT_FILE)
}


pub struct PlayerRespawnPoints(pub Vec<Vec2>);

pub fn load_level_collisions(world: &mut World) {
    let project = load_project();

    let entity_layer = project.levels[0]
        .layer_instances
        .as_ref()
        .unwrap()
        .iter()
        .find(|l| l.identifier == *"Entities")
        .unwrap();

    let mut player_respawn_points = PlayerRespawnPoints(vec![]);

    for entity in entity_layer.entity_instances.iter() {
        println!("Entity identifier: {}", entity.identifier);
        println!("Entity px: {:?}", entity.px);
        player_respawn_points
            .0
            .push(vec2(entity.px[0] as f32, entity.px[1] as f32));
    }

    world.add_unique(player_respawn_points).unwrap();

    let mut physics: Physics = Physics::new();

    let collision_layer = project.levels[0]
        .layer_instances
        .as_ref()
        .unwrap()
        .iter()
        .find(|l| l.identifier == *"Collisions")
        .unwrap();

    let grid_size = vec2(
        collision_layer.grid_size as f32,
        collision_layer.grid_size as f32,
    );

    let grid_width = collision_layer.c_wid as usize;
    let grid_height = collision_layer.c_hei as usize;
    let mut collisions = vec![false; grid_width * grid_height];

    for tile in collision_layer.int_grid.iter() {
        collisions[tile.coord_id as usize] = true;
    }

    physics.add_static_tiled_layer(
        collisions,
        grid_size.x,
        grid_size.y,
        grid_width,
        1,
        GREEN,
    );

    world.add_unique(physics).unwrap();
}

