use macroquad::prelude::*;
use shared::math::remap;
use shared::timer::Timer;
use shared::{ClientInfo, LobbyInfo, PlayersScore};
use shipyard::UniqueView;

use std::net::SocketAddr;
use std::time::Duration;

use crate::{RX, RY, UPSCALE};

pub fn draw_text_upscaled(text: &str, x: f32, y: f32, font_size: f32, color: Color) {
    draw_text(text, x * UPSCALE, y * UPSCALE, font_size * UPSCALE, color);
}

pub fn draw_rectangle_lines_upscaled(x: f32, y: f32, w: f32, h: f32, thickness: f32, color: Color) {
    draw_rectangle_lines(
        x * UPSCALE,
        y * UPSCALE,
        w * UPSCALE,
        h * UPSCALE,
        thickness * UPSCALE,
        color,
    );
}

pub struct UiState {
    pub connect_error: Option<String>,
    input_name: InputState,
    input_ip: InputState,
    connection_text_timer: Timer,
    dot_count: usize,
}

impl Default for UiState {
    fn default() -> Self {
        let input_name = InputState {
            rect: Rect::new(RX / 2. - 50., 80.0, 150., 20.),
            label: "Name:".into(),
            ..Default::default()
        };

        let input_ip = InputState {
            rect: Rect::new(RX / 2. - 50., 110.0, 150., 20.),
            label: "IP:".into(),
            text: "127.0.0.1:5000".into(),
            ..Default::default()
        };

        Self {
            connect_error: None,
            input_ip,
            input_name,
            connection_text_timer: Timer::new(Duration::from_millis(400)),
            dot_count: 0,
        }
    }
}

struct InputState {
    focused: bool,
    text: String,
    label: String,
    rect: Rect,
    max_text_length: usize,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            max_text_length: 20,
            label: "Label:".into(),
            rect: Rect::new(0., 0., 300., 30.),
            focused: false,
            text: String::new(),
        }
    }
}

impl InputState {
    fn update(&mut self, mouse_position: Vec2) {
        if is_mouse_button_pressed(MouseButton::Left) {
            mouse_to_screen();
            self.focused = self.rect.contains(mouse_position);
            while get_char_pressed().is_some() {}
        }

        while self.text.len() > self.max_text_length {
            self.text.pop();
        }

        if self.focused {
            while let Some(char) = get_char_pressed() {
                self.text.push(char);
            }
            if is_key_pressed(KeyCode::Backspace) {
                self.text.pop();
            }
        }
    }

    fn draw(&self) {
        let color = if self.focused { YELLOW } else { WHITE };

        draw_text_upscaled(&self.label, self.rect.x - 40., self.rect.y - 2., 16., WHITE);

        draw_rectangle_lines_upscaled(
            self.rect.x,
            self.rect.y,
            self.rect.w,
            self.rect.h,
            2.0,
            color,
        );
        draw_text_upscaled(&self.text, self.rect.x + 4., self.rect.y - 2., 16., WHITE);
    }
}

fn draw_button(rect: Rect, text: &str) -> bool {
    let hover = rect.contains(mouse_to_screen());
    let clicked = hover && is_mouse_button_pressed(MouseButton::Left);

    let color = if clicked {
        YELLOW
    } else if hover {
        DARKGRAY
    } else {
        WHITE
    };

    draw_rectangle_lines_upscaled(rect.x, rect.y, rect.w, rect.h, 2.0, color);
    draw_text_upscaled(text, rect.x + 4., rect.y - 2., 16., color);

    clicked
}

pub fn draw_connect_menu(ui: &mut UiState) -> Option<SocketAddr> {
    let mouse_position = mouse_to_screen();
    ui.input_ip.update(mouse_position);
    ui.input_ip.draw();

    ui.input_name.update(mouse_position);
    ui.input_name.draw();

    if let Some(error) = ui.connect_error.as_ref() {
        draw_text_upscaled(&error, (RX - 100.) / 2., 130., 16., RED);
    }

    if draw_button(Rect::new((RX - 60.) / 2., 160.0, 58., 20.), &"connect") {
        if let Ok(ip) = ui.input_ip.text.parse() {
            return Some(ip);
        } else {
            ui.connect_error = Some("Invalid IP address".into());
        }
    }
    None
}

pub fn mouse_to_screen() -> Vec2 {
    let mut pos: Vec2 = mouse_position().into();
    
    let desired_aspect_ratio = RX / RY;
    let current_aspect_ratio = screen_width() / screen_height();

    let viewport_width = screen_height() * desired_aspect_ratio;
    let viewport_height = screen_width() / desired_aspect_ratio;

    if current_aspect_ratio > desired_aspect_ratio {
        let start = -remap(
            (screen_width() - viewport_width) / 2.,
            0.0..=viewport_width,
            0.0..=RX,
        );
        let end = RX - start;
        pos.x = remap(pos.x, 0.0..=screen_width(), start..=end);
        pos.y = remap(pos.y, 0.0..=screen_height(), 0.0..=RY);
    } else if current_aspect_ratio < desired_aspect_ratio {
        let start = -remap(
            (screen_height() - viewport_height) / 2.,
            0.0..=viewport_height,
            0.0..=RY,
        );
        let end = RY - start;
        pos.y = remap(pos.y, 0.0..=screen_height(), start..=end);
        pos.x = remap(pos.x, 0.0..=screen_width(), 0.0..=RX);
    }

    pos
}

pub fn draw_connection_screen(ui: &mut UiState) {
    if ui.connection_text_timer.is_finished() {
        ui.connection_text_timer.reset();
        ui.dot_count += 1;
        ui.dot_count %= 4;
    }

    let text = format!("Connecting{}", ".".repeat(ui.dot_count));
    draw_text_upscaled(&text, (RX - 140.) / 2. , (RY - 40.) / 2., 32., WHITE);
}

pub fn draw_lobby(lobby_info: &LobbyInfo, id: u64) -> bool {
    let mut response = false;
    let mut clients: Vec<(&u64, &ClientInfo)> = lobby_info.clients.iter().collect();
    clients.sort_by(|a, b| a.0.cmp(b.0));
    for (i, (&client_id, client_info)) in clients.iter().enumerate() {
        let x = 10. + i as f32 * 80.;
        draw_text_upscaled(&client_id.to_string(), x, 20., 16., WHITE);
        let text = if client_info.ready {
            "ready"
        } else {
            "waiting"
        };
        if client_id == id {
            response = draw_button(Rect::new(x + 5., 70., 60., 20.), &text);
        } else {
            draw_text_upscaled(&text, x + 5., 68., 16., WHITE);
        }
    }
    response
}

pub fn draw_score(players_score: UniqueView<PlayersScore>) {
    let mut offset_x = 0.;
    for (client_id, score) in players_score.score.iter() {
        let text = format!("{}: {}", client_id, score);
        draw_rectangle_lines_upscaled(10. + offset_x, 10., 70., 16., 2., WHITE);
        draw_text_upscaled(&text, 14. + offset_x, 10., 10., WHITE);
        offset_x += 80.;
    }
}
