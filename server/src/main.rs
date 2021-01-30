use shared::{
    channels, AnimationController, Player, PlayerAction, PlayerAnimation, PlayerInput, Projectile,
    ProjectileType, ServerFrame, Transform,
};

use alto_logger::TermLogger;
use bincode::{deserialize, serialize};
use renet::{
    endpoint::EndpointConfig,
    error::RenetError,
    protocol::unsecure::UnsecureServerProtocol,
    server::{Server, ServerConfig, ServerEvent},
};

use glam::{vec2, Vec2};
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

type PlayerMapping = HashMap<u64, EntityId>;

fn server(ip: String) -> Result<(), RenetError> {
    let socket = UdpSocket::bind(ip)?;
    let server_config = ServerConfig::default();
    let endpoint_config = EndpointConfig::default();

    let mut server: Server<UnsecureServerProtocol> =
        Server::new(socket, server_config, endpoint_config, channels())?;

    let world = World::new();

    world.add_unique(PlayerMapping::new()).unwrap();

    loop {
        let start = Instant::now();

        server.update(start);
        for (client_id, messages) in server.get_messages_from_channel(0).iter() {
            for message in messages.iter() {
                let input: PlayerInput = deserialize(message).expect("Failed to deserialize.");
                world
                    .run(
                        |player_mapping: UniqueView<PlayerMapping>,
                         entities: EntitiesView,
                         mut inputs: ViewMut<PlayerInput>| {
                            if let Some(entity_id) = player_mapping.get(client_id) {
                                entities.add_component(*entity_id, &mut inputs, input);
                            }
                        },
                    )
                    .unwrap();
            }
        }

        for (client_id, messages) in server.get_messages_from_channel(2).iter() {
            for message in messages.iter() {
                let player_action: PlayerAction =
                    deserialize(message).expect("Failed to deserialize.");
                match player_action {
                    PlayerAction::CastFireball(cast_target) => {
                        world.run(
                            |player_mapping: UniqueView<PlayerMapping>,
                             mut entities: EntitiesViewMut,
                             mut transforms: ViewMut<Transform>,
                             mut projectiles: ViewMut<Projectile>| {
                                if let Some(entity_id) = player_mapping.get(client_id) {
                                    let transform = (&transforms).get(*entity_id).unwrap();
                                    let projectile = Projectile::new(ProjectileType::Fireball);
                                    let direction =
                                        (cast_target.position - transform.position).normalize();
                                    let rotation = direction.angle_between(Vec2::unit_x());
                                    let transform = Transform::new(transform.position, rotation);
                                    entities.add_entity(
                                        (&mut projectiles, &mut transforms),
                                        (projectile, transform),
                                    );
                                }
                            },
                        ).unwrap();
                    }
                    PlayerAction::CastTeleport(cast_target) => {
                        world.run(
                            |player_mapping: UniqueView<PlayerMapping>,
                             mut transforms: ViewMut<Transform>| {
                                if let Some(entity_id) = player_mapping.get(client_id) {
                                    let mut transform = (&mut transforms).get(*entity_id).unwrap();
                                    transform.position.x = cast_target.position.x;
                                    transform.position.y = cast_target.position.y;
                                }
                            },
                        ).unwrap();
                    }
                }
            }
        }

        world.run(update_animations).unwrap();
        world.run(update_players).unwrap();
        world.run(update_projectiles).unwrap();

        // world.run(debug::<Player>).unwrap();
        // world.run(debug::<PlayerInput>).unwrap();
        // world.run(debug::<Transform>).unwrap();

        let server_frame = ServerFrame::from_world(&world);
        println!("{:?}", server_frame);

        let server_frame = serialize(&server_frame).expect("Failed to serialize state");
        println!("Server Frame Size: {} bytes", server_frame.len());

        server.send_message_to_all_clients(1, server_frame.into_boxed_slice());
        server.send_packets();

        while let Some(event) = server.get_event() {
            match event {
                ServerEvent::ClientConnected(id) => {
                    world.run_with_data(create_player, id).unwrap();
                }
                ServerEvent::ClientDisconnected(id) => {
                    world.run_with_data(remove_player, id).unwrap();
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

fn update_animations(mut animations_controller: ViewMut<AnimationController>) {
    for mut animation_controller in (&mut animations_controller).iter() {
        animation_controller.update();
    }
}

fn update_projectiles(mut all_storages: AllStoragesViewMut) {
    let mut remove = vec![];
    {
        let mut projectiles = all_storages.borrow::<ViewMut<Projectile>>().unwrap();
        let mut transforms = all_storages.borrow::<ViewMut<Transform>>().unwrap();

        for (entity_id, (mut projectile, mut transform)) in
            (&mut projectiles, &mut transforms).iter().with_id()
        {
            let direction = Vec2::new(transform.rotation.cos(), -transform.rotation.sin());
            transform.position.x += direction.x * 4.0;
            transform.position.y += direction.y * 4.0;
            projectile.duration = projectile
                .duration
                .checked_sub(Duration::from_micros(16666))
                .unwrap_or_else(|| Duration::from_micros(0));
            if projectile.duration.as_nanos() == 0 {
                remove.push(entity_id);
            }
        }
    }
    for entity_id in remove {
        all_storages.delete_entity(entity_id);
    }
}

fn update_players(
    players: View<Player>,
    inputs: View<PlayerInput>,
    mut transforms: ViewMut<Transform>,
    mut animations: ViewMut<AnimationController>,
) {
    for (_, input, mut transform, mut animation) in
        (&players, &inputs, &mut transforms, &mut animations).iter()
    {
        let x = (input.right as i8 - input.left as i8) as f32;
        let y = (input.down as i8 - input.up as i8) as f32;
        let mut direction = vec2(x, y);

        if direction.length() != 0.0 {
            direction = direction.normalize();
            transform.position.x += direction.x * 4.0;
            transform.position.y += direction.y * 4.0;
        }

        if input.right ^ input.left || input.down ^ input.up {
            animation.change_animation(PlayerAnimation::Run.into());
        } else {
            animation.change_animation(PlayerAnimation::Idle.into());
        }
    }
}

fn create_player(
    client_id: u64,
    mut entities: EntitiesViewMut,
    mut transforms: ViewMut<Transform>,
    mut players: ViewMut<Player>,
    mut animations: ViewMut<AnimationController>,
    mut player_mapping: UniqueViewMut<PlayerMapping>,
) {
    let player = Player::new(client_id);
    let transform = Transform::default();
    let animation = PlayerAnimation::Idle.get_animation_controller();

    let entity_id = entities.add_entity(
        (&mut players, &mut transforms, &mut animations),
        (player, transform, animation),
    );

    player_mapping.insert(client_id, entity_id);
}

fn remove_player(client_id: u64, mut all_storages: AllStoragesViewMut) {
    let player_entity_id = {
        let player_mapping = all_storages.borrow::<UniqueView<PlayerMapping>>().unwrap();
        player_mapping.get(&client_id).copied()
    };
    if let Some(entity_id) = player_entity_id {
        all_storages.delete_entity(entity_id);
    }
}

#[allow(dead_code)]
fn debug<T: std::fmt::Debug + 'static>(view: View<T>) {
    for entity in view.iter() {
        println!("{:?}", entity);
    }
}
