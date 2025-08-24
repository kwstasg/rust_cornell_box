use bevy::prelude::*;
use bevy::window::{PresentMode, WindowMode, MonitorSelection, PrimaryWindow};
use bevy::core_pipeline::fxaa::Fxaa;
use bevy::core_pipeline::{bloom::Bloom, tonemapping::Tonemapping};
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, DiagnosticsStore};
use bevy::ui::{Node, PositionType, Val, BackgroundColor, BorderColor, Outline};
use bevy::pbr::{PointLightShadowMap, VolumetricLight, FogVolume, VolumetricFog};

// ------------ Tunables ------------------------------------------------------
const ROOM_W: f32 = 2.0;
const ROOM_H: f32 = 2.0;
const ROOM_D: f32 = 2.5;
const WALL_T: f32 = 0.05;

const PANEL_W: f32 = 0.70;
const PANEL_D: f32 = 0.70;

const GRID: usize = 2;
const SHADOW_MAP_SIZE: usize = 2048;

const USE_MSAA_SAMPLE2: bool = true;
const USE_FXAA: bool = true;

const AMBIENT_BRIGHTNESS: f32 = 0.015;

// base intensities at slider "1.0"
const BASE_CENTER_INTENSITY: f32 = 4000.0;
const BASE_OTHER_INTENSITY: f32 = 2000.0;

// Slider visual + behavior
const SLIDER_WIDTH_PX: f32 = 340.0;
const SLIDER_HEIGHT_PX: f32 = 14.0;
const SLIDER_KNOB_SIZE_PX: f32 = 18.0;
const SLIDER_BOTTOM_MARGIN_PX: f32 = 14.0;
const SLIDER_GRAB_EXTRA_Y_PX: f32 = 28.0;

// ---- Volumetric tuning knobs ----------------------------------------------
const LIGHT_RANGE: f32 = 30.0;
const LIGHT_COLOR: Color =Color::srgb(1.0, 0.95, 0.8);
const LIGHT_RADIUS: f32 = 0.25;
const FOG_DENSITY_FACTOR: f32 = 0.001;
// ---------------------------------------------------------------------------

#[derive(Component)]
struct FpsText;

#[derive(Component)]
struct CeilingLight {
    center: bool,
}

#[derive(Resource, Clone)]
struct LightControl {
    value: f32,      // 0.0 .. 1.0 from slider
    min_scale: f32,  // intensity multiplier at 0.0
    max_scale: f32,  // intensity multiplier at 1.0
}
impl LightControl {
    fn current_scales(&self) -> (f32, f32) {
        let s = self.min_scale + (self.max_scale - self.min_scale) * self.value.clamp(0.0, 1.0);
        (s, s)
    }
}

#[derive(Component)]
struct SliderTrack;
#[derive(Component)]
struct SliderKnob;

#[derive(Resource, Default)]
struct SliderDrag {
    active: bool,
}

fn main() {
    App::new()
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: AMBIENT_BRIGHTNESS,
            affects_lightmapped_meshes: false,
        })
        .insert_resource(PointLightShadowMap { size: SHADOW_MAP_SIZE })
        .insert_resource(LightControl { value: 0.25, min_scale: 0.0, max_scale: 14.0 })
        .insert_resource(SliderDrag::default())
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Cornell Box + Volumetric Fog".to_string(),
                    mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                    present_mode: PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            })
        )
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_systems(Startup, (setup_camera_and_scene, setup_fps_ui, setup_slider))
        .add_systems(Update, (slider_input, apply_intensity_to_scene, slider_visual).chain())
        .add_systems(Update, update_fps_ui)
        .run();
}

fn setup_camera_and_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    light_ctl: Res<LightControl>,
) {
    // ----- Camera (single camera for 3D + UI) -----
    let mut t = Transform::from_xyz(0.0, 1.0, 3.2);
    t.look_at(Vec3::new(0.0, 0.9, 0.0), Vec3::Y);

    let cam3d = (
        Camera3d::default(),
        Camera { order: 0, hdr: true, ..default() }, // HDR required for bloom/volumetrics
        Tonemapping::AcesFitted,
        Bloom::default(),
        t,
    );
    let cam_entity = commands.spawn(cam3d).id();

    // No environment/skybox, so disable ambient contribution in the volumetric pass.
    commands.entity(cam_entity).insert(VolumetricFog {
        ambient_intensity: 0.0,
        ..default()
    });

    if USE_MSAA_SAMPLE2 { commands.entity(cam_entity).insert(Msaa::Sample2); }
    if USE_FXAA { commands.entity(cam_entity).insert(Fxaa::default()); }

    // ----- Scene (Cornell-style box) -----
    let mut cuboid = |size: Vec3| -> Mesh3d { Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))) };

    let white_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.96, 0.96),
        perceptual_roughness: 1.0, metallic: 0.0, ..default()
    });
    let red_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.63, 0.065, 0.05),
        perceptual_roughness: 1.0, metallic: 0.0, ..default()
    });
    let green_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.45, 0.091),
        perceptual_roughness: 1.0, metallic: 0.0, ..default()
    });

    // room
    commands.spawn((cuboid(Vec3::new(ROOM_W, WALL_T, ROOM_D)), MeshMaterial3d(white_mat.clone()), Transform::from_xyz(0.0, WALL_T * 0.5, 0.0)));
    commands.spawn((cuboid(Vec3::new(ROOM_W, WALL_T, ROOM_D)), MeshMaterial3d(white_mat.clone()), Transform::from_xyz(0.0, ROOM_H - WALL_T * 0.5, 0.0)));
    commands.spawn((cuboid(Vec3::new(ROOM_W, ROOM_H, WALL_T)), MeshMaterial3d(white_mat.clone()), Transform::from_xyz(0.0, ROOM_H * 0.5, -ROOM_D * 0.5 + WALL_T * 0.5)));
    commands.spawn((cuboid(Vec3::new(WALL_T, ROOM_H, ROOM_D)), MeshMaterial3d(red_mat.clone()), Transform::from_xyz(-ROOM_W * 0.5 + WALL_T * 0.5, ROOM_H * 0.5, 0.0)));
    commands.spawn((cuboid(Vec3::new(WALL_T, ROOM_H, ROOM_D)), MeshMaterial3d(green_mat.clone()), Transform::from_xyz(ROOM_W * 0.5 - WALL_T * 0.5, ROOM_H * 0.5, 0.0)));

    // panel (plain white, no emissive)
    let panel_handle = materials.add(StandardMaterial { base_color: Color::srgb(0.98, 0.98, 0.98), ..default() });
    commands.spawn((
        cuboid(Vec3::new(PANEL_W, WALL_T * 0.5, PANEL_D)),
        MeshMaterial3d(panel_handle),
        Transform::from_xyz(0.0, ROOM_H - WALL_T - 0.001, 0.0),
    ));

    // grid lights (volumetric)
    let step_x = if GRID > 1 { PANEL_W / (GRID as f32 - 1.0) } else { 0.0 };
    let step_z = if GRID > 1 { PANEL_D / (GRID as f32 - 1.0) } else { 0.0 };
    let start_x = -PANEL_W * 0.5;
    let start_z = -PANEL_D * 0.5;
    let y = ROOM_H - 0.12;

    let (scale, _) = light_ctl.current_scales();
    let cur_center = BASE_CENTER_INTENSITY * scale;
    let cur_other  = BASE_OTHER_INTENSITY * scale;

    for ix in 0..GRID {
        for iz in 0..GRID {
            let x = start_x + ix as f32 * step_x;
            let z = start_z + iz as f32 * step_z;

            let is_center = (ix == GRID / 2) && (iz == GRID / 2);

            commands.spawn((
                PointLight {
                    intensity: if is_center { cur_center } else { cur_other },
                    range: LIGHT_RANGE,  // was 40.0
                    radius: LIGHT_RADIUS,
                    color: LIGHT_COLOR,
                    shadows_enabled: true, // enable on all so every light can cast volumetric shafts
                    ..default()
                },
                VolumetricLight,     // participates in volumetric pass
                Transform::from_xyz(x, y, z),
                CeilingLight { center: is_center },
            ));
        }
    }

    // Fog volume sized *slightly smaller* vertically than the room -> reduces perceived density
    commands.spawn((
        FogVolume {
            density_factor: FOG_DENSITY_FACTOR,
            absorption: 0.18,
            fog_color: LIGHT_COLOR,
            ..default()
        },
        Transform {
            translation: Vec3::new(0.0, ROOM_H * 0.5, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(ROOM_W * 1.05, ROOM_H * 1.05, ROOM_D * 1.05),
        },
    ));

    // props
    {
        let size = Vec3::new(0.6, 0.6, 0.6);
        let mut t = Transform::from_xyz(-0.42, WALL_T + size.y * 0.5, -0.55);
        t.rotate_y(15f32.to_radians());
        commands.spawn((cuboid(size), MeshMaterial3d(white_mat.clone()), t));
    }
    {
        let size = Vec3::new(0.5, 1.2, 0.5);
        let mut t = Transform::from_xyz(0.52, WALL_T + size.y * 0.5, -0.32);
        t.rotate_y(-12f32.to_radians());
        commands.spawn((cuboid(size), MeshMaterial3d(white_mat.clone()), t));
    }
}

fn setup_fps_ui(mut commands: Commands) {
    commands
        .spawn((
            Text::new("FPS: "),
            TextFont { font_size: 22.0, ..default() },
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                right: Val::Px(12.0),
                ..default()
            },
        ))
        .with_child((
            TextSpan::default(),
            TextFont { font_size: 22.0, ..default() },
            TextColor(Color::srgb(1.0, 0.9, 0.2)),
            FpsText,
        ));
}

fn update_fps_ui(diagnostics: Res<DiagnosticsStore>, mut q: Query<&mut TextSpan, With<FpsText>>) {
    if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(value) = fps.smoothed() {
            for mut span in &mut q {
                *span = TextSpan::new(format!("{value:.1}"));
            }
        }
    }
}

// ---------------- Slider UI -------------------------------------------------

fn setup_slider(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(SLIDER_BOTTOM_MARGIN_PX),
                left: Val::Percent(50.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(0.0),
                        left: Val::Px(-SLIDER_WIDTH_PX * 0.5),
                        width: Val::Px(SLIDER_WIDTH_PX),
                        height: Val::Px(SLIDER_HEIGHT_PX),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.2, 0.22)),
                    BorderColor(Color::srgb(0.9, 0.9, 0.95)),
                    Outline::default(),
                    SliderTrack,
                ))
                .with_children(|track| {
                    track.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: Val::Px(-(SLIDER_KNOB_SIZE_PX - SLIDER_HEIGHT_PX) * 0.5),
                            left: Val::Px(0.0),
                            width: Val::Px(SLIDER_KNOB_SIZE_PX),
                            height: Val::Px(SLIDER_KNOB_SIZE_PX),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.95, 0.95, 0.98)),
                        BorderColor(Color::srgb(0.1, 0.1, 0.12)),
                        SliderKnob,
                    ));
                });
        });
}

fn slider_input(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut ctl: ResMut<LightControl>,
    mut drag: ResMut<SliderDrag>,
) {
    let Ok(window) = windows.single() else { return; };
    let Some(cursor) = window.cursor_position() else {
        if !buttons.pressed(MouseButton::Left) { drag.active = false; }
        return;
    };

    let w = window.width();
    let h = window.height();
    let cx = w * 0.5;

    let track_left = cx - SLIDER_WIDTH_PX * 0.5;
    let track_right = cx + SLIDER_WIDTH_PX * 0.5;
    let track_bottom_y = h - SLIDER_BOTTOM_MARGIN_PX;
    let track_top_y = h - (SLIDER_BOTTOM_MARGIN_PX + SLIDER_HEIGHT_PX + SLIDER_GRAB_EXTRA_Y_PX);

    let inside_x = cursor.x >= track_left && cursor.x <= track_right;
    let inside_y = cursor.y >= track_top_y && cursor.y <= track_bottom_y;
    let inside = inside_x && inside_y;

    if buttons.just_pressed(MouseButton::Left) { drag.active = inside; }
    else if buttons.just_released(MouseButton::Left) { drag.active = false; }

    if drag.active {
        let v = ((cursor.x - track_left) / (track_right - track_left)).clamp(0.0, 1.0);
        ctl.value = v;
    }
}

fn apply_intensity_to_scene(
    ctl: Res<LightControl>,
    mut q: Query<(&CeilingLight, &mut PointLight)>,
) {
    if !ctl.is_changed() { return; }
    let (scale_center, scale_other) = ctl.current_scales();
    for (tag, mut pl) in &mut q {
        if tag.center { pl.intensity = BASE_CENTER_INTENSITY * scale_center; }
        else { pl.intensity = BASE_OTHER_INTENSITY * scale_other; }
    }
}

fn slider_visual(ctl: Res<LightControl>, mut knobs: Query<&mut Node, With<SliderKnob>>) {
    if !ctl.is_changed() { return; }
    let knob_left = (SLIDER_WIDTH_PX - SLIDER_KNOB_SIZE_PX) * ctl.value;
    for mut node in &mut knobs { node.left = Val::Px(knob_left); }
}
