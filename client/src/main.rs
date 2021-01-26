// use shared::channels;
use async_once::AsyncOnce;
use lazy_static::lazy_static;
use macroquad::prelude::*;
use shared::{
    channels, AnimationManager, CastTarget, Player, PlayerAction, PlayerAnimations, PlayerInput,
    PlayerState, Projectile, ProjectileState, ServerFrame, NetworkState, NetworkId
};

use alto_logger::TermLogger;
use renet::{
    client::{ClientConnected, RequestConnection},
    endpoint::EndpointConfig,
    error::RenetError,
    protocol::unsecure::UnsecureClientProtocol,
};
use shipyard::*;

use std::collections::HashMap;
use std::hash::Hash;
use std::net::UdpSocket;
use std::rc::Rc;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct App {
    id: u32,
    world: World,
    connection: ClientConnected,
    entity_mapping: EntityMapping,
}

type PlayerTexture = HashMap<PlayerAnimations, TextureAnimation>;
type EntityMapping = HashMap<u32, EntityId>;

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

impl App {
    fn new(id: u32, connection: ClientConnected) -> Self {
        let world = World::new();
        Self {
            id,
            world,
            connection,
            entity_mapping: HashMap::new(),
        }
    }

    async fn load_texture(&mut self) {
        let mut animations = HashMap::new();
        let idle_texture: Texture2D = load_texture("Blue_witch/B_witch_idle.png").await;
        let run_texture: Texture2D = load_texture("Blue_witch/B_witch_run.png").await;

        let idle_animation = TextureAnimation::new(idle_texture, 32, 48, 1, 6);
        let run_animation = TextureAnimation::new(run_texture, 32, 48, 1, 8);

        animations.insert(PlayerAnimations::Idle, idle_animation);
        animations.insert(PlayerAnimations::Run, run_animation);
        self.world.add_unique(animations).unwrap();
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

        if is_mouse_button_pressed(MouseButton::Left) {
            let cast_target = CastTarget {
                position: mouse_position().into(),
            };
            let cast_fireball = PlayerAction::CastFireball(cast_target);

            let message = bincode::serialize(&cast_fireball).expect("Failed to serialize message.");
            self.connection.send_message(2, message.into_boxed_slice());
        }

        if is_key_pressed(KeyCode::Space) {
            let mut cast_target = CastTarget {
                position: mouse_position().into()
            };
            cast_target.position = cast_target.position - vec2(16.0, 24.0);
            let cast_teleport = PlayerAction::CastTeleport(cast_target);

            let message = bincode::serialize(&cast_teleport).expect("Failed to serialize message.");
            self.connection.send_message(2, message.into_boxed_slice());
        }

        let message = bincode::serialize(&input).expect("Failed to serialize message.");
        self.connection.send_message(0, message.into_boxed_slice());
        self.connection.send_packets().unwrap();

        if let Err(e) = self.connection.process_events(Instant::now()) {
            println!("{}", e);
        };

        // let network_info = self.connection.network_info();
        // println!("{:?}", network_info);

        for payload in self.connection.receive_all_messages_from_channel(1).iter() {
            let server_frame: ServerFrame =
                bincode::deserialize(payload).expect("Failed to deserialize state.");

            self.world.run_with_data(
                update_network_state::<Player>,
                (&server_frame.players, &mut self.entity_mapping),
            ).unwrap();

            self.world.run_with_data(
                update_network_state::<Projectile>,
                (&server_frame.projectiles, &mut self.entity_mapping),
            ).unwrap();
        }

        // println!("{:?}", self.world);
        // self.world.run(debug::<Player>);
        // self.world.run(debug::<Projectile>);

        self.world.run(draw_players).unwrap();
        self.world.run(draw_projectiles).unwrap();
    }
}

#[macroquad::main("Renet macroquad demo")]
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
    app.load_texture().await;

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

fn update_network_state<T: NetworkState + 'static + Send + Sync>(
    (entities_state, entity_mapping): (&[T::State], &mut EntityMapping),
    mut all_storages: AllStoragesViewMut,
) {
    let removed_entities: Vec<EntityId> = {
        let mut entities = all_storages.borrow::<EntitiesViewMut>().unwrap();
        let mut network_entities = all_storages.borrow::<ViewMut<T>>().unwrap();

        for state in entities_state.iter() {
            let entity_id = entity_mapping.entry(state.id()).or_insert_with(|| {
                let entity = T::from_state(state);
                entities.add_entity(&mut network_entities, entity)
            });
            if let Ok(mut entity) = (&mut network_entities).get(*entity_id) {
                entity.update_from_state(&state);
            }
        }

        let network_entities_id: Vec<u32> = entities_state.iter().map(|p| p.id()).collect();

        let removed_id: Vec<u32> = network_entities
            .iter()
            .filter(|entity| !network_entities_id.contains(&entity.id()))
            .map(|entity| entity.id())
            .collect();

        let mut removed = vec![];

        for id in removed_id {
            if let Some(entity_id) = entity_mapping.remove(&id) {
                removed.push(entity_id);
            }
        }
        removed
    };

    for id in removed_entities {
        all_storages.delete_entity(id);
    }
}

fn draw_players(player_texture: UniqueView<PlayerTexture>, players: View<Player>) {
    for player in players.iter() {
        let current_animation = &player.animation_manager.current_animation;
        let texture_animation = player_texture.get(current_animation).unwrap();

        let ac = player.animation_manager.current_animation_controller();
        let texture_x = ac.frame % texture_animation.h_frames * texture_animation.width;
        let texture_y = ac.frame / texture_animation.h_frames * texture_animation.height;
        let draw_rect = Rect::new(
            texture_x as f32,
            texture_y as f32,
            texture_animation.width as f32,
            texture_animation.height as f32,
        );

        let mut x = player.position.x;
        let y = player.position.y;

        let mut params = DrawTextureParams::default();
        params.source = Some(draw_rect);
        let mut x_size = texture_animation.width as f32;
        if player.animation_manager.h_flip {
            x_size *= -1.0;
            x += texture_animation.width as f32;
        }
        params.dest_size = Some(vec2(x_size, texture_animation.height as f32));

        draw_texture_ex(texture_animation.texture, x, y, WHITE, params)
    }
}

fn draw_projectiles(projectiles: View<Projectile>) {
    for projectile in projectiles.iter() {
        draw_rectangle(
            projectile.position.x,
            projectile.position.y,
            16.0,
            16.0,
            RED,
        );
    }
}

fn debug<T: std::fmt::Debug + 'static>(view: View<T>) {
    for entity in view.iter() {
        println!("{:?}", entity);
    }
}
