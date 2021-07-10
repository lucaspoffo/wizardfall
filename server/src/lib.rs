use shared::{
    animation::{AnimationController, AnimationEntity},
    channels,
    ldtk::{load_level_collisions, PlayerRespawnPoints},
    message::{ClientAction, ServerMessages},
    network::ServerFrame,
    physics::Physics,
    player::{Player, PlayerInput},
    projectile::{Projectile, ProjectileType},
    timer::Timer,
    Channels, ClientInfo, Health, LobbyInfo, PlayersScore, Transform,
};

use bincode::{deserialize, serialize};
use renet::{
    client::LocalClientConnected,
    error::RenetError,
    protocol::unsecure::UnsecureServerProtocol,
    remote_connection::ConnectionConfig,
    server::{Server, ServerConfig, ServerEvent},
};

use glam::{vec2, Vec2};
use shipyard::*;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::time::Duration;

enum Scene {
    Lobby,
    Gameplay,
}

pub struct Game {
    pub world: World,
    scene: Scene,
    server: Server<UnsecureServerProtocol>,
    lobby_info: LobbyInfo,
    lobby_updated: bool,
}

struct GameplayInfo {
    respawn_players: bool,
    respawn_players_timer: Timer,
}

#[derive(Debug)]
pub struct GameplayConfig {
    pub dash_speed: f32,
    pub jump_speed: f32,
    pub walk_speed: f32,
    pub player_gravity: f32,
    pub dash_duration: f32,
    pub dash_cooldown: f32,
    pub fireball_cooldown: f32,
}

impl Default for GameplayConfig {
    fn default() -> Self {
        Self {
            dash_speed: 160.,
            jump_speed: 180.,
            walk_speed: 80.,
            player_gravity: 550.,
            dash_duration: 0.,
            dash_cooldown: 0.,
            fireball_cooldown: 0.,
        }
    }
}

type PlayerMapping = HashMap<u64, EntityId>;

impl Game {
    pub fn new(addr: SocketAddr) -> Result<Self, RenetError> {
        let socket = UdpSocket::bind(addr)?;
        let server_config = ServerConfig::default();
        let connection_config = ConnectionConfig::default();

        let server: Server<UnsecureServerProtocol> =
            Server::new(socket, server_config, connection_config, channels())?;

        let mut world = World::new();
        load_level_collisions(&mut world);

        let server_info = GameplayInfo {
            respawn_players: false,
            respawn_players_timer: Timer::new(Duration::from_secs(3)),
        };

        world.add_unique(server_info).unwrap();
        world.add_unique(PlayerMapping::new()).unwrap();
        world.add_unique(PlayersScore::default()).unwrap();
        world.add_unique(GameplayConfig::default()).unwrap();

        world.borrow::<ViewMut<Player>>().unwrap().track_deletion();
        world
            .borrow::<ViewMut<Projectile>>()
            .unwrap()
            .track_deletion();

        Ok(Self {
            world,
            server,
            scene: Scene::Lobby,
            lobby_info: LobbyInfo::default(),
            lobby_updated: false,
        })
    }

    pub fn get_host_client(&mut self, client_id: u64) -> LocalClientConnected {
        self.server.create_local_client(client_id)
    }

    pub fn update(&mut self) {
        self.lobby_updated = false;
        if let Err(e) = self.server.update() {
            println!("{}", e);
        }
        for client_id in self.server.get_clients_id().iter() {
            while let Ok(Some(message)) = self
                .server
                .receive_message(*client_id, Channels::ReliableCritical)
            {
                let input: PlayerInput = deserialize(&message).expect("Failed to deserialize.");
                self.world
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

            while let Ok(Some(message)) =
                self.server.receive_message(*client_id, Channels::Reliable)
            {
                let player_action: ClientAction = deserialize(&message).unwrap();
                self.handle_client_action(player_action, client_id);
            }
        }

        while let Some(event) = self.server.get_event() {
            match event {
                ServerEvent::ClientConnected(id) => {
                    self.lobby_info.clients.insert(id, ClientInfo::default());
                    self.lobby_updated = true;

                    self.world
                        .run(|mut players_score: UniqueViewMut<PlayersScore>| {
                            players_score.score.insert(id, 0);
                            players_score.updated = true;
                        })
                        .unwrap();
                }
                ServerEvent::ClientDisconnected(id) => {
                    self.lobby_info.clients.remove(&id);
                    self.lobby_updated = true;

                    self.world.run_with_data(remove_player, id).unwrap();
                    self.world
                        .run(|mut players_score: UniqueViewMut<PlayersScore>| {
                            players_score.score.remove(&id);
                            players_score.updated = true;
                        })
                        .unwrap();
                }
            }
        }

        match self.scene {
            Scene::Lobby => {
                let start_lobby = self.lobby_info.clients.len() > 1
                    && self.lobby_info.clients.values().all(|c| c.ready);
                if start_lobby {
                    self.scene = Scene::Gameplay;
                    let start_gameplay = ServerMessages::StartGameplay;
                    let start_gameplay = serialize(&start_gameplay).unwrap();
                    self.server
                        .broadcast_message(Channels::Reliable, start_gameplay);
                }
                if self.lobby_updated {
                    let lobby_update = ServerMessages::UpdateLobby(self.lobby_info.clone());
                    let lobby_update = serialize(&lobby_update).unwrap();
                    self.server
                        .broadcast_message(Channels::Reliable, lobby_update);
                }
            }
            Scene::Gameplay => {
                self.update_gameplay();
            }
        }

        self.server.send_packets();
    }

    fn update_gameplay(&mut self) {
        // Game logic
        self.world.run(update_players_cooldown).unwrap();
        self.world.run(update_animations).unwrap();
        self.world.run(update_players).unwrap();
        self.world.run(update_projectiles).unwrap();
        self.world.run(cast_fireball_player).unwrap();
        self.world.run(sync_physics).unwrap();

        // Clear dead entities
        self.world.run(remove_zero_health).unwrap();
        self.world.run(remove_dead).unwrap();
        self.world.run(destroy_physics_entities).unwrap();

        let should_check_win = self
            .world
            .run(|info: UniqueView<GameplayInfo>| {
                !info.respawn_players && self.lobby_info.clients.len() > 1
            })
            .unwrap();

        if should_check_win && self.world.run(check_win_condition).unwrap() {
            self.world.run(cleanup_world).unwrap();
            self.world
                .run(|mut info: UniqueViewMut<GameplayInfo>| {
                    info.respawn_players_timer.reset();
                    info.respawn_players = true;
                })
                .unwrap();
        }

        let respawn = self
            .world
            .run_with_data(respawn_players, self.lobby_info.clients.len())
            .unwrap();
        if respawn {
            for &client_id in self.lobby_info.clients.keys() {
                self.world.run_with_data(create_player, client_id).unwrap();
            }
        }

        let server_frame = ServerFrame::from_world(&self.world);
        let server_frame = serialize(&server_frame).unwrap();
        self.server
            .broadcast_message(Channels::Unreliable, server_frame);

        // Send score update to clients
        {
            let mut score = self.world.borrow::<UniqueViewMut<PlayersScore>>().unwrap();
            if score.updated {
                let score_message = ServerMessages::UpdateScore((*score).clone());
                let score_message = serialize(&score_message).unwrap();
                self.server
                    .broadcast_message(Channels::Reliable, score_message);
                score.updated = false;
            }
        }
    }

    fn handle_client_action(&mut self, action: ClientAction, client_id: &u64) {
        match action {
            ClientAction::LobbyReady => {
                let client_info = self.lobby_info.clients.get_mut(client_id).unwrap();
                client_info.ready = !client_info.ready;
                self.lobby_updated = true;
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
        let mut deads = all_storages.borrow::<ViewMut<Dead>>().unwrap();
        let mut health = all_storages.borrow::<ViewMut<Health>>().unwrap();
        let players = all_storages.borrow::<View<Player>>().unwrap();
        let mut physics = all_storages.borrow::<UniqueViewMut<Physics>>().unwrap();

        for (entity_id, mut projectile) in (&mut projectiles).iter().with_id() {
            projectile.duration = projectile
                .duration
                .checked_sub(Duration::from_micros(16666))
                .unwrap_or_else(|| Duration::from_micros(0));
            if projectile.duration.as_micros() == 0 {
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
                    health.take_damage(1, Some(player.client_id));
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
            let pos = transform.position + vec2(4., 6.);

            let entity_id = entities.add_entity((), ());
            physics.add_actor(entity_id, pos, 4, 4);

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
    gameplay: UniqueView<GameplayConfig>,
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
            player.speed = dash_direction * gameplay.dash_speed;
        }

        let pos = physics.actor_pos(entity_id);
        let on_ground = physics.collide_check(entity_id, pos + vec2(0., 1.));

        if player.current_dash_duration > 0.0 {
            player.current_dash_duration -= get_frame_time();
            if player.current_dash_duration <= 0.0 {
                player.speed = player.speed.normalize() * gameplay.walk_speed;
            }
        } else {
            if !on_ground {
                player.speed.y += gameplay.player_gravity * get_frame_time();
            } else {
                player.speed.y = gameplay.player_gravity * get_frame_time();
            }

            player.speed.x = movement_direction.x * gameplay.walk_speed;
            if input.jump && on_ground {
                player.speed.y = -gameplay.jump_speed;
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
            animation.play_animation("run");
        } else {
            animation.play_animation("idle");
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
    let rand = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_micros();

    let mut player_position =
        player_respawn_points.0[rand as usize % player_respawn_points.0.len()];
    // player_respawn_points.0[rand::rand() as usize % player_respawn_points.0.len()];

    player_position.y -= 16.;

    physics.add_actor(entity_id, player_position, 8, 12);

    let player = Player::new(client_id);
    let transform = Transform::default();
    let animation = AnimationEntity::Player.new_animation_controller();

    let player_health = Health::new(2);

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

fn respawn_players(connected_players: usize, mut info: UniqueViewMut<GameplayInfo>) -> bool {
    let mut respawn = false;
    if info.respawn_players && info.respawn_players_timer.is_finished() && connected_players > 1 {
        info.respawn_players = false;
        respawn = true;
    }
    respawn
}

fn get_frame_time() -> f32 {
    0.0166667
}
