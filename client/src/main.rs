// use shared::channels;
use macroquad::prelude::*;
use shared::{Player, PlayerInput, ServerFrame, channels};

use alto_logger::TermLogger;
use renet::{
    client::{ClientConnected, RequestConnection},
    endpoint::EndpointConfig,
    error::RenetError,
    protocol::unsecure::UnsecureClientProtocol, 
};
use std::net::UdpSocket;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct App {
    id: u64,
    players: Vec<Player>,
    connection: ClientConnected,
}

impl App {
    fn new(id: u64, connection: ClientConnected) -> Self {
        Self {
            id,
            connection,
            players: vec![],
        }
    }

    fn update(&mut self) {
        let up = is_key_down(KeyCode::W) || is_key_down(KeyCode::Up);
        let down = is_key_down(KeyCode::S) || is_key_down(KeyCode::Down);
        let left = is_key_down(KeyCode::A) || is_key_down(KeyCode::Left);
        let right = is_key_down(KeyCode::D) || is_key_down(KeyCode::Right);

        let input = PlayerInput {
            up,
            down,
            left,
            right,
        };

        let message = bincode::serialize(&input).expect("Failed to serialize message.");
        self.connection.send_message(0, message.into_boxed_slice());
        self.connection.send_packets().unwrap();

        self.connection.process_events(Instant::now()).unwrap();
        for payload in self.connection.receive_all_messages_from_channel(0).iter() {
            let server_frame: ServerFrame =
                bincode::deserialize(payload).expect("Failed to deserialize state.");
            self.players = server_frame.players;
        }

        const SIZE: f32 = 32.0;
        for player in self.players.iter_mut() {
            draw_rectangle(
                f32::from(player.x),
                f32::from(player.y),
                SIZE,
                SIZE,
                Color::new(player.color.0, player.color.1, player.color.2, 1.0),
            );
        }
    }
}

#[macroquad::main("NaiaMacroquadExample")]
async fn main() {
    TermLogger::default().init().unwrap();
    rand::srand(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());

    let id = rand::rand() as u64;
    let connection = get_connection("127.0.0.1:5000".to_string(), id).unwrap();
    let mut app = App::new(id, connection);

    loop {
        clear_background(BLACK);

        app.update();

        next_frame().await
    }
}

fn get_connection(ip: String, id: u64) -> Result<ClientConnected, RenetError> {
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    let endpoint_config = EndpointConfig::default();

    println!("Id: {}", id);

    let mut request_connection = RequestConnection::new(
        id,
        socket,
        ip.parse().unwrap(),
        Box::new(UnsecureClientProtocol::new(id)),
        endpoint_config,
        channels(),
    )?;

    loop {
        println!("connectiong");
        if let Some(connection) = request_connection.update()? {
            return Ok(connection);
        };
        sleep(Duration::from_millis(20));
    }
}
