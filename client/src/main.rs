// use shared::channels;
use async_once::AsyncOnce;
use lazy_static::lazy_static;
use macroquad::prelude::*;
use shared::{
    channels, AnimationController, AnimationManager, Player, PlayerAnimations, PlayerInput,
    PlayerState, ServerFrame,
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
    id: u64,
    players: HashMap<u64, PlayerClient>,
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
    animation_manager_client: AnimationManagerClient<PlayerAnimations>
}

impl PlayerClient {
    pub async fn from_state(state: &PlayerState) -> Self {
        Self {
            inner: Player::from_state(state),
            animation_manager_client: AnimationManagerClient::new().await
        }
    }
}

struct AnimationManagerClient<T> {
    animations_texture: Rc<HashMap<T, TextureAnimation>>,
}

impl<T: Eq + Hash + Clone> AnimationManagerClient<T> {
    pub fn draw(&self, x: f32, y: f32, animation_manager: &AnimationManager<T>) {
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
    fn new(id: u64, connection: ClientConnected) -> Self {
        Self {
            id,
            connection,
            players: HashMap::new(),
        }
    }

    async fn update_players(&mut self, players_state: Vec<PlayerState>) {
        for player_state in players_state.iter() {
            if let Some(player) = self.players.get_mut(&player_state.id) {
                player.inner.update_from_state(&player_state);
            } else {
                self.players
                    .insert(player_state.id, PlayerClient::from_state(player_state).await);
            }
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

        let message = bincode::serialize(&input).expect("Failed to serialize message.");
        self.connection.send_message(0, message.into_boxed_slice());
        self.connection.send_packets().unwrap();

        self.connection.process_events(Instant::now()).unwrap();
        for payload in self.connection.receive_all_messages_from_channel(1).iter() {
            let server_frame: ServerFrame =
                bincode::deserialize(payload).expect("Failed to deserialize state.");
            self.update_players(server_frame.players).await;
        }

        const SIZE: f32 = 32.0;
        for player in self.players.values() {
            player.animation_manager_client.draw(player.inner.x as f32, player.inner.y as f32, &player.inner.animation_manager);
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

    let id = rand::rand() as u64;
    let connection = get_connection("127.0.0.1:5000".to_string(), id).unwrap();
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

    println!("Id: {}", id);

    let mut request_connection = RequestConnection::new(
        id,
        socket,
        ip.parse().unwrap(),
        Box::new(UnsecureClientProtocol::new(id)),
        endpoint_config,
        channels(),
    )?;

    loop {
        println!("connectiong");
        if let Some(connection) = request_connection.update()? {
            return Ok(connection);
        };
        sleep(Duration::from_millis(20));
    }
}

struct Animation {
    texture: Texture2D,
    h_frames: u32,
    v_frames: u32,
    height: u32,
    width: u32,
    frame: u32,
    speed: Duration,
    last_updated: Instant,
    total_frames: u32,
}

impl Animation {
    pub fn new(
        texture: Texture2D,
        h_frames: u32,
        v_frames: u32,
        width: u32,
        height: u32,
        fps: u64,
    ) -> Self {
        let speed = Duration::from_millis(1000 / fps);
        Self {
            texture,
            h_frames,
            v_frames,
            height,
            width,
            speed,
            frame: 0,
            last_updated: Instant::now(),
            total_frames: h_frames * v_frames,
        }
    }

    pub fn update(&mut self) {
        let current_time = Instant::now();
        if current_time - self.last_updated > self.speed {
            self.frame += 1;
            self.frame = self.frame % self.total_frames;
            self.last_updated = current_time;
        }
    }

    pub fn draw(&self, x: f32, y: f32) {
        let texture_x = self.frame % self.h_frames * self.width;
        let texture_y = self.frame / self.h_frames * self.height;
        let draw_rect = Rect::new(
            texture_x as f32,
            texture_y as f32,
            self.width as f32,
            self.height as f32,
        );

        let mut params = DrawTextureParams::default();
        params.source = Some(draw_rect);
        println!("frame: {}\n{:?}", self.frame, params.source);

        draw_texture_ex(self.texture, x, y, WHITE, params)
    }
}
