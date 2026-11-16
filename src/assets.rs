use bevy::asset::AssetEvent;
use bevy::image::{ImageAddressMode, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::TextureViewDimension;
use bevy_asset_loader::prelude::{AssetCollection, AssetCollectionApp};
use bevy_common_assets::ron::RonAssetPlugin;

use crate::terrain::PrototypeWorld;

pub struct AssetSupportPlugin;

impl Plugin for AssetSupportPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RonAssetPlugin::<PrototypeConfig>::new(&["prototype.ron"]))
            .init_collection::<TemplateAssets>()
            .add_systems(
                Update,
                (
                    log_when_template_assets_are_ready,
                    refresh_voxel_texture_on_change,
                ),
            );
    }
}

#[derive(AssetCollection, Resource)]
pub struct TemplateAssets {
    #[asset(path = "example_voxel_texture.png")]
    pub voxel_texture: Handle<Image>,
    #[asset(path = "config/default.prototype.ron")]
    pub prototype_config: Handle<PrototypeConfig>,
}

#[derive(Asset, TypePath, Debug, Clone, PartialEq, serde::Deserialize)]
pub struct PrototypeConfig {
    pub world_seed: u32,
    pub terrain_period: f32,
    pub player_spawn_height_offset: f32,
    pub voxel_texture_layers: u32,
    pub ambient_occlusion: bool,
}

impl Default for PrototypeConfig {
    fn default() -> Self {
        Self {
            world_seed: 7,
            terrain_period: 256.0,
            player_spawn_height_offset: 8.0,
            voxel_texture_layers: 4,
            ambient_occlusion: true,
        }
    }
}

fn log_when_template_assets_are_ready(
    template_assets: Option<Res<TemplateAssets>>,
    prototype_configs: Res<Assets<PrototypeConfig>>,
    mut logged: Local<bool>,
) {
    if *logged {
        return;
    }

    let Some(template_assets) = template_assets else {
        return;
    };

    let _voxel_texture = template_assets.voxel_texture.id();
    let Some(config) = prototype_configs.get(&template_assets.prototype_config) else {
        return;
    };

    info!(
        world_seed = config.world_seed,
        terrain_period = config.terrain_period,
        player_spawn_height_offset = config.player_spawn_height_offset,
        voxel_texture_layers = config.voxel_texture_layers,
        ambient_occlusion = config.ambient_occlusion,
        "template asset support ready"
    );
    *logged = true;
}

fn refresh_voxel_texture_on_change(
    template_assets: Option<Res<TemplateAssets>>,
    world: Option<Res<PrototypeWorld>>,
    mut image_events: MessageReader<AssetEvent<Image>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(template_assets) = template_assets else {
        return;
    };
    let Some(world) = world else {
        return;
    };

    let texture_id = template_assets.voxel_texture.id();
    let mut should_refresh = world.is_changed();

    for event in image_events.read() {
        if event.is_loaded_with_dependencies(texture_id) || event.is_modified(texture_id) {
            should_refresh = true;
        }
    }

    if !should_refresh {
        return;
    }

    let Some(image) = images.get(&template_assets.voxel_texture) else {
        return;
    };
    if image.texture_descriptor.size.depth_or_array_layers == world.voxel_texture_layers()
        && image
            .texture_view_descriptor
            .as_ref()
            .and_then(|view| view.dimension)
            == Some(TextureViewDimension::D2Array)
        && matches!(&image.sampler, ImageSampler::Descriptor(sampler)
            if sampler.address_mode_u == ImageAddressMode::Repeat
                && sampler.address_mode_v == ImageAddressMode::Repeat
                && sampler.address_mode_w == ImageAddressMode::Repeat)
    {
        return;
    }

    let layers = world.voxel_texture_layers();
    let mut image = images.get_mut(&template_assets.voxel_texture).unwrap();
    if image.texture_descriptor.size.depth_or_array_layers != layers
        && let Err(error) = image.reinterpret_stacked_2d_as_array(layers)
    {
        warn!("Could not prepare voxel texture array: {error}");
        return;
    }
    let descriptor = image.sampler.get_or_init_descriptor();
    descriptor.address_mode_u = ImageAddressMode::Repeat;
    descriptor.address_mode_v = ImageAddressMode::Repeat;
    descriptor.address_mode_w = ImageAddressMode::Repeat;
    image
        .texture_view_descriptor
        .get_or_insert_default()
        .dimension = Some(TextureViewDimension::D2Array);
}
