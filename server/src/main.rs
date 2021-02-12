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
    EntityType, Health, PlayersScore, Transform,
};

use alto_logger::TermLogger;
use bincode::{deserialize, serialize};
use platform_physics_shipyard::{render::render_physics, HitCollision, Physics};
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

    load_level_collisions(&mut world);

    world.add_unique(PlayerMapping::new()).unwrap();
    world.add_unique(PlayerRespawn::default()).unwrap();
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
        world.run(sync_physics).unwrap();

        world.run(render_physics::<EntityType>).unwrap();

        world.run(remove_zero_health).unwrap();
        world.run(remove_dead).unwrap();

        world.run(add_player_respawn).unwrap();
        world.run(destroy_physics_entities).unwrap();

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
    mut physics: UniqueViewMut<Physics<EntityType>>,
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

        let entity_id = entities.add_entity((), ());
        let transform = (&transforms).get(*player_entity).unwrap();
        physics.add_actor(entity_id, EntityType::Player, transform.position, 16, 16);

        let direction = (cast_target.position - transform.position).normalize();
        let projectile =
            Projectile::new(ProjectileType::Fireball, direction * 200., *player_entity);
        let rotation = direction.angle_between(Vec2::unit_x());

        let transform = Transform::new(transform.position, rotation);
        entities.add_component(
            entity_id,
            (&mut projectiles, &mut transforms),
            (projectile, transform),
        );
    }
}

fn cast_teleport(
    (client_id, cast_target): (&u64, CastTarget),
    entities: EntitiesView,
    mut players: ViewMut<Player>,
    player_mapping: UniqueView<PlayerMapping>,
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
        let player_mapping = all_storages.borrow::<UniqueView<PlayerMapping>>().unwrap();
        let mut physics = all_storages
            .borrow::<UniqueViewMut<Physics<EntityType>>>()
            .unwrap();

        for (entity_id, mut projectile) in (&mut projectiles).iter().with_id() {
            projectile.duration = projectile
                .duration
                .checked_sub(Duration::from_micros(16666))
                .unwrap_or_else(|| Duration::from_micros(0));
            if projectile.duration.as_nanos() == 0 {
                remove.push(entity_id);
            }

            let mut handle_collision = |hit_collision: HitCollision<EntityType>| {
                match hit_collision.entity_type {
                    EntityType::Player => {
                        if let Some(owner_id) = hit_collision.entity_id {
                            // Fireball colliding with owner
                            if owner_id == projectile.owner {
                                return;
                            }
                        }
                        // With we are hitting a player we always have an entity_id
                        let player = hit_collision.entity_id.unwrap();
                        let mut health = (&mut health).get(player).unwrap();
                        let client_id = player_mapping
                            .iter()
                            .find(|(_, v)| **v == player)
                            .map(|(k, _)| *k);
                        health.take_damage(10, client_id);
                        deads.add_component_unchecked(entity_id, Dead);
                    }
                    _ => {
                        deads.add_component_unchecked(entity_id, Dead);
                    }
                }
            };

            if let Some(hit_collision) =
                physics.move_h(entity_id, projectile.speed.x * get_frame_time())
            {
                handle_collision(hit_collision);
            }
            if let Some(hit_collision) =
                physics.move_v(entity_id, projectile.speed.y * get_frame_time())
            {
                handle_collision(hit_collision);
            }

            for (player_id, player) in players.iter().with_id() {
                if player_id != projectile.owner {
                    if physics.overlaps_actor(entity_id, player_id) {
                         // With we are hitting a player we always have an entity_id
                        let mut health = (&mut health).get(player_id).unwrap();
                        health.take_damage(10, Some(player.client_id));
                        deads.add_component_unchecked(entity_id, Dead);

                    }
                }
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
    mut physics: UniqueViewMut<Physics<EntityType>>,
) {
    for (entity_id, (mut player, input, mut animation)) in
        (&mut players, &inputs, &mut animations).iter().with_id()
    {
        let x = (input.right as i8 - input.left as i8) as f32;
        let y = (input.up as i8 - input.down as i8) as f32;
        let movement_direction = vec2(x, y);
        player.direction = input.direction;

        let pos = physics.actor_pos(entity_id);
        let on_ground = physics
            .collide_check(entity_id, pos + vec2(0., 1.))
            .is_some();

        if !on_ground {
            player.speed.y += 500. * get_frame_time();
        } else {
            player.speed.y = 500. * get_frame_time();
        }

        player.speed.x = movement_direction.x * 100.;
        if input.jump && on_ground {
            player.speed.y = -270.;
        }

        if let Some(hit_collision) = physics.move_h(entity_id, player.speed.x * get_frame_time()) {
            match hit_collision.entity_type {
                _ => {}
            }
        }
        if let Some(hit_collision) = physics.move_v(entity_id, player.speed.y * get_frame_time()) {
            match hit_collision.entity_type {
                _ => {}
            }
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
    mut animations: ViewMut<AnimationController>,
    mut player_mapping: UniqueViewMut<PlayerMapping>,
    mut physics: UniqueViewMut<Physics<EntityType>>,
) {
    let entity_id = entities.add_entity((), ());
    let mut player_position =
        player_respawn_points.0[rand::rand() as usize % player_respawn_points.0.len()];

    player_position.y -= 48.;

    physics.add_actor(entity_id, EntityType::Player, player_position, 32, 48);

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
    mut physics: UniqueViewMut<Physics<EntityType>>,
) {
    for (entity_id, player) in players.take_deleted().iter() {
        physics.remove_actor(entity_id);
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

fn sync_physics(
    players: View<Player>,
    projectiles: View<Projectile>,
    mut transforms: ViewMut<Transform>,
    physics: UniqueView<Physics<EntityType>>,
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
    mut physics: UniqueViewMut<Physics<EntityType>>,
    mut projectiles: ViewMut<Projectile>,
) {
    for (entity_id, _) in projectiles.take_deleted().iter() {
        physics.remove_actor(entity_id);
    }
}
