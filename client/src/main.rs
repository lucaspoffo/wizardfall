use macroquad::prelude::*;
use shared::{
    channels_config,
    message::{ClientAction, ServerMessages},
    network::ServerFrame,
    physics::render_physics,
    player::Player,
    projectile::Projectile,
    Channel, EntityMapping, LobbyInfo, PlayersScore, Transform,
};

use renet_udp::{client::UdpClient, renet::remote_connection::ConnectionConfig};

use alto_logger::TermLogger;
use shipyard::*;
use ui::{draw_connect_menu, draw_lobby, draw_score, ConnectMenuResponse, UiState};

use std::net::{SocketAddr, UdpSocket};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, time::Instant};

use level::{draw_level, load_project_and_assets};

mod animation;
mod level;
mod player;
mod ui;

use crate::animation::{AnimationTextures, Textures};
use crate::player::{draw_players, load_player_texture, player_input, track_client_entity};

use server::Game;

#[macroquad::main("Renet macroquad demo")]
async fn main() {
    TermLogger::default().init().unwrap();
    rand::srand(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );

    let id = "127.0.0.1:0".parse().unwrap();
    let mut app = App::new(id);

    let mut args = std::env::args();
    args.next();
    if args.next().is_some() {
        app.host("127.0.0.1:5000".parse().unwrap());
    }

    load_player_texture(&mut app.world).await;
    load_project_and_assets(&app.world).await;

    loop {
        clear_background(BLACK);

        app.update().await;

        next_frame().await
    }
}

pub const RX: f32 = 336.;
pub const RY: f32 = 192.;
pub const UPSCALE: f32 = 10.;

pub enum Screen {
    Connect,
    Lobby,
    Gameplay,
}

struct App {
    id: SocketAddr,
    screen: Screen,
    world: World,
    camera: Camera2D,
    render_target: RenderTarget,
    client: Option<UdpClient>,
    lobby_info: LobbyInfo,
    ui: UiState,
    server: Option<Game>,
    last_updated: Instant,
}

pub struct ClientState {
    pub client_id: SocketAddr,
    pub entity_id: Option<EntityId>,
}

impl App {
    fn new(id: SocketAddr) -> Self {
        let render_target = render_target((RX * UPSCALE) as u32, (RY * UPSCALE) as u32);
        render_target.texture.set_filter(FilterMode::Nearest);

        let camera = Camera2D {
            zoom: vec2(1.0 / (RX * UPSCALE) * 2., 1.0 / (RY * UPSCALE) * 2.),
            render_target: Some(render_target),
            target: vec2((RX * UPSCALE) / 2., (RY * UPSCALE) / 2.),
            ..Default::default()
        };

        let world = World::new();

        let client_info = ClientState {
            client_id: id,
            entity_id: None,
        };

        world.add_unique(client_info).unwrap();
        world.add_unique(AnimationTextures(HashMap::new())).unwrap();
        world.add_unique(Textures(HashMap::new())).unwrap();

        let mapping: EntityMapping = HashMap::new();
        world.add_unique(mapping).unwrap();
        world.add_unique(PlayersScore::default()).unwrap();

        // Tracking of components
        world.borrow::<ViewMut<Player>>().unwrap().track_all();

        let server = None;
        let client: Option<UdpClient> = None;
        let screen = Screen::Connect;

        Self {
            render_target,
            id,
            ui: UiState::default(),
            world,
            camera,
            screen,
            lobby_info: LobbyInfo::default(),
            client,
            server,
            last_updated: Instant::now(),
        }
    }

    async fn update(&mut self) {
        set_camera(&self.camera);
        clear_background(BLACK);

        if let Some(server) = self.server.as_mut() {
            server.update();
        }

        let now = Instant::now();
        let frame_duration = now - self.last_updated;
        self.last_updated = now;
        let mut has_client_error = false;
        if let Some(client) = self.client.as_mut() {
            if let Err(e) = client.update(frame_duration) {
                self.ui.connect_error = Some(format!("{}", e));
                self.screen = Screen::Connect;
                self.server = None;
                has_client_error = true;
                println!("Client update error: {}", e);
            } else {
                while let Some(message) = client.receive_message(Channel::Reliable.id()) {
                    let server_message: ServerMessages = bincode::deserialize(&message).unwrap();
                    match server_message {
                        ServerMessages::UpdateScore(score) => {
                            let mut player_scores =
                                self.world.borrow::<UniqueViewMut<PlayersScore>>().unwrap();
                            player_scores.score = score.score;
                        }
                        ServerMessages::UpdateLobby(lobby_info) => {
                            self.lobby_info = lobby_info;
                        }
                        ServerMessages::StartGameplay => {
                            self.screen = Screen::Gameplay;
                        }
                    }
                }
            }
        }
        if has_client_error {
            self.client = None;
        }

        match self.screen {
            Screen::Gameplay => {
                self.render_gameplayer();
            }
            Screen::Connect => {
                let ConnectMenuResponse {
                    addr,
                    host,
                    connect,
                } = draw_connect_menu(&mut self.ui);
                if let Some(server_addr) = addr {
                    if host {
                        self.host(server_addr);
                    } else if connect {
                        self.ui.connect_error = None;
                        self.screen = Screen::Lobby;
                        let socket = UdpSocket::bind(self.id).unwrap();
                        let connection_config = ConnectionConfig {
                            channels_config: channels_config(),
                            ..Default::default()
                        };
                        self.id = socket.local_addr().unwrap();
                        let client =
                            UdpClient::new(socket, server_addr, connection_config).unwrap();
                        self.client = Some(client);
                    }
                }
            }
            Screen::Lobby => {
                if let Some(connection) = self.client.as_mut() {
                    if draw_lobby(&self.lobby_info, self.id) {
                        let message = bincode::serialize(&ClientAction::LobbyReady).unwrap();
                        if let Err(e) = connection.send_message(Channel::Reliable.id(), message) {
                            println!("error sending message: {}", e);
                        }
                    }
                } else {
                    self.screen = Screen::Connect;
                    self.lobby_info = LobbyInfo::default();
                }
            }
        }

        // Send messages to server
        if let Some(connection) = self.client.as_mut() {
            if let Err(e) = connection.send_packets() {
                error!("{}", e);
            }
        }

        set_default_camera();
        clear_background(RED);

        let desired_aspect_ratio = RX / RY;
        let current_aspect_ratio = screen_width() / screen_height();
        let mut viewport_height = screen_width() / desired_aspect_ratio;
        let mut viewport_width = screen_height() * desired_aspect_ratio;
        let mut draw_x = 0.;
        let mut draw_y = 0.;

        if current_aspect_ratio > desired_aspect_ratio {
            viewport_height = screen_height();
            draw_x = (screen_width() - viewport_width) / 2.;
        } else if current_aspect_ratio < desired_aspect_ratio {
            viewport_width = screen_width();
            draw_y = (screen_height() - viewport_height) / 2.;
        }

        draw_texture_ex(
            self.render_target.texture,
            draw_x,
            draw_y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(viewport_width, viewport_height)),
                ..Default::default()
            },
        );
    }

    fn host(&mut self, server_addr: SocketAddr) {
        let s = Game::new(server_addr).unwrap();
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        self.id = socket.local_addr().unwrap();

        let connection_config = ConnectionConfig {
            channels_config: channels_config(),
            ..Default::default()
        };
        let client_udp = UdpClient::new(socket, server_addr, connection_config).unwrap();
        self.client = Some(client_udp);
        self.screen = Screen::Lobby;
        self.server = Some(s);
    }

    fn render_gameplayer(&mut self) {
        if self.client.is_none() {
            return;
        }

        let connection = self.client.as_mut().unwrap();
        self.world
            .run_with_data(track_client_entity, self.id)
            .unwrap();

        let input = self.world.run(player_input).unwrap();
        let message = bincode::serialize(&input).expect("failed to serialize message.");
        if let Err(e) = connection.send_message(Channel::ReliableCritical.id(), message) {
            println!("Error sending message: {}", e);
        }

        while let Some(message) = connection.receive_message(Channel::Unreliable.id()) {
            let server_frame = bincode::deserialize::<ServerFrame>(&message);
            if let Ok(server_frame) = server_frame {
                server_frame.apply_in_world(&self.world);
            } else {
                println!("Error deserializing {:?}", server_frame);
            }
        }

        self.world.run(draw_level).unwrap();
        self.world.run(draw_players).unwrap();
        self.world.run(draw_projectiles).unwrap();
        self.world.run(draw_score).unwrap();

        // Debug server physics when host
        if let Some(server) = self.server.as_ref() {
            if false {
                server.world.run_with_data(render_physics, UPSCALE).unwrap();
            }
        }
    }
}

fn draw_projectiles(projectiles: View<Projectile>, transform: View<Transform>) {
    for (_, transform) in (&projectiles, &transform).iter() {
        draw_rectangle(
            transform.position.x * UPSCALE,
            transform.position.y * UPSCALE,
            4.0 * UPSCALE,
            4.0 * UPSCALE,
            RED,
        );
    }
}

#[allow(dead_code)]
fn debug<T: std::fmt::Debug + 'static>(view: View<T>) {
    for (entity_id, component) in view.iter().with_id() {
        println!("[Debug] {:?}: {:?}", entity_id, component);
    }
}
