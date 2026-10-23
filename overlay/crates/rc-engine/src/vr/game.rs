//! Stereo views of the ORIGINAL ReRAC world and its live game, including level changes.
//! No level loader, entity renderer, hero physics or weapon implementation is duplicated here.
use super::{
    Settings,
    game_input::{GameInputPlugin, GamePad},
};
use bevy::{
    app::PluginGroupBuilder,
    camera::{
        Camera3dDepthLoadOp, CameraOutputMode, ClearColorConfig, RenderTarget,
        visibility::RenderLayers,
    },
    core_pipeline::{Core2d, Core3d, core_3d::Transparent3d},
    ecs::schedule::SingleThreadedExecutor,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        pipelined_rendering::PipelinedRenderingPlugin,
        render_phase::ViewSortedRenderPhases,
        render_resource::{BlendState, PipelineCache, TextureFormat},
        view::{ExtractedView, Msaa},
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
    Arc,
    atomic::{AtomicU64, Ordering},
};

pub(super) const PANEL_LAYER: usize = 62;
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
    world_yaw: f32,
    turn: f32,
    head: Option<Vec3>,
    previous_head: Option<Vec3>,
    snap_held: bool,
    pub(super) revision: u64,
    reset_held: bool,
    eye_height: Option<f32>,
    gravity_frame: Quat,
    front_root: Option<Transform>,
    rail_heading: Option<f32>,
    rail_center: Option<Vec3>,
    rail_pitch: f32,
    rail_time: Option<f64>,
}
pub(super) fn native_panel(play: &crate::gameplay::Play) -> bool {
    let gold_bolt = play
        .game
        .mobys
        .mobys
        .iter()
        .any(|m| m.o_class == 1134 && m.state == 2);
    play.game.camera.out.class == 5
        && play.game.hero.items.slot.id == 26
        && play.game.hero.state == 0x72
        && !gold_bolt
}
impl Rig {
    pub(super) fn surface_up(&self) -> Vec3 {
        self.gravity_frame * Vec3::Y
    }
    pub(super) fn snap(&mut self, x: f32) {
        if x.abs() < 0.25 {
            self.snap_held = false;
        }
        if x.abs() > 0.7 && !self.snap_held && self.center.is_some() {
            self.turn -= x.signum() * std::f32::consts::FRAC_PI_6;
            self.snap_held = true;
            self.revision = self.revision.wrapping_add(1);
        }
    }
    pub(super) fn root_for(&self, game: &rc_game::tick::Game, scale: f32) -> Option<Transform> {
        if crate::saves::front_end_active() {
            return self.front_root;
        }
        let mut center = self.center?;
        if let Some(head) = self.rail_center.or(self.head) {
            center.x = head.x;
            center.z = head.z;
        }
        let rotation = self.gravity_frame
            * Quat::from_rotation_y(self.world_yaw + self.turn - self.yaw)
            * Quat::from_rotation_x(self.rail_pitch);
        let eye = crate::tfrag_render::game_to_bevy(game.hero.position())
            + self.gravity_frame * Vec3::Y * self.eye_height.unwrap_or(1.6);
        // Room-scale displacement has already been consumed by the native capsule.
        Some(Transform {
            translation: eye - rotation * (center * scale),
            rotation,
            scale: Vec3::splat(scale),
        })
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
        // A launch preset is authoritative: a previous debug shell cannot grant items to Normal.
        for key in [
            "RC_GIVE_ITEMS",
            "RC_GIVE_ITEMS_EQUIP",
            "RC_GIVE_HYDROPACK",
            "RC_GIVE_BOLTS",
            "RC_GIVE_GOLD_BOLTS",
            "RC_GIVE_GOLD_WEAPONS",
            "RC_UNLOCK_PLANETS",
            "RC_GAME_BEATEN",
            "RC_SKIP_LOGOS",
            "RC_VR_SPAWN_GAME",
        ] {
            if !s.mode.debug() || key != "RC_VR_SPAWN_GAME" {
                std::env::remove_var(key);
            }
        }
        std::env::set_var("RC_PLAY", "1");
        std::env::set_var("RC_PLAY_FLY", "0");
        std::env::set_var("RC_MSAA", "0");
        std::env::set_var("RC_FRONTEND", if s.mode.debug() { "0" } else { "1" });
        if s.mode.debug() {
            std::env::set_var("RC_LEVEL", if s.scene == "kerwan" { "3" } else { "1" });
            std::env::set_var("RC_SCENE", "0");
            let planets = (0..rc_game::travel::LEVEL_COUNT)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",");
            std::env::set_var("RC_UNLOCK_PLANETS", planets);
            if s.mode == super::LaunchMode::DebugGold {
                let gold = rc_game::moby_update::classes::item_offer::GOLD_ITEMS
                    .map(|id| id.to_string())
                    .join(",");
                std::env::set_var("RC_GIVE_GOLD_WEAPONS", gold);
            }
        } else {
            std::env::remove_var("RC_LEVEL");
            std::env::remove_var("RC_SCENE");
        }
        if std::env::var_os("RC_SAVE_DIR").is_none() {
            std::env::set_var(
                "RC_SAVE_DIR",
                std::path::Path::new("work/vr/saves").join(s.mode.name()),
            );
        }
    }
    println!(
        "VR_GAME launch_mode={} frontend={} save_dir={} gold_items={}",
        s.mode.name(),
        !s.mode.debug(),
        std::env::var("RC_SAVE_DIR").unwrap_or_default(),
        std::env::var("RC_GIVE_GOLD_WEAPONS").unwrap_or_default()
    );

    println!(
        "VR_GAME original ReRAC application; scene={} preview={} simulation=60Hz first-person",
        s.scene, s.desktop_preview
    );
    println!(
        "VR_GAME first person: left stick move; right stick snap turn 30 degrees; A jump/double jump; right grip hold wrench, swing to hit, release a moving hand to throw; Y interact or hold and move left hand for quick select; right trigger fires the selected Blaster/Pyrocitor or prepares a bomb for gesture release; Ratchet brown gloves; F8 recenter"
    );
    Ok(())
}
pub fn game_plugins(plugins: PluginGroupBuilder) -> PluginGroupBuilder {
    if !super::game_requested() {
        return plugins;
    }
    // Pipelined rendering (Bevy's default, kept by the desktop game) overlaps the next frame's
    // simulation with this frame's render and its xrEndFrame GPU wait; bevy_mod_openxr then
    // predicts poses one display period further. RC_VR_PIPELINED=0 restores the serial loop.
    let plugins = if std::env::var("RC_VR_PIPELINED").is_ok_and(|s| s.trim() == "0") {
        plugins.disable::<PipelinedRenderingPlugin>()
    } else {
        plugins
    };
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
    // 0 (default): the window's world view stays frozen during VR gameplay (SteamVR's VR View
    // mirrors the headset). Each re-activation of a window camera rebuilds its specializations and
    // render phases from scratch, which cost a quarter of the render thread at a cadence of 4.
    let mirror_every = std::env::var("RC_VR_MIRROR_EVERY")
        .unwrap_or_else(|_| "0".into())
        .parse::<u32>()?;
    anyhow::ensure!(
        (0..=120).contains(&mirror_every),
        "RC_VR_MIRROR_EVERY must be 0..=120"
    );
    let frames = Rendered::default();
    let draw = DrawAudit::default();
    let now = std::time::Instant::now();
    app.insert_resource(settings.clone())
        .init_resource::<Rig>()
        .add_plugins(super::embodied::EmbodiedPlugin)
        .add_plugins(super::vendor::VendorPlugin)
        .add_plugins(super::spatial_menu::SpatialMenuPlugin)
        .add_plugins(super::menu_pointer::MenuPointerPlugin)
        .add_plugins(super::weapon_screen::WeaponScreenPlugin)
        .insert_resource(frames.clone())
        .insert_resource(draw.clone())
        .insert_resource(MonitorCadence {
            // The preview shows the window every frame unless RC_VR_MIRROR_EVERY asks for the
            // headset cadence (matched measurements).
            every: if settings.desktop_preview && std::env::var_os("RC_VR_MIRROR_EVERY").is_none() {
                1
            } else {
                mirror_every
            },
            frame: 0,
            restore: Vec::new(),
        })
        .add_systems(First, restore_monitor_cameras)
        .add_systems(
            PreUpdate,
            (align_body, follow_game_camera)
                .chain()
                .after(bevy_mod_xr::spaces::XrSpaceSyncSet)
                .before(super::game_input::sample),
        )
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
            (follow_game_camera, super::bomb_glove::follow_held)
                .chain()
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
            ),
        );
    app.sub_app_mut(RenderApp)
        .insert_resource(frames)
        .insert_resource(draw)
        .add_systems(Render, count.after(RenderSystems::Render));
    // Each camera's pipeline (Core3d/Core2d) is a chain of passes: Bevy's multithreaded executor made
    // the render thread wait for a worker per pass and camera (about 0.7 ms a frame). The passes still
    // encode in parallel internally. RC_VR_CAMERA_EXECUTOR=multi restores Bevy's executor.
    if !std::env::var("RC_VR_CAMERA_EXECUTOR").is_ok_and(|s| s.trim() == "multi") {
        let render = app.sub_app_mut(RenderApp);
        render.edit_schedule(Core3d, |s| {
            s.set_executor(SingleThreadedExecutor::new());
        });
        render.edit_schedule(Core2d, |s| {
            s.set_executor(SingleThreadedExecutor::new());
        });
    }
    app.add_plugins(super::fullscreen_refresh::FullscreenRefreshPlugin)
        .add_plugins(super::timing::FrameTimingPlugin {
            xr: !settings.desktop_preview,
        });
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
            .after(crate::flight_render::FlightSkySet)
            .after(crate::title_world::TitleWorldViewSet)
            .after(update_views)
            .before(TransformSystems::Propagate),
    );
    Ok(())
}
// The monitor is an extra full world render. Refresh it every `every` frames during VR gameplay
// (never at 0); the headset and simulation still update every frame. Menus retain their original
// full frame.
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
    play: Option<Res<crate::gameplay::Play>>,
    mut cams: Query<
        (Entity, &mut Camera),
        Or<(
            With<crate::fly_cam::FlyCam>,
            With<crate::sky_render::SkyCamera>,
        )>,
    >,
) {
    cadence.frame = cadence.frame.wrapping_add(1);
    if mode.state.mode != rc_game::menus::mode::Mode::Gameplay
        || play
            .as_deref()
            .is_some_and(|p| native_panel(p) || crate::visibomb_view::missile_view(Some(p)))
        || (cadence.every > 0 && cadence.frame % cadence.every == 0)
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
    println!(
        "VR_GAME stereo attached to original world; hero, mobys, ties, shrubs, sky, water and audio use upstream plugins"
    );
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
    // RC_VR_PREVIEW_SIZE: eye size in pixels (default 640; the headset's 1920 for GPU-load runs).
    let size = std::env::var("RC_VR_PREVIEW_SIZE")
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .filter(|s| (64..=4096).contains(s))
        .unwrap_or(640);
    for (i, x) in [-0.032, 0.032].into_iter().enumerate() {
        let target = images.add(Image::new_target_texture(
            size,
            size,
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
    println!(
        "VR_GAME desktop verification: original playable window plus two offscreen stereo views; no headset simulation"
    );
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
    // Only snap turning changes virtual yaw; physical head motion supplies pitch and roll.
    let rotation = Quat::from_rotation_y(heading(base.rotation) - yaw);
    Transform {
        translation: base.translation - rotation * (center * scale),
        rotation,
        scale: Vec3::splat(scale),
    }
}
fn align_body(
    settings: Res<Settings>,
    views: Option<Res<OxrViews>>,
    focus: Option<Res<super::game_input::Focused>>,
    mode: Res<crate::menu_render::MenuMode>,
    play: Option<ResMut<crate::gameplay::Play>>,
    level: Res<crate::Level>,
    mut rig: ResMut<Rig>,
) {
    if !settings.desktop_preview && !focus.as_ref().is_some_and(|f| f.0) {
        rig.previous_head = None;
        return;
    }
    let Some(head) = pose(views.as_deref()) else {
        return;
    };
    rig.head = Some(head.translation);
    if rig.center.is_none() || !mode.free_movement() {
        rig.previous_head = None;
        return;
    }
    if play.as_ref().is_some_and(|p| p.game.hero.group == 0xf) {
        // Lean around the rail attachment; room-scale walking cannot push the capsule off its spline.
        rig.previous_head = None;
        return;
    }
    let previous = rig
        .previous_head
        .replace(head.translation)
        .or(rig.rail_center);
    let Some(previous) = previous else {
        // Rebase after focus/menu changes instead of moving the avatar from an old desk/room pose.
        if let Some(center) = rig.center.as_mut() {
            center.y = head.translation.y;
        }
        rig.revision = rig.revision.wrapping_add(1);
        return;
    };
    let mut delta = head.translation - previous;
    delta.y = 0.0;
    if delta.length() > 0.5 {
        rig.revision = rig.revision.wrapping_add(1);
        return;
    }
    if delta.length_squared() < 1e-10 {
        return;
    }
    let (Some(mut play), Some(coll)) = (play, level.0.collision.as_ref()) else {
        return;
    };
    if play.death_pending || play.game.hero.health <= 0 {
        return;
    }
    let rotation = rig.gravity_frame * Quat::from_rotation_y(rig.world_yaw + rig.turn - rig.yaw);
    let delta = rotation * delta * settings.units_per_metre;
    play.roomscale_move(coll, super::embodied::game_point(delta));
}

fn follow_game_camera(
    settings: Res<Settings>,
    views: Option<Res<OxrViews>>,
    play: Option<Res<crate::gameplay::Play>>,
    mode: Res<crate::menu_render::MenuMode>,
    focus: Option<Res<super::game_input::Focused>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut rig: ResMut<Rig>,
    mut roots: Query<&mut Transform, (With<XrTrackingRoot>, Without<crate::fly_cam::FlyCam>)>,
    mut main: Query<&mut Transform, (With<crate::fly_cam::FlyCam>, Without<XrTrackingRoot>)>,
    time: Res<Time<Real>>,
    travel: Res<crate::travel_render::Travel>,
) {
    let Some(play) = play else { return };
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
        rig.world_yaw = play.game.hero.rot[2].to_f32() - std::f32::consts::FRAC_PI_2;
        rig.turn = 0.0;
        rig.rail_heading = None;
        rig.rail_center = None;
        rig.rail_pitch = 0.0;
        rig.head = Some(head.translation);
        rig.previous_head = Some(head.translation);
        rig.revision = rig.revision.wrapping_add(1);
        println!(
            "VR_GAME recentered from focused headset pose: center={:?} yaw={:.3}",
            head.translation, rig.yaw
        );
    }
    rig.reset_held = reset;
    if crate::saves::front_end_active() {
        // The title is its own native scene, far from the parked gameplay hero.
        // Its authored camera supplies the VR origin, while head tracking owns view rotation.
        if let Some(base) = main.iter().next().copied() {
            let root = rig_transform(
                base,
                rig.center.unwrap_or(head.translation),
                rig.yaw,
                settings.units_per_metre,
            );
            rig.front_root = Some(root);
            for mut r in &mut roots {
                *r = root;
            }
        }
        return;
    }
    rig.front_root = None;
    let boots = &play.game.hero.boots;
    let on_rail = play.game.hero.group == 0xf && boots.rail.is_some() && boots.off_rail == 0;
    if on_rail {
        let rail = boots.yaw - std::f32::consts::FRAC_PI_2;
        if let Some(previous) = rig.rail_heading {
            let change = (rail - previous).sin().atan2((rail - previous).cos());
            rig.world_yaw += change;
        } else {
            // Begin facing along travel, then retain all subsequent physical look-around.
            rig.world_yaw = rail - heading(head.rotation) + rig.yaw - rig.turn;
            rig.rail_center = Some(head.translation);
        }
        rig.rail_heading = Some(rail);
    } else {
        rig.rail_heading = None;
        rig.rail_center = None;
    }
    let target_pitch = if on_rail { boots.pitch } else { 0.0 };
    let now = time.elapsed_secs_f64();
    let dt = rig
        .rail_time
        .replace(now)
        .map_or(0.0, |old| (now - old).clamp(0.0, 0.05) as f32);
    rig.rail_pitch += (target_pitch - rig.rail_pitch).clamp(-dt * 1.8, dt * 1.8);
    let up = if play.game.hero.gravity_mode != 0 {
        crate::tfrag_render::game_to_bevy(
            [0, 1, 2].map(|i| play.game.hero.moby_rows[2][i].to_f32()),
        )
        .normalize_or_zero()
    } else {
        Vec3::Y
    };
    if up.length_squared() > 0.9 {
        let old_up = rig.gravity_frame * Vec3::Y;
        rig.gravity_frame = (Quat::from_rotation_arc(old_up, up) * rig.gravity_frame).normalize();
    }
    // Surface swimming uses the native horizontal swimmer's eye height. Blend the posture change;
    // head rotation always remains physically tracked.
    let target_height = if matches!(play.game.hero.group, 0x11 | 0x12) {
        0.45
    } else if play.game.hero.state == 4
        || (play.game.hero.state == 0x73 && play.game.pad.held & rc_game::pad::button::CROUCH != 0)
    {
        0.85
    } else {
        1.6
    };
    let height = rig.eye_height.unwrap_or(target_height);
    rig.eye_height = Some(
        height + (target_height - height).clamp(-time.delta_secs() * 2.0, time.delta_secs() * 2.0),
    );
    // The flight is drawn on a separate world layer, about its authored camera, not about the
    // hero left on the departure planet. Keep head-driven pitch/roll and show the actual voyage.
    let root = if travel.blanked() {
        main.iter().next().map(|native| {
            rig_transform(
                *native,
                rig.center.unwrap_or(head.translation),
                rig.yaw,
                settings.units_per_metre,
            )
        })
    } else {
        rig.root_for(&play.game, settings.units_per_metre)
    };
    let Some(root) = root else {
        return;
    };
    for mut r in &mut roots {
        *r = root;
    }
    let center = root.mul_transform(head);
    if mode.free_movement()
        && !native_panel(&play)
        && !crate::visibomb_view::missile_view(Some(&play))
    {
        for mut t in &mut main {
            // Keep authored camera shots in the original menu/cinematic panel.
            *t = center;
        }
    }
}
fn sky_policy(
    play: Option<Res<crate::gameplay::Play>>,
    mut cams: Query<
        (&mut Camera, &mut Camera3d, &mut RenderLayers, Has<EyeSky>),
        Or<(With<EyeSky>, With<GameEye>)>,
    >,
    main: Query<
        (&Camera, Option<&RenderLayers>),
        (
            With<crate::fly_cam::FlyCam>,
            Without<EyeSky>,
            Without<GameEye>,
        ),
    >,
    native_sky: Query<
        &RenderLayers,
        (
            With<crate::sky_render::SkyCamera>,
            Without<EyeSky>,
            Without<GameEye>,
        ),
    >,
) {
    let sky_layers = native_sky
        .iter()
        .next()
        .cloned()
        .unwrap_or_else(RenderLayers::none);
    let (world_clear, world_layers) = main
        .iter()
        .next()
        .map(|(c, layers)| (c.clear_color.clone(), layers.cloned().unwrap_or_default()))
        .unwrap_or((ClearColorConfig::Default, RenderLayers::default()));
    for (mut c, mut d, mut layers, sky) in &mut cams {
        if sky {
            *layers = if play
                .as_deref()
                .is_some_and(|p| crate::visibomb_view::missile_view(Some(p)))
            {
                RenderLayers::layer(1)
            } else {
                sky_layers.clone()
            };
            c.order = 10;
            c.clear_color = ClearColorConfig::Default;
            c.output_mode = CameraOutputMode::Skip;
            d.depth_load_op = Camera3dDepthLoadOp::Clear(0.0);
        } else {
            *layers = world_layers.clone().with(PANEL_LAYER);
            c.order = 11;
            c.clear_color = world_clear.clone();
            c.output_mode = CameraOutputMode::Write {
                blend_state: Some(BlendState::REPLACE),
                clear_color: ClearColorConfig::None,
            };
            // The kept XR sky eye always initializes the shared depth, even for a blank sky layer.
            d.depth_load_op = Camera3dDepthLoadOp::Load;
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
    play: Option<Res<crate::gameplay::Play>>,
) {
    use rc_game::menus::mode::Mode;
    // Ship/engine movies already exist in the stereo world. Only actual menus, flat video and
    // gameplay gadget views need the composed frame; a cinematic camera class must not enable it.
    let menu = match mode.state.mode {
        Mode::Gameplay => play.as_deref().is_some_and(native_panel),
        Mode::Menu | Mode::Freeze | Mode::Vendor | Mode::Movie | Mode::Slideshow => true,
        _ => false,
    };
    let head = pose(views.as_deref()).unwrap_or_else(|| Transform::from_xyz(0.0, 1.6, 0.0));
    let spatial_pause = matches!(mode.state.mode, Mode::Menu | Mode::Freeze);
    let missile = play
        .as_deref()
        .is_some_and(|p| crate::visibomb_view::missile_view(Some(p)));
    for (panel, mut t, mut visibility, material) in &mut panels {
        if spatial_pause || missile || (mode.spatial_vendor && mode.state.mode == Mode::Vendor) {
            *visibility = Visibility::Hidden;
            continue;
        }
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
        // Composite order toward the viewer: frame, HUD, static.
        *t = head.mul_transform(Transform::from_xyz(0.0, 0.0, -2.0 + panel.0 as f32 * 0.002));
    }
}
fn audit(
    settings: Res<Settings>,
    rig: Res<Rig>,
    focus: Option<Res<super::game_input::Focused>>,
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
        println!(
            "VR_GAME live level={} ticks={} hero={:?} hero_state={:?} mesh_entities={} ratchet_meshes={} world_eyes={} sky_eyes={} render_frames={} min_world_draws={} min_sky_draws={} waiting_pipelines={} controls_active={} head_views={}",
            crate::level_load::level_index(),
            tick,
            play.as_ref().map(|p| p.game.hero.position()),
            play.as_ref().map(|p| p.game.hero.state),
            meshes.iter().count(),
            ratchet.iter().count(),
            eyes.iter().count(),
            sky.iter().count(),
            rendered.0.load(Ordering::Relaxed),
            draw.world.load(Ordering::Relaxed),
            draw.sky.load(Ordering::Relaxed),
            draw.waiting.load(Ordering::Relaxed),
            pad.as_ref().is_some_and(|p| p.active),
            xr_views.as_ref().map_or(0, |v| v.len())
        );
        if let (Some(play), Some(camera)) = (play.as_ref(), main.iter().next()) {
            if let (Some(root), Some(head)) = (
                rig.root_for(&play.game, settings.units_per_metre),
                pose(xr_views.as_deref()),
            ) {
                let eye = root.mul_transform(head).translation;
                let body = crate::tfrag_render::game_to_bevy(play.game.hero.position());
                let horizontal = Vec2::new(eye.x - body.x, eye.z - body.z).length();
                println!(
                    "VR_GAME body_alignment horizontal_error={horizontal:.5} eye_height={:.3} focused={}",
                    eye.y - body.y,
                    focus.as_ref().is_some_and(|f| f.0)
                );
            }
            let hero = crate::tfrag_render::game_to_bevy(play.game.hero.position()) + Vec3::Y;
            let local = camera.compute_affine().inverse().transform_point3(hero);
            println!(
                "VR_GAME hero_view visible_meshes={} local={local:.3?} distance={:.3} camera_class={} first_person=true native_camera_first_person={}",
                ratchet
                    .iter()
                    .filter(|(v, inherited)| **v != Visibility::Hidden && inherited.get())
                    .count(),
                camera.translation.distance(hero),
                play.game.camera.out.class,
                play.game.camera.first_person_flag()
            );
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
            println!(
                "VR_GAME ORIGINAL_GAME_SMOKE_{} ticks={tick} ratchet_meshes={} mesh_entities={} stereo_world_views={} sky_views={} render_frames={} min_world_draws={} min_sky_draws={}",
                if ok { "PASS" } else { "FAIL" },
                ratchet.iter().count(),
                meshes.iter().count(),
                eyes.iter().count(),
                sky.iter().count(),
                rendered.0.load(Ordering::Relaxed),
                draw.world.load(Ordering::Relaxed),
                draw.sky.load(Ordering::Relaxed)
            );
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
