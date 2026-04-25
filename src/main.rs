mod assets;
mod chunk_colliders;
mod player;
mod terrain;
mod terrain_meshing;
mod terrain_noise;

use assets::AssetSupportPlugin;
use avian3d::prelude::*;
use bevy::asset::AssetMetaCheck;
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
use bevy::input::{InputSystems, mouse::AccumulatedMouseMotion};
use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, Window};
use bevy_ahoy::prelude::AhoyPlugins;
use bevy_enhanced_input::prelude::{EnhancedInputPlugin, EnhancedInputSystems};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};

use chunk_colliders::ChunkColliderPlugin;
use player::PlayerPlugin;
use terrain::TerrainPlugin;

#[derive(Resource, Default)]
struct InspectorMode {
    enabled: bool,
}

#[derive(Resource, Default)]
struct CursorLockState {
    ignore_next_motion: bool,
}

impl CursorLockState {
    fn arm_motion_suppression(&mut self) {
        self.ignore_next_motion = true;
    }

    fn suppress_motion(&mut self, accumulated_mouse_motion: &mut AccumulatedMouseMotion) {
        if self.ignore_next_motion && accumulated_mouse_motion.delta.length_squared() > 0.0 {
            accumulated_mouse_motion.delta = Vec2::ZERO;
            self.ignore_next_motion = false;
        }
    }
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.53, 0.74, 0.94)))
        .init_resource::<InspectorMode>()
        .init_resource::<CursorLockState>()
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
            EguiPlugin::default(),
            WorldInspectorPlugin::default().run_if(inspector_mode_active),
            AhoyPlugins::default(),
            AssetSupportPlugin,
            TerrainPlugin,
            ChunkColliderPlugin,
            PlayerPlugin,
        ))
        .add_systems(
            PreUpdate,
            suppress_first_lock_mouse_motion
                .after(InputSystems)
                .before(EnhancedInputSystems::Prepare),
        )
        .add_systems(Startup, (setup_lighting, lock_cursor))
        .add_systems(Update, (toggle_inspector_mode, recapture_cursor))
        .run();
}

fn setup_lighting(mut commands: Commands) {
    let cascade_shadow_config = CascadeShadowConfigBuilder { ..default() }.build();
    commands.spawn((
        Name::new("Sun"),
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            illuminance: 18_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::new(0.35, -0.8, 0.25), Vec3::Y),
        cascade_shadow_config,
    ));

    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.72, 0.78, 0.9),
        brightness: 500.0,
        affects_lightmapped_meshes: true,
    });
}

fn lock_cursor(
    mut cursor_lock_state: ResMut<CursorLockState>,
    mut primary_window: Single<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    let (window, cursor_options) = &mut *primary_window;
    set_cursor_locked(window, cursor_options, true, &mut cursor_lock_state);
}

fn toggle_inspector_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut inspector_mode: ResMut<InspectorMode>,
    mut cursor_lock_state: ResMut<CursorLockState>,
    mut primary_window: Single<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        inspector_mode.enabled = !inspector_mode.enabled;
        let (window, cursor_options) = &mut *primary_window;
        set_cursor_locked(
            window,
            cursor_options,
            !inspector_mode.enabled,
            &mut cursor_lock_state,
        );
    }
}

fn recapture_cursor(
    buttons: Res<ButtonInput<MouseButton>>,
    inspector_mode: Res<InspectorMode>,
    mut cursor_lock_state: ResMut<CursorLockState>,
    mut primary_window: Single<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    let (window, cursor_options) = &mut *primary_window;

    if !inspector_mode.enabled && buttons.just_pressed(MouseButton::Left) && cursor_options.visible
    {
        set_cursor_locked(window, cursor_options, true, &mut cursor_lock_state);
    }
}

fn inspector_mode_active(inspector_mode: Res<InspectorMode>) -> bool {
    inspector_mode.enabled
}

fn set_cursor_locked(
    window: &mut Window,
    cursor_options: &mut CursorOptions,
    locked: bool,
    cursor_lock_state: &mut CursorLockState,
) {
    if locked {
        center_cursor_in_window(window);
        cursor_lock_state.arm_motion_suppression();
        cursor_options.visible = false;
        cursor_options.grab_mode = CursorGrabMode::Locked;
    } else {
        cursor_options.visible = true;
        cursor_options.grab_mode = CursorGrabMode::None;
    }
}

fn center_cursor_in_window(window: &mut Window) {
    let center = Vec2::new(window.width() * 0.5, window.height() * 0.5);
    window.set_cursor_position(Some(center));
}

fn suppress_first_lock_mouse_motion(
    mut cursor_lock_state: ResMut<CursorLockState>,
    mut accumulated_mouse_motion: ResMut<AccumulatedMouseMotion>,
) {
    cursor_lock_state.suppress_motion(&mut accumulated_mouse_motion);
}
