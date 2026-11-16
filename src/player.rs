use std::time::Duration;

use avian3d::prelude::*;
use bevy::input::InputSystems;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use bevy_ahoy::input::AccumulatedInput;
use bevy_ahoy::prelude::*;
use bevy_enhanced_input::prelude::*;
use bevy_voxel_world::prelude::{VoxelWorld, VoxelWorldCamera, WorldVoxel};

use crate::terrain::{PrototypeWorld, TERRAIN_VIEW_DISTANCE, TerrainMaterial};
use crate::world_environment::sky_color;

const FOG_START: f32 = 96.0;
const FOG_END: f32 = TERRAIN_VIEW_DISTANCE;
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
const BOMB_RADIUS: i32 = 5;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_input_context::<PlayerInput>()
            .add_systems(Startup, spawn_player)
            .add_systems(
                PreUpdate,
                sync_player_input
                    .after(InputSystems)
                    .before(EnhancedInputSystems::Prepare),
            )
            .add_systems(
                FixedPostUpdate,
                (update_player_movement_speed, filter_held_jump)
                    .chain()
                    .before(AhoySystems::MoveCharacters),
            )
            .add_systems(Update, (edit_voxels, reset_player_to_world_spawn));
    }
}

#[derive(Component, Clone, Default)]
struct Player;

#[derive(Component, Clone, Default)]
struct PlayerInput;

#[derive(Component)]
struct PlayerCamera;

#[derive(Component, Clone, Default)]
struct HeldJumpFilter {
    was_grounded: bool,
}

#[derive(InputAction)]
#[action_output(bool)]
struct Sprint;

#[derive(InputAction)]
#[action_output(bool)]
struct BreakVoxel;

#[derive(InputAction)]
#[action_output(bool)]
struct PlaceVoxel;

#[derive(InputAction)]
#[action_output(bool)]
struct PlaceEmitter;

#[derive(InputAction)]
#[action_output(bool)]
struct DetonateBomb;

fn spawn_player(mut commands: Commands, world: Res<PrototypeWorld>) {
    let spawn_position = world.player_spawn_position();
    let player = commands
        .spawn_scene(bsn! {
            Name::new("Player")
            Player
            PlayerInput
            HeldJumpFilter
            Collider::cylinder(PLAYER_RADIUS, PLAYER_HEIGHT)
            Transform::from_translation(spawn_position)
            Actions<PlayerInput> [
                Action<Movement>::new()
                DeadZone
                Bindings [
                    {(
                        preset::cardinal::wasd_keys(),
                        preset::cardinal::arrows(),
                        preset::cardinal::dpad(),
                        preset::axial::left_stick(),
                    )}
                ]
                --
                Action<Jump>::new()
                Bindings [Binding(KeyCode::Space) -- Binding(GamepadButton::North)]
                --
                Action<Crouch>::new()
                Bindings [
                    Binding(KeyCode::ControlLeft)
                    --
                    Binding(KeyCode::ControlRight)
                    --
                    Binding(GamepadButton::LeftThumb)
                ]
                --
                Action<Sprint>::new()
                Bindings [
                    Binding(KeyCode::ShiftLeft)
                    --
                    Binding(KeyCode::ShiftRight)
                    --
                    Binding(GamepadButton::West)
                ]
                --
                Action<BreakVoxel>::new()
                Bindings [Binding(MouseButton::Left) -- Binding(GamepadButton::South)]
                --
                Action<PlaceVoxel>::new()
                Bindings [Binding(MouseButton::Right) -- Binding(GamepadButton::East)]
                --
                Action<PlaceEmitter>::new()
                Bindings [Binding(KeyCode::KeyE)]
                --
                Action<DetonateBomb>::new()
                Bindings [Binding(KeyCode::KeyB)]
                --
                Action<RotateCamera>::new()
                Bindings [
                    Binding::mouse_motion()
                    ~Scale::splat(0.08)
                    --
                    {preset::axial::new(
                        (GamepadAxis::RightStickX, Scale::splat(2.5)),
                        (GamepadAxis::RightStickY, Scale::splat(2.5)),
                    )}
                ]
            ]
        })
        .insert(player_controller())
        .id();

    commands.spawn((
        Name::new("PlayerCamera"),
        PlayerCamera,
        Camera3d::default(),
        DistanceFog {
            color: sky_color(),
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

fn sync_player_input(
    window: Single<(&Window, &CursorOptions), With<PrimaryWindow>>,
    player: Single<(Entity, &ContextActivity<PlayerInput>, &mut AccumulatedInput)>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut commands: Commands,
) {
    let (window, cursor) = *window;
    let (entity, activity, mut input) = player.into_inner();
    // Left click also recaptures the cursor. Let it go before gameplay resumes.
    let left_click = mouse.pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Left);
    let active = window.focused && !cursor.visible && (**activity || !left_click);
    if **activity != active {
        commands
            .entity(entity)
            .insert(ContextActivity::<PlayerInput>::new(active));
        if !active {
            *input = AccumulatedInput::default();
        }
    }
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
    movement: Single<&Action<Movement>>,
    sprint: Single<&Action<Sprint>>,
    crouch: Single<&Action<Crouch>>,
    mut player: Single<(&mut CharacterController, &CharacterControllerState), With<Player>>,
) {
    let movement = movement.into_inner();
    let sprint = sprint.into_inner();
    let crouch = crouch.into_inner();

    let sprinting = **sprint && movement.y > 0.0 && !**crouch && !player.1.crouching;

    player.0.speed = if sprinting {
        PLAYER_SPRINT_SPEED
    } else {
        PLAYER_WALK_SPEED
    };
}

fn filter_held_jump(
    jump: Single<&Action<Jump>>,
    mut player: Single<
        (
            &CharacterControllerState,
            &mut AccumulatedInput,
            &mut HeldJumpFilter,
        ),
        With<Player>,
    >,
) {
    let jump = jump.into_inner();

    let (state, input, filter) = &mut *player;
    let holding_jump = **jump;

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
    break_events: Single<&ActionEvents, With<Action<BreakVoxel>>>,
    place_events: Single<&ActionEvents, With<Action<PlaceVoxel>>>,
    emitter_events: Single<&ActionEvents, With<Action<PlaceEmitter>>>,
    bomb_events: Single<&ActionEvents, With<Action<DetonateBomb>>>,
    cursor_options: Single<&CursorOptions, With<PrimaryWindow>>,
    camera: Single<&GlobalTransform, (With<PlayerCamera>, Without<Player>)>,
    mut voxel_world: VoxelWorld<PrototypeWorld>,
) {
    if cursor_options.visible {
        return;
    }

    let breaking = break_events.contains(ActionEvents::START);
    let bombing = bomb_events.contains(ActionEvents::START);
    let placement = if emitter_events.contains(ActionEvents::START) {
        Some(TerrainMaterial::Emitter)
    } else if place_events.contains(ActionEvents::START) {
        Some(TerrainMaterial::Grass)
    } else {
        None
    };

    if !breaking && !bombing && placement.is_none() {
        return;
    }

    let ray = Ray3d::new(camera.translation(), camera.forward());
    let Some(hit) = voxel_world.raycast(ray, &|(_, voxel)| matches!(voxel, WorldVoxel::Solid(_)))
    else {
        return;
    };

    if breaking || bombing {
        let radius = if bombing { BOMB_RADIUS } else { 0 };
        for position in destruction_positions(hit.position.as_ivec3(), radius) {
            if matches!(voxel_world.get_voxel(position), WorldVoxel::Solid(_)) {
                voxel_world.set_voxel(position, WorldVoxel::Air);
            }
        }
    }

    if let Some(material) = placement {
        let Some(normal) = hit.normal else {
            return;
        };
        let placement_position = (hit.position + normal).as_ivec3();

        if matches!(
            voxel_world.get_voxel(placement_position),
            WorldVoxel::Air | WorldVoxel::Unset
        ) {
            voxel_world.set_voxel(placement_position, material.voxel());
        }
    }
}

fn destruction_positions(center: IVec3, radius: i32) -> impl Iterator<Item = IVec3> {
    (-radius..=radius).flat_map(move |x| {
        (-radius..=radius).flat_map(move |y| {
            (-radius..=radius).filter_map(move |z| {
                let offset = IVec3::new(x, y, z);
                (offset.length_squared() <= radius * radius).then_some(center + offset)
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_input_resumes_after_the_recapture_click_is_released() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::input::InputPlugin,
            bevy::scene::ScenePlugin,
            EnhancedInputPlugin,
        ))
        .init_resource::<PrototypeWorld>()
        .add_input_context::<PlayerInput>()
        .add_systems(Startup, spawn_player)
        .add_systems(
            PreUpdate,
            sync_player_input
                .after(InputSystems)
                .before(EnhancedInputSystems::Prepare),
        );
        let window = app
            .world_mut()
            .spawn((
                PrimaryWindow,
                Window {
                    focused: true,
                    ..default()
                },
                CursorOptions::default(),
            ))
            .id();

        app.finish();
        app.cleanup();
        app.update();
        let world = app.world_mut();
        let player = world
            .query_filtered::<Entity, With<Player>>()
            .single(world)
            .unwrap();
        let break_action = world
            .query_filtered::<Entity, With<Action<BreakVoxel>>>()
            .single(world)
            .unwrap();
        let active = |app: &App| {
            **app
                .world()
                .get::<ContextActivity<PlayerInput>>(player)
                .unwrap()
        };
        let breaking = |app: &App| {
            app.world()
                .get::<ActionEvents>(break_action)
                .unwrap()
                .contains(ActionEvents::START)
        };
        assert!(!active(&app));

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(!breaking(&app));

        app.world_mut()
            .get_mut::<CursorOptions>(window)
            .unwrap()
            .visible = false;
        app.update();
        assert!(!active(&app));
        assert!(!breaking(&app));

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert!(active(&app));

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(breaking(&app));

        app.world_mut()
            .get_mut::<CursorOptions>(window)
            .unwrap()
            .visible = true;
        app.update();
        assert!(!active(&app));
        assert!(!breaking(&app));
    }

    #[test]
    fn bomb_matches_the_gi_lab_sphere() {
        let center = IVec3::new(-32, 17, 64);
        let targets: std::collections::HashSet<_> =
            destruction_positions(center, BOMB_RADIUS).collect();
        assert_eq!(targets.len(), 515);
        assert!(targets.contains(&center));
        assert!(targets.contains(&(center + IVec3::X * 5)));
        assert!(!targets.contains(&(center + IVec3::X * 6)));
        assert!(targets.iter().all(|p| (*p - center).length_squared() <= 25));
    }
}
