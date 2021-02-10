use macroquad::prelude::*;
use shared::{
    animation::AnimationController,
    channels,
    ldtk::{load_level_collisions, PlayerRespawnPoints},
    message::ServerMessages,
    network::ServerFrame,
    player::{CastTarget, Player, PlayerAction, PlayerAnimation, PlayerInput},
    projectile::{Projectile, ProjectileType},
    timer::Timer,
    EntityType, EntityUserData, Health, PlayersScore, Transform,
};

use alto_logger::TermLogger;
use bincode::{deserialize, serialize};
use renet::{
    endpoint::EndpointConfig,
    error::RenetError,
    protocol::unsecure::UnsecureServerProtocol,
    server::{Server, ServerConfig, ServerEvent},
};
use shipyard_rapier2d::{
    na::{Vector2, Point2},
    physics::{
        create_body_and_collider_system, create_joints_system, destroy_body_and_collider_system,
        setup_physics, step_world_system, EventQueue, RapierConfiguration,
        RigidBodyHandleComponent,
    },
    rapier::{
        dynamics::{RigidBodyBuilder, RigidBodySet},
        geometry::{ColliderBuilder, ColliderSet, Ray, InteractionGroups},
        pipeline::QueryPipeline
    },
    render::render_colliders,
};

use glam::{vec2, Vec2};
use shipyard::*;

use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::{Duration, Instant};
// use std::thread::sleep;

#[macroquad::main("Renet macroquad demo")]
async fn main() {
    TermLogger::default().init().unwrap();

    let ip = "127.0.0.1:5000".to_string();
    server(ip).await.unwrap();
}

type PlayerMapping = HashMap<u64, EntityId>;

#[derive(Debug, Default)]
struct PlayerRespawn(HashMap<u64, Timer>);

async fn server(ip: String) -> Result<(), RenetError> {
    let socket = UdpSocket::bind(ip)?;
    let server_config = ServerConfig::default();
    let endpoint_config = EndpointConfig::default();

    let mut server: Server<UnsecureServerProtocol> =
        Server::new(socket, server_config, endpoint_config, channels())?;

    let mut world = World::new();
    world.run(setup_physics).unwrap();

    world
        .run(|mut rapier_config: UniqueViewMut<RapierConfiguration>| {
            rapier_config.gravity = Vector2::new(0., 0.);
        })
        .unwrap();
    load_level_collisions(&mut world);

    world.add_unique(PlayerMapping::new()).unwrap();
    world.add_unique(PlayerRespawn::default()).unwrap();
    world.add_unique(PlayersScore::default()).unwrap();

    world.borrow::<ViewMut<Player>>().unwrap().track_deletion();

    let viewport_height = 600.0;
    let aspect = screen_width() / screen_height();
    let viewport_width = viewport_height * aspect;

    let camera = Camera2D {
        zoom: vec2(
            1.0 / viewport_width as f32 * 2.,
            -1.0 / viewport_height as f32 * 2.,
        ),
        // TODO: remove tile size magic numbers
        target: vec2(viewport_width / 2. - 100., -viewport_height / 2.),
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

        // Repawn player
        let respawn_players = world.run(player_respawn).unwrap();
        for client_id in respawn_players.iter() {
            world.run_with_data(create_player, *client_id).unwrap();
        }

        // Game logic
        world.run(update_players_cooldown).unwrap();
        world.run(update_animations).unwrap();
        world.run(update_players).unwrap();
        world.run(update_projectiles).unwrap();

        // Systems to update physics world
        world.run(create_body_and_collider_system).unwrap();
        world.run(create_joints_system).unwrap();
        world.run_with_data(step_world_system, 0.0016666).unwrap();

        world.run(sync_transform_rapier).unwrap();

        world.run(render_colliders).unwrap();
        world.run(display_events).unwrap();
        world.run(remove_zero_health).unwrap();
        // world.run(debug::<Player>).unwrap();
        // world.run(debug::<PlayerInput>).unwrap();
        // world.run(debug::<Transform>).unwrap();
        // world.run(debug::<Velocity>).unwrap();
        world.run(remove_dead).unwrap();

        // Remove
        world.run(destroy_body_and_collider_system).unwrap();

        world.run(add_player_respawn).unwrap();

        let server_frame = ServerFrame::from_world(&world);
        // println!("{:?}", server_frame);

        let server_frame = serialize(&server_frame).expect("Failed to serialize state");
        // println!("Server Frame Size: {} bytes", server_frame.len());

        server.send_message_to_all_clients(1, server_frame.into_boxed_slice());
        server.send_packets();

        // Serd score update to clients
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
                    world.run_with_data(create_player, id).unwrap();
                    world
                        .run(|mut players_score: UniqueViewMut<PlayersScore>| {
                            players_score.score.insert(id, 0);
                            players_score.updated = true;
                        })
                        .unwrap();
                }
                ServerEvent::ClientDisconnected(id) => {
                    world.run_with_data(remove_player, id).unwrap();
                    world
                        .run(|mut players_score: UniqueViewMut<PlayersScore>| {
                            players_score.score.remove(&id);
                            players_score.updated = true;
                        })
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

fn handle_player_action(world: &mut World, player_action: PlayerAction, client_id: &u64) {
    match player_action {
        PlayerAction::CastFireball(cast_target) => {
            world
                .run_with_data(cast_fireball, (client_id, cast_target))
                .unwrap();
        }
        PlayerAction::CastTeleport(cast_target) => {
            world
                .run_with_data(cast_teleport, (client_id, cast_target))
                .unwrap();
        }
    }
}

fn cast_fireball(
    (client_id, cast_target): (&u64, CastTarget),
    player_mapping: UniqueView<PlayerMapping>,
    mut players: ViewMut<Player>,
    mut entities: EntitiesViewMut,
    mut transforms: ViewMut<Transform>,
    mut projectiles: ViewMut<Projectile>,
    mut colliders_builder: ViewMut<ColliderBuilder>,
    mut bodies_builder: ViewMut<RigidBodyBuilder>,
) {
    if let Some(player_entity) = player_mapping.get(client_id) {
        if !entities.is_alive(*player_entity) {
            return;
        }

        // Fireball cooldown
        let mut player = (&mut players).get(*player_entity).unwrap();
        if !player.fireball_cooldown.is_finished() {
            return;
        } else {
            player.fireball_cooldown.reset();
        }

        let transform = (&transforms).get(*player_entity).unwrap();

        let projectile = Projectile::new(ProjectileType::Fireball, *player_entity);
        let direction = (cast_target.position - transform.position).normalize();
        let rotation = direction.angle_between(Vec2::unit_x());

        let entity_id = entities.add_entity((), ());
        let user_data = EntityUserData::new(entity_id, EntityType::Fireball);
        // TODO: remove magic number fireball speed.
        let rigid_body = RigidBodyBuilder::new_dynamic()
            .translation(transform.position.x, transform.position.y)
            .linvel(direction.x * 200., direction.y * 200.)
            .rotation(rotation);
        let collider_builder = ColliderBuilder::ball(8.)
            .sensor(true)
            .user_data(user_data.into());

        let transform = Transform::new(transform.position, rotation);
        entities.add_component(
            entity_id,
            (
                &mut projectiles,
                &mut transforms,
                &mut bodies_builder,
                &mut colliders_builder,
            ),
            (projectile, transform, rigid_body, collider_builder),
        );
    }
}

fn cast_teleport(
    (client_id, cast_target): (&u64, CastTarget),
    entities: EntitiesView,
    mut players: ViewMut<Player>,
    player_mapping: UniqueView<PlayerMapping>,
    body_handles: View<RigidBodyHandleComponent>,
    mut rigid_bodies: UniqueViewMut<RigidBodySet>,
) {
    if let Some(player_entity) = player_mapping.get(client_id) {
        if !entities.is_alive(*player_entity) {
            return;
        }

        // Teleport cooldown
        // TODO: add util for this
        let mut player = (&mut players).get(*player_entity).unwrap();
        if !player.teleport_cooldown.is_finished() {
            return;
        } else {
            player.teleport_cooldown.reset();
        }

        if let Ok(rigid_body) = body_handles.get(*player_entity) {
            if let Some(rb) = rigid_bodies.get_mut(rigid_body.handle()) {
                let mut pos = *rb.position();
                pos.translation.x = cast_target.position.x;
                pos.translation.y = cast_target.position.y;
                rb.set_position(pos, true);
            }
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

        for (entity_id, mut projectile) in (&mut projectiles).iter().with_id() {
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
    mut players: ViewMut<Player>,
    inputs: View<PlayerInput>,
    mut animations: ViewMut<AnimationController>,
    body_handles: View<RigidBodyHandleComponent>,
    mut rigid_bodies: UniqueViewMut<RigidBodySet>,
) {
    for (mut player, input, body_handle, mut animation) in
        (&mut players, &inputs, &body_handles, &mut animations).iter()
    {
        let x = (input.right as i8 - input.left as i8) as f32;
        let y = (input.up as i8 - input.down as i8) as f32;
        let mut movement_direction = Vector2::new(x, y);

        // Update the velocity on the rigid_body_component,
        if let Some(rb) = rigid_bodies.get_mut(body_handle.handle()) {
            if movement_direction.magnitude() != 0.0 {
                movement_direction = movement_direction.normalize();
                rb.set_linvel(movement_direction * 170.0, true);
            } else {
                rb.set_linvel(Vector2::zeros(), true);
            }
        }

        player.direction = input.direction;

        if input.right ^ input.left || input.down ^ input.up {
            animation.change_animation(PlayerAnimation::Run.into());
        } else {
            animation.change_animation(PlayerAnimation::Idle.into());
        }
    }
}

fn update_players_cooldown(mut players: ViewMut<Player>) {
    for mut player in (&mut players).iter() {
        player.fireball_cooldown.update(get_frame_time());
        player.teleport_cooldown.update(get_frame_time());
    }
}

fn create_player(
    client_id: u64,
    player_respawn_points: UniqueView<PlayerRespawnPoints>,
    mut entities: EntitiesViewMut,
    mut transforms: ViewMut<Transform>,
    mut players: ViewMut<Player>,
    mut health: ViewMut<Health>,
    mut colliders_builder: ViewMut<ColliderBuilder>,
    mut bodies_builder: ViewMut<RigidBodyBuilder>,
    mut animations: ViewMut<AnimationController>,
    mut player_mapping: UniqueViewMut<PlayerMapping>,
) {
    let player = Player::new(client_id);
    let transform = Transform::default();
    let animation = PlayerAnimation::Idle.get_animation_controller();

    let player_position =
        player_respawn_points.0[rand::rand() as usize % player_respawn_points.0.len()];

    let rigid_body = RigidBodyBuilder::new_dynamic()
        .lock_rotations()
        .translation(player_position.x, player_position.y);

    let entity_id = entities.add_entity((), ());
    let user_data = EntityUserData::new(entity_id, EntityType::Player);
    let player_health = Health::new(50);

    let collider_builder = ColliderBuilder::cuboid(16., 24.).user_data(user_data.into());

    entities.add_component(
        entity_id,
        (
            &mut players,
            &mut transforms,
            &mut animations,
            &mut bodies_builder,
            &mut colliders_builder,
            &mut health,
        ),
        (
            player,
            transform,
            animation,
            rigid_body,
            collider_builder,
            player_health,
        ),
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

fn display_events(
    events: UniqueViewMut<EventQueue>,
    colliders: UniqueView<ColliderSet>,
    projectiles: View<Projectile>,
    player_mapping: UniqueView<PlayerMapping>,
    mut health: ViewMut<Health>,
    mut deads: ViewMut<Dead>,
) {
    while let Ok(intersection_event) = events.intersection_events.pop() {
        let collider1 = colliders.get(intersection_event.collider1).unwrap();
        let collider2 = colliders.get(intersection_event.collider2).unwrap();
        let entity_data1 = EntityUserData::from_user_data(collider1.user_data);
        let entity_data2 = EntityUserData::from_user_data(collider2.user_data);
        match (&entity_data1, &entity_data2) {
            (
                EntityUserData {
                    entity_type: EntityType::Wall,
                    ..
                },
                EntityUserData {
                    entity_type: EntityType::Fireball,
                    entity_id: fireball,
                },
            )
            | (
                EntityUserData {
                    entity_type: EntityType::Fireball,
                    entity_id: fireball,
                },
                EntityUserData {
                    entity_type: EntityType::Wall,
                    ..
                },
            ) => {
                deads.add_component_unchecked(*fireball, Dead);
            }
            (
                EntityUserData {
                    entity_type: EntityType::Player,
                    entity_id: player,
                },
                EntityUserData {
                    entity_type: EntityType::Fireball,
                    entity_id: fireball,
                },
            )
            | (
                EntityUserData {
                    entity_type: EntityType::Fireball,
                    entity_id: fireball,
                },
                EntityUserData {
                    entity_type: EntityType::Player,
                    entity_id: player,
                },
            ) => {
                let projectile = projectiles.get(*fireball).unwrap();
                if projectile.owner == *player {
                    // Fireball colliding with owner
                    continue;
                }
                let mut health = (&mut health).get(*player).unwrap();
                let client_id = player_mapping
                    .iter()
                    .find(|(_, v)| *v == player)
                    .map(|(k, _)| *k);
                health.take_damage(10, client_id);
                deads.add_component_unchecked(*fireball, Dead);
            }
            _ => {
                println!(
                    "Unhandled collision event\nEntity Type 1: {:?}\nEntityType 2:{:?}",
                    entity_data1.entity_type, entity_data2.entity_type
                );
            }
        }
    }

    while let Ok(contact_event) = events.contact_events.pop() {
        println!("Received contact event: {:?}", contact_event);
    }
}

fn sync_transform_rapier(
    mut transforms: ViewMut<Transform>,
    rigid_bodies: View<RigidBodyHandleComponent>,
    bodies_set: UniqueView<RigidBodySet>,
) {
    for (mut transform, rigid_body) in (&mut transforms, &rigid_bodies).iter() {
        if let Some(rb) = bodies_set.get(rigid_body.handle()) {
            let pos = *rb.position();
            // TODO: Check if is need to check interpolate with previous pos
            transform.position = Vec2::new(pos.translation.vector.x, pos.translation.vector.y);
        }
    }
}

fn remove_dead(mut all_storages: AllStoragesViewMut) {
    all_storages.delete_any::<SparseSet<Dead>>();
}

fn remove_zero_health(
    health: View<Health>,
    players: View<Player>,
    mut deads: ViewMut<Dead>,
    mut players_score: UniqueViewMut<PlayersScore>,
) {
    for (entity_id, h) in health.iter().with_id() {
        if h.is_dead() {
            deads.add_entity(entity_id, Dead);
            if let Ok(_) = players.get(entity_id) {
                if let Some(killer) = h.killer {
                    let score = players_score.score.entry(killer).or_insert(0);
                    *score += 1;
                    players_score.updated = true;
                }
            }
        }
    }
}

fn add_player_respawn(
    player_mapping: UniqueView<PlayerMapping>,
    mut players: ViewMut<Player>,
    mut player_respawn: UniqueViewMut<PlayerRespawn>,
) {
    for (_, player) in players.take_deleted().iter() {
        if player_mapping.get(&player.client_id).is_some() {
            let respawn_timer = Timer::new(Duration::from_secs(5));
            player_respawn.0.insert(player.client_id, respawn_timer);
        }
    }
}

fn player_respawn(mut player_respawn: UniqueViewMut<PlayerRespawn>) -> Vec<u64> {
    let mut respawn_players: Vec<u64> = vec![];
    for (client_id, timer) in player_respawn.0.iter() {
        if timer.is_finished() {
            respawn_players.push(*client_id);
        }
    }

    player_respawn.0.retain(|_, timer| !timer.is_finished());

    respawn_players
}
