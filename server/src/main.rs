use shared::{
    channels, Player, PlayerAction, PlayerInput, Projectile, ProjectileType, ServerFrame,
};

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
    players: HashMap<u32, Player>,
    players_input: HashMap<u32, PlayerInput>,
    projectiles: Vec<Projectile>,
}

impl ServerState {
    fn set_player_input(&mut self, id: u32, input: PlayerInput) {
        self.players_input.insert(id, input);
    }

    fn update_players(&mut self) {
        for player in self.players.values_mut() {
            if let Some(input) = self.players_input.get(&player.id) {
                player.update_from_input(&input);
                player.animation_manager.update();
            }
        }
    }

    fn update_projectiles(&mut self) {
        for projectile in self.projectiles.iter_mut() {
            projectile.position.x += projectile.direction.x * 4.0;
            projectile.position.y += projectile.direction.y * 4.0;
            projectile.duration = projectile
                .duration
                .checked_sub(Duration::from_micros(16666))
                .unwrap_or(Duration::from_micros(0));
        }

        self.projectiles.retain(|p| {
            !(p.duration.as_nanos() == 0)
        });
    }
}

fn server(ip: String) -> Result<(), RenetError> {
    let socket = UdpSocket::bind(ip)?;
    let server_config = ServerConfig::default();
    let endpoint_config = EndpointConfig::default();

    let mut server: Server<UnsecureServerProtocol> =
        Server::new(socket, server_config, endpoint_config, channels())?;

    let mut server_state = ServerState {
        projectiles: vec![],
        players: HashMap::new(),
        players_input: HashMap::new(),
    };

    loop {
        let start = Instant::now();

        server.update(start.clone());
        for (client_id, messages) in server.get_messages_from_channel(0).iter() {
            for message in messages.iter() {
                let input: PlayerInput = deserialize(message).expect("Failed to deserialize.");
                server_state.set_player_input(*client_id as u32, input);
            }
        }

        for (client_id, messages) in server.get_messages_from_channel(2).iter() {
            for message in messages.iter() {
                let player_action: PlayerAction =
                    deserialize(message).expect("Failed to deserialize.");
                let client_id = *client_id as u32;
                match player_action {
                    PlayerAction::CastFireball(cast_target) => {
                        if let Some(player) = server_state.players.get(&client_id) {
                            let projectile = Projectile::from_cast_target(
                                ProjectileType::Fireball,
                                client_id,
                                cast_target,
                                player.position,
                            );
                            server_state.projectiles.push(projectile);
                        }
                    }
                    PlayerAction::CastTeleport(cast_target) => {
                        if let Some(player) = server_state.players.get_mut(&client_id) {
                            player.position.x = cast_target.position.x;
                            player.position.y = cast_target.position.y;
                        }
                    }
                }
            }
        }

        server_state.update_players();
        server_state.update_projectiles();

        let server_frame = ServerFrame {
            players: server_state.players.values().map(|p| p.state()).collect(),
            projectiles: server_state.projectiles.iter().map(|p| p.state()).collect(),
        };

        // println!("{:?}", server_frame);
        let server_frame = serialize(&server_frame).expect("Failed to serialize state");
        println!("Server Frame Size: {} bytes", server_frame.len());

        server.send_message_to_all_clients(1, server_frame.into_boxed_slice());
        server.send_packets();

        while let Some(event) = server.get_event() {
            match event {
                ServerEvent::ClientConnected(id) => {
                    let player = Player::new(id as u32);
                    server_state.players.insert(player.id, player);
                }
                ServerEvent::ClientDisconnected(id) => {
                    let id = id as u32;
                    if let Some(_) = server_state.players.remove(&id) {
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
