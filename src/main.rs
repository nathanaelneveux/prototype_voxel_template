mod ambient_occlusion;
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
use bevy::render::{RenderPlugin, render_resource::WgpuFeatures, settings::WgpuSettings};
use bevy_ahoy::prelude::AhoyPlugins;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;

use chunk_colliders::ChunkColliderPlugin;
use player::PlayerPlugin;
use terrain::TerrainPlugin;
use world_environment::WorldEnvironmentPlugin;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    watch_for_changes_override: Some(true),
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: WgpuSettings {
                        features: WgpuFeatures::POLYGON_MODE_LINE,
                        ..default()
                    }
                    .into(),
                    ..default()
                }),
        )
        .add_plugins((
            PhysicsPlugins::default(),
            // Uncomment the next line for physics debugging
            //PhysicsDebugPlugin,
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
