use bevy::input::{InputSystems, mouse::AccumulatedMouseMotion};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, Window};
use bevy_enhanced_input::prelude::{
    Action, ActionEvents, EnhancedInputSystems, InputAction, InputContextAppExt, actions, bindings,
};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};

pub struct AppControlsPlugin;

impl Plugin for AppControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            EguiPlugin::default(),
            WorldInspectorPlugin::default().run_if(inspector_mode_active),
        ))
        .init_resource::<InspectorMode>()
        .init_resource::<CursorLockState>()
        .add_input_context::<AppInput>()
        .add_systems(Startup, (spawn_app_input, lock_cursor))
        .add_systems(
            PreUpdate,
            suppress_first_lock_mouse_motion
                .after(InputSystems)
                .before(EnhancedInputSystems::Prepare),
        )
        .add_systems(Update, toggle_inspector_mode)
        .add_systems(PostUpdate, recapture_cursor);
    }
}

#[derive(Resource, Default)]
struct InspectorMode {
    enabled: bool,
}

#[derive(Resource, Default)]
struct CursorLockState {
    ignore_next_motion: bool,
}

#[derive(Component, Default)]
struct AppInput;

#[derive(InputAction)]
#[action_output(bool)]
struct ToggleInspectorMode;

#[derive(InputAction)]
#[action_output(bool)]
struct RecaptureCursor;

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

fn spawn_app_input(mut commands: Commands) {
    commands.spawn((
        Name::new("AppInput"),
        AppInput,
        actions!(AppInput[
            (
                Action::<ToggleInspectorMode>::new(),
                bindings![KeyCode::Escape],
            ),
            (
                Action::<RecaptureCursor>::new(),
                bindings![MouseButton::Left],
            ),
        ]),
    ));
}

fn lock_cursor(
    mut cursor_lock_state: ResMut<CursorLockState>,
    mut primary_window: Single<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    let (window, cursor_options) = &mut *primary_window;
    set_cursor_locked(window, cursor_options, true, &mut cursor_lock_state);
}

fn toggle_inspector_mode(
    toggle_inspector_events: Single<&ActionEvents, With<Action<ToggleInspectorMode>>>,
    mut inspector_mode: ResMut<InspectorMode>,
    mut cursor_lock_state: ResMut<CursorLockState>,
    mut primary_window: Single<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    if !toggle_inspector_events.contains(ActionEvents::START) {
        return;
    }

    inspector_mode.enabled = !inspector_mode.enabled;
    let (window, cursor_options) = &mut *primary_window;
    set_cursor_locked(
        window,
        cursor_options,
        !inspector_mode.enabled,
        &mut cursor_lock_state,
    );
}

fn recapture_cursor(
    recapture_events: Single<&ActionEvents, With<Action<RecaptureCursor>>>,
    inspector_mode: Res<InspectorMode>,
    mut cursor_lock_state: ResMut<CursorLockState>,
    mut primary_window: Single<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    if !recapture_events.contains(ActionEvents::START) {
        return;
    }

    let (window, cursor_options) = &mut *primary_window;

    if !inspector_mode.enabled && cursor_options.visible {
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
