use shared::{channels, Player, PlayerInput, ServerFrame};

use alto_logger::TermLogger;
use bincode::{deserialize, serialize};
use renet::{
    endpoint::EndpointConfig,
    error::RenetError,
    protocol::unsecure::UnsecureServerProtocol,
    server::{Server, ServerConfig, ServerEvent},
};

use std::collections::HashMap;
use std::net::UdpSocket;
use std::thread::sleep;
use std::time::{Duration, Instant};

fn main() -> Result<(), RenetError> {
    TermLogger::default().init().unwrap();

    let ip = "127.0.0.1:5000".to_string();
    server(ip)?;
    Ok(())
}

struct ServerState {
    players: Vec<Player>,
    players_input: HashMap<u64, PlayerInput>,
}

impl ServerState {
    fn set_player_input(&mut self, id: u64, input: PlayerInput) {
        self.players_input.insert(id, input);
    }

    fn update_players(&mut self) {
        for player in self.players.iter_mut() {
            if let Some(input) = self.players_input.get(&player.id) {
                player.update_from_input(&input);
                player.animation_manager.update();
            }
        }
    }
}

fn server(ip: String) -> Result<(), RenetError> {
    let socket = UdpSocket::bind(ip)?;
    let server_config = ServerConfig::default();
    let endpoint_config = EndpointConfig::default();

    let mut server: Server<UnsecureServerProtocol> =
        Server::new(socket, server_config, endpoint_config, channels())?;

    let mut server_state = ServerState {
        players: vec![],
        players_input: HashMap::new(),
    };

    loop {
        let start = Instant::now();

        server.update(start.clone());
        for (client_id, messages) in server.get_messages_from_channel(0).iter() {
            for message in messages.iter() {
                let input: PlayerInput = deserialize(message).expect("Failed to deserialize.");
                server_state.set_player_input(*client_id, input);
            }
        }

        server_state.update_players();

        let server_frame = ServerFrame {
            players: server_state.players.iter().map(|p| p.state()).collect(),
        };
        let server_frame = serialize(&server_frame).expect("Failed to serialize state");

        server.send_message_to_all_clients(1, server_frame.into_boxed_slice());
        server.send_packets();

        while let Some(event) = server.get_event() {
            match event {
                ServerEvent::ClientConnected(id) => {
                    let player = Player::new(id);
                    server_state.players.push(player);
                }
                ServerEvent::ClientDisconnected(id) => {
                    if let Some(pos) = server_state.players.iter().position(|p| p.id == id) {
                        server_state.players.remove(pos);
                        server_state.players_input.remove(&id);
                    }
                }
            }
        }

        let now = Instant::now();
        let frame_duration = Duration::from_micros(16666);
        if let Some(wait) = (start + frame_duration).checked_duration_since(now) {
            sleep(wait);
        }
    }
}
