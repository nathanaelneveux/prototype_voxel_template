use bevy::camera::visibility::VisibilitySystems;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor,
};
use bevy::shader::ShaderRef;
use bevy::ui::FocusPolicy;
use bevy::ui_render::ui_material::UiMaterialKey;
use bevy::window::{CursorOptions, PrimaryWindow};

pub struct CrosshairPlugin;

impl Plugin for CrosshairPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<CrosshairMaterial>::default())
            .add_systems(Startup, spawn_crosshair)
            .add_systems(
                PostUpdate,
                sync_crosshair_visibility.before(VisibilitySystems::VisibilityPropagate),
            );
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct CrosshairMaterial {}

impl UiMaterial for CrosshairMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/crosshair.wesl".into()
    }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, _: UiMaterialKey<Self>) {
        // Minecraft Java's INVERT blend yields 1 - destination RGB for white.
        if let Some(target) = descriptor
            .fragment
            .as_mut()
            .and_then(|f| f.targets[0].as_mut())
        {
            target.blend = Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::OneMinusDst,
                    dst_factor: BlendFactor::OneMinusSrc,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent::REPLACE,
            });
        }
    }
}

#[derive(Component)]
struct CrosshairRoot;

fn spawn_crosshair(mut commands: Commands, mut materials: ResMut<Assets<CrosshairMaterial>>) {
    commands
        .spawn((
            Name::new("Crosshair"),
            CrosshairRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            FocusPolicy::Pass,
            Visibility::Hidden,
        ))
        .with_child((
            Node {
                width: px(15),
                height: px(15),
                ..default()
            },
            FocusPolicy::Pass,
            MaterialNode(materials.add(CrosshairMaterial {})),
        ));
}

fn sync_crosshair_visibility(
    window: Single<(&Window, &CursorOptions), With<PrimaryWindow>>,
    mut crosshair: Single<&mut Visibility, With<CrosshairRoot>>,
) {
    let (window, cursor) = *window;
    let visibility = if window.focused && !cursor.visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    crosshair.set_if_neq(visibility);
}
