use macroquad::prelude::*;
use shared::{
    animation::AnimationController,
    channels,
    ldtk::{load_level_collisions, PlayerRespawnPoints},
    message::ServerMessages,
    network::ServerFrame,
    physics::{render_physics, Physics},
    player::{Player, PlayerAction, PlayerAnimation, PlayerInput},
    projectile::{Projectile, ProjectileType},
    timer::Timer,
    Health, PlayersScore, Transform,
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

use std::collections::{HashMap, HashSet};
use std::net::UdpSocket;
use std::time::{Duration, Instant};

struct ServerInfo {
    respawn_players: bool,
    respawn_players_timer: Timer,
    connected_players: HashSet<u64>,
}

#[macroquad::main("Renet macroquad demo")]
async fn main() {
    TermLogger::default().init().unwrap();

    let ip = "127.0.0.1:5000".to_string();
    server(ip).await.unwrap();
}

type PlayerMapping = HashMap<u64, EntityId>;

async fn server(ip: String) -> Result<(), RenetError> {
    let socket = UdpSocket::bind(ip)?;
    let server_config = ServerConfig::default();
    let endpoint_config = EndpointConfig::default();

    let mut server: Server<UnsecureServerProtocol> =
        Server::new(socket, server_config, endpoint_config, channels())?;

    let mut world = World::new();

    load_level_collisions(&mut world);

    let server_info = ServerInfo {
        respawn_players: false,
        respawn_players_timer: Timer::new(Duration::from_secs(3)),
        connected_players: HashSet::new(),
    };

    world.add_unique(server_info).unwrap();
    world.add_unique(PlayerMapping::new()).unwrap();
    world.add_unique(PlayersScore::default()).unwrap();

    world.borrow::<ViewMut<Player>>().unwrap().track_deletion();
    world
        .borrow::<ViewMut<Projectile>>()
        .unwrap()
        .track_deletion();

    let viewport_height = 600.0;
    let aspect = screen_width() / screen_height();
    let viewport_width = viewport_height * aspect;

    let camera = Camera2D {
        zoom: vec2(
            1.0 / viewport_width as f32 * 2.,
            -1.0 / viewport_height as f32 * 2.,
        ),
        // TODO: remove tile size magic numbers
        target: vec2(viewport_width / 2., viewport_height / 2.),
        ..Default::default()
    };

    loop {
        clear_background(BLACK);
        set_camera(camera);
        let start = Instant::now();

        server.update(start);
        for (client_id, messages) in server.get_messages_from_channel(0).iter() {
            for message in messages.iter() {
                let input: PlayerInput = deserialize(message).expect("Failed to deserialize.");
                world
                    .run(
                        |player_mapping: UniqueView<PlayerMapping>,
                         mut inputs: ViewMut<PlayerInput>| {
                            if let Some(entity_id) = player_mapping.get(client_id) {
                                inputs.add_component_unchecked(*entity_id, input);
                            }
                        },
                    )
                    .unwrap();
            }
        }

        for (client_id, messages) in server.get_messages_from_channel(2).iter() {
            for message in messages.iter() {
                let player_action: PlayerAction = deserialize(message).unwrap();
                handle_player_action(&mut world, player_action, client_id);
            }
        }

        // Game logic
        world.run(update_players_cooldown).unwrap();
        world.run(update_animations).unwrap();
        world.run(update_players).unwrap();
        world.run(update_projectiles).unwrap();
        world.run(cast_fireball_player).unwrap();
        world.run(sync_physics).unwrap();

        // Debug physics
        world.run(render_physics).unwrap();

        world.run(remove_zero_health).unwrap();
        world.run(remove_dead).unwrap();

        world.run(destroy_physics_entities).unwrap();

        let should_check_win = world
            .run(|info: UniqueView<ServerInfo>| {
                !info.respawn_players && info.connected_players.len() > 1
            })
            .unwrap();

        if should_check_win {
            if world.run(check_win_condition).unwrap() {
                world.run(cleanup_world).unwrap();
                world
                    .run(|mut info: UniqueViewMut<ServerInfo>| {
                        info.respawn_players_timer.reset();
                        info.respawn_players = true;
                    })
                    .unwrap();
            }
        }

        // Repawn player
        if world.run(respawn_players).unwrap() {
            let clients_id: Vec<u64> = world
                .run(|info: UniqueView<ServerInfo>| {
                    info.connected_players.iter().copied().collect()
                })
                .unwrap();

            for &client_id in clients_id.iter() {
                world.run_with_data(create_player, client_id).unwrap();
            }
        }

        let server_frame = ServerFrame::from_world(&world);
        // println!("{:?}", server_frame);

        let server_frame = serialize(&server_frame).expect("Failed to serialize state");
        // println!("Server Frame Size: {} bytes", server_frame.len());

        server.send_message_to_all_clients(1, server_frame.into_boxed_slice());
        server.send_packets();

        // Send score update to clients
        {
            let mut score = world.borrow::<UniqueViewMut<PlayersScore>>().unwrap();
            if score.updated {
                let score_message = ServerMessages::UpdateScore((*score).clone());
                let score_message = serialize(&score_message).expect("Failed to serialize score");
                server.send_message_to_all_clients(2, score_message.into_boxed_slice());
                score.updated = false;
            }
        }

        while let Some(event) = server.get_event() {
            match event {
                ServerEvent::ClientConnected(id) => {
                    world
                        .run(
                            |mut players_score: UniqueViewMut<PlayersScore>,
                             mut info: UniqueViewMut<ServerInfo>| {
                                info.connected_players.insert(id);
                                players_score.score.insert(id, 0);
                                players_score.updated = true;
                            },
                        )
                        .unwrap();
                }
                ServerEvent::ClientDisconnected(id) => {
                    world.run_with_data(remove_player, id).unwrap();
                    world
                        .run(
                            |mut players_score: UniqueViewMut<PlayersScore>,
                             mut info: UniqueViewMut<ServerInfo>| {
                                info.connected_players.remove(&id);
                                players_score.score.remove(&id);
                                players_score.updated = true;
                            },
                        )
                        .unwrap();
                }
            }
        }

        /*
        let now = Instant::now();
        let frame_duration = Duration::from_micros(16666);
        if let Some(wait) = (start + frame_duration).checked_duration_since(now) {
            sleep(wait);
        }*/
        next_frame().await;
    }
}

fn handle_player_action(_world: &mut World, player_action: PlayerAction, _client_id: &u64) {
    match player_action {
        _ => {}
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
        let mut deads = all_storages.borrow::<ViewMut<Dead>>().unwrap();
        let mut health = all_storages.borrow::<ViewMut<Health>>().unwrap();
        let players = all_storages.borrow::<View<Player>>().unwrap();
        let mut physics = all_storages.borrow::<UniqueViewMut<Physics>>().unwrap();

        for (entity_id, mut projectile) in (&mut projectiles).iter().with_id() {
            projectile.duration = projectile
                .duration
                .checked_sub(Duration::from_micros(16666))
                .unwrap_or_else(|| Duration::from_micros(0));
            if projectile.duration.as_nanos() == 0 {
                remove.push(entity_id);
            }

            // Apply gravity to projectiles
            projectile.speed.y += 1000. * get_frame_time();

            if physics.move_h(entity_id, projectile.speed.x * get_frame_time())
                || physics.move_v(entity_id, projectile.speed.y * get_frame_time())
            {
                deads.add_component_unchecked(entity_id, Dead);
                return;
            }

            for (player_id, (player, mut health)) in (&players, &mut health).iter().with_id() {
                if player_id == projectile.owner {
                    continue;
                }

                if physics.overlaps_actor(entity_id, player_id) {
                    health.take_damage(10, Some(player.client_id));
                    deads.add_component_unchecked(entity_id, Dead);
                }
            }
        }
    }
    for entity_id in remove {
        all_storages.delete_entity(entity_id);
    }
}

fn cast_fireball_player(
    mut players: ViewMut<Player>,
    inputs: View<PlayerInput>,
    mut entities: EntitiesViewMut,
    mut transforms: ViewMut<Transform>,
    mut projectiles: ViewMut<Projectile>,
    mut physics: UniqueViewMut<Physics>,
) {
    let mut created_projectiles = vec![];
    for (player_id, (mut player, input, transform)) in
        (&mut players, &inputs, &transforms).iter().with_id()
    {
        if input.fire && player.fireball_cooldown.is_finished() {
            player.fireball_charge += get_frame_time();
            player.fireball_charge = player
                .fireball_charge
                .clamp(0.0, player.fireball_max_charge);
        } else if !input.fire && player.fireball_charge > 0. {
            // Fireball cooldown
            if !player.fireball_cooldown.is_finished() {
                player.fireball_charge = 0.;
                return;
            }
            let pos = transform.position + vec2(8., 16.);

            let entity_id = entities.add_entity((), ());
            physics.add_actor(entity_id, pos, 16, 16);

            let speed = input.direction * (200. * (1. + player.fireball_charge * 3.));
            let projectile = Projectile::new(ProjectileType::Fireball, speed, player_id);
            let rotation = input.direction.angle_between(Vec2::unit_x());

            let projectile_transform = Transform::new(pos, rotation);
            created_projectiles.push((entity_id, (projectile, projectile_transform)));

            player.fireball_cooldown.reset();
            player.fireball_charge = 0.;
        }
    }

    for (entity_id, components) in created_projectiles.iter() {
        entities.add_component(
            *entity_id,
            (&mut projectiles, &mut transforms),
            components.clone(),
        );
    }
}

fn update_players(
    mut players: ViewMut<Player>,
    inputs: View<PlayerInput>,
    mut animations: ViewMut<AnimationController>,
    mut physics: UniqueViewMut<Physics>,
) {
    for (entity_id, (mut player, input, mut animation)) in
        (&mut players, &inputs, &mut animations).iter().with_id()
    {
        let x = (input.right as i8 - input.left as i8) as f32;
        let y = (input.down as i8 - input.up as i8) as f32;
        let movement_direction = vec2(x, y);
        player.direction = if input.direction.length() != 0.0 {
            input.direction.normalize()
        } else {
            input.direction
        };

        if input.dash && player.dash_cooldown.is_finished() {
            player.dash_cooldown.reset();
            player.current_dash_duration = player.dash_duration;

            // If there is no player input use player facing direction
            let dash_direction = if movement_direction.length() != 0.0 {
                movement_direction.normalize()
            } else {
                vec2(input.direction.x.signum(), 0.)
            };
            player.speed = dash_direction * 300.;
        }

        let pos = physics.actor_pos(entity_id);
        let on_ground = physics.collide_check(entity_id, pos + vec2(0., 1.));

        if player.current_dash_duration > 0.0 {
            player.current_dash_duration -= get_frame_time();
            if player.current_dash_duration <= 0.0 {
                player.speed = player.speed.normalize() * 100.;
            }
        } else {
            if !on_ground {
                player.speed.y += 1000. * get_frame_time();
            } else {
                player.speed.y = 500. * get_frame_time();
            }

            player.speed.x = movement_direction.x * 100.;
            if input.jump && on_ground {
                player.speed.y = -360.;
            }
        }

        if physics.move_h(entity_id, player.speed.x * get_frame_time()) {
            player.current_dash_duration = 0.;
        }
        if physics.move_v(entity_id, player.speed.y * get_frame_time()) {
            player.current_dash_duration = 0.;
            player.speed.y = 0.0;
        }

        // Update animation
        if input.right ^ input.left || input.down ^ input.up || !on_ground {
            animation.change_animation(PlayerAnimation::Run.into());
        } else {
            animation.change_animation(PlayerAnimation::Idle.into());
        }
    }
}

fn update_players_cooldown(mut players: ViewMut<Player>) {
    for mut player in (&mut players).iter() {
        player.fireball_cooldown.update(get_frame_time());
        player.dash_cooldown.update(get_frame_time());
    }
}

fn create_player(
    client_id: u64,
    player_respawn_points: UniqueView<PlayerRespawnPoints>,
    mut entities: EntitiesViewMut,
    mut transforms: ViewMut<Transform>,
    mut players: ViewMut<Player>,
    mut health: ViewMut<Health>,
    mut animations: ViewMut<AnimationController>,
    mut player_mapping: UniqueViewMut<PlayerMapping>,
    mut physics: UniqueViewMut<Physics>,
) {
    let entity_id = entities.add_entity((), ());
    let mut player_position =
        player_respawn_points.0[rand::rand() as usize % player_respawn_points.0.len()];

    player_position.y -= 48.;

    physics.add_actor(entity_id, player_position, 32, 48);

    let player = Player::new(client_id);
    let transform = Transform::default();
    let animation = PlayerAnimation::Idle.get_animation_controller();

    let player_health = Health::new(50);

    entities.add_component(
        entity_id,
        (&mut players, &mut transforms, &mut animations, &mut health),
        (player, transform, animation, player_health),
    );

    player_mapping.insert(client_id, entity_id);
}

fn remove_player(client_id: u64, mut all_storages: AllStoragesViewMut) {
    let player_entity_id = {
        let mut player_mapping = all_storages
            .borrow::<UniqueViewMut<PlayerMapping>>()
            .unwrap();
        player_mapping.remove(&client_id)
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

struct Dead;

fn remove_dead(mut all_storages: AllStoragesViewMut) {
    all_storages.delete_any::<SparseSet<Dead>>();
}

fn remove_zero_health(health: View<Health>, mut deads: ViewMut<Dead>) {
    for (entity_id, h) in health.iter().with_id() {
        if h.is_dead() {
            deads.add_entity(entity_id, Dead);
        }
    }
}

fn sync_physics(
    players: View<Player>,
    projectiles: View<Projectile>,
    mut transforms: ViewMut<Transform>,
    physics: UniqueView<Physics>,
) {
    for (entity_id, (_, mut transform)) in (&players, &mut transforms).iter().with_id() {
        let pos = physics.actor_pos(entity_id);
        transform.position = pos;
    }

    for (entity_id, (_, mut transform)) in (&projectiles, &mut transforms).iter().with_id() {
        let pos = physics.actor_pos(entity_id);
        transform.position = pos;
    }
}

fn destroy_physics_entities(
    mut physics: UniqueViewMut<Physics>,
    mut projectiles: ViewMut<Projectile>,
) {
    for (entity_id, _) in projectiles.take_deleted().iter() {
        physics.remove_actor(entity_id);
    }
}

fn check_win_condition(
    players: View<Player>,
    mut players_score: UniqueViewMut<PlayersScore>,
) -> bool {
    let win_codition = players.iter().count() <= 1;
    println!("Player Components couny: {}", win_codition);
    if win_codition {
        if let Some(player) = players.iter().next() {
            let score = players_score.score.entry(player.client_id).or_insert(0);
            *score += 1;
            players_score.updated = true;
        }
    }
    win_codition
}

fn cleanup_world(mut all_storages: AllStoragesViewMut) {
    all_storages.delete_any::<SparseSet<Player>>();
    all_storages.delete_any::<SparseSet<Projectile>>();
}

fn respawn_players(mut info: UniqueViewMut<ServerInfo>) -> bool {
    let mut respawn = false;
    if info.respawn_players
        && info.respawn_players_timer.is_finished()
        && info.connected_players.len() > 1
    {
        info.respawn_players = false;
        respawn = true;
    }
    respawn
}
