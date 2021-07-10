use macroquad::prelude::*;
use shared::{
    channels,
    message::{ClientAction, ServerMessages},
    network::ServerFrame,
    physics::render_physics,
    player::Player,
    projectile::Projectile,
    Channels, EntityMapping, LobbyInfo, PlayersScore, Transform,
};

use alto_logger::TermLogger;
use renet::{
    client::{Client, RemoteClient},
    protocol::unsecure::UnsecureClientProtocol,
    remote_connection::ConnectionConfig,
};
use shipyard::*;
use ui::{draw_connect_menu, draw_connection_screen, draw_lobby, draw_score, UiState};

use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::{SystemTime, UNIX_EPOCH};

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

    let id = rand::rand() as u64;
    let mut app = App::new(id);

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
    MainMenu,
    Connect,
    Lobby,
    Gameplay,
    Connecting,
}

struct App {
    id: u64,
    screen: Screen,
    world: World,
    camera: Camera2D,
    render_target: RenderTarget,
    connection: Option<Box<dyn Client>>,
    lobby_info: LobbyInfo,
    ui: UiState,
    server: Option<Game>,
}

pub struct ClientInfo {
    pub client_id: u64,
    pub entity_id: Option<EntityId>,
}

impl App {
    fn new(id: u64) -> Self {
        let render_target = render_target((RX * UPSCALE) as u32, (RY * UPSCALE) as u32);
        set_texture_filter(render_target.texture, FilterMode::Nearest);

        let camera = Camera2D {
            zoom: vec2(1.0 / (RX * UPSCALE) * 2., 1.0 / (RY * UPSCALE) * 2.),
            render_target: Some(render_target),
            target: vec2((RX * UPSCALE) / 2., (RY * UPSCALE) / 2.),
            ..Default::default()
        };

        let world = World::new();

        let client_info = ClientInfo {
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

        let mut args = std::env::args();
        args.next();

        let mut server = None;
        let mut connection: Option<Box<dyn Client>> = None;
        let mut screen = Screen::Connect;
        if args.next().is_some() {
            let mut s = Game::new("127.0.0.1:5000".parse().unwrap()).unwrap();
            connection = Some(Box::new(s.get_host_client(id)));
            screen = Screen::Lobby;
            server = Some(s);
        }

        Self {
            render_target,
            id,
            ui: UiState::default(),
            world,
            camera,
            screen,
            lobby_info: LobbyInfo::default(),
            connection,
            server,
        }
    }

    async fn update(&mut self) {
        set_camera(self.camera);
        clear_background(BLACK);

        if let Some(server) = self.server.as_mut() {
            server.update();
        }

        if let Some(connection) = self.connection.as_mut() {
            if let Err(e) = connection.update() {
                println!("Client process events error: {}", e);
            };
            while let Ok(Some(message)) = connection.receive_message(Channels::Reliable.into()) {
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

        match self.screen {
            Screen::Gameplay => {
                self.render_gameplayer();
            }
            Screen::Connect => {
                if let Some(server_ip) = draw_connect_menu(&mut self.ui) {
                    self.screen = Screen::Connecting;
                    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
                    let connection_config = ConnectionConfig::default();

                    println!("Client ID: {}", self.id);

                    let connection = RemoteClient::new(
                        self.id,
                        socket,
                        server_ip,
                        channels(),
                        UnsecureClientProtocol::new(self.id),
                        connection_config,
                    )
                    .unwrap();

                    self.connection = Some(Box::new(connection));
                }
            }
            Screen::Connecting => {
                draw_connection_screen(&mut self.ui);
                if self.connection.as_mut().unwrap().is_connected() {
                    self.screen = Screen::Lobby;
                    self.ui.connect_error = None;
                };

                /*
                if self.connection.as_mut().unwrap().is_disconnected() {
                        self.screen = Screen::Connect;
                        self.request_connection = None;
                        self.ui.connect_error = Some("Server timed out.".into());
                    }
                }
                */
            }
            Screen::Lobby => {
                if let Some(connection) = self.connection.as_mut() {
                    if draw_lobby(&self.lobby_info, self.id) {
                        let message = bincode::serialize(&ClientAction::LobbyReady).unwrap();
                        if let Err(e) = connection.send_message(Channels::Reliable.into(), message)
                        {
                            println!("error sending message: {}", e);
                        }
                    }
                } else {
                    self.screen = Screen::Connect;
                    self.lobby_info = LobbyInfo::default();
                }
            }
            Screen::MainMenu => {}
        }

        // Send messages to server
        if let Some(connection) = self.connection.as_mut() {
            connection.send_packets().unwrap();
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

    fn render_gameplayer(&mut self) {
        if self.connection.is_none() {
            return;
        }

        let connection = self.connection.as_mut().unwrap();

        self.world.run(track_client_entity).unwrap();

        let input = self.world.run(player_input).unwrap();
        let message = bincode::serialize(&input).expect("failed to serialize message.");
        if let Err(e) = connection.send_message(Channels::ReliableCritical.into(), message) {
            println!("Error sending message: {}", e);
        }

        while let Ok(Some(message)) = connection.receive_message(Channels::Unreliable.into()) {
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
            server.world.run_with_data(render_physics, UPSCALE).unwrap();
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
