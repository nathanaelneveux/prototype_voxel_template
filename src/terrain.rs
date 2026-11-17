use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::AssetEvent;
use bevy::prelude::*;
use bevy_voxel_world::custom_meshing::{CHUNK_SIZE_F, scale_texture_coordinates};
use bevy_voxel_world::prelude::*;

use crate::assets::{PrototypeConfig, TemplateAssets};
use crate::terrain_meshing::build_chunk_mesh;
use crate::terrain_noise::{BEDROCK_FLOOR_Y, MAX_SURFACE_Y, TerrainColumn, TerrainNoise};

pub type MaterialIndex = u8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TerrainMaterial {
    Grass = 0,
    Brick = 1,
    SnowBrick = 2,
    Snow = 3,
    Emitter = 4,
}

impl TerrainMaterial {
    pub const fn index(self) -> MaterialIndex {
        self as MaterialIndex
    }

    pub fn voxel(self) -> WorldVoxel<MaterialIndex> {
        WorldVoxel::Solid(self.index())
    }

    fn from_index(index: MaterialIndex) -> Option<Self> {
        match index {
            0 => Some(Self::Grass),
            1 => Some(Self::Brick),
            2 => Some(Self::SnowBrick),
            3 => Some(Self::Snow),
            4 => Some(Self::Emitter),
            _ => None,
        }
    }

    fn texture_indices(index: MaterialIndex) -> [u32; 3] {
        const SNOW_TEXTURE: u32 = 0;
        const SNOW_BRICK_TEXTURE: u32 = 1;
        const BRICK_TEXTURE: u32 = 2;
        const GRASS_TEXTURE: u32 = 3;

        match Self::from_index(index).unwrap_or(Self::Brick) {
            Self::Grass => [GRASS_TEXTURE, GRASS_TEXTURE, GRASS_TEXTURE],
            Self::Brick => [BRICK_TEXTURE, BRICK_TEXTURE, BRICK_TEXTURE],
            Self::SnowBrick => [SNOW_TEXTURE, SNOW_BRICK_TEXTURE, BRICK_TEXTURE],
            Self::Snow | Self::Emitter => [SNOW_TEXTURE, SNOW_TEXTURE, SNOW_TEXTURE],
        }
    }
}

const WATERLINE_Y: i32 = 2;
pub const SPAWNING_DISTANCE: u32 = 10;
pub const TERRAIN_VIEW_DISTANCE: f32 = (SPAWNING_DISTANCE - 1) as f32 * CHUNK_SIZE_F;
const MIN_DESPAWN_DISTANCE: u32 = 2;

pub struct TerrainPlugin;

#[derive(Event)]
pub(crate) struct TerrainRegenerationRequested;

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
}

impl VoxelWorldConfig for PrototypeWorld {
    type MaterialIndex = MaterialIndex;
    type ChunkUserBundle = ();

    fn spawning_distance(&self) -> u32 {
        SPAWNING_DISTANCE
    }

    fn min_despawn_distance(&self) -> u32 {
        MIN_DESPAWN_DISTANCE
    }

    fn light_propagation(&self) -> Option<LightPropagation> {
        Some(LightPropagation {
            look: LightLook {
                ao: if self.config.ambient_occlusion {
                    LightAo::IndirectLight
                } else {
                    LightAo::Off
                },
                ..default()
            },
            ..default()
        })
    }

    fn emissive_materials(&self) -> Vec<(MaterialIndex, Emission)> {
        vec![(TerrainMaterial::Emitter.index(), Emission::level(15.0))]
    }

    fn voxel_texture(&self) -> Option<(String, u32)> {
        Some((
            "example_voxel_texture.png".into(),
            self.config.voxel_texture_layers,
        ))
    }

    fn texture_index_mapper(&self) -> Arc<dyn Fn(Self::MaterialIndex) -> [u32; 3] + Send + Sync> {
        Arc::new(TerrainMaterial::texture_indices)
    }

    fn voxel_lookup_delegate(&self) -> VoxelLookupDelegate<Self::MaterialIndex> {
        let terrain_noise = Arc::clone(&self.noise);

        Box::new(move |_, _, _| {
            let mut column_cache = HashMap::<(i32, i32), TerrainColumn>::new();
            let noise = Arc::clone(&terrain_noise);

            Box::new(move |pos: IVec3, _previous| {
                if pos.y <= BEDROCK_FLOOR_Y {
                    return TerrainMaterial::Brick.voxel();
                }
                if pos.y > MAX_SURFACE_Y {
                    return WorldVoxel::Air;
                }

                let column = *column_cache
                    .entry((pos.x, pos.z))
                    .or_insert_with(|| noise.sample_column(pos.x, pos.z));

                if pos.y > column.surface_y {
                    return WorldVoxel::Air;
                }

                let depth = column.surface_y - pos.y;
                let material = if depth <= column.soil_depth && column.surface_y <= WATERLINE_Y {
                    TerrainMaterial::Snow
                } else if depth == 0 {
                    TerrainMaterial::Grass
                } else if depth <= column.soil_depth {
                    TerrainMaterial::SnowBrick
                } else {
                    TerrainMaterial::Brick
                };

                material.voxel()
            })
        })
    }

    fn chunk_meshing_delegate(
        &self,
    ) -> ChunkMeshingDelegate<Self::MaterialIndex, Self::ChunkUserBundle> {
        Some(Box::new(|_, lod, _, _, _| {
            Box::new(
                move |voxels, data_shape, mesh_shape, texture_index_mapper| {
                    let mut mesh =
                        build_chunk_mesh(voxels, data_shape, mesh_shape, texture_index_mapper);
                    // BVW scales the positions; keep texture density in world units too.
                    scale_texture_coordinates(&mut mesh, (1 << lod) as f32);
                    (mesh, None)
                },
            )
        }))
    }
}

fn apply_live_prototype_config(
    mut commands: Commands,
    template_assets: Option<Res<TemplateAssets>>,
    prototype_configs: Res<Assets<PrototypeConfig>>,
    mut prototype_config_events: MessageReader<AssetEvent<PrototypeConfig>>,
    mut prototype_world: ResMut<PrototypeWorld>,
    mut voxel_world: VoxelWorld<PrototypeWorld>,
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

    let previous = &prototype_world.config;
    if previous == config {
        return;
    }

    let ao_only = *config
        == (PrototypeConfig {
            ambient_occlusion: config.ambient_occlusion,
            ..previous.clone()
        });
    if ao_only {
        // BVW watches the world config and updates the light look without remeshing.
        prototype_world.config.ambient_occlusion = config.ambient_occlusion;
    } else {
        *prototype_world = PrototypeWorld::from_config(config.clone());
        voxel_world.regenerate_chunks();
        commands.trigger(TerrainRegenerationRequested);
    }

    info!(
        world_seed = config.world_seed,
        terrain_period = config.terrain_period,
        player_spawn_height_offset = config.player_spawn_height_offset,
        voxel_texture_layers = config.voxel_texture_layers,
        ambient_occlusion = config.ambient_occlusion,
        "applied prototype config"
    );
}
