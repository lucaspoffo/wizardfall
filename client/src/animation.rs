use macroquad::prelude::*;
use shared::animation::AnimationController;
use std::collections::HashMap;

use crate::UPSCALE;

pub struct AnimationTextures(pub HashMap<String, TextureAnimation>);
pub struct Textures(pub HashMap<String, Texture2D>);

pub struct TextureAnimation {
    pub texture: Texture2D,
    pub width: u32,
    pub height: u32,
    pub offset: Vec2,
}

impl TextureAnimation {
    pub fn new(texture: Texture2D, width: u32, height: u32, offset: Vec2) -> Self {
        Self {
            texture,
            width,
            height,
            offset,
        }
    }

    pub fn draw(&self, x: f32, y: f32, flip_x: bool, animation_controller: &AnimationController) {
        let animation = &animation_controller.animations[animation_controller.current_animation];
        let texture_x = animation_controller.frame * self.width;
        let texture_y = animation.row * self.height;
        let draw_rect = Rect::new(
            texture_x as f32,
            texture_y as f32,
            self.width as f32,
            self.height as f32,
        );

        let mut x_size = self.width as f32;
        let mut draw_x = x;
        if flip_x {
            x_size *= -1.0;
            draw_x += self.width as f32;
            draw_x -= self.width as f32 / 2. + self.offset.x;
        } else {
            draw_x += self.offset.x;
        }

        let params = DrawTextureParams {
            source: Some(draw_rect),
            dest_size: Some(vec2(x_size * UPSCALE, self.height as f32 * UPSCALE)),
            ..Default::default()
        };

        draw_texture_ex(
            self.texture,
            draw_x * UPSCALE,
            (y + self.offset.y) * UPSCALE,
            WHITE,
            params,
        );
    }
}
