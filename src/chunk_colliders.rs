use std::collections::{HashMap, VecDeque};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_voxel_world::prelude::{ChunkPublished, VoxelWorld, VoxelWorldSet, WorldVoxel};

use crate::terrain::PrototypeWorld;

const CHUNK_COLLIDER_VOXEL_SIZE: f32 = 1.0;
const CHUNK_COLLIDER_CACHE_MAX_ENTRIES: usize = 512;
const MIN_PADDED_CHUNK_DIMENSION: u32 = 3;

type ColliderCacheKey = u64;

pub struct ChunkColliderPlugin;

impl Plugin for ChunkColliderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChunkColliderCache>().add_systems(
            PreUpdate,
            update_chunk_colliders.after(VoxelWorldSet::<PrototypeWorld>::default()),
        );
    }
}

#[derive(Resource, Default)]
struct ChunkColliderCache {
    colliders: HashMap<ColliderCacheKey, Collider>,
    access_order: VecDeque<ColliderCacheKey>,
}

impl ChunkColliderCache {
    fn get(&mut self, key: ColliderCacheKey) -> Option<Collider> {
        let collider = self.colliders.get(&key).cloned();
        if collider.is_some() {
            self.touch(key);
        }
        collider
    }

    fn insert(&mut self, key: ColliderCacheKey, collider: Collider) {
        self.insert_with_capacity(key, collider, CHUNK_COLLIDER_CACHE_MAX_ENTRIES);
    }

    fn insert_with_capacity(&mut self, key: ColliderCacheKey, collider: Collider, capacity: usize) {
        if capacity == 0 {
            return;
        }

        let is_new = !self.colliders.contains_key(&key);
        self.colliders.insert(key, collider);
        self.touch(key);

        if is_new
            && self.colliders.len() > capacity
            && let Some(oldest) = self.access_order.pop_front()
        {
            self.colliders.remove(&oldest);
        }
    }

    fn touch(&mut self, key: ColliderCacheKey) {
        if let Some(index) = self
            .access_order
            .iter()
            .position(|existing| *existing == key)
        {
            self.access_order.remove(index);
        }

        self.access_order.push_back(key);
    }
}

fn update_chunk_colliders(
    mut commands: Commands,
    mut chunk_collider_cache: ResMut<ChunkColliderCache>,
    voxel_world: VoxelWorld<PrototypeWorld>,
    mut published: MessageReader<ChunkPublished<PrototypeWorld>>,
) {
    for event in published.read() {
        if event.chunk_key.lod_level != 0 {
            continue;
        }
        let Ok(mut entity) = commands.get_entity(event.entity) else {
            continue;
        };
        let Some(chunk_data) = voxel_world.get_chunk_data(event.chunk_key) else {
            continue;
        };

        if chunk_data.is_empty() {
            entity.remove::<(RigidBody, Collider)>();
            continue;
        }

        let voxels_hash = chunk_data.voxels_hash();
        let collider = if let Some(cached) = chunk_collider_cache.get(voxels_hash) {
            cached
        } else {
            let shape = chunk_data.data_shape();
            let [sx, sy, sz] = shape.to_array();
            if sx < MIN_PADDED_CHUNK_DIMENSION
                || sy < MIN_PADDED_CHUNK_DIMENSION
                || sz < MIN_PADDED_CHUNK_DIMENSION
            {
                continue;
            }

            let collider = if chunk_data.is_full() {
                let size = (shape - UVec3::splat(2)).as_vec3() * CHUNK_COLLIDER_VOXEL_SIZE;
                let center = size * 0.5 + Vec3::splat(CHUNK_COLLIDER_VOXEL_SIZE);
                Collider::compound(vec![(
                    center,
                    Quat::IDENTITY,
                    Collider::cuboid(size.x, size.y, size.z),
                )])
            } else {
                let Some(voxels) = chunk_data.voxels_arc() else {
                    continue;
                };
                let coordinates = collect_solid_voxel_coordinates(voxels.as_ref(), sx, sy, sz);
                if coordinates.is_empty() {
                    entity.remove::<(RigidBody, Collider)>();
                    continue;
                }

                Collider::voxels(Vec3::splat(CHUNK_COLLIDER_VOXEL_SIZE), &coordinates)
            };
            chunk_collider_cache.insert(voxels_hash, collider.clone());
            collider
        };

        entity.insert((RigidBody::Static, collider));
    }
}

fn collect_solid_voxel_coordinates<I: Copy + PartialEq>(
    voxels: &[WorldVoxel<I>],
    sx: u32,
    sy: u32,
    sz: u32,
) -> Vec<IVec3> {
    let mut coords = Vec::with_capacity(interior_voxel_count(sx, sy, sz).min(voxels.len()));
    let yz_stride = sx * sy;

    for z in 1..sz.saturating_sub(1) {
        for y in 1..sy.saturating_sub(1) {
            for x in 1..sx.saturating_sub(1) {
                let index = (x + sx * y + yz_stride * z) as usize;
                if matches!(voxels[index], WorldVoxel::Solid(_)) {
                    coords.push(IVec3::new(x as i32, y as i32, z as i32));
                }
            }
        }
    }

    coords
}

fn interior_voxel_count(sx: u32, sy: u32, sz: u32) -> usize {
    (sx.saturating_sub(2) * sy.saturating_sub(2) * sz.saturating_sub(2)) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_evicts_oldest_when_capacity_is_reached() {
        let mut cache = ChunkColliderCache::default();
        let collider = Collider::cuboid(1.0, 1.0, 1.0);

        cache.insert_with_capacity(1, collider.clone(), 2);
        cache.insert_with_capacity(2, collider.clone(), 2);
        cache.insert_with_capacity(3, collider, 2);

        assert!(cache.get(1).is_none());
        assert!(cache.get(2).is_some());
        assert!(cache.get(3).is_some());
    }

    #[test]
    fn cache_updates_duplicate_key_and_preserves_single_entry() {
        let mut cache = ChunkColliderCache::default();
        let collider_a = Collider::cuboid(1.0, 1.0, 1.0);
        let collider_b = Collider::cuboid(2.0, 2.0, 2.0);

        cache.insert_with_capacity(1, collider_a, 2);
        cache.insert_with_capacity(1, collider_b, 2);

        assert_eq!(cache.colliders.len(), 1);
        assert_eq!(cache.access_order.len(), 1);
        assert!(cache.get(1).is_some());
    }
}
