// use shared::channels;
use async_once::AsyncOnce;
use lazy_static::lazy_static;
use macroquad::prelude::*;
use shared::{
    channels, AnimationManager, CastTarget, Player, PlayerAction, PlayerAnimations, PlayerInput,
    PlayerState, Projectile, ServerFrame, ProjectileState
};

use alto_logger::TermLogger;
use renet::{
    client::{ClientConnected, RequestConnection},
    endpoint::EndpointConfig,
    error::RenetError,
    protocol::unsecure::UnsecureClientProtocol,
};
use std::collections::HashMap;
use std::hash::Hash;
use std::net::UdpSocket;
use std::rc::Rc;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct App {
    id: u32,
    players: HashMap<u32, PlayerClient>,
    projectiles: HashMap<u32, Projectile>,
    connection: ClientConnected,
}

lazy_static! {
    static ref PLAYERS_TEXTURE: AsyncOnce<Rc<HashMap<PlayerAnimations, TextureAnimation>>> =
        AsyncOnce::new(async {
            let mut animations = HashMap::new();
            let idle_texture: Texture2D = load_texture("Blue_witch/B_witch_idle.png").await;
            let run_texture: Texture2D = load_texture("Blue_witch/B_witch_run.png").await;

            let idle_animation = TextureAnimation::new(idle_texture, 32, 48, 1, 6);
            let run_animation = TextureAnimation::new(run_texture, 32, 48, 1, 8);

            animations.insert(PlayerAnimations::Idle, idle_animation);
            animations.insert(PlayerAnimations::Run, run_animation);
            Rc::new(animations)
        });
}

struct TextureAnimation {
    texture: Texture2D,
    width: u32,
    height: u32,
    h_frames: u32,
    v_frames: u32,
}

impl TextureAnimation {
    pub fn new(texture: Texture2D, width: u32, height: u32, h_frames: u32, v_frames: u32) -> Self {
        Self {
            texture,
            width,
            height,
            h_frames,
            v_frames,
        }
    }
}

struct PlayerClient {
    inner: Player,
    animation_manager_client: AnimationManagerClient<PlayerAnimations>,
}

impl PlayerClient {
    pub async fn from_state(state: &PlayerState) -> Self {
        Self {
            inner: Player::from_state(state),
            animation_manager_client: AnimationManagerClient::new().await,
        }
    }
}

struct AnimationManagerClient<T> {
    animations_texture: Rc<HashMap<T, TextureAnimation>>,
}

impl<T: Eq + Hash + Clone> AnimationManagerClient<T> {
    pub fn draw(&self, mut x: f32, y: f32, animation_manager: &AnimationManager<T>) {
        let texture_animation = self
            .animations_texture
            .get(&animation_manager.current_animation)
            .unwrap();
        let ac = animation_manager.current_animation_controller();
        let texture_x = ac.frame % texture_animation.h_frames * texture_animation.width;
        let texture_y = ac.frame / texture_animation.h_frames * texture_animation.height;
        let draw_rect = Rect::new(
            texture_x as f32,
            texture_y as f32,
            texture_animation.width as f32,
            texture_animation.height as f32,
        );

        let mut params = DrawTextureParams::default();
        params.source = Some(draw_rect);
        let mut x_size = texture_animation.width as f32;
        if animation_manager.h_flip {
            x_size *= -1.0;
            x += texture_animation.width as f32;
        }
        params.dest_size = Some(vec2(x_size, texture_animation.height as f32));

        draw_texture_ex(texture_animation.texture, x, y, WHITE, params)
    }
}

impl AnimationManagerClient<PlayerAnimations> {
    pub async fn new() -> Self {
        Self {
            animations_texture: PLAYERS_TEXTURE.get().await.clone(),
        }
    }
}

impl App {
    fn new(id: u32, connection: ClientConnected) -> Self {
        Self {
            id,
            connection,
            players: HashMap::new(),
            projectiles: HashMap::new(),
        }
    }

    async fn update_players(&mut self, players_state: Vec<PlayerState>) {
        for player_state in players_state.iter() {
            if let Some(player) = self.players.get_mut(&player_state.id) {
                player.inner.update_from_state(&player_state);
            } else {
                self.players.insert(
                    player_state.id,
                    PlayerClient::from_state(player_state).await,
                );
            }
        }

        let players_id: Vec<u32> = players_state.iter().map(|p| p.id).collect();
        let removed_players: Vec<u32> = self
            .players
            .keys()
            .filter(|player_id| !players_id.contains(player_id))
            .map(|id| id.clone())
            .collect();
        for id in removed_players {
            self.players.remove(&id);
        }
    }

    fn update_projectiles(&mut self, projectiles_state: Vec<ProjectileState>) {
        for projectile_state in projectiles_state.iter() {
            if let Some(projectile) = self.projectiles.get_mut(&projectile_state.id) {
                projectile.update_from_state(projectile_state);
            } else {
                self.projectiles.insert(
                    projectile_state.id,
                    Projectile::from_state(projectile_state),
                );
            }
        }

        let projectiles_id: Vec<u32> = projectiles_state.iter().map(|p| p.id).collect();
        let removed_projectiles: Vec<u32> = self
            .projectiles
            .keys()
            .filter(|player_id| !projectiles_id.contains(player_id))
            .map(|id| id.clone())
            .collect();
        for id in removed_projectiles {
            self.projectiles.remove(&id);
        }
    }

    async fn update(&mut self) {
        let up = is_key_down(KeyCode::W) || is_key_down(KeyCode::Up);
        let down = is_key_down(KeyCode::S) || is_key_down(KeyCode::Down);
        let left = is_key_down(KeyCode::A) || is_key_down(KeyCode::Left);
        let right = is_key_down(KeyCode::D) || is_key_down(KeyCode::Right);

        let input = PlayerInput {
            up,
            down,
            left,
            right,
        };

        if is_mouse_button_down(MouseButton::Left) {
            let mouse_pos = mouse_position();
            let cast_target = CastTarget {
                x: mouse_pos.0,
                y: mouse_pos.1,
            };
            let cast_fireball = PlayerAction::CastFireball(cast_target);

            let message = bincode::serialize(&cast_fireball).expect("Failed to serialize message.");
            self.connection.send_message(2, message.into_boxed_slice());
        }

        let message = bincode::serialize(&input).expect("Failed to serialize message.");
        self.connection.send_message(0, message.into_boxed_slice());
        self.connection.send_packets().unwrap();

        self.connection.process_events(Instant::now()).unwrap();

        let network_info = self.connection.network_info();
        println!("{:?}", network_info);

        for payload in self.connection.receive_all_messages_from_channel(1).iter() {
            let server_frame: ServerFrame =
                bincode::deserialize(payload).expect("Failed to deserialize state.");
            self.update_players(server_frame.players).await;
            self.update_projectiles(server_frame.projectiles);
        }

        for player in self.players.values() {
            player.animation_manager_client.draw(
                player.inner.x,
                player.inner.y,
                &player.inner.animation_manager,
            );
        }

        for projectile in self.projectiles.values() {
            draw_rectangle(projectile.x, projectile.y, 16.0, 16.0, RED);
        }
    }
}

#[macroquad::main("NaiaMacroquadExample")]
async fn main() {
    TermLogger::default().init().unwrap();
    rand::srand(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );

    let id = rand::rand() as u32;
    let connection = get_connection("127.0.0.1:5000".to_string(), id as u64).unwrap();
    let mut app = App::new(id, connection);

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
