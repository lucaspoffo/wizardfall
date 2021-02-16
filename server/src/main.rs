
use alto_logger::TermLogger;

use server::Game;

use std::time::{Duration, Instant};

fn main() {
    TermLogger::default().init().unwrap();

    let mut game = Game::new("127.0.0.1:5000".parse().unwrap()).unwrap();
    loop {
        let start = Instant::now();

        game.update();

        let now = Instant::now();
        let frame_duration = Duration::from_micros(16666);
        if let Some(wait) = (start + frame_duration).checked_duration_since(now) {
            std::thread::sleep(wait);
        }
    }
}

