# Prototype Voxel Template

`prototype_voxel_template` is a minimal Bevy voxel game starter built for fast prototyping. It combines modern Bevy, `bevy_voxel_world`, `noiz`, `block-mesh-bgm`, `bevy_ahoy`, and Avian so you can start testing gameplay ideas without rebuilding the usual terrain, movement, and chunk setup every time.

## What It Includes

- Rolling voxel terrain generated with `bevy_voxel_world` and `noiz`
- Binary greedy chunk meshing with `block-mesh-bgm`
- Ambient occlusion-ready voxel shading
- First-person player controller using `bevy_ahoy`
- Chunk collider generation with caching for solid terrain
- `bevy-inspector-egui` on an `Esc` toggle
- `bevy_asset_loader` and `bevy_common_assets` wired in from the start
- Hot-reloadable prototype config and voxel texture

## Run

```bash
cargo run
```

## Controls

- `WASD`: move
- `Space`: jump
- `Left Ctrl`: crouch
- Mouse: look
- Left click: remove voxel
- Right click: place voxel
- `Esc`: toggle inspector and release/capture cursor

## Config

Prototype settings live in [assets/config/default.prototype.ron](assets/config/default.prototype.ron).

Current runtime config includes:

- `world_seed`
- `terrain_period`
- `player_spawn_height_offset`
- `voxel_texture_layers`
- `ambient_occlusion`

The config file and `assets/example_voxel_texture.png` are set up for hot reload during development.

## Why This Exists

Most Bevy voxel prototypes need the same early plumbing: terrain generation, player movement, chunk meshing, collision, asset loading, and a quick inspector. This template keeps those pieces in place while staying small enough to fork and reshape quickly.

## Stack

- `bevy`
- `bevy_voxel_world`
- `block-mesh-bgm`
- `noiz`
- `bevy_ahoy`
- `avian3d`

## License

Licensed under `CC0-1.0 OR MIT OR Apache-2.0`. See [LICENSE-CC0-1.0.txt](LICENSE-CC0-1.0.txt), [LICENSE-MIT.txt](LICENSE-MIT.txt), and [LICENSE-Apache-2.0.txt](LICENSE-Apache-2.0.txt).
