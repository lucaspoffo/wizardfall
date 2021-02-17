use macroquad::prelude::*;
use shared::{
    animation::AnimationController,
    player::{Player, PlayerInput},
    Health, Transform,
};

use shipyard::*;

use crate::animation::{AnimationTextures, TextureAnimation, Textures};
use crate::ui::mouse_to_screen;
use crate::ClientInfo;
use crate::UPSCALE;

pub fn draw_players(
    player_texture: UniqueView<AnimationTextures>,
    textures: UniqueView<Textures>,
    players: View<Player>,
    transforms: View<Transform>,
    health: View<Health>,
    animation_controller: View<AnimationController>,
) {
    for (player, transform, animation_controller, player_health) in
        (&players, &transforms, &animation_controller, &health).iter()
    {
        let texture_animation = player_texture.0.get("player").unwrap();
        let x = transform.position.x;
        let y = transform.position.y;
        let flip_x =
            player.direction.angle_between(Vec2::unit_x()).abs() > std::f32::consts::PI / 2.0;

        texture_animation.draw(x, y, flip_x, animation_controller);

        // Draw wand
        let center_x = x + (texture_animation.width as f32 / 2.0);
        let center_y = y + 2.0 + (texture_animation.height as f32 / 2.0);

        // let wand_size = 12.0;
        // let wand_x = center_x + player.direction.x * wand_size;
        // let wand_y = center_y + player.direction.y * wand_size;

        /*
        draw_line(center_x, center_y, wand_x, wand_y, 3.0, YELLOW);
        if player.fireball_charge > 0. {
            draw_circle(wand_x, wand_y, 3.0 + player.fireball_charge * 4., RED);
        } else if player.fireball_cooldown.is_finished() {
            draw_circle(wand_x, wand_y, 3.0, PURPLE);
        } else {
            draw_circle(wand_x, wand_y, 3.0, BLACK);
        }
        */
        let wand_texture = textures.0.get("wand").unwrap();
        let wand_params = DrawTextureParams {
            dest_size: Some(vec2(16. * UPSCALE, 16. * UPSCALE)),
            pivot: Some(vec2(center_x * UPSCALE, center_y * UPSCALE)),
            rotation: -player.direction.angle_between(Vec2::unit_x()),
            ..Default::default()
        };
        draw_texture_ex(
            *wand_texture,
            (center_x - 6.) * UPSCALE,
            (center_y - 12.) * UPSCALE,
            WHITE,
            wand_params,
        );

        // Draw Player Health
        let current_life_percent = (player_health.current as f32) / (player_health.max as f32);
        let max_bar_width = 8.;
        let bar_width = current_life_percent * max_bar_width;
        let health_x = x - 2.;
        let health_y = y - 2.;
        draw_rectangle(
            health_x * UPSCALE,
            health_y * UPSCALE,
            max_bar_width * UPSCALE,
            2. * UPSCALE,
            RED,
        );
        draw_rectangle(
            health_x * UPSCALE,
            health_y * UPSCALE,
            bar_width * UPSCALE,
            2. * UPSCALE,
            GREEN,
        );
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

    let direction = (mouse_to_screen() - (transform.position + vec2(16., 24.))).normalize();

    let up = is_key_down(KeyCode::W) || is_key_down(KeyCode::Up);
    let down = is_key_down(KeyCode::S) || is_key_down(KeyCode::Down);
    let left = is_key_down(KeyCode::A) || is_key_down(KeyCode::Left);
    let right = is_key_down(KeyCode::D) || is_key_down(KeyCode::Right);

    let jump = is_key_down(KeyCode::Space);
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
    let idle_texture: Texture2D = load_texture("../levels/atlas/Wizard.png").await;
    set_texture_filter(idle_texture, FilterMode::Nearest);

    let player_animation = TextureAnimation::new(idle_texture, 16, 16, vec2(-1., -3.));
    let wand = load_texture("Arm.png").await;
    set_texture_filter(wand, FilterMode::Nearest);

    world
        .borrow::<UniqueViewMut<Textures>>()
        .unwrap()
        .0
        .insert("wand".to_string(), wand);

    world
        .borrow::<UniqueViewMut<AnimationTextures>>()
        .unwrap()
        .0
        .insert("player".to_string(), player_animation);
}
