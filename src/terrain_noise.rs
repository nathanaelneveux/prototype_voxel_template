use bevy::math::Vec2;
use noiz::prelude::{Noise, SampleableFor, ScalableNoise, SeedableNoise, common_noise};

use crate::assets::PrototypeConfig;

pub const BEDROCK_FLOOR_Y: i32 = -24;
pub const SURFACE_BASE_HEIGHT: i32 = 10;
const LARGE_HILL_AMPLITUDE: f32 = 18.0;
const DETAIL_HILL_AMPLITUDE: f32 = 5.0;
const SOIL_DEPTH_BASE: i32 = 3;
const SOIL_DEPTH_VARIATION: i32 = 2;

pub const MAX_SURFACE_Y: i32 =
    SURFACE_BASE_HEIGHT + LARGE_HILL_AMPLITUDE as i32 + DETAIL_HILL_AMPLITUDE as i32;

#[derive(Clone, Copy)]
pub struct TerrainColumn {
    pub surface_y: i32,
    pub soil_depth: i32,
}

#[derive(Clone)]
pub struct TerrainNoise {
    broad_hills: Noise<common_noise::Perlin>,
    detail_hills: Noise<common_noise::Perlin>,
    soil_depth: Noise<common_noise::Perlin>,
}

impl TerrainNoise {
    pub fn from_config(config: &PrototypeConfig) -> Self {
        let terrain_period = config.terrain_period.max(16.0);

        let mut broad_hills = Noise::<common_noise::Perlin>::default();
        broad_hills.set_seed(config.world_seed);
        broad_hills.set_period(terrain_period);

        let mut detail_hills = Noise::<common_noise::Perlin>::default();
        detail_hills.set_seed(config.world_seed ^ 0x9E37_79B9);
        detail_hills.set_period((terrain_period * 0.28125).max(24.0));

        let mut soil_depth = Noise::<common_noise::Perlin>::default();
        soil_depth.set_seed(config.world_seed.rotate_left(13) ^ 0x85EB_CA6B);
        soil_depth.set_period((terrain_period * 0.7).max(48.0));

        Self {
            broad_hills,
            detail_hills,
            soil_depth,
        }
    }

    pub fn sample_column(&self, x: i32, z: i32) -> TerrainColumn {
        let pos = Vec2::new(x as f32, z as f32);
        let broad: f32 = self.broad_hills.sample(pos);
        let detail: f32 = self.detail_hills.sample(pos);
        let soil_variation: f32 = self.soil_depth.sample(pos);

        let surface_y = SURFACE_BASE_HEIGHT
            + (broad * LARGE_HILL_AMPLITUDE).round() as i32
            + (detail * DETAIL_HILL_AMPLITUDE).round() as i32;
        let soil_depth = (SOIL_DEPTH_BASE
            + (soil_variation * SOIL_DEPTH_VARIATION as f32).round() as i32)
            .clamp(2, 6);

        TerrainColumn {
            surface_y,
            soil_depth,
        }
    }

    pub fn player_spawn_height(&self, x: i32, z: i32) -> f32 {
        self.sample_column(x, z).surface_y as f32
    }
}
