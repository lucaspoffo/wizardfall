use alto_logger::TermLogger;
use eframe::{egui, epi};
use shipyard::UniqueViewMut;

use server::{Game, GameplayConfig};

struct ServerApp {
    game: Game,
}

fn main() {
    TermLogger::default().init().unwrap();

    let game = Game::new("127.0.0.1:5000".parse().unwrap()).unwrap();
    let server_app = ServerApp { game };
    eframe::run_native(Box::new(server_app));
}

impl epi::App for ServerApp {
    fn name(&self) -> &str {
        "Server"
    }

    fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        self.game.update();

        ctx.request_repaint();

        egui::CentralPanel::default().show(ctx, |ui| {
            self.game.world.run(|mut config: UniqueViewMut<GameplayConfig>| {
                ui.heading("Gameplay Configuration:");
                let grid = egui::Grid::new("my_grid")
                    .striped(true)
                    .spacing([40.0, 4.0]);
                grid.show(ui, |ui| {
                    ui.label("Dash speed:");
                    ui.add(egui::Slider::f32(&mut config.dash_speed, 0.0..=1000.0).text("value"));
                    ui.end_row();

                    ui.label("Jump speed:");
                    ui.add(egui::Slider::f32(&mut config.jump_speed, 0.0..=1000.0).text("value"));
                    ui.end_row();

                    ui.label("Walk speed:");
                    ui.add(egui::Slider::f32(&mut config.walk_speed, 0.0..=1000.0).text("value"));
                    ui.end_row();

                    ui.label("Player gravity:");
                    ui.add(egui::Slider::f32(&mut config.player_gravity, 0.0..=1000.0).text("value"));
                    ui.end_row();
                });
            }).unwrap();
        });

        // Resize the native window to be just the size we need it to be:
        frame.set_window_size(ctx.used_size());
    }
}

