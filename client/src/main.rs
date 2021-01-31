// use shared::channels;
use macroquad::prelude::*;
use shared::{
    channels, Animation, AnimationController, CastTarget, EntityMapping,
    Player, PlayerAction, PlayerAnimation, PlayerInput, Projectile, ServerFrame,
    Transform, 
    physics::{Velocity, CollisionShape, calculate_collisions, update_position, sync_transform}
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
use std::net::UdpSocket;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct App {
    id: u64,
    world: World,
    connection: ClientConnected,
}

type AnimationTexture = HashMap<Animation, TextureAnimation>;

struct TextureAnimation {
    texture: Texture2D,
    width: u32,
    height: u32,
    h_frames: u8,
    v_frames: u8,
}

impl TextureAnimation {
    pub fn new(texture: Texture2D, width: u32, height: u32, h_frames: u8, v_frames: u8) -> Self {
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
    fn new(id: u64, connection: ClientConnected) -> Self {
        let world = World::new();
        Self {
            id,
            world,
            connection,
        }
    }

    async fn load_texture(&mut self) {
        let mut animations: AnimationTexture = HashMap::new();
        let idle_texture: Texture2D = load_texture("Blue_witch/B_witch_idle.png").await;
        let run_texture: Texture2D = load_texture("Blue_witch/B_witch_run.png").await;

        let idle_animation = TextureAnimation::new(idle_texture, 32, 48, 1, 6);
        let run_animation = TextureAnimation::new(run_texture, 32, 48, 1, 8);

        animations.insert(PlayerAnimation::Idle.into(), idle_animation);
        animations.insert(PlayerAnimation::Run.into(), run_animation);
        self.world.add_unique(animations).unwrap();
    }

    async fn update(&mut self) {
        let up = is_key_down(KeyCode::W) || is_key_down(KeyCode::Up);
        let down = is_key_down(KeyCode::S) || is_key_down(KeyCode::Down);
        let left = is_key_down(KeyCode::A) || is_key_down(KeyCode::Left);
        let right = is_key_down(KeyCode::D) || is_key_down(KeyCode::Right);

        let direction = self.world.run_with_data(player_direction, self.id).unwrap();

        let input = PlayerInput {
            up,
            down,
            left,
            right,
            direction,
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
                position: mouse_position().into(),
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

            server_frame.apply_in_world(&self.world);
        }

        // println!("{:?}", self.world);
        // self.world.run(debug::<Player>);
        // self.world.run(debug::<Projectile>);
        // self.world.run(debug::<Transform>);

        // self.world.run(draw_players).unwrap();
        self.world.run(draw_players).unwrap();
        self.world.run(draw_projectiles).unwrap();
    }
}

struct PlayerTest;

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

    app.load_texture().await;

    let player = CollisionShape {
        rect: Rect::new(0.0, 0.0, 50.0, 50.0),
    };
    let transform = Transform {
        position: vec2(0.0, 0.0),
        rotation: 0.0,
    };
    let velocity = Velocity(Vec2::zero());
    app.world
        .add_entity((player, PlayerTest, transform, velocity));

    for i in 0..32 {
        let transform = Transform {
            position: vec2(100.0 + i as f32 * 32.0, 100.0),
            rotation: 0.0,
        };
        let collision_shape = CollisionShape {
            rect: Rect::new(100.0 + i as f32 * 32.0, 100.0, 32.0, 32.0),
        };
        app.world.add_entity((collision_shape, transform));
    }

    loop {
        clear_background(BLACK);

        app.update().await;

        app.world
            .run(
                |players: View<PlayerTest>,
                 mut velocities: ViewMut<Velocity>,
                 transforms: View<Transform>| {
                    for (transform, mut velocity, _) in
                        (&transforms, &mut velocities, &players).iter()
                    {
                        if is_mouse_button_down(MouseButton::Left) {
                            let vel = {
                                let mouse_pos: Vec2 = mouse_position().into();
                                let result = (mouse_pos
                                    - vec2(transform.position.x, transform.position.y))
                                .normalize()
                                    * 100.0;
                                if result.is_nan().any() {
                                    Vec2::zero()
                                } else {
                                    result
                                }
                            };
                            velocity.0.x = vel.x;
                            velocity.0.y = vel.y;
                        }
                    }
                },
            )
            .unwrap();

        app.world.run(sync_transform).unwrap();
        app.world.run(draw_collisions).unwrap();
        app.world.run_with_data(calculate_collisions, get_frame_time()).unwrap();
        app.world.run_with_data(update_position, get_frame_time()).unwrap();

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

fn draw_players(
    player_texture: UniqueView<AnimationTexture>,
    players: View<Player>,
    transforms: View<Transform>,
    animation_controller: View<AnimationController>,
) {
    for (player, transform, animation_controller) in
        (&players, &transforms, &animation_controller).iter()
    {
        let texture_animation = player_texture.get(&animation_controller.animation).unwrap();
        if animation_controller.frame > texture_animation.h_frames * texture_animation.v_frames {
            println!(
                "Invalid animation frame {} for texture player",
                animation_controller.frame
            );
            continue;
        }

        let texture_x = (animation_controller.frame % texture_animation.h_frames) as u32
            * texture_animation.width;
        let texture_y = (animation_controller.frame / texture_animation.h_frames) as u32
            * texture_animation.height;
        let draw_rect = Rect::new(
            texture_x as f32,
            texture_y as f32,
            texture_animation.width as f32,
            texture_animation.height as f32,
        );

        let x = transform.position.x;
        let y = transform.position.y;

        let mut params = DrawTextureParams::default();
        params.source = Some(draw_rect);
        let mut x_size = texture_animation.width as f32;
        let mut draw_x = x;
        if player.direction.angle_between(Vec2::unit_x()).abs() > std::f32::consts::PI / 2.0 {
            x_size *= -1.0;
            draw_x += texture_animation.width as f32;
        }
        params.dest_size = Some(vec2(x_size, texture_animation.height as f32));

        draw_texture_ex(texture_animation.texture, draw_x, y, WHITE, params);

        let center_x = x + (texture_animation.width as f32 / 2.0);
        let center_y = y + 4.0 + (texture_animation.height as f32 / 2.0);

        let wand_size = 12.0;
        let wand_x = center_x + player.direction.x * wand_size;
        let wand_y = center_y + player.direction.y * wand_size;

        draw_line(center_x, center_y, wand_x, wand_y, 3.0, YELLOW);
        draw_circle(wand_x, wand_y, 3.0, RED);
    }
}

fn draw_projectiles(projectiles: View<Projectile>, transform: View<Transform>) {
    for (_, transform) in (&projectiles, &transform).iter() {
        draw_rectangle(transform.position.x, transform.position.y, 16.0, 16.0, RED);
    }
}

fn player_direction(client_id: u64, players: View<Player>, transforms: View<Transform>) -> Vec2 {
    for (player, transform) in (&players, &transforms).iter() {
        if player.client_id == client_id {
            let mouse_pos: Vec2 = mouse_position().into();
            return (mouse_pos - transform.position).normalize();
        }
    }

    Vec2::unit_x()
}

#[allow(dead_code)]
fn debug<T: std::fmt::Debug + 'static>(view: View<T>) {
    for (entity_id, component) in view.iter().with_id() {
        println!("[Debug] {:?}: {:?}", entity_id, component);
    }
}

fn draw_collisions(collision_shapes: View<CollisionShape>) {
    for collision_shape in collision_shapes.iter() {
        draw_rectangle(
            collision_shape.rect.x,
            collision_shape.rect.y,
            collision_shape.rect.w,
            collision_shape.rect.h,
            WHITE,
        );
    }
}
