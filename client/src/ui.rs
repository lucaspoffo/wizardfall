use macroquad::prelude::*;
use shared::math::remap;
use shared::{ClientInfo, LobbyInfo, PlayersScore};
use shipyard::UniqueView;

use std::net::SocketAddr;

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
    input_ip: TextInputState,
}

impl Default for UiState {
    fn default() -> Self {
        let input_ip = TextInputState {
            label: "IP:".into(),
            text: "127.0.0.1:5000".into(),
            ..Default::default()
        };

        Self {
            connect_error: None,
            input_ip,
        }
    }
}

struct TextInputState {
    focused: bool,
    text: String,
    label: String,
    max_text_length: usize,
}

impl Default for TextInputState {
    fn default() -> Self {
        Self {
            max_text_length: 20,
            label: "Label:".into(),
            focused: false,
            text: String::new(),
        }
    }
}

impl TextInputState {
    fn update(&mut self, rect: Rect, mouse_position: Vec2) {
        if is_mouse_button_pressed(MouseButton::Left) {
            self.focused = rect.contains(mouse_position);
            // Clear input queue
            while let Some(_) = get_char_pressed() {}
        }

        while self.text.len() > self.max_text_length {
            self.text.pop();
        }

        if self.focused {
            while let Some(c) = get_char_pressed() {
                self.text.push(c);
            }
            if is_key_pressed(KeyCode::Backspace) {
                self.text.pop();
            }
        }
    }

    fn draw(&self, rect: Rect) {
        let color = if self.focused { YELLOW } else { WHITE };

        draw_text_upscaled(
            &self.label,
            rect.x - 40.,
            rect.y + rect.h / 2. + 4.,
            16.,
            WHITE,
        );

        draw_rectangle_lines_upscaled(rect.x, rect.y, rect.w, rect.h, 2.0, color);
        draw_text_upscaled(
            &self.text,
            rect.x + 4.,
            rect.y + rect.h / 2. + 4.,
            16.,
            WHITE,
        );
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
    draw_text_upscaled(text, rect.x + 4., rect.y + rect.h / 2. + 4., 16., color);

    clicked
}

pub struct ConnectMenuResponse {
    pub connect: bool,
    pub host: bool,
    pub addr: Option<SocketAddr>,
}

pub fn draw_connect_menu(ui: &mut UiState) -> ConnectMenuResponse {
    let mouse_position = mouse_to_screen();
    let rect = Rect::new(RX / 2. - 50., 50.0, 150., 20.);
    ui.input_ip.update(rect, mouse_position);
    ui.input_ip.draw(rect);

    if let Some(error) = ui.connect_error.as_ref() {
        let mut offset = 0.;
        for error_line in error.split(':') {
            draw_text_upscaled(error_line.trim(), (RX - 100.) / 2., 82. + offset, 12., RED);
            offset += 10.;
        }
    }

    let host = draw_button(Rect::new((RX - 36.) / 2., 100.0, 36., 20.), &"host");
    let connect = draw_button(Rect::new((RX - 58.) / 2., 130.0, 58., 20.), &"connect");

    let ip_error = String::from("Invalid :IP address");
    let mut addr = None;
    if let Ok(ip) = ui.input_ip.text.parse() {
        addr = Some(ip);
        if ui.connect_error == Some(ip_error) {
            ui.connect_error = None;
        }
    } else {
        ui.connect_error = Some(ip_error);
    }

    ConnectMenuResponse {
        connect,
        host,
        addr,
    }
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

pub fn draw_lobby(lobby_info: &LobbyInfo, id: SocketAddr) -> bool {
    let mut response = false;
    let mut clients: Vec<(&SocketAddr, &ClientInfo)> = lobby_info.clients.iter().collect();
    clients.sort_by(|a, b| a.0.cmp(b.0));
    for (i, (&client_id, client_info)) in clients.iter().enumerate() {
        let x = 10. + i as f32 * 80.;
        draw_text_upscaled(&client_id.port().to_string(), x + 16., 20., 16., WHITE);
        let text = if client_info.ready {
            "ready"
        } else {
            "waiting"
        };
        if client_id.port() == id.port() {
            response = draw_button(Rect::new(x + 5., 30., 60., 20.), &text);
        } else {
            draw_text_upscaled(&text, x + 9., 44., 16., WHITE);
        }
    }
    response
}

pub fn draw_score(players_score: UniqueView<PlayersScore>) {
    let mut offset_x = 0.;
    for (client_id, score) in players_score.score.iter() {
        let text = format!("{}: {}", client_id.port(), score);
        draw_rectangle_lines_upscaled(10. + offset_x, 4., 50., 16., 2., WHITE);
        draw_text_upscaled(&text, 14. + offset_x, 14., 10., WHITE);
        offset_x += 60.;
    }
}
