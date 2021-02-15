use macroquad::prelude::*;
use shared::{
    channels,
    message::{ClientAction, ServerMessages},
    network::ServerFrame,
    player::Player,
    projectile::Projectile,
    EntityMapping, LobbyInfo, PlayersScore, Transform,
};

use alto_logger::TermLogger;
use renet::{
    client::{ClientConnected, RequestConnection},
    endpoint::EndpointConfig,
    protocol::unsecure::UnsecureClientProtocol,
};
use shipyard::*;
use ui::{draw_connect_menu, draw_connection_screen, draw_lobby, draw_score, UiState};

use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use level::{draw_level, load_project_and_assets};

mod animation;
mod player;
mod ui;
mod level;

use crate::player::{draw_players, load_player_texture, player_input, track_client_entity};

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

    let mapping: EntityMapping = HashMap::new();
    app.world.add_unique(mapping).unwrap();
    app.world.add_unique(PlayersScore::default()).unwrap();

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
    connection: Option<ClientConnected>,
    request_connection: Option<RequestConnection>,
    lobby_info: LobbyInfo,
    ui: UiState,
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

        // Tracking of components
        world.borrow::<ViewMut<Player>>().unwrap().track_all();

        Self {
            render_target,
            id,
            ui: UiState::default(),
            world,
            camera,
            screen: Screen::Connect,
            request_connection: None,
            lobby_info: LobbyInfo::default(),
            connection: None,
        }
    }

    async fn update(&mut self) {
        set_camera(self.camera);
        clear_background(BLACK);

        if let Some(connection) = self.connection.as_mut() {
            if let Err(e) = connection.process_events(Instant::now()) {
                println!("{}", e);
            };
            for payload in connection.receive_all_messages_from_channel(2).iter() {
                let server_message: ServerMessages = bincode::deserialize(payload).unwrap();
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
                    let endpoint_config = EndpointConfig::default();

                    println!("Client ID: {}", self.id);

                    let request_connection = RequestConnection::new(
                        self.id,
                        socket,
                        server_ip,
                        Box::new(UnsecureClientProtocol::new(self.id)),
                        endpoint_config,
                        channels(),
                    )
                    .unwrap();

                    self.request_connection = Some(request_connection);
                }
            }
            Screen::Connecting => {
                draw_connection_screen(&mut self.ui);
                match self.request_connection.as_mut().unwrap().update() {
                    Ok(Some(connection)) => {
                        self.connection = Some(connection);
                        self.request_connection = None;
                        self.screen = Screen::Lobby;
                        self.ui.connect_error = None;
                    }
                    Ok(None) => {}
                    Err(_) => {
                        self.screen = Screen::Connect;
                        self.request_connection = None;
                        self.ui.connect_error = Some("Server timed out.".into());
                    }
                }
            }
            Screen::Lobby => {
                if let Some(connection) = self.connection.as_mut() {
                    if draw_lobby(&self.lobby_info, self.id) {
                        let message = bincode::serialize(&ClientAction::LobbyReady).unwrap();
                        connection.send_message(2, message.into_boxed_slice());
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
        connection.send_message(0, message.into_boxed_slice());

        for payload in connection.receive_all_messages_from_channel(1).iter() {
            let server_frame: ServerFrame = bincode::deserialize(payload).unwrap();
            server_frame.apply_in_world(&self.world);
        }

        self.world.run(draw_level).unwrap();
        self.world.run(draw_players).unwrap();
        self.world.run(draw_projectiles).unwrap();
        self.world.run(draw_score).unwrap();
    }
}

fn draw_projectiles(projectiles: View<Projectile>, transform: View<Transform>) {
    for (_, transform) in (&projectiles, &transform).iter() {
        draw_rectangle(transform.position.x * UPSCALE, transform.position.y * UPSCALE, 16.0 * UPSCALE, 16.0 * UPSCALE, RED);
    }
}

#[allow(dead_code)]
fn debug<T: std::fmt::Debug + 'static>(view: View<T>) {
    for (entity_id, component) in view.iter().with_id() {
        println!("[Debug] {:?}: {:?}", entity_id, component);
    }
}
