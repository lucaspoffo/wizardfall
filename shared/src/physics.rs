use macroquad::math::{vec2, Rect, Vec2};
use shipyard::*;

use super::Transform;

pub struct CollisionShape {
    pub rect: Rect,
}

pub struct Velocity(pub Vec2);

pub fn sync_transform(transforms: View<Transform>, mut collision_shapes: ViewMut<CollisionShape>) {
    for (mut collision_shape, transform) in (&mut collision_shapes, &transforms).iter() {
        collision_shape.rect.x = transform.position.x;
        collision_shape.rect.y = transform.position.y;
    }
}

pub fn update_position(
    delta_time: f32,
    mut transforms: ViewMut<Transform>,
    velocities: View<Velocity>,
) {
    for (mut transform, velocity) in (&mut transforms, &velocities).iter() {
        transform.position.x += velocity.0.x * delta_time;
        transform.position.y += velocity.0.y * delta_time;
    }
}

pub fn calculate_collisions(
    delta_time: f32,
    collisions: View<CollisionShape>,
    mut velocities: ViewMut<Velocity>,
) {
    for (entity_id, (collision_shape, mut velocity)) in
        (&collisions, &mut velocities).iter().with_id()
    {
        for (inner_entity_id, inner_collision_shape) in (&collisions).iter().with_id() {
            if entity_id == inner_entity_id {
                continue;
            }
            let collision = rect_rect_collision(
                &collision_shape.rect,
                &inner_collision_shape.rect,
                velocity.0,
                delta_time,
            );
            if let Some(HitCollision {
                contact_normal,
                contact_time,
                ..
            }) = collision
            {
                let velocity_resolution = contact_normal
                    * vec2(f32::abs(velocity.0.x), f32::abs(velocity.0.y))
                    * (1.0 - contact_time);
                velocity.0 += velocity_resolution;
            }
        }
    }
}

pub struct HitCollision {
    pub contact_point: Vec2,
    pub contact_normal: Vec2,
    pub contact_time: f32,
}

pub fn ray_rect_collision(origin: Vec2, direction: Vec2, rect: Rect) -> Option<HitCollision> {
    let rect_pos = vec2(rect.x, rect.y);
    let rect_size = vec2(rect.w, rect.h);
    let mut t_near = (rect_pos - origin) / direction;
    let mut t_far = (rect_pos + rect_size - origin) / direction;

    if t_near.is_nan().any() || t_far.is_nan().any() {
        return None;
    }

    if t_near.x > t_far.x {
        std::mem::swap(&mut t_near.x, &mut t_far.x);
    }
    if t_near.y > t_far.y {
        std::mem::swap(&mut t_near.y, &mut t_far.y);
    }

    if t_near.x > t_far.y || t_near.y > t_far.x {
        return None;
    }

    let t_hit_near = f32::max(t_near.x, t_near.y);
    let t_hit_far = f32::min(t_far.x, t_far.y);

    if t_hit_far < 0.0 || t_hit_near > 1.0 || t_hit_near < 0.0 {
        return None;
    }

    let contact_point = origin + direction * t_hit_near;

    let contact_normal = if t_near.x > t_near.y {
        if direction.x < 0.0 {
            vec2(1.0, 0.0)
        } else {
            vec2(-1.0, 0.0)
        }
    } else if t_near.x < t_near.y {
        if direction.y < 0.0 {
            vec2(0.0, 1.0)
        } else {
            vec2(0.0, -1.0)
        }
    } else {
        Vec2::zero()
    };

    Some(HitCollision {
        contact_point,
        contact_normal,
        contact_time: t_hit_near,
    })
}

pub fn rect_rect_collision(
    source: &Rect,
    target: &Rect,
    vel: Vec2,
    delta_time: f32,
) -> Option<HitCollision> {
    if vel.x == 0.0 && vel.y == 0.0 {
        return None;
    }

    let expanded_origin = vec2(target.x - source.w / 2.0, target.y - source.h / 2.0);
    let expanded_size = vec2(target.w + source.w, target.h + source.h);
    let expanded_target = Rect::new(
        expanded_origin.x,
        expanded_origin.y,
        expanded_size.x,
        expanded_size.y,
    );

    let ray_origin = vec2(source.x + source.w / 2.0, source.y + source.w / 2.0);

    return ray_rect_collision(ray_origin, vel * delta_time, expanded_target);
}

pub fn load_level_collisions(world: &mut World) {
    for i in 0..32 {
        let transform = Transform {
            position: vec2(100.0 + i as f32 * 32.0, 100.0),
            rotation: 0.0,
        };
        let collision_shape = CollisionShape {
            rect: Rect::new(100.0 + i as f32 * 32.0, 100.0, 32.0, 32.0),
        };
        world.add_entity((collision_shape, transform));
    }
}
