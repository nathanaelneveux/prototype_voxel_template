use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::AssetEvent;
use bevy::prelude::*;
use bevy_voxel_world::custom_meshing::CHUNK_SIZE_I;
use bevy_voxel_world::prelude::*;

use crate::assets::{PrototypeConfig, TemplateAssets};
use crate::terrain_noise::{BEDROCK_FLOOR_Y, MAX_SURFACE_Y, TerrainColumn, TerrainNoise};

pub const MATERIAL_GRASS: u8 = 0;
pub const MATERIAL_DIRT: u8 = 1;
pub const MATERIAL_STONE: u8 = 2;
pub const MATERIAL_SAND: u8 = 3;

const WATERLINE_Y: i32 = 2;
const SPAWNING_DISTANCE: u32 = 10;
const MIN_DESPAWN_DISTANCE: u32 = 2;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(VoxelWorldPlugin::with_config(PrototypeWorld::default()))
            .add_systems(Update, apply_live_prototype_config);
    }
}

#[derive(Resource, Clone)]
pub struct PrototypeWorld {
    config: PrototypeConfig,
    noise: Arc<TerrainNoise>,
}

impl Default for PrototypeWorld {
    fn default() -> Self {
        Self::from_config(PrototypeConfig::default())
    }
}

impl PrototypeWorld {
    pub fn from_config(config: PrototypeConfig) -> Self {
        Self {
            noise: Arc::new(TerrainNoise::from_config(&config)),
            config,
        }
    }

    pub fn player_spawn_position(&self) -> Vec3 {
        Vec3::new(
            0.0,
            self.noise.player_spawn_height(0, 0) + self.config.player_spawn_height_offset,
            0.0,
        )
    }

    pub fn voxel_texture_layers(&self) -> u32 {
        self.config.voxel_texture_layers
    }

    fn matches_config(&self, config: &PrototypeConfig) -> bool {
        &self.config == config
    }
}

impl VoxelWorldConfig for PrototypeWorld {
    type MaterialIndex = u8;
    type ChunkUserBundle = ();

    fn spawning_distance(&self) -> u32 {
        SPAWNING_DISTANCE
    }

    fn min_despawn_distance(&self) -> u32 {
        MIN_DESPAWN_DISTANCE
    }

    fn attach_chunks_to_root(&self) -> bool {
        false
    }

    fn voxel_texture(&self) -> Option<(String, u32)> {
        Some((
            "example_voxel_texture.png".into(),
            self.config.voxel_texture_layers,
        ))
    }

    fn texture_index_mapper(&self) -> Arc<dyn Fn(Self::MaterialIndex) -> [u32; 3] + Send + Sync> {
        Arc::new(|material| match material {
            MATERIAL_GRASS => [3, 3, 3],
            MATERIAL_DIRT => [2, 2, 2],
            MATERIAL_STONE => [1, 1, 1],
            MATERIAL_SAND => [0, 0, 0],
            _ => [1, 1, 1],
        })
    }

    fn voxel_lookup_delegate(&self) -> VoxelLookupDelegate<Self::MaterialIndex> {
        let terrain_noise = Arc::clone(&self.noise);

        Box::new(move |chunk_pos, _lod, _previous| {
            let chunk_min_y = chunk_pos.y * CHUNK_SIZE_I;
            let chunk_max_y = chunk_min_y + CHUNK_SIZE_I - 1;

            if chunk_max_y < BEDROCK_FLOOR_Y {
                return Box::new(|_, _| WorldVoxel::Solid(MATERIAL_STONE));
            }

            if chunk_min_y > MAX_SURFACE_Y + 1 {
                return Box::new(|_, _| WorldVoxel::Air);
            }

            let mut column_cache = HashMap::<(i32, i32), TerrainColumn>::new();
            let noise = Arc::clone(&terrain_noise);

            Box::new(move |pos: IVec3, _previous| {
                if pos.y <= BEDROCK_FLOOR_Y {
                    return WorldVoxel::Solid(MATERIAL_STONE);
                }

                let column = *column_cache
                    .entry((pos.x, pos.z))
                    .or_insert_with(|| noise.sample_column(pos.x, pos.z));

                if pos.y > column.surface_y {
                    return WorldVoxel::Air;
                }

                let depth = column.surface_y - pos.y;
                let material = if depth == 0 {
                    if column.surface_y <= WATERLINE_Y {
                        MATERIAL_SAND
                    } else {
                        MATERIAL_GRASS
                    }
                } else if depth <= column.soil_depth {
                    if column.surface_y <= WATERLINE_Y {
                        MATERIAL_SAND
                    } else {
                        MATERIAL_DIRT
                    }
                } else {
                    MATERIAL_STONE
                };

                WorldVoxel::Solid(material)
            })
        })
    }
}

fn apply_live_prototype_config(
    template_assets: Option<Res<TemplateAssets>>,
    prototype_configs: Res<Assets<PrototypeConfig>>,
    mut prototype_config_events: MessageReader<AssetEvent<PrototypeConfig>>,
    mut prototype_world: ResMut<PrototypeWorld>,
    mut commands: Commands,
    chunks: Query<Entity, (With<Chunk<PrototypeWorld>>, Without<NeedsDespawn>)>,
) {
    let Some(template_assets) = template_assets else {
        return;
    };

    let config_id = template_assets.prototype_config.id();
    let mut should_apply = false;

    for event in prototype_config_events.read() {
        if event.is_loaded_with_dependencies(config_id) || event.is_modified(config_id) {
            should_apply = true;
        }
    }

    if !should_apply {
        return;
    }

    let Some(config) = prototype_configs.get(&template_assets.prototype_config) else {
        return;
    };

    if prototype_world.matches_config(config) {
        return;
    }

    *prototype_world = PrototypeWorld::from_config(config.clone());

    for entity in &chunks {
        commands.entity(entity).try_insert(NeedsDespawn);
    }

    info!(
        "applied prototype config: seed={}, terrain_period={}, spawn_height_offset={}, texture_layers={}",
        config.world_seed,
        config.terrain_period,
        config.player_spawn_height_offset,
        config.voxel_texture_layers
    );
}
