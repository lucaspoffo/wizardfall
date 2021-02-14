use macroquad::prelude::*;
use shared::{
    animation::AnimationController,
    player::{Player, PlayerAnimation, PlayerInput},
    Health, Transform,
};

use shipyard::*;
use std::collections::HashMap;

use crate::animation::{AnimationTexture, TextureAnimation};
use crate::ui::mouse_to_screen;
use crate::ClientInfo;

pub fn draw_players(
    player_texture: UniqueView<AnimationTexture>,
    players: View<Player>,
    transforms: View<Transform>,
    health: View<Health>,
    animation_controller: View<AnimationController>,
) {
    for (player, transform, animation_controller, player_health) in
        (&players, &transforms, &animation_controller, &health).iter()
    {
        let texture_animation = player_texture.get(&animation_controller.animation).unwrap();
        let x = transform.position.x;
        let y = transform.position.y;
        let flip_x =
            player.direction.angle_between(Vec2::unit_x()).abs() > std::f32::consts::PI / 2.0;

        texture_animation.draw(x, y, flip_x, animation_controller);

        // Draw wand
        let center_x = x + (texture_animation.width as f32 / 2.0);
        let center_y = y + 4.0 + (texture_animation.height as f32 / 2.0);

        let wand_size = 12.0;
        let wand_x = center_x + player.direction.x * wand_size;
        let wand_y = center_y + player.direction.y * wand_size;

        draw_line(center_x, center_y, wand_x, wand_y, 3.0, YELLOW);
        if player.fireball_charge > 0. {
            draw_circle(wand_x, wand_y, 3.0 + player.fireball_charge * 4., RED);
        } else if player.fireball_cooldown.is_finished() {
            draw_circle(wand_x, wand_y, 3.0, PURPLE);
        } else {
            draw_circle(wand_x, wand_y, 3.0, BLACK);
        }
        // Draw Player Health
        let current_life_percent = (player_health.current as f32) / (player_health.max as f32);
        let max_bar_width = 40.;
        let bar_width = current_life_percent * max_bar_width;
        let health_x = x - 5.;
        let health_y = y - 5.;
        draw_rectangle(health_x, health_y, max_bar_width, 5., RED);
        draw_rectangle(health_x, health_y, bar_width, 5., GREEN);
    }
}

pub fn player_input(
    transforms: View<Transform>,
    client_info: UniqueView<ClientInfo>,
) -> PlayerInput {
    if client_info.entity_id.is_none() {
        return PlayerInput::default();
    }

    let entity_id = client_info.entity_id.unwrap();
    let transform = transforms.get(entity_id).unwrap();

    let direction = (mouse_to_screen() - transform.position).normalize();

    let up = is_key_down(KeyCode::W) || is_key_down(KeyCode::Up);
    let down = is_key_down(KeyCode::S) || is_key_down(KeyCode::Down);
    let left = is_key_down(KeyCode::A) || is_key_down(KeyCode::Left);
    let right = is_key_down(KeyCode::D) || is_key_down(KeyCode::Right);

    let jump = is_key_pressed(KeyCode::Space);
    let dash = is_key_pressed(KeyCode::LeftShift);
    let fire = is_mouse_button_down(MouseButton::Left);
    PlayerInput {
        up,
        down,
        left,
        right,
        jump,
        fire,
        dash,
        direction,
    }
}

pub fn track_client_entity(
    mut players: ViewMut<Player>,
    mut client_info: UniqueViewMut<ClientInfo>,
) {
    for (entity_id, player) in players.inserted().iter().with_id() {
        if player.client_id == client_info.client_id {
            client_info.entity_id = Some(entity_id);
        }
    }

    for (_, player) in players.take_deleted().iter() {
        if player.client_id == client_info.client_id {
            client_info.entity_id = None;
        }
    }
}

pub async fn load_player_texture(world: &mut World) {
    let mut animations: AnimationTexture = HashMap::new();
    let idle_texture: Texture2D = load_texture("Blue_witch/B_witch_idle.png").await;
    set_texture_filter(idle_texture, FilterMode::Nearest);
    let run_texture: Texture2D = load_texture("Blue_witch/B_witch_run.png").await;
    set_texture_filter(run_texture, FilterMode::Nearest);

    let idle_animation = TextureAnimation::new(idle_texture, 32, 48, 1, 6);
    let run_animation = TextureAnimation::new(run_texture, 32, 48, 1, 8);

    animations.insert(PlayerAnimation::Idle.into(), idle_animation);
    animations.insert(PlayerAnimation::Run.into(), run_animation);
    world.add_unique(animations).unwrap();
}
