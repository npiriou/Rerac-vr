//! Stereo views of the ORIGINAL ReRAC world and its live game, including level changes.
//! No level loader, entity renderer, hero physics or weapon implementation is duplicated here.
use super::{
    game_input::{GameInputPlugin, GamePad},
    Settings,
};
use bevy::{
    app::PluginGroupBuilder,
    camera::{
        visibility::RenderLayers, Camera3dDepthLoadOp, CameraOutputMode, ClearColorConfig,
        RenderTarget,
    },
    core_pipeline::core_3d::Transparent3d,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        pipelined_rendering::PipelinedRenderingPlugin,
        render_phase::ViewSortedRenderPhases,
        render_resource::{BlendState, PipelineCache, TextureFormat},
        view::{ExtractedView, Msaa},
        Render, RenderApp, RenderSystems,
    },
    transform::TransformSystems,
};
use bevy_mod_openxr::{
    add_xr_plugins,
    render::update_views,
    resources::{OxrSessionConfig, OxrViews},
};
use bevy_mod_xr::{
    camera::{XrCamera, XrProjection, XrViewInit},
    session::{XrPreDestroySession, XrSessionCreated, XrTrackingRoot},
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

const PANEL_LAYER: usize = 62;
#[derive(Component, Clone, ExtractComponent)]
pub struct GameEye;
#[derive(Component, Clone, ExtractComponent)]
struct EyeSky;
#[derive(Component)]
struct Panel(u8);
#[derive(Resource, Default)]
pub(super) struct Rig {
    center: Option<Vec3>,
    yaw: f32,
    reset_held: bool,
}
impl Rig {
    pub(super) fn movement_from_head(&self, movement: Vec2, head: Quat) -> Vec2 {
        if self.center.is_none() {
            return movement;
        }
        let relative = heading(head) - self.yaw;
        Vec2::from_angle(relative).rotate(movement)
    }
}
#[derive(Resource, Clone, Default)]
struct Rendered(Arc<AtomicU64>);
#[derive(Resource, Clone, Default)]
struct DrawAudit {
    world: Arc<AtomicU64>,
    sky: Arc<AtomicU64>,
    waiting: Arc<AtomicU64>,
}
#[derive(Resource)]
struct MonitorCadence {
    every: u32,
    frame: u32,
    restore: Vec<Entity>,
}
#[derive(Resource)]
struct Audit {
    start: std::time::Instant,
    last: std::time::Instant,
    first_tick: Option<u64>,
    last_tick: u64,
}

pub fn prepare_game() -> anyhow::Result<()> {
    let s = Settings::from_env()?;
    // Before any original plugin is built. Keep 60 Hz logic, let xrWaitFrame pace rendering.
    unsafe {
        std::env::set_var("RC_FRAME_CAP", "0");
        // PS2 visibility masks were authored for one narrow view. Use original frustum/LOD culling with
        // an HMD envelope instead, so leaning/looking around doesn't reveal missing objects.
        std::env::set_var("RC_OCCL", "0");
        for (key, value) in [
            ("RC_LEVEL", "1"),
            ("RC_FRONTEND", "0"),
            ("RC_SCENE", "0"),
            ("RC_PLAY", "1"),
            ("RC_MSAA", "0"),
        ] {
            if std::env::var_os(key).is_none() {
                std::env::set_var(key, value);
            }
        }
    }
    println!(
        "VR_GAME original ReRAC application; scene={} preview={} simulation=60Hz third-person",
        s.scene, s.desktop_preview
    );
    println!("VR_GAME controls: left stick move; right stick game camera; A cross/jump/confirm; B square/wrench; X circle/weapon; Y triangle/quick select/back; left stick click select/map; right stick click start/pause; right trigger weapon; left trigger strafe; right grip crouch; keyboard F8 recenter");
    Ok(())
}
pub fn game_plugins(plugins: PluginGroupBuilder) -> PluginGroupBuilder {
    if !super::game_requested() {
        return plugins;
    }
    let plugins = plugins.disable::<PipelinedRenderingPlugin>();
    if std::env::var("RC_VR_DESKTOP_PREVIEW").is_ok_and(|s| s == "1") {
        plugins.set(bevy::render::RenderPlugin {
            render_creation: bevy::render::settings::RenderCreation::Automatic(Box::new(
                bevy::render::settings::WgpuSettings {
                    backends: Some(bevy::render::settings::Backends::VULKAN),
                    ..default()
                },
            )),
            ..default()
        })
    } else {
        add_xr_plugins(plugins)
    }
}
pub fn install_game(app: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(
        crate::disc_source::current() == crate::disc_source::Game::Rac1,
        "VR gameplay currently requires RAC1 data"
    );
    let settings = Settings::from_env()?;
    let mirror_every = std::env::var("RC_VR_MIRROR_EVERY")
        .unwrap_or_else(|_| "4".into())
        .parse::<u32>()?;
    anyhow::ensure!(
        (1..=120).contains(&mirror_every),
        "RC_VR_MIRROR_EVERY must be 1..=120"
    );
    let frames = Rendered::default();
    let draw = DrawAudit::default();
    let now = std::time::Instant::now();
    app.insert_resource(settings.clone())
        .init_resource::<Rig>()
        .insert_resource(frames.clone())
        .insert_resource(draw.clone())
        .insert_resource(MonitorCadence {
            every: if settings.desktop_preview {
                1
            } else {
                mirror_every
            },
            frame: 0,
            restore: Vec::new(),
        })
        .add_systems(First, restore_monitor_cameras)
        .add_systems(
            PostUpdate,
            pace_monitor_cameras.before(bevy::camera::CameraUpdateSystems),
        )
        .add_plugins((
            ExtractComponentPlugin::<GameEye>::default(),
            ExtractComponentPlugin::<EyeSky>::default(),
        ))
        .insert_resource(Audit {
            start: now,
            last: now,
            first_tick: None,
            last_tick: 0,
        })
        .add_systems(
            RunFixedMainLoop,
            follow_game_camera
                .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop)
                .after(crate::play_camera::apply)
                .after(crate::scene_render::apply_camera)
                .after(crate::scene_render::hold_camera),
        )
        .add_systems(
            PostUpdate,
            update_panels.before(TransformSystems::Propagate),
        )
        .add_systems(Last, audit)
        .add_systems(
            PostUpdate,
            (
                mirror_pass::<crate::scene_render::SceneFade>,
                mirror_pass::<crate::fog_state::UnderwaterTint>,
                mirror_pass::<crate::screen_tint::ScreenTint>,
                mirror_pass::<crate::visibomb_view::MissileViewOverlay>,
            ),
        );
    app.sub_app_mut(RenderApp)
        .insert_resource(frames)
        .insert_resource(draw)
        .add_systems(Render, count.after(RenderSystems::Render));
    if settings.desktop_preview {
        app.add_systems(Startup, preview_eyes);
    } else {
        app.insert_resource(OxrSessionConfig::default())
            .add_plugins(GameInputPlugin)
            .add_systems(Startup, super::require_headset)
            .add_systems(XrPreDestroySession, reset_session)
            .add_systems(XrSessionCreated, configure_eyes.after(XrViewInit));
    }
    // Run after the original sky's setup and after its camera-load policy: it must not overwrite
    // the stereo sky's clear/depth/output settings.
    app.add_systems(
        PostUpdate,
        sky_policy
            .after(crate::sky_render::follow_main_camera)
            .after(update_views)
            .before(TransformSystems::Propagate),
    );
    Ok(())
}
// The monitor is an extra full world render. Refresh it less often during VR gameplay;
// the headset and simulation still update every frame. Menus retain their original full frame.
// Restore before the original systems run, so their own camera activation remains authoritative.
fn restore_monitor_cameras(mut cadence: ResMut<MonitorCadence>, mut cams: Query<&mut Camera>) {
    for e in cadence.restore.drain(..) {
        if let Ok(mut c) = cams.get_mut(e) {
            c.is_active = true;
        }
    }
}
fn pace_monitor_cameras(
    mut cadence: ResMut<MonitorCadence>,
    mode: Res<crate::menu_render::MenuMode>,
    mut cams: Query<
        (Entity, &mut Camera),
        Or<(
            With<crate::fly_cam::FlyCam>,
            With<crate::sky_render::SkyCamera>,
        )>,
    >,
) {
    cadence.frame = cadence.frame.wrapping_add(1);
    if mode.state.mode != rc_game::menus::mode::Mode::Gameplay || cadence.frame % cadence.every == 0
    {
        return;
    }
    for (e, mut cam) in &mut cams {
        if cam.is_active {
            cam.is_active = false;
            cadence.restore.push(e);
        }
    }
}
fn count(
    r: Res<Rendered>,
    draw: Res<DrawAudit>,
    phases: Res<ViewSortedRenderPhases<Transparent3d>>,
    views: Query<(&ExtractedView, Has<GameEye>, Has<EyeSky>)>,
    pipelines: Res<PipelineCache>,
) {
    r.0.fetch_add(1, Ordering::Relaxed);
    let mut world = u64::MAX;
    let mut sky = u64::MAX;
    for (view, is_world, is_sky) in &views {
        let count = phases
            .get(&view.retained_view_entity)
            .map_or(0, |p| p.items.len()) as u64;
        if is_world {
            world = world.min(count);
        }
        if is_sky {
            sky = sky.min(count);
        }
    }
    draw.world
        .store(if world == u64::MAX { 0 } else { world }, Ordering::Relaxed);
    draw.sky
        .store(if sky == u64::MAX { 0 } else { sky }, Ordering::Relaxed);
    draw.waiting.store(
        pipelines.waiting_pipelines().count() as u64,
        Ordering::Relaxed,
    );
}
fn reset_session(mut commands: Commands, panels: Query<Entity, With<Panel>>, mut rig: ResMut<Rig>) {
    for e in &panels {
        commands.entity(e).despawn();
    }
    *rig = Rig::default();
}
fn mirror_pass<T: Component + Clone + PartialEq>(
    mut commands: Commands,
    main: Query<Option<&T>, With<crate::fly_cam::FlyCam>>,
    eyes: Query<(Entity, Option<&T>), With<GameEye>>,
) {
    let source = main.iter().next().flatten();
    for (e, have) in &eyes {
        match (source, have) {
            (Some(want), Some(have)) if want == have => {}
            (Some(want), _) => {
                commands.entity(e).insert(want.clone());
            }
            (None, Some(_)) => {
                commands.entity(e).remove::<T>();
            }
            _ => {}
        }
    }
}

fn configure_eyes(
    mut commands: Commands,
    mut eyes: Query<(Entity, &XrCamera, &mut Projection), Without<EyeSky>>,
    root: Single<Entity, With<XrTrackingRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    commands
        .entity(*root)
        .insert(crate::level_switch::KeepAcrossLevels);
    for (e, eye, mut p) in &mut eyes {
        if let Projection::Custom(c) = &mut *p {
            if let Some(p) = c.get_mut::<XrProjection>() {
                p.near = 0.03;
            }
        }
        commands.entity(e).insert((
            GameEye,
            crate::level_switch::KeepAcrossLevels,
            crate::shadow_render::ShadowCamera,
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
            RenderLayers::from_layers(&[0, PANEL_LAYER]),
            Name::new(format!("ReRAC world eye {}", eye.0)),
        ));
        // Same XrCamera index makes the backend late-latch sky and world from the SAME predicted pose.
        commands.spawn((
            XrCamera(eye.0),
            EyeSky,
            Projection::custom(XrProjection {
                near: 0.03,
                ..default()
            }),
            crate::level_switch::KeepAcrossLevels,
            RenderLayers::layer(crate::sky_render::SKY_LAYER),
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
            Name::new(format!("ReRAC sky eye {}", eye.0)),
        ));
    }
    spawn_panels(&mut commands, *root, &mut meshes, &mut mats);
    println!("VR_GAME stereo attached to original world; hero, mobys, ties, shrubs, sky, water and audio use upstream plugins");
}
fn preview_eyes(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let root = commands
        .spawn((XrTrackingRoot, crate::level_switch::KeepAcrossLevels))
        .id();
    for (i, x) in [-0.032, 0.032].into_iter().enumerate() {
        let target = images.add(Image::new_target_texture(
            640,
            640,
            TextureFormat::Bgra8UnormSrgb,
            None,
        ));
        for sky in [true, false] {
            let mut e = commands.spawn((
                Camera3d::default(),
                Projection::custom(XrProjection {
                    near: 0.03,
                    projection_matrix: Mat4::perspective_infinite_reverse_rh(1.7, 1.0, 0.03),
                }),
                RenderTarget::Image(target.clone().into()),
                Transform::from_xyz(x, 1.6, 0.0),
                ChildOf(root),
                crate::level_switch::KeepAcrossLevels,
                Msaa::Off,
                Tonemapping::None,
                DebandDither::Disabled,
                Name::new(format!(
                    "ReRAC preview {} eye {i}",
                    if sky { "sky" } else { "world" }
                )),
            ));
            if sky {
                e.insert((EyeSky, RenderLayers::layer(crate::sky_render::SKY_LAYER)));
            } else {
                e.insert((
                    GameEye,
                    crate::shadow_render::ShadowCamera,
                    RenderLayers::from_layers(&[0, PANEL_LAYER]),
                ));
            }
        }
    }
    spawn_panels(&mut commands, root, &mut meshes, &mut mats);
    println!("VR_GAME desktop verification: original playable window plus two offscreen stereo views; no headset simulation");
}
fn spawn_panels(
    commands: &mut Commands,
    root: Entity,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
) {
    for kind in 0..3 {
        commands.spawn((
            Mesh3d(meshes.add(Rectangle::new(2.0, 1.5))),
            MeshMaterial3d(mats.add(StandardMaterial {
                unlit: true,
                alpha_mode: if kind == 0 {
                    AlphaMode::Opaque
                } else {
                    AlphaMode::Premultiplied
                },
                cull_mode: None,
                ..default()
            })),
            Transform::from_xyz(0.0, 1.6, -2.0 - kind as f32 * 0.001),
            ChildOf(root),
            Visibility::Hidden,
            RenderLayers::layer(PANEL_LAYER),
            Panel(kind),
            crate::level_switch::KeepAcrossLevels,
            Name::new(format!("Original ReRAC UI layer {kind}")),
        ));
    }
}
pub(super) fn pose(views: Option<&OxrViews>) -> Option<Transform> {
    let v = views?.first()?;
    let q = v.pose.orientation;
    let p = v.pose.position;
    let mut t = Transform::from_translation(Vec3::new(p.x, p.y, p.z))
        .with_rotation(Quat::from_xyzw(q.x, q.y, q.z, q.w));
    if let Some(v) = views?.get(1) {
        let p = v.pose.position;
        let q = v.pose.orientation;
        t.translation = (t.translation + Vec3::new(p.x, p.y, p.z)) * 0.5;
        t.rotation = t.rotation.slerp(Quat::from_xyzw(q.x, q.y, q.z, q.w), 0.5);
    }
    Some(t)
}
fn heading(rotation: Quat) -> f32 {
    let forward = rotation * Vec3::NEG_Z;
    (-forward.x).atan2(-forward.z)
}
fn rig_transform(base: Transform, center: Vec3, yaw: f32, scale: f32) -> Transform {
    // Camera orbit changes yaw; only physical head motion supplies pitch and roll.
    let rotation = Quat::from_rotation_y(heading(base.rotation) - yaw);
    Transform {
        translation: base.translation - rotation * (center * scale),
        rotation,
        scale: Vec3::splat(scale),
    }
}
fn follow_game_camera(
    settings: Res<Settings>,
    views: Option<Res<OxrViews>>,
    view: Option<Res<crate::play_camera::PlayView>>,
    focus: Option<Res<super::game_input::Focused>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut rig: ResMut<Rig>,
    mut roots: Query<&mut Transform, (With<XrTrackingRoot>, Without<crate::fly_cam::FlyCam>)>,
    mut main: Query<&mut Transform, (With<crate::fly_cam::FlyCam>, Without<XrTrackingRoot>)>,
) {
    let Some(view) = view else { return };
    // Startup poses while the headset is on the desk are not the player's neutral pose.
    if !settings.desktop_preview && !focus.as_ref().is_some_and(|f| f.0) {
        return;
    }
    let head = pose(views.as_deref()).or_else(|| {
        settings
            .desktop_preview
            .then(|| Transform::from_xyz(0.0, 1.6, 0.0))
    });
    let Some(head) = head else { return };
    let reset = keys.pressed(KeyCode::F8);
    if rig.center.is_none() || (reset && !rig.reset_held) {
        rig.center = Some(head.translation);
        rig.yaw = heading(head.rotation);
        println!("VR_GAME recentered from focused headset pose: center={:?} yaw={:.3}", head.translation, rig.yaw);
    }
    rig.reset_held = reset;
    let base = main
        .iter()
        .next()
        .map(|t| *t)
        .unwrap_or_else(|| crate::play_camera::view_transform(&view.view));
    let root = rig_transform(base, rig.center.unwrap(), rig.yaw, settings.units_per_metre);
    for mut r in &mut roots {
        *r = root;
    }
    let center = root.mul_transform(head);
    for mut t in &mut main {
        // Original camera projection stays intact for the window; projection_tans expands only culling.
        *t = center;
    }
}
fn sky_policy(
    mut cams: Query<(&mut Camera, &mut Camera3d, Has<EyeSky>, Has<GameEye>)>,
    level: Res<crate::Level>,
) {
    let has_sky = level.0.sky.is_some();
    for (mut c, mut d, sky, world) in &mut cams {
        if sky {
            c.order = 10;
            c.clear_color = ClearColorConfig::Default;
            c.output_mode = CameraOutputMode::Skip;
            d.depth_load_op = Camera3dDepthLoadOp::Clear(0.0);
        } else if world {
            c.order = 11;
            c.clear_color = if has_sky {
                ClearColorConfig::None
            } else {
                ClearColorConfig::Default
            };
            c.output_mode = CameraOutputMode::Write {
                blend_state: Some(BlendState::REPLACE),
                clear_color: ClearColorConfig::None,
            };
            d.depth_load_op = if has_sky {
                Camera3dDepthLoadOp::Load
            } else {
                Camera3dDepthLoadOp::Clear(0.0)
            };
        }
    }
}
fn update_panels(
    views: Option<Res<OxrViews>>,
    settings: Res<Settings>,
    frame: Res<crate::display::GameFrame>,
    mode: Res<crate::menu_render::MenuMode>,
    hud: Query<
        &MaterialNode<crate::hud_render::HudComposite>,
        With<crate::hud_render::HudCompositeNode>,
    >,
    composites: Res<Assets<crate::hud_render::HudComposite>>,
    static_nodes: Query<&MaterialNode<crate::hud_render::HudStaticComposite>>,
    static_mats: Res<Assets<crate::hud_render::HudStaticComposite>>,
    mut panels: Query<(
        &Panel,
        &mut Transform,
        &mut Visibility,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    use rc_game::menus::mode::Mode;
    let menu = matches!(
        mode.state.mode,
        Mode::Menu | Mode::Freeze | Mode::Vendor | Mode::Movie | Mode::Slideshow | Mode::Ship
    );
    let head = pose(views.as_deref()).unwrap_or_else(|| Transform::from_xyz(0.0, 1.6, 0.0));
    for (panel, mut t, mut visibility, material) in &mut panels {
        let image = match panel.0 {
            0 if menu => Some(frame.image.clone()),
            1 if !menu => hud
                .iter()
                .next()
                .and_then(|h| composites.get(&h.0))
                .map(|m| m.image.clone()),
            2 if !menu => static_nodes
                .iter()
                .next()
                .and_then(|h| static_mats.get(&h.0))
                .map(|m| m.image.clone()),
            _ => None,
        };
        *visibility = if image.is_some() && !settings.desktop_preview {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if let Some(image) = image {
            if let Some(mut m) = mats.get_mut(&material.0) {
                m.base_color_texture = Some(image);
            }
        }
        *t = head.mul_transform(Transform::from_xyz(0.0, 0.0, -2.0 - panel.0 as f32 * 0.002));
    }
}
fn audit(
    settings: Res<Settings>,
    mut audit: ResMut<Audit>,
    rendered: Res<Rendered>,
    draw: Res<DrawAudit>,
    play: Option<Res<crate::gameplay::Play>>,
    pad: Option<Res<GamePad>>,
    xr_views: Option<Res<OxrViews>>,
    eyes: Query<Entity, With<GameEye>>,
    sky: Query<Entity, With<EyeSky>>,
    meshes: Query<Entity, With<Mesh3d>>,
    ratchet: Query<(&Visibility, &InheritedVisibility), With<crate::gameplay::RatchetMesh>>,
    main: Query<&Transform, With<crate::fly_cam::FlyCam>>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = std::time::Instant::now();
    let elapsed = now.duration_since(audit.start).as_secs_f32();
    let tick = play.as_ref().map_or(0, |p| p.game.counter);
    if tick > 0 && audit.first_tick.is_none() {
        audit.first_tick = Some(tick);
    }
    if now.duration_since(audit.last).as_secs_f32() > 5.0 {
        println!("VR_GAME live level={} ticks={} hero={:?} hero_state={:?} mesh_entities={} ratchet_meshes={} world_eyes={} sky_eyes={} render_frames={} min_world_draws={} min_sky_draws={} waiting_pipelines={} controls_active={} head_views={}",
            crate::level_load::level_index(),tick,play.as_ref().map(|p|p.game.hero.position()),play.as_ref().map(|p|p.game.hero.state),
            meshes.iter().count(),ratchet.iter().count(),eyes.iter().count(),sky.iter().count(),rendered.0.load(Ordering::Relaxed),
            draw.world.load(Ordering::Relaxed),draw.sky.load(Ordering::Relaxed),draw.waiting.load(Ordering::Relaxed),
            pad.as_ref().is_some_and(|p|p.active),xr_views.as_ref().map_or(0,|v|v.len()));
        if let (Some(play), Some(camera)) = (play.as_ref(), main.iter().next()) {
            let hero = crate::tfrag_render::game_to_bevy(play.game.hero.position()) + Vec3::Y;
            let local = camera.compute_affine().inverse().transform_point3(hero);
            println!("VR_GAME hero_view visible_meshes={} local={local:.3?} distance={:.3} camera_class={} first_person={}",
                ratchet.iter().filter(|(v, inherited)| **v != Visibility::Hidden && inherited.get()).count(),
                camera.translation.distance(hero), play.game.camera.out.class, play.game.camera.first_person_flag());
        }
        audit.last = now;
    }
    audit.last_tick = tick;
    if let Some(seconds) = settings.preview_seconds {
        if elapsed >= seconds
            && rendered.0.load(Ordering::Relaxed) >= 30
            && draw.waiting.load(Ordering::Relaxed) == 0
            && draw.world.load(Ordering::Relaxed) > 0
            && draw.sky.load(Ordering::Relaxed) > 0
        {
            let ok = eyes.iter().count() == 2
                && sky.iter().count() == 2
                && !ratchet.is_empty()
                && tick > audit.first_tick.unwrap_or(tick) + 10;
            println!("VR_GAME ORIGINAL_GAME_SMOKE_{} ticks={tick} ratchet_meshes={} mesh_entities={} stereo_world_views={} sky_views={} render_frames={} min_world_draws={} min_sky_draws={}",
                if ok {"PASS"}else{"FAIL"},ratchet.iter().count(),meshes.iter().count(),eyes.iter().count(),sky.iter().count(),rendered.0.load(Ordering::Relaxed),draw.world.load(Ordering::Relaxed),draw.sky.load(Ordering::Relaxed));
            exit.write(if ok {
                AppExit::Success
            } else {
                AppExit::error()
            });
        } else if elapsed > seconds + 120.0 {
            error!("VR_GAME original game did not render or tick");
            exit.write(AppExit::error());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recenter_places_head_on_follow_camera_and_preserves_stereo_separation() {
        let base =
            Transform::from_xyz(162.0, 62.0, -136.0).with_rotation(Quat::from_rotation_y(1.2));
        let head = Transform::from_xyz(0.5, 1.1, -0.4);
        let root = rig_transform(base, head.translation, 0.0, 1.0);
        assert!(
            root.mul_transform(head)
                .translation
                .distance(base.translation)
                < 1e-4
        );
        let l = root.mul_transform(head.with_translation(head.translation - Vec3::X * 0.032));
        let r = root.mul_transform(head.with_translation(head.translation + Vec3::X * 0.032));
        assert!((l.translation.distance(r.translation) - 0.064).abs() < 1e-4);
    }
}
