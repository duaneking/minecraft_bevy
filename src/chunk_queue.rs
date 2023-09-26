use crate::{chunk::*, Block, BlockMaterial, BlockRegistry, Perlin, GEN_SEED};
use bevy::utils::hashbrown::{hash_map::Iter, HashMap};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
};
use bevy_meshem::prelude::*;
use noise::NoiseFn;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Component)]
pub struct ComputeChunk(pub Task<Option<((Mesh, MeshMD<Block>), [Block; CHUNK_LEN], [i32; 2])>>);

#[derive(Resource, Default)]
pub struct ChunkQueue {
    // true = spawn, false= despawn
    queue: Vec<([i32; 2], bool)>,
    pub panic_when_cant_find_chunk: bool,
}

#[derive(Resource, Default)]
pub struct ChunkMap {
    pos_to_ent: HashMap<[i32; 2], Entity>,
}

impl ChunkMap {
    pub fn get_ent(&self, cords: [i32; 2]) -> Option<Entity> {
        Some(*self.pos_to_ent.get(&cords)?)
    }

    pub fn insert_ent(&mut self, cords: [i32; 2], ent: Entity) {
        self.pos_to_ent.insert(cords, ent);
    }

    pub fn remove_ent(&mut self, cords: [i32; 2], ent: Entity) {
        assert_eq!(self.pos_to_ent.remove(&cords).unwrap(), ent);
    }

    pub fn exists(&self, cords: [i32; 2]) -> bool {
        self.pos_to_ent.contains_key(&cords)
    }

    pub fn iter_keys(&self) -> bevy::utils::hashbrown::hash_map::Keys<'_, [i32; 2], Entity> {
        self.pos_to_ent.keys()
    }

    pub fn iter(&self) -> bevy::utils::hashbrown::hash_map::Iter<'_, [i32; 2], Entity> {
        self.pos_to_ent.iter()
    }

    pub fn change_ent(&mut self, cords: [i32; 2], ent: Entity) {
        *(self
            .pos_to_ent
            .get_mut(&cords)
            .expect("Couldn't find chunk entity")) = ent;
    }
}

impl ChunkQueue {
    pub fn queue_spawn(&mut self, pos: [i32; 2]) {
        self.queue.push((pos, true));
    }

    pub fn queue_despawn(&mut self, pos: [i32; 2]) {
        self.queue.push((pos, false));
    }

    pub fn dequeue_all(
        &mut self,
        mut commands: Commands,
        breg: Arc<BlockRegistry>,
        chunk_map: &mut ChunkMap,
    ) {
        if self.queue.is_empty() {
            return;
        }
        let noise = Perlin::new(GEN_SEED);
        let thread_pool = AsyncComputeTaskPool::get();
        for chunk in self.queue.as_slice() {
            if !chunk.1 {
                let ent;
                if let Some(e) = chunk_map.get_ent(chunk.0) {
                    if e != Entity::PLACEHOLDER {
                        ent = commands.entity(e).id();
                    } else {
                        continue;
                    }
                } else {
                    if self.panic_when_cant_find_chunk {
                        panic!("Can't despawn chunk, because it was not found in internal data.");
                    } else {
                        continue;
                    }
                }
                chunk_map.remove_ent(chunk.0, ent);
                commands.entity(ent).despawn();
                continue;
            }

            let task;

            if chunk_map.exists(chunk.0) {
                if self.panic_when_cant_find_chunk {
                    panic!("Can't spawn chunk because it is already in the world.");
                } else {
                    continue;
                }
            } else {
                chunk_map.insert_ent(chunk.0, Entity::PLACEHOLDER);

                let breg = Arc::clone(&breg);
                let cords = chunk.0;
                task = thread_pool.spawn(async move {
                    let grid = generate_chunk(cords, &noise);
                    let t =
                        mesh_grid(CHUNK_DIMS, grid.to_vec(), &*breg, MeshingAlgorithm::Culling)?;
                    // t.0.compute_flat_normals();
                    Some((t, grid, cords))
                });
            }

            commands.spawn(ComputeChunk(task));
        }
        self.queue.clear();
    }
}
