use ldtk_rust::{Project, TileInstance};
use macroquad::prelude::*;
use shipyard::{UniqueView, World};

use std::collections::HashMap;

use shipyard_rapier2d::{
    rapier::{
        dynamics::RigidBodyBuilder,
        geometry::ColliderBuilder
    }
};

#[derive(Debug)]
pub struct TextureAtlas {
    texture: Texture2D,
    tile_size: Vec2,
    grid_size: Vec2,
}

const PATH_ROOT: &str = "../levels/";

impl TextureAtlas {
    pub fn new(texture: Texture2D, tile_size: Vec2, grid_size: Vec2) -> Self {
        Self {
            texture,
            tile_size,
            grid_size,
        }
    }

    pub fn draw_tile(&self, tile: &TileInstance, grid_height: i64) {
        let draw_rect = Rect::new(
            tile.src[0] as f32,
            tile.src[1] as f32,
            self.tile_size.x,
            self.tile_size.y,
        );

        let mut flip_x = false;
        let mut flip_y = false;
        match tile.f {
            1 => flip_x = true,
            2 => flip_y = true,
            3 => {
                flip_x = true;
                flip_y = true
            }
            _ => (),
        }

        let mut dest_size = self.tile_size;
        let pos_x = (tile.px[0] as f32) + self.tile_size.x;
        let pos_y = (tile.px[1] - grid_height) as f32 + self.tile_size.y;
        let mut draw_pos = vec2(pos_x, pos_y);
        if flip_x {
            dest_size.x *= -1.0;
            draw_pos.x += self.tile_size.x;
        }
        if flip_y {
            dest_size.y *= -1.0;
            draw_pos.y += self.tile_size.y;
        }

        let dest_size = Some(dest_size);

        let params: DrawTextureParams = DrawTextureParams {
            source: Some(draw_rect),
            dest_size,
            ..Default::default()
        };
        draw_texture_ex(self.texture, draw_pos.x, draw_pos.y, WHITE, params)
    }
}

pub fn load_project() -> Project {
    Project::new("../levels/Typical_TopDown_example.ldtk".to_string())
}

#[derive(Debug)]
pub struct SpriteSheets(HashMap<i64, TextureAtlas>);

pub async fn load_project_and_assets(world: &World) {
    let project = load_project();
    let mut sprite_sheets = SpriteSheets(HashMap::new());
    for tileset in project.defs.as_ref().unwrap().tilesets.iter() {
        let texture_path = format!("{}{}", PATH_ROOT, &tileset.rel_path[..]);
        println!("Texture path: {}", texture_path);
        let texture = load_texture(&texture_path).await;

        let tile_size = Vec2::new(tileset.tile_grid_size as f32, tileset.tile_grid_size as f32);
        let grid_size = vec2(
            (tileset.px_wid / tileset.tile_grid_size) as f32,
            (tileset.px_hei / tileset.tile_grid_size) as f32,
        );
        let texture_atlas = TextureAtlas::new(texture, tile_size, grid_size);

        sprite_sheets.0.insert(tileset.uid, texture_atlas);
    }

    world.add_unique(project).unwrap();
    world.add_unique(sprite_sheets).unwrap();
}

pub fn load_level_collisions(world: &mut World) {
    let project = load_project();
    let collision_layer = project.levels[0]
        .layer_instances
        .as_ref()
        .unwrap()
        .iter()
        .find(|l| l.identifier == "Collisions".to_string())
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

    let mut rects = collapse(collisions, grid_width, grid_height);

    for mut rect in rects.iter_mut() {
        rect.h *= grid_size.x / 2.0;
        rect.w *= grid_size.y / 2.0;
        rect.x *= grid_size.x;
        rect.y *= grid_size.y;
        rect.x += rect.w;
        rect.y += rect.h;
         
        
        println!("Created collision: {:?}", rect);
        let rigid_body = RigidBodyBuilder::new_static().translation(rect.x, rect.y);
        let collider = ColliderBuilder::cuboid(rect.w, rect.h);
 
        world.add_entity((rigid_body, collider));
    }
}

pub fn draw_level(project: UniqueView<Project>, sprite_sheets: UniqueView<SpriteSheets>) {
    for (_, layer) in project.levels[0]
        .layer_instances
        .as_ref()
        .unwrap()
        .iter()
        .enumerate()
        .rev()
    {
        // This gets us a unique ID to refer to the tileset if there is one.
        // If there's no tileset, it's value is set to -1, which could be used
        // as a check. Currently it is used only as a key to the hash of asset
        // handles.
        let tileset_uid = layer.tileset_def_uid.unwrap_or(-1);
        let sprite_sheet = match sprite_sheets.0.get(&tileset_uid) {
            Some(x) => x,
            None => continue,
        };

        let grid_height = layer.c_hei * layer.grid_size;
        // Finally we match on the four possible kinds of Layer Instances and
        // handle each accordingly.
        match &layer.layer_instance_type[..] {
            "Tiles" => {
                //println!("Generating Tile Layer: {}", layer.identifier);
                for tile in layer.grid_tiles.iter().rev() {
                    sprite_sheet.draw_tile(&tile, grid_height);
                }
            }
            "AutoLayer" => {
                //println!("Generating AutoTile Layer: {}", layer.identifier);
                for tile in layer.auto_layer_tiles.iter() {
                    sprite_sheet.draw_tile(&tile, grid_height);
                }
            }
            _ => {
                //println!("Not Implemented: {}", layer.identifier);
            }
        }
    }
}

fn collapse(mut collisions: Vec<bool>, width: usize, height: usize) -> Vec<Rect> {
    assert_eq!(width * height, collisions.len());
    let mut rects: Vec<Rect> = vec![];

    for j in 0..height {
        for i in 0..width {
            let mut x_len = 0;
            let mut y_len = 1;
            while collisions[i + x_len + j * width] {
                if i + x_len < width {
                    x_len += 1;
                } else {
                    break;
                }
            }

            // No collision found
            if x_len == 0 {
                continue;
            }

            loop {
                let mut can_expand = true;
                // Can't expand down
                if j + y_len == height {
                    break;
                }

                for a in 0..x_len {
                    can_expand &= collisions[i + a + (j + y_len) * width];
                }

                if !can_expand {
                    break;
                }

                y_len += 1;
            }

            rects.push(Rect::new(i as f32, j as f32, x_len as f32, y_len as f32));

            // Remove collision
            for y in j..(j + y_len) {
                for x in i..(i + x_len) {
                    collisions[x + y * width] = false;
                }
            }
        }
    }
    rects
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collapse() {
        let collisions: Vec<bool> = vec![
            true, true, false, false, true, true, false, false, false, false, true, true, false,
            false, true, true,
        ];

        let rects = collapse(collisions, 4, 4);
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].x, 0.0);
        assert_eq!(rects[0].y, 0.0);
        assert_eq!(rects[0].w, 2.0);
        assert_eq!(rects[0].h, 2.0);
        assert_eq!(rects[1].x, 2.0);
        assert_eq!(rects[1].y, 2.0);
        assert_eq!(rects[1].w, 2.0);
        assert_eq!(rects[1].h, 2.0);
    }

    #[test]
    fn test_collapse_full() {
        let collisions: Vec<bool> = vec![
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true,
        ];

        let rects = collapse(collisions, 4, 4);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].x, 0.0);
        assert_eq!(rects[0].y, 0.0);
        assert_eq!(rects[0].w, 4.0);
        assert_eq!(rects[0].h, 4.0);
    }

    #[test]
    fn test_collapse_double() {
        let collisions: Vec<bool> = vec![
            false, false, true, true, true, true, true, true, true, true, true, true, true, true,
            false, false,
        ];

        let rects = collapse(collisions, 4, 4);
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].x, 2.0);
        assert_eq!(rects[0].y, 0.0);
        assert_eq!(rects[0].w, 2.0);
        assert_eq!(rects[0].h, 3.0);
        assert_eq!(rects[1].x, 0.0);
        assert_eq!(rects[1].y, 1.0);
        assert_eq!(rects[1].w, 2.0);
        assert_eq!(rects[1].h, 3.0);
    }

    #[test]
    fn test_collapse_width_height() {
        let collisions: Vec<bool> = vec![
            true, true, true, true, true, false, false, false, false, false, true, true, true,
            true, true, false, false, false, false, false,
        ];

        let rects = collapse(collisions, 5, 4);
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].x, 0.0);
        assert_eq!(rects[0].y, 0.0);
        assert_eq!(rects[0].w, 5.0);
        assert_eq!(rects[0].h, 1.0);
        assert_eq!(rects[1].x, 0.0);
        assert_eq!(rects[1].y, 2.0);
        assert_eq!(rects[1].w, 5.0);
        assert_eq!(rects[1].h, 1.0);
    }
}
