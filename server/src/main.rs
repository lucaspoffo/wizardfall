use shared::{
    channels, Player, PlayerAction, PlayerInput, PlayerState, Projectile, ProjectileState,
    ProjectileType, ServerFrame, NetworkState
};

use alto_logger::TermLogger;
use bincode::{deserialize, serialize};
use renet::{
    endpoint::EndpointConfig,
    error::RenetError,
    protocol::unsecure::UnsecureServerProtocol,
    server::{Server, ServerConfig, ServerEvent},
};

use shipyard::*;

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
    player_mapping: HashMap<u64, EntityId>,
}

fn server(ip: String) -> Result<(), RenetError> {
    let socket = UdpSocket::bind(ip)?;
    let server_config = ServerConfig::default();
    let endpoint_config = EndpointConfig::default();

    let mut server: Server<UnsecureServerProtocol> =
        Server::new(socket, server_config, endpoint_config, channels())?;

    let mut server_state = ServerState {
        player_mapping: HashMap::new(),
    };

    let world = World::new();

    loop {
        let start = Instant::now();

        server.update(start.clone());
        for (client_id, messages) in server.get_messages_from_channel(0).iter() {
            for message in messages.iter() {
                let input: PlayerInput = deserialize(message).expect("Failed to deserialize.");
                if let Some(player_entity_id) = server_state.player_mapping.get(client_id) {
                    world.run(|entities: EntitiesView, mut inputs: ViewMut<PlayerInput>| {
                        entities.add_component(&mut inputs, input, *player_entity_id);
                    });
                }
            }
        }

        for (client_id, messages) in server.get_messages_from_channel(2).iter() {
            for message in messages.iter() {
                let player_action: PlayerAction =
                    deserialize(message).expect("Failed to deserialize.");
                match player_action {
                    PlayerAction::CastFireball(cast_target) => {
                        if let Some(player_entity_id) = server_state.player_mapping.get(&client_id) {
                            world.run(|mut entities: EntitiesViewMut, mut players: View<Player>, mut projectiles: ViewMut<Projectile>| {
                                let player = (&mut players).get(*player_entity_id);
                                let projectile = Projectile::from_cast_target(
                                    ProjectileType::Fireball,
                                    *client_id as u32,
                                    cast_target,
                                    player.position,
                                );
                                entities.add_entity(&mut projectiles, projectile);
                            });
                        }
                    }
                    PlayerAction::CastTeleport(cast_target) => {
                        if let Some(entity_id) = server_state.player_mapping.get(&client_id) {
                            world.run(|mut players: ViewMut<Player>| {
                                let player = (&mut players).get(*entity_id);
                                player.position.x = cast_target.position.x;
                                player.position.y = cast_target.position.y;
                            });
                        }
                    }
                }
            }
        }

        world.run(update_players);
        world.run(update_projectiles);

        let server_frame = world.run(world_server_frame);

        let server_frame = serialize(&server_frame).expect("Failed to serialize state");
        println!("Server Frame Size: {} bytes", server_frame.len());

        server.send_message_to_all_clients(1, server_frame.into_boxed_slice());
        server.send_packets();

        while let Some(event) = server.get_event() {
            match event {
                ServerEvent::ClientConnected(id) => {
                    world.run(
                        |mut entities: EntitiesViewMut, mut players: ViewMut<Player>| {
                            let player = Player::new(id as u32);
                            let entity_id = entities.add_entity(&mut players, player);
                            server_state.player_mapping.insert(id, entity_id);
                        },
                    );
                }
                ServerEvent::ClientDisconnected(id) => {
                    if let Some(entity_id) = server_state.player_mapping.remove(&id) {
                        world.run(|mut all_storages: AllStoragesViewMut| {
                            all_storages.delete(entity_id);
                        });
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

fn update_players(mut players: ViewMut<Player>, inputs: View<PlayerInput>) {
    for (player, input) in (&mut players, &inputs).iter() {
        player.update_from_input(input);
        player.animation_manager.update();
    }
}

fn update_projectiles(mut all_storages: AllStoragesViewMut) {
    let mut remove = vec![];

    {
        let mut projectiles = all_storages.borrow::<ViewMut<Projectile>>();
        for (entity_id, projectile) in (&mut projectiles).iter().with_id() {
            projectile.position.x += projectile.direction.x * 4.0;
            projectile.position.y += projectile.direction.y * 4.0;
            projectile.duration = projectile
                .duration
                .checked_sub(Duration::from_micros(16666))
                .unwrap_or(Duration::from_micros(0));
            if projectile.duration.as_nanos() == 0 {
                remove.push(entity_id);
            }
        }
    }
    for entity_id in remove {
        all_storages.delete(entity_id);
    }
}

fn world_server_frame(player: View<Player>, projectiles: View<Projectile>) -> ServerFrame {
    let players: Vec<PlayerState> = player.iter().map(|p| p.state()).collect();
    let projectiles: Vec<ProjectileState> = projectiles.iter().map(|p| p.state()).collect();

    ServerFrame {
        players,
        projectiles,
    }
}
