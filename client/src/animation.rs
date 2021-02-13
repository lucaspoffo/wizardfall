use macroquad::prelude::*;
use shared::animation::{Animation, AnimationController};
use std::collections::HashMap;

pub type AnimationTexture = HashMap<Animation, TextureAnimation>;

pub struct TextureAnimation {
    pub texture: Texture2D,
    pub width: u32,
    pub height: u32,
    pub h_frames: u8,
    pub v_frames: u8,
}

impl TextureAnimation {
    pub fn new(texture: Texture2D, width: u32, height: u32, h_frames: u8, v_frames: u8) -> Self {
        Self {
            texture,
            width,
            height,
            h_frames,
            v_frames,
        }
    }

    pub fn draw(&self, x: f32, y: f32, flip_x: bool, animation_controller: &AnimationController) {
        if animation_controller.frame > self.h_frames * self.v_frames {
            println!(
                "Invalid animation frame {} for texture player",
                animation_controller.frame
            );
            return;
        }

        let texture_x = (animation_controller.frame % self.h_frames) as u32 * self.width;
        let texture_y = (animation_controller.frame / self.h_frames) as u32 * self.height;
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
        }
        let params = DrawTextureParams {
            source: Some(draw_rect),
            dest_size: Some(vec2(x_size, self.height as f32)),
            ..Default::default()
        };

        draw_texture_ex(self.texture, draw_x, y, WHITE, params);
    }
}
