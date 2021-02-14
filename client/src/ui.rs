use macroquad::prelude::*;
use shared::math::remap;
use shared::timer::Timer;

use std::net::SocketAddr;
use std::time::Duration;

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
            rect: Rect::new((640. - 264.) / 2., 120.0, 300., 30.),
            label: "Name:".into(),
            ..Default::default()
        };

        let input_ip = InputState {
            rect: Rect::new((640. - 264.) / 2., 180.0, 300., 30.),
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

        draw_text(&self.label, self.rect.x - 70., self.rect.y - 8., 32., WHITE);

        draw_rectangle_lines(
            self.rect.x,
            self.rect.y,
            self.rect.w,
            self.rect.h,
            4.0,
            color,
        );
        draw_text(&self.text, self.rect.x + 10., self.rect.y - 8., 32., WHITE);
    }
}

pub fn draw_connect_menu(ui: &mut UiState) -> Option<SocketAddr> {
    let mouse_position = mouse_to_screen();
    ui.input_ip.update(mouse_position);
    ui.input_ip.draw();

    ui.input_name.update(mouse_position);
    ui.input_name.draw();

    if let Some(error) = ui.connect_error.as_ref() {
        draw_text(&error,(640. - 270.) / 2. , 215., 32., RED);
    }

    if draw_button(Rect::new((640. - 120.) / 2., 270.0, 120., 30.), &"connect") {
        if let Ok(ip) = ui.input_ip.text.parse() {
            return Some(ip);
        } else {
            ui.connect_error = Some("Invalid IP address".into());
        }
    }
    None
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

    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, color);
    draw_text(text, rect.x + 10., rect.y - 8., 32., color);

    clicked
}

pub fn mouse_to_screen() -> Vec2 {
    let mut pos: Vec2 = mouse_position().into();

    let desired_aspect_ratio = 640. / 320.;
    let current_aspect_ratio = screen_width() / screen_height();

    let viewport_width = screen_height() * desired_aspect_ratio;
    let viewport_height = screen_width() / desired_aspect_ratio;

    if current_aspect_ratio > desired_aspect_ratio {
        let start = -remap(
            (screen_width() - viewport_width) / 2.,
            0.0..=viewport_width,
            0.0..=640.,
        );
        let end = 640. - start;
        pos.x = remap(pos.x, 0.0..=screen_width(), start..=end);
        pos.y = remap(pos.y, 0.0..=screen_height(), 0.0..=320.);
    } else if current_aspect_ratio < desired_aspect_ratio {
        let start = -remap(
            (screen_height() - viewport_height) / 2.,
            0.0..=viewport_height,
            0.0..=320.,
        );
        let end = 320. - start;
        pos.y = remap(pos.y, 0.0..=screen_height(), start..=end);
        pos.x = remap(pos.x, 0.0..=screen_width(), 0.0..=640.);
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
    draw_text(&text, 160., 120., 64., WHITE);
}
