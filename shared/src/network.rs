use serde::{de::DeserializeOwned, Deserialize, Serialize};
use shipyard::{
    AllStoragesViewMut, EntitiesView, EntitiesViewMut, EntityId, Get, IntoIter, IntoWithId,
    UniqueViewMut, View, ViewMut, World,
};

use crate::{
    Transform, EntityMapping,
    player::Player,
    projectile::Projectile,
    animation::AnimationController
};

pub trait NetworkState {
    type State: Clone + std::fmt::Debug + Serialize + DeserializeOwned;

    fn from_state(state: Self::State) -> Self;
    fn update_from_state(&mut self, state: Self::State);
    fn state(&self) -> Self::State;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerFrame {
    entities: Vec<EntityId>,
    players: NetworkComponent<Player>,
    projectiles: NetworkComponent<Projectile>,
    transforms: NetworkComponent<Transform>,
    animations: NetworkComponent<AnimationController>,
}

impl ServerFrame {
    pub fn from_world(world: &World) -> Self {
        let entities: Vec<EntityId> = world
            .run(|entities: EntitiesView| entities.iter().collect())
            .unwrap();

        Self {
            players: NetworkComponent::<Player>::from_world(&entities, world),
            projectiles: NetworkComponent::<Projectile>::from_world(&entities, world),
            transforms: NetworkComponent::<Transform>::from_world(&entities, world),
            animations: NetworkComponent::<AnimationController>::from_world(&entities, world),
            entities,
        }
    }

    pub fn apply_in_world(&self, world: &World) {
        self.players.apply_in_world(&self.entities, world);
        self.projectiles.apply_in_world(&self.entities, world);
        self.transforms.apply_in_world(&self.entities, world);
        self.animations.apply_in_world(&self.entities, world);

        // Remove entities that are not in the network frame
        world
            .run(|mut all_storages: AllStoragesViewMut| {
                let removed_entities: Vec<EntityId> = {
                    let mut mapping = all_storages
                        .borrow::<UniqueViewMut<EntityMapping>>()
                        .unwrap();
                    let mut removed_entities: Vec<EntityId> = vec![];
                    for (server_id, client_id) in mapping.clone().iter() {
                        if !self.entities.contains(server_id) {
                            removed_entities.push(*client_id);
                            mapping.remove(server_id);
                        }
                    }

                    removed_entities
                };

                for id in removed_entities.iter() {
                    all_storages.delete_entity(*id);
                }
            })
            .unwrap();
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct NetworkComponent<T: NetworkState> {
    bitmask: Vec<bool>,
    values: Vec<T::State>,
}

impl<T: 'static + Sync + Send + Clone + NetworkState> NetworkComponent<T> {
    fn from_world(entities_id: &[EntityId], world: &World) -> NetworkComponent<T> {
        let mut bitmask: Vec<bool> = vec![false; entities_id.len()];
        let mut values: Vec<Option<T::State>> = vec![None; entities_id.len()];
        world
            .run(|components: View<T>| {
                for (entity_id, component) in components.iter().with_id() {
                    let id_pos = entities_id
                        .iter()
                        .position(|&x| x == entity_id)
                        .expect("Network component EntityID not found.");

                    bitmask[id_pos] = true;
                    values[id_pos] = Some(component.state());
                }
            })
            .unwrap();

        let values = values.iter_mut().filter_map(|v| v.take()).collect();

        NetworkComponent { bitmask, values }
    }

    fn apply_in_world(&self, entities_id: &[EntityId], world: &World) {
        let entities_state = entities_id
            .iter()
            .zip(self.bitmask.iter())
            .filter_map(|(id, &presence)| if presence { Some(id) } else { None })
            .zip(self.values.clone().into_iter());

        // TODO: instead of filter map we could remove component when is None
        world
            .run(
                |mut entities: EntitiesViewMut,
                 mut components: ViewMut<T>,
                 mut mapping: UniqueViewMut<EntityMapping>| {
                    for (entity_id, state) in entities_state {
                        if let Some(mapped_id) = mapping.get(entity_id) {
                            if let Ok(mut component) = (&mut components).get(*mapped_id) {
                                component.update_from_state(state);
                            } else {
                                let component = T::from_state(state);
                                entities.add_component(*mapped_id, &mut components, component);
                            }
                        } else {
                            let component = T::from_state(state);
                            let client_entity_id = entities.add_entity(&mut components, component);
                            mapping.insert(*entity_id, client_entity_id);
                        }
                    }
                },
            )
            .unwrap();
    }
}
