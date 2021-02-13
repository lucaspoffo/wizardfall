use macroquad::prelude::*;
use shared::{
    channels,
    ldtk::{draw_level, load_project_and_assets},
    message::ServerMessages,
    network::ServerFrame,
    player::Player,
    projectile::Projectile,
    EntityMapping, PlayersScore, Transform,
};

use alto_logger::TermLogger;
use renet::{
    client::{ClientConnected, RequestConnection},
    endpoint::EndpointConfig,
    error::RenetError,
    protocol::unsecure::UnsecureClientProtocol,
};
use shipyard::*;

use std::net::UdpSocket;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::collections::HashMap;

mod player;
mod animation;

use crate::player::{load_player_texture, draw_players, track_client_entity, player_input};

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
    let connection = get_connection("127.0.0.1:5000".to_string(), id as u64).unwrap();
    let mut app = App::new(id, connection);

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

fn get_connection(ip: String, id: u64) -> Result<ClientConnected, RenetError> {
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    let endpoint_config = EndpointConfig::default();

    println!("Client ID: {}", id);

    let mut request_connection = RequestConnection::new(
        id,
        socket,
        ip.parse().unwrap(),
        Box::new(UnsecureClientProtocol::new(id)),
        endpoint_config,
        channels(),
    )?;

    loop {
        println!("Connecting with server.");
        if let Some(connection) = request_connection.update()? {
            return Ok(connection);
        };
        sleep(Duration::from_millis(20));
    }
}

struct App {
    _id: u64,
    world: World,
    camera: Camera2D,
    render_target: RenderTarget,
    connection: ClientConnected,
}


pub struct ClientInfo {
    pub client_id: u64,
    pub entity_id: Option<EntityId>,
}

impl App {
    fn new(id: u64, connection: ClientConnected) -> Self {
        let render_target = render_target(640, 368);
        set_texture_filter(render_target.texture, FilterMode::Nearest);

        let camera = Camera2D {
            zoom: vec2(
                1.0 / 640. * 2.,
                1.0 /  320. * 2.,
            ),
            render_target: Some(render_target),
            target: vec2(320., 160.),
            ..Default::default()
        };

        let world = World::new();

        let client_info = ClientInfo { client_id: id, entity_id: None };
        world.add_unique(client_info).unwrap();

        // Tracking of components
        world.borrow::<ViewMut<Player>>().unwrap().track_all();

        Self {
            render_target,
            _id: id,
            world,
            camera,
            connection,
        }
    }

    async fn update(&mut self) {
        set_camera(self.camera);
        clear_background(BLACK);
        
        self.world.run(track_client_entity).unwrap();

        let input = self.world.run_with_data(player_input, &self.camera).unwrap();
        let message = bincode::serialize(&input).expect("Failed to serialize message.");
        self.connection.send_message(0, message.into_boxed_slice());
        self.connection.send_packets().unwrap();

        if let Err(e) = self.connection.process_events(Instant::now()) {
            println!("{}", e);
        };

        for payload in self.connection.receive_all_messages_from_channel(1).iter() {
            let server_frame: ServerFrame =
                bincode::deserialize(payload).expect("Failed to deserialize state.");

            server_frame.apply_in_world(&self.world);
        }

        for payload in self.connection.receive_all_messages_from_channel(2).iter() {
            let server_message: ServerMessages =
                bincode::deserialize(payload).expect("Failed to deserialize state.");
            match server_message {
                ServerMessages::UpdateScore(score) => {
                    let mut player_scores =
                        self.world.borrow::<UniqueViewMut<PlayersScore>>().unwrap();
                    player_scores.score = score.score;
                }
            }
        }
    
        self.world.run(draw_level).unwrap();
        self.world.run(draw_players).unwrap();
        self.world.run(draw_projectiles).unwrap();
        
        set_default_camera();
        clear_background(RED);

        let desired_aspect_ratio = 640. / 320.;
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

        self.world.run(draw_score).unwrap();
    }
}

fn draw_score(players_score: UniqueView<PlayersScore>) {
    let mut offset_x = 0.;
    for (client_id, score) in players_score.score.iter() {
        let text = format!("{}: {}", client_id, score);
        draw_rectangle_lines(27. + offset_x, 23., 190., 40., 1., WHITE);
        draw_text(&text, 30. + offset_x, 20., 30., WHITE);
        offset_x += 200.;
    }
}

fn draw_projectiles(projectiles: View<Projectile>, transform: View<Transform>) {
    for (_, transform) in (&projectiles, &transform).iter() {
        draw_rectangle(
            transform.position.x,
            transform.position.y,
            16.0,
            16.0,
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
