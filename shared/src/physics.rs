use macroquad::prelude::*;
use shipyard::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct StaticTiledLayer {
    static_colliders: Vec<bool>,
    tile_width: f32,
    tile_height: f32,
    width: usize,
    tag: u8,
    debug_color: Color,
}

#[derive(Debug)]
pub struct Physics {
    static_tiled_layers: Vec<StaticTiledLayer>,
    solids: HashMap<EntityId, Collider>,
    actors: HashMap<EntityId, Collider>,
}

#[derive(Clone, Debug)]
pub struct Collider {
    collidable: bool,
    squished: bool,
    pos: Vec2,
    width: i32,
    height: i32,
    x_remainder: f32,
    y_remainder: f32,
    squishers: HashSet<EntityId>,
}

impl Collider {
    pub fn rect(&self) -> Rect {
        Rect::new(
            self.pos.x,
            self.pos.y,
            self.width as f32,
            self.height as f32,
        )
    }
}

impl Physics {
    pub fn new() -> Physics {
        Physics {
            static_tiled_layers: vec![],
            actors: HashMap::new(),
            solids: HashMap::new(),
        }
    }

    pub fn add_static_tiled_layer(
        &mut self,
        static_colliders: Vec<bool>,
        tile_width: f32,
        tile_height: f32,
        width: usize,
        tag: u8,
        debug_color: Color,
    ) {
        self.static_tiled_layers.push(StaticTiledLayer {
            static_colliders,
            tile_width,
            tile_height,
            width,
            tag,
            debug_color,
        });
    }

    pub fn add_actor(&mut self, entity_id: EntityId, pos: Vec2, width: i32, height: i32) {
        self.actors.insert(
            entity_id,
            Collider {
                collidable: true,
                squished: false,
                pos,
                width,
                height,
                x_remainder: 0.,
                y_remainder: 0.,
                squishers: HashSet::new(),
            },
        );
    }

    pub fn add_solid(&mut self, entity_id: EntityId, pos: Vec2, width: i32, height: i32) {
        self.solids.insert(
            entity_id,
            Collider {
                collidable: true,
                squished: false,
                pos,
                width,
                height,
                x_remainder: 0.,
                y_remainder: 0.,
                squishers: HashSet::new(),
            },
        );
    }

    pub fn remove_actor(&mut self, actor: &EntityId) {
        self.actors.remove(actor);
    }

    pub fn remove_solid(&mut self, solid: &EntityId) {
        self.solids.remove(solid);
    }

    pub fn set_actor_position(&mut self, actor: &EntityId, pos: Vec2) {
        let mut collider = &mut self.actors.get_mut(actor).unwrap();

        collider.x_remainder = 0.0;
        collider.y_remainder = 0.0;
        collider.pos = pos;
    }

    pub fn move_v(&mut self, actor: EntityId, dy: f32) -> bool {
        let mut collider = self.actors[&actor].clone();

        collider.y_remainder += dy;

        let mut move_ = collider.y_remainder.round() as i32;
        if move_ != 0 {
            collider.y_remainder -= move_ as f32;
            let sign = move_.signum();

            while move_ != 0 {
                if self.collide_solids(
                    collider.pos + vec2(0., sign as f32),
                    collider.width,
                    collider.height,
                ) {
                    self.actors.insert(actor, collider);
                    return true;
                } else {
                    collider.pos.y += sign as f32;
                    move_ -= sign;
                }
            }
        }

        self.actors.insert(actor, collider);
        false
    }

    pub fn move_h(&mut self, actor: EntityId, dy: f32) -> bool {
        let mut collider = self.actors[&actor].clone();
        collider.x_remainder += dy;

        let mut move_ = collider.x_remainder.round() as i32;
        if move_ != 0 {
            collider.x_remainder -= move_ as f32;
            let sign = move_.signum();

            while move_ != 0 {
                if self.collide_solids(
                    collider.pos + vec2(sign as f32, 0.),
                    collider.width,
                    collider.height,
                ) {
                    self.actors.insert(actor, collider);
                    return true;
                } else {
                    collider.pos.x += sign as f32;
                    move_ -= sign;
                }
            }
        }

        self.actors.insert(actor, collider);
        return false;
    }

    pub fn solid_move(&mut self, solid: EntityId, dx: f32, dy: f32) {
        let mut collider = self.solids.get_mut(&solid).unwrap();

        collider.x_remainder += dx;
        collider.y_remainder += dy;
        let move_x = collider.x_remainder.round() as i32;
        let move_y = collider.y_remainder.round() as i32;

        let mut riding_actors = vec![];
        let mut pushing_actors = vec![];

        let riding_rect = Rect::new(
            collider.pos.x,
            collider.pos.y - 1.0,
            collider.width as f32,
            1.0,
        );
        let pushing_rect = Rect::new(
            collider.pos.x + move_x as f32,
            collider.pos.y,
            collider.width as f32 - 1.0,
            collider.height as f32,
        );

        for (actor, actor_collider) in &mut self.actors {
            let rider_rect = Rect::new(
                actor_collider.pos.x,
                actor_collider.pos.y + actor_collider.height as f32 - 1.0,
                actor_collider.width as f32,
                1.0,
            );

            if riding_rect.overlaps(&rider_rect) {
                riding_actors.push(*actor);
            } else if pushing_rect.overlaps(&actor_collider.rect())
                && actor_collider.squished == false
            {
                pushing_actors.push(*actor);
            }

            if pushing_rect.overlaps(&actor_collider.rect()) == false {
                actor_collider.squishers.remove(&solid);
                if actor_collider.squishers.len() == 0 {
                    actor_collider.squished = false;
                }
            }
        }

        self.solids.get_mut(&solid).unwrap().collidable = false;
        for actor in riding_actors {
            self.move_h(actor, move_x as f32);
        }
        for actor in pushing_actors {
            if self.move_h(actor, move_x as f32) {
                self.actors.get_mut(&actor).unwrap().squished = true;
                self.actors.get_mut(&actor).unwrap().squishers.insert(solid);
            }
        }
        self.solids.get_mut(&solid).unwrap().collidable = true;

        let collider = self.solids.get_mut(&solid).unwrap();
        if move_x != 0 {
            collider.x_remainder -= move_x as f32;
            collider.pos.x += move_x as f32;
        }
        if move_y != 0 {
            collider.y_remainder -= move_y as f32;
            collider.pos.y += move_y as f32;
        }
    }

    pub fn solid_at(&self, pos: Vec2) -> bool {
        self.tag_at(pos, 1)
    }

    pub fn tag_at(&self, pos: Vec2, tag: u8) -> bool {
        for StaticTiledLayer {
            tile_width,
            tile_height,
            width,
            static_colliders,
            tag: layer_tag,
            ..
        } in &self.static_tiled_layers
        {
            let y = (pos.y / tile_width) as i32;
            let x = (pos.x / tile_height) as i32;
            let ix = y * (*width as i32) + x;

            if ix >= 0 && ix < static_colliders.len() as i32 && static_colliders[ix as usize] {
                return *layer_tag == tag;
            }
        }

        self.solids.values().any(|collider| {
            if collider.collidable {
                return false;
            }
            collider.rect().contains(pos)
        })
    }

    pub fn collide_solids(&self, pos: Vec2, width: i32, height: i32) -> bool {
        self.collide_tag(1, pos, width, height)
            || self.solids.values().any(|collider| {
                collider.collidable
                    && collider.rect().overlaps(&Rect::new(
                        pos.x,
                        pos.y,
                        width as f32,
                        height as f32,
                    ))
            })
    }

    pub fn collide_tag(&self, tag: u8, pos: Vec2, width: i32, height: i32) -> bool {
        for StaticTiledLayer {
            tile_width,
            tile_height,
            width: layer_width,
            static_colliders,
            tag: layer_tag,
            ..
        } in &self.static_tiled_layers
        {
            let check = |pos: Vec2| {
                let y = (pos.y / tile_width) as i32;
                let x = (pos.x / tile_height) as i32;
                let ix = y * (*layer_width as i32) + x;
                if ix >= 0 && ix < static_colliders.len() as i32 && static_colliders[ix as usize] {
                    return *layer_tag == tag;
                }
                false
            };

            if check(pos)
                || check(pos + vec2(width as f32 - 1.0, 0.0))
                || check(pos + vec2(width as f32 - 1.0, height as f32 - 1.0))
                || check(pos + vec2(0.0, height as f32 - 1.0))
            {
                return true;
            }

            if width > *tile_width as i32 {
                let mut x = pos.x + tile_width;

                while x < pos.x + width as f32 - 1. {
                    if check(vec2(x, pos.y)) || check(vec2(x, pos.y + height as f32 - 1.0)) {
                        return true;
                    }
                    x += tile_width;
                }
            }

            if height > *tile_height as i32 {
                let mut y = pos.y + tile_height;

                while y < pos.y + height as f32 - 1. {
                    if check(vec2(pos.x, y)) || check(vec2(pos.x + width as f32 - 1., y)) {
                        return true;
                    }
                    y += tile_height;
                }
            }
        }

        false
    }

    pub fn squished(&self, actor: EntityId) -> bool {
        self.actors[&actor].squished
    }

    pub fn actor_pos(&self, actor: EntityId) -> Vec2 {
        self.actors[&actor].pos
    }

    pub fn solid_pos(&self, solid: EntityId) -> Vec2 {
        self.solids[&solid].pos
    }

    pub fn collide_check(&self, collider: EntityId, pos: Vec2) -> bool {
        let collider = &self.actors[&collider];

        self.collide_solids(pos, collider.width, collider.height)
    }

    pub fn overlaps_actor(&self, collider: EntityId, target: EntityId) -> bool {
        self.actors[&collider]
            .rect()
            .overlaps(&self.actors[&target].rect())
    }
}

pub fn render_physics(upscale: f32, world: UniqueView<Physics>) {
    // Draw Static Layer
    for layer in world.static_tiled_layers.iter() {
        for (i, &collider) in layer.static_colliders.iter().enumerate() {
            if collider {
                let x = (i % layer.width) as f32 * layer.tile_width;
                let y = (i / layer.width) as f32 * layer.tile_height;
                draw_rectangle_lines(
                    x * upscale,
                    y * upscale,
                    layer.tile_width * upscale,
                    layer.tile_height * upscale,
                    1.0 * upscale,
                    layer.debug_color,
                )
            }
        }
    }

    for (_, collider) in world.solids.iter() {
        draw_collider(collider, BLUE);
    }

    for (_, collider) in world.actors.iter() {
        draw_collider(collider, RED);
    }
}

pub fn draw_collider(collider: &Collider, color: Color) {
    draw_rectangle_lines(
        collider.pos.x,
        collider.pos.y,
        collider.width as f32,
        collider.height as f32,
        1.0,
        color,
    );
}
