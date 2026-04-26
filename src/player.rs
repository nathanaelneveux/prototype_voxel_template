use std::time::Duration;

use avian3d::prelude::*;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use bevy_ahoy::input::AccumulatedInput;
use bevy_ahoy::prelude::*;
use bevy_enhanced_input::prelude::*;
use bevy_voxel_world::{
    custom_meshing::CHUNK_SIZE_I,
    prelude::{VoxelWorld, VoxelWorldCamera, WorldVoxel},
};

use crate::terrain::{MATERIAL_GRASS, PrototypeWorld};

const FOG_START: f32 = 96.0;
const FOG_END: f32 = 160.0;
const PLAYER_RADIUS: f32 = 0.3;
const PLAYER_HEIGHT: f32 = 1.8;
const PLAYER_CROUCH_HEIGHT: f32 = 1.5;
const PLAYER_STANDING_VIEW_HEIGHT: f32 = 1.62;
const PLAYER_CROUCH_VIEW_HEIGHT: f32 = 1.27;
const PLAYER_WALK_SPEED: f32 = 4.32;
const PLAYER_SPRINT_SPEED: f32 = PLAYER_WALK_SPEED * 1.5;
const PLAYER_CROUCH_SPEED_SCALE: f32 = 0.3;
const PLAYER_JUMP_HEIGHT: f32 = 1.0;
const PLAYER_GRAVITY: f32 = 32.0;
const PLAYER_STEP_SIZE: f32 = 0.55;
const PLAYER_MAX_SPEED: f32 = 80.0;
const PLAYER_AIR_ACCELERATION_HZ: f32 = 8.0;
const PLAYER_MAX_AIR_WISH_SPEED: f32 = 0.35;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_input_context::<PlayerInput>()
            .add_systems(Startup, spawn_player)
            .add_systems(
                FixedPostUpdate,
                (update_player_movement_speed, filter_held_jump)
                    .chain()
                    .before(AhoySystems::MoveCharacters),
            )
            .add_systems(Update, (edit_voxels, reset_player_to_world_spawn));
    }
}

#[derive(Component)]
struct Player;

#[derive(Component, Default)]
struct PlayerInput;

#[derive(Component)]
struct PlayerCamera;

#[derive(Component, Default)]
struct HeldJumpFilter {
    was_grounded: bool,
}

fn spawn_player(mut commands: Commands, world: Res<PrototypeWorld>) {
    let player = commands
        .spawn((
            Name::new("Player"),
            Player,
            PlayerInput,
            HeldJumpFilter::default(),
            player_controller(),
            Collider::cylinder(PLAYER_RADIUS, PLAYER_HEIGHT),
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

fn player_controller() -> CharacterController {
    CharacterController {
        crouch_height: PLAYER_CROUCH_HEIGHT,
        standing_view_height: PLAYER_STANDING_VIEW_HEIGHT,
        crouch_view_height: PLAYER_CROUCH_VIEW_HEIGHT,
        ground_distance: 0.04,
        min_walk_cos: 45.0_f32.to_radians().cos(),
        stop_speed: PLAYER_WALK_SPEED,
        friction_hz: 12.0,
        acceleration_hz: 16.0,
        air_acceleration_hz: PLAYER_AIR_ACCELERATION_HZ,
        gravity: PLAYER_GRAVITY,
        step_size: PLAYER_STEP_SIZE,
        step_down_detection_distance: PLAYER_STEP_SIZE * 2.0,
        crouch_speed_scale: PLAYER_CROUCH_SPEED_SCALE,
        speed: PLAYER_WALK_SPEED,
        max_speed: PLAYER_MAX_SPEED,
        jump_height: PLAYER_JUMP_HEIGHT,
        max_air_wish_speed: PLAYER_MAX_AIR_WISH_SPEED,
        coyote_time: Duration::from_millis(50),
        jump_input_buffer: Duration::from_millis(50),
        ..default()
    }
}

fn update_player_movement_speed(
    keys: Res<ButtonInput<KeyCode>>,
    mut player: Single<(&mut CharacterController, &CharacterControllerState), With<Player>>,
) {
    let sprinting = (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight))
        && keys.pressed(KeyCode::KeyW)
        && !keys.pressed(KeyCode::ControlLeft)
        && !keys.pressed(KeyCode::ControlRight)
        && !player.1.crouching;

    player.0.speed = if sprinting {
        PLAYER_SPRINT_SPEED
    } else {
        PLAYER_WALK_SPEED
    };
}

fn filter_held_jump(
    keys: Res<ButtonInput<KeyCode>>,
    mut player: Single<
        (
            &CharacterControllerState,
            &mut AccumulatedInput,
            &mut HeldJumpFilter,
        ),
        With<Player>,
    >,
) {
    let (state, input, filter) = &mut *player;
    let holding_jump = keys.pressed(KeyCode::Space);

    if !holding_jump {
        filter.was_grounded = state.grounded.is_some();
        return;
    }

    if state.grounded.is_none() {
        filter.was_grounded = false;
        return;
    }

    if !filter.was_grounded {
        input.jumped = None;
    }

    filter.was_grounded = true;
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
