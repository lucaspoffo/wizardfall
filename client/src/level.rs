use ldtk_rust::{Project, TileInstance};
use macroquad::prelude::*;
use shared::ldtk::{load_project, BASE_DIR};
use shipyard::{UniqueView, UniqueViewMut, World};
use std::collections::HashMap;

use crate::UPSCALE;

#[derive(Debug)]
pub struct TextureAtlas {
    texture: Texture2D,
    tile_size: Vec2,
    grid_size: Vec2,
}

impl TextureAtlas {
    pub fn new(texture: Texture2D, tile_size: Vec2, grid_size: Vec2) -> Self {
        Self {
            texture,
            tile_size,
            grid_size,
        }
    }

    pub fn draw_tile(&self, tile: &TileInstance) {
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
        let pos_x = tile.px[0] as f32;
        let pos_y = tile.px[1] as f32;
        let mut draw_pos = vec2(pos_x, pos_y);
        if flip_x {
            dest_size.x *= -1.0;
            draw_pos.x += self.tile_size.x;
        }
        if flip_y {
            dest_size.y *= -1.0;
            draw_pos.y += self.tile_size.y;
        }

        let dest_size = Some(dest_size * UPSCALE);

        let params: DrawTextureParams = DrawTextureParams {
            source: Some(draw_rect),
            dest_size,
            ..Default::default()
        };

        draw_texture_ex(
            self.texture,
            draw_pos.x * UPSCALE,
            draw_pos.y * UPSCALE,
            WHITE,
            params,
        )
    }
}

#[derive(Debug)]
pub struct SpriteSheets(HashMap<i64, TextureAtlas>);

pub async fn load_project_and_assets(world: &World) {
    let project = load_project();
    let mut sprite_sheets = SpriteSheets(HashMap::new());
    for tileset in project.defs.as_ref().unwrap().tilesets.iter() {
        let texture_path = format!("{}{}", BASE_DIR, &tileset.rel_path[..]);
        println!("Texture path: {}", texture_path);
        let texture = load_texture(&texture_path).await;
        set_texture_filter(texture, FilterMode::Nearest);

        let tile_size = Vec2::new(tileset.tile_grid_size as f32, tileset.tile_grid_size as f32);
        let grid_size = vec2(
            (tileset.px_wid / tileset.tile_grid_size) as f32,
            (tileset.px_hei / tileset.tile_grid_size) as f32,
        );
        let texture_atlas = TextureAtlas::new(texture, tile_size, grid_size);

        sprite_sheets.0.insert(tileset.uid, texture_atlas);
    }

    let mut textures = world
        .borrow::<UniqueViewMut<HashMap<String, Texture2D>>>()
        .unwrap();
    for level in project.levels.iter() {
        if let Some(bg_rel_path) = level.bg_rel_path.as_ref() {
            if !textures.contains_key(bg_rel_path) {
                let texture_path = format!("{}{}", BASE_DIR, &bg_rel_path[..]);
                let bg_texture = load_texture(&texture_path).await;
                textures.insert(bg_rel_path.clone(), bg_texture);
            }
        }
    }

    world.add_unique(project).unwrap();
    world.add_unique(sprite_sheets).unwrap();
}

pub fn draw_level(
    project: UniqueView<Project>,
    sprite_sheets: UniqueView<SpriteSheets>,
    textures: UniqueView<HashMap<String, Texture2D>>,
) {
    // Draw background
    if let Some(bg_path) = project.levels[0].bg_rel_path.as_ref() {
        if let Some(bg_texture) = textures.get(bg_path) {
            let dest_size = vec2(bg_texture.width(), bg_texture.height());
            let dest_size = Some(dest_size * UPSCALE);

            let params: DrawTextureParams = DrawTextureParams {
                dest_size,
                ..Default::default()
            };
            draw_texture_ex(*bg_texture, 0., 0., WHITE, params);
        }
    }

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

        // Finally we match on the four possible kinds of Layer Instances and
        // handle each accordingly.
        match &layer.layer_instance_type[..] {
            "Tiles" => {
                //println!("Generating Tile Layer: {}", layer.identifier);
                for tile in layer.grid_tiles.iter().rev() {
                    sprite_sheet.draw_tile(&tile);
                }
            }
            "AutoLayer" => {
                //println!("Generating AutoTile Layer: {}", layer.identifier);
                for tile in layer.auto_layer_tiles.iter() {
                    sprite_sheet.draw_tile(&tile);
                }
            }
            "IntGrid" => {
                // println!("Generating Entities Layer: {}", layer.identifier);
                for tile in layer.auto_layer_tiles.iter() {
                    sprite_sheet.draw_tile(&tile);
                }
            }
            _ => {
                //println!("Not Implemented: {}", layer.identifier);
            }
        }
    }
}
