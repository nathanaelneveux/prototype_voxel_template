mod app_controls;
mod assets;
mod chunk_colliders;
mod player;
mod terrain;
mod terrain_meshing;
mod terrain_noise;
mod world_environment;

use app_controls::AppControlsPlugin;
use assets::AssetSupportPlugin;
use avian3d::prelude::*;
use bevy::asset::AssetMetaCheck;
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_ahoy::prelude::AhoyPlugins;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;

use chunk_colliders::ChunkColliderPlugin;
use player::PlayerPlugin;
use terrain::TerrainPlugin;
use world_environment::WorldEnvironmentPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            meta_check: AssetMetaCheck::Never,
            watch_for_changes_override: Some(true),
            ..default()
        }))
        .add_plugins((
            PhysicsPlugins::default(),
            FrameTimeDiagnosticsPlugin::default(),
            LogDiagnosticsPlugin::default(),
            EnhancedInputPlugin,
            AhoyPlugins::default(),
            AssetSupportPlugin,
            WorldEnvironmentPlugin,
            AppControlsPlugin,
            TerrainPlugin,
            ChunkColliderPlugin,
            PlayerPlugin,
        ))
        .run();
}
