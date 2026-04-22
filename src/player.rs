use avian3d::prelude::*;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use bevy_ahoy::prelude::*;
use bevy_enhanced_input::prelude::*;
use bevy_voxel_world::{
    custom_meshing::CHUNK_SIZE_I,
    prelude::{VoxelWorld, VoxelWorldCamera, WorldVoxel},
};

use crate::terrain::{MATERIAL_GRASS, PrototypeWorld};

const FOG_START: f32 = 96.0;
const FOG_END: f32 = 160.0;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_input_context::<PlayerInput>()
            .add_systems(Startup, spawn_player)
            .add_systems(Update, (edit_voxels, reset_player_to_world_spawn));
    }
}

#[derive(Component)]
struct Player;

#[derive(Component, Default)]
struct PlayerInput;

#[derive(Component)]
struct PlayerCamera;

fn spawn_player(mut commands: Commands, world: Res<PrototypeWorld>) {
    let player = commands
        .spawn((
            Name::new("Player"),
            Player,
            PlayerInput,
            CharacterController {
                speed: 11.0,
                max_speed: 28.0,
                air_speed: 1.35,
                jump_height: 1.75,
                step_size: 0.8,
                standing_view_height: 1.65,
                crouch_view_height: 1.05,
                ..default()
            },
            Collider::cylinder(0.45, 1.75),
            Transform::from_translation(world.player_spawn_position()),
            actions!(PlayerInput[
                (
                    Action::<Movement>::new(),
                    DeadZone::default(),
                    Bindings::spawn(Cardinal::wasd_keys()),
                ),
                (
                    Action::<Jump>::new(),
                    bindings![KeyCode::Space],
                ),
                (
                    Action::<Crouch>::new(),
                    bindings![KeyCode::ControlLeft],
                ),
                (
                    Action::<RotateCamera>::new(),
                    Bindings::spawn(Spawn((Binding::mouse_motion(), Scale::splat(0.08)))),
                ),
            ]),
        ))
        .id();

    commands.spawn((
        Name::new("PlayerCamera"),
        PlayerCamera,
        Camera3d::default(),
        DistanceFog {
            color: Color::srgb(0.53, 0.74, 0.94),
            falloff: FogFalloff::Linear {
                start: FOG_START,
                end: FOG_END,
            },
            ..default()
        },
        CharacterControllerCameraOf::new(player),
        VoxelWorldCamera::<PrototypeWorld>::default(),
    ));
}

fn reset_player_to_world_spawn(
    world: Res<PrototypeWorld>,
    mut player: Single<(&mut Transform, Option<&mut LinearVelocity>), With<Player>>,
) {
    if !world.is_changed() {
        return;
    }

    let (transform, velocity) = &mut *player;
    transform.translation = world.player_spawn_position();

    if let Some(velocity) = velocity.as_mut() {
        **velocity = LinearVelocity::ZERO;
    }
}

fn edit_voxels(
    buttons: Res<ButtonInput<MouseButton>>,
    cursor_options: Single<&CursorOptions, With<PrimaryWindow>>,
    camera: Single<&GlobalTransform, (With<PlayerCamera>, Without<Player>)>,
    mut voxel_world: VoxelWorld<PrototypeWorld>,
) {
    if cursor_options.visible
        || (!buttons.just_pressed(MouseButton::Left) && !buttons.just_pressed(MouseButton::Right))
    {
        return;
    }

    let ray = Ray3d::new(camera.translation(), camera.forward());
    let Some(hit) = voxel_world.raycast(ray, &|(_, voxel)| matches!(voxel, WorldVoxel::Solid(_)))
    else {
        return;
    };

    if buttons.just_pressed(MouseButton::Left) {
        let edited_position = hit.position.as_ivec3();
        let _ = set_voxel_state(&mut voxel_world, edited_position, WorldVoxel::Air);
    }

    if buttons.just_pressed(MouseButton::Right) {
        let Some(normal) = hit.normal else {
            return;
        };
        let placement_position = (hit.position + normal).as_ivec3();

        if matches!(
            voxel_world.get_voxel(placement_position),
            WorldVoxel::Air | WorldVoxel::Unset
        ) {
            let _ = set_voxel_state(
                &mut voxel_world,
                placement_position,
                WorldVoxel::Solid(MATERIAL_GRASS),
            );
        }
    }
}

fn set_voxel_state(
    world: &mut VoxelWorld<PrototypeWorld>,
    position: IVec3,
    desired: WorldVoxel<u8>,
) -> bool {
    if world.get_voxel(position) == desired {
        return false;
    }

    world.set_voxel(position, desired);
    mark_adjacent_chunks(world, position);
    true
}

fn mark_adjacent_chunks(world: &mut VoxelWorld<PrototypeWorld>, position: IVec3) {
    let chunk_size = IVec3::splat(CHUNK_SIZE_I);
    let local = position.rem_euclid(chunk_size);
    let last = CHUNK_SIZE_I - 1;

    if local.x == 0 {
        mark_chunk(world, position - IVec3::X);
    }
    if local.x == last {
        mark_chunk(world, position + IVec3::X);
    }
    if local.y == 0 {
        mark_chunk(world, position - IVec3::Y);
    }
    if local.y == last {
        mark_chunk(world, position + IVec3::Y);
    }
    if local.z == 0 {
        mark_chunk(world, position - IVec3::Z);
    }
    if local.z == last {
        mark_chunk(world, position + IVec3::Z);
    }
}

fn mark_chunk(world: &mut VoxelWorld<PrototypeWorld>, position: IVec3) {
    let chunk_position = position.div_euclid(IVec3::splat(CHUNK_SIZE_I));
    if world.get_chunk_data(chunk_position).is_none() {
        return;
    }

    let current = world.get_voxel(position);
    world.set_voxel(position, current);
}
