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
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    core_pipeline::{Core2d, Core3d, core_3d::Transparent3d},
    ecs::schedule::SingleThreadedExecutor,
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
/// The eye heights above the feet (rc_game::hero::tracked), and Giant Clank's original first-person eye.
pub(super) use rc_game::hero::tracked::{CLANK_EYE, RATCHET_EYE};
pub(super) const GIANT_CLANK_EYE: f32 = 9.5;
/// Clank's ledge climb jumps 1.55 from the hang, its apex 0.85 above the lip (Ratchet's: 0.6): the eye rises at most
/// this far above the lip while the body keeps the native arc.
const CLANK_CLIMB_RISE: f32 = 0.3;
/// The flown ships (Pokitaru's jet, Gemlik's ship, the fleet's ship: one fighter model) carry the eye just above the
/// model's canopy, at the class scale: its glass texels (opaque, the hull drawn two-sided around them) span 0.08 back
/// to 1.83 forward and 0.48 to 0.96 up; the eye over its rear third, 0.34 above its top, clear of the tail fins (1.18
/// high, 1.5 back).
const SHIP_SEAT: [f32; 3] = [0.8, 0.0, 1.3];

/// Ratchet flying a ship (hero state 0x32, the vehicle record naming one of the flown ships): the eye in its canopy,
/// the view along its heading and pitch (its roll is left to the head), and the tracking scale of its flight size
/// (the ships fly at a quarter of their class scale).
pub(super) fn ship_view(p: &crate::gameplay::Play) -> Option<(Vec3, Quat, f32)> {
    use rc_game::moby_update::{
        ClassData,
        classes::units::{fleet_ship, gemlik_ship, pokitaru_jet},
    };
    let h = &p.game.hero;
    if h.mode != 0 || h.state != 0x32 {
        return None;
    }
    let m = p.game.mobys.mobys.get(p.svc.vehicle.moby?)?;
    if ![pokitaru_jet::CLASS, gemlik_ship::CLASS, fleet_ship::CLASS].contains(&m.o_class) {
        return None;
    }
    let class_scale = p.classes.info(m.o_class).map(|c| c.scale).filter(|s| *s > 0.0)?;
    let ratio = m.scale / class_scale;
    let axis = |i: usize| Vec3::new(m.rows[i][0], m.rows[i][1], m.rows[i][2]).normalize_or_zero();
    let (forward, left, up) = (axis(0), axis(1), axis(2));
    let seat = forward * SHIP_SEAT[0] + left * SHIP_SEAT[1] + up * SHIP_SEAT[2];
    let eye = Vec3::new(m.position[0], m.position[1], m.position[2]) + seat * ratio;
    let forward = crate::tfrag_render::game_to_bevy(forward.to_array());
    (forward.length_squared() > 0.5 && ratio.is_finite() && ratio > 0.0).then(|| {
        let rotation = Transform::IDENTITY.looking_to(forward, Vec3::Y).rotation;
        (crate::tfrag_render::game_to_bevy(eye.to_array()), rotation, ratio)
    })
}

/// How fast the eye height follows a posture change (metres per second).
const EYE_BLEND: f32 = 4.0;
#[derive(Component, Clone, ExtractComponent)]
pub struct GameEye;
#[derive(Component, Clone, ExtractComponent)]
struct EyeSky;
#[derive(Component)]
struct DestinationWarmup;
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
    /// How far the eye stays below the body during Clank's ledge climb (metres, see [`CLANK_CLIMB_RISE`]).
    climb_drop: f32,
    /// The hero's body (`Hero::mode`: 0 Ratchet, 1 Clank, 2 Giant Clank, 3 the Hologuise disguise) the eye and scale
    /// were set for.
    body: u8,
    gravity_frame: Quat,
    front_root: Option<Transform>,
    cinematic_root: Option<Transform>,
    /// Where the head was in the room when the cinematic (or pilot) view began: it sees from the actor's eye there.
    cinematic_head: Option<Vec3>,
    /// The scene's view heading, set when it starts: the head's heading then becomes the actor's gaze.
    cinematic_yaw: Option<f32>,
    last_live_eye: Option<Vec3>,
    eye_level: u32,
    pilot_rotation: Option<Quat>,
    rail_heading: Option<f32>,
    rail_center: Option<Vec3>,
    rail_pitch: f32,
    rail_time: Option<f64>,
    rail_position: Option<Vec3>,
    /// The last two ticks' hero positions on a rail, and the eye's offset from the last one (the overstep's prediction,
    /// fading out after the rail).
    rail_samples: Option<[(u64, Vec3); 2]>,
    rail_offset: Vec3,
    /// The camera flip (VrOptions::camera_flip): Ratchet's head joint when the flip began, the last two ticks' turns
    /// since then, the fade weight and this frame's turn of the view.
    flip_start: Option<Quat>,
    flip_samples: Option<(u64, Quat, Quat)>,
    flip_weight: f32,
    flip: Quat,
    /// The hero's group and, on the hoverboard, his position of the last frame (its start and respawns teleport him).
    hero_group: i32,
    board_pos: Option<Vec3>,
    /// On the hoverboard, the head's and the neck's room positions when the board was placed (or the view recentred):
    /// the view stays anchored at the head's, as on a rail, and the lean is the neck's sideways offset from its own
    /// ([`Rig::board_lean`]).
    board_center: Option<(Vec3, Vec3)>,
}
pub(crate) fn native_panel(play: &crate::gameplay::Play) -> bool {
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
    /// On the hoverboard, the player's lean (−1 left .. 1 right, 0 without lean steering) from the head's room pose:
    /// the neck's offset from where it was when the board was placed, along the room's side axis (the board's, since
    /// the view follows the board).
    pub(super) fn board_lean(&self, head: &Transform, lean: bool) -> Option<f32> {
        let (_, center) = self.board_center?;
        if !lean {
            return Some(0.0);
        }
        let side = Quat::from_rotation_y(self.yaw) * Vec3::X;
        let offset = (neck(head) - center).dot(side);
        Some(offset.signum() * ((offset.abs() - BOARD_LEAN_DEAD) / (BOARD_LEAN_FULL - BOARD_LEAN_DEAD)).clamp(0.0, 1.0))
    }
    pub(super) fn cinematic_active(&self) -> bool {
        self.cinematic_root.is_some()
    }
    pub(super) fn surface_up(&self) -> Vec3 {
        self.gravity_frame * Vec3::Y
    }
    /// While snap turning is blocked, follow the stick without turning: a stick still pushed when it unblocks (quick
    /// select paging, the shop, a menu) must return to centre before it turns.
    pub(super) fn hold_snap(&mut self, x: f32) {
        self.snap_held = x.abs() >= 0.25;
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
    /// [`Self::root_for`] turned about the head by the camera flip: for drawing only (the aim, the hands and the
    /// walking direction keep the upright root).
    pub(super) fn view_root_for(&self, game: &rc_game::tick::Game, scale: f32, head: Transform) -> Option<Transform> {
        let root = self.root_for(game, scale)?;
        if self.cinematic_root.is_some() || crate::saves::front_end_active() || self.flip == Quat::IDENTITY {
            return Some(root);
        }
        let eye = root.transform_point(head.translation);
        Some(Transform { translation: eye + self.flip * (root.translation - eye), rotation: self.flip * root.rotation, scale: root.scale })
    }
    /// The eye the body carries: above the (rail-smoothed) feet by the posture's eye height, less Clank's climb clamp.
    pub(super) fn body_eye(&self, game: &rc_game::tick::Game) -> Vec3 {
        self.rail_position
            .unwrap_or_else(|| crate::tfrag_render::game_to_bevy(game.hero.position()))
            + self.surface_up() * (self.eye_height.unwrap_or(RATCHET_EYE) - self.climb_drop)
    }
    /// Giant Clank's tracking space is his size: the head, hands and room-scale steps grow with his eye height.
    pub(super) fn body_scale(&self) -> f32 {
        if self.body == 2 { GIANT_CLANK_EYE / 1.6 } else { 1.0 }
    }
    pub(super) fn root_for(&self, game: &rc_game::tick::Game, scale: f32) -> Option<Transform> {
        if crate::saves::front_end_active() {
            return self.front_root;
        }
        if let Some(root) = self.cinematic_root {
            return Some(root);
        }
        let scale = scale * self.body_scale();
        let mut center = self.center?;
        if let Some(head) = self.rail_center.or(self.board_center.map(|(head, _)| head)).or(self.head) {
            center.x = head.x;
            center.z = head.z;
        }
        // The rail's slope pitches about the rail's side axis in the world (the tracking space's own X axis is only
        // that axis when the player faced the recentre direction).
        let heading = self.world_yaw + self.turn;
        let side = Quat::from_rotation_y(self.rail_heading.unwrap_or(heading)) * Vec3::X;
        let rotation = self.gravity_frame
            * Quat::from_axis_angle(side, self.rail_pitch)
            * Quat::from_rotation_y(heading - self.yaw);
        let body_eye = self.body_eye(game);
        // Death scripts can park the swallowed body far outside the level. Keep the
        // last living viewpoint, with normal head tracking, until the respawn.
        let eye = if eye_parked(game) && self.eye_level == crate::level_load::level_index()
        {
            self.last_live_eye.unwrap_or(body_eye)
        } else {
            body_eye
        };
        // Room-scale displacement has already been consumed by the native capsule.
        Some(Transform {
            translation: eye - rotation * (center * scale),
            rotation,
            scale: Vec3::splat(scale),
        })
    }
}
/// The body no longer carries the eye: dead, eaten (group 0x14, 0x78), or held hidden by a Rilgar lurker (class 35 in
/// its grab state 3, which carries the body 20 above itself until he drowns).
fn eye_parked(game: &rc_game::tick::Game) -> bool {
    let h = &game.hero;
    h.health <= 0
        || h.group == 0x14
        || h.state == 0x78
        || (h.state == 0x72 && h.f13f5 != 0 && game.mobys.mobys.iter().any(|m| m.o_class == 35 && m.state == 3))
}
/// The states the camera flip follows: the flip, the double jump, the wall jump and the grind jumps.
const FLIP_STATES: [i32; 5] = [0x0b, 0x0e, 0x11, 0x29, 0x2a];
/// The flip's fade in and out (seconds).
const FLIP_FADE: f32 = 0.15;

/// The camera flip's turn this frame: Ratchet's head joint's turn since the flip began (his animated body flips, his
/// rotation does not), in the world, sampled per tick and interpolated by the overstep, faded in and out.
fn update_flip(
    rig: &mut Rig,
    play: &crate::gameplay::Play,
    attach: Option<&crate::moby_attach::MobyAttach>,
    on: bool,
    dt: f32,
    overstep: f32,
) {
    let hero = &play.game.hero;
    let body = on && hero.mode == 0 && !play.death_pending && !eye_parked(&play.game);
    let flipping = body && FLIP_STATES.contains(&hero.state);
    let head = attach.filter(|_| body).and_then(|a| a.hero_head_rotation(play));
    if flipping && rig.flip_start.is_none() {
        rig.flip_start = head;
        rig.flip_samples = None;
    }
    let tick = play.game.counter;
    if rig.flip_samples.is_none_or(|(t, _, _)| t != tick) {
        let turn = rig.flip_start.zip(head).map_or(Quat::IDENTITY, |(start, now)| {
            let r = hero.moby_rows;
            let col = |i: usize| Vec3::new(r[i][0].to_f32(), r[i][1].to_f32(), r[i][2].to_f32()).normalize_or_zero();
            let body = Quat::from_mat3(&Mat3::from_cols(col(0), col(1), col(2))).normalize();
            let q = body * (now * start.inverse()) * body.inverse();
            // Game (x, y, z) to Bevy (x, z, −y).
            Quat::from_xyzw(q.x, q.z, -q.y, q.w).normalize()
        });
        let previous = rig.flip_samples.map_or(turn, |s| s.2);
        rig.flip_samples = Some((tick, previous, turn));
    }
    let target = if flipping { 1.0 } else { 0.0 };
    rig.flip_weight = (rig.flip_weight + (target - rig.flip_weight).clamp(-dt / FLIP_FADE, dt / FLIP_FADE)).clamp(0.0, 1.0);
    if rig.flip_weight == 0.0 && !flipping {
        (rig.flip_start, rig.flip_samples, rig.flip) = (None, None, Quat::IDENTITY);
        return;
    }
    let Some((_, a, b)) = rig.flip_samples else { return };
    let w = rig.flip_weight * rig.flip_weight * (3.0 - 2.0 * rig.flip_weight);
    rig.flip = Quat::IDENTITY.slerp(a.slerp(b, overstep.clamp(0.0, 1.0)), w);
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
        // Port Options owns AA quality, including its High default and persisted user changes.
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
        "VR_GAME first person: left stick move; right stick snap turn 30 degrees; A jump/double jump; right grip hold wrench, swing to hit, release a moving hand to throw; right stick click interacts or hold and move right hand for quick select; Y pause/skip; right trigger fires the selected Blaster/Pyrocitor or prepares a bomb for gesture release; Ratchet brown gloves; F8 recenter"
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
    app.insert_resource(crate::render_settings::VrOptions::startup())
        .insert_resource(settings.clone())
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
            (pace_monitor_cameras, eye_msaa).before(bevy::camera::CameraUpdateSystems),
        )
        .add_plugins((
            ExtractComponentPlugin::<GameEye>::default(),
            ExtractComponentPlugin::<EyeSky>::default(),
        ))
        .insert_resource(Audit {
            start: now,
            last: now,
            first_tick: None,
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
            super::bomb_glove::follow_held
                .after(super::solid_contact::Presented)
                .before(crate::gameplay::upload),
        )
        .add_systems(
            PostUpdate,
            update_panels.before(TransformSystems::Propagate),
        )
        .add_systems(Last, audit)
        .add_systems(
            PostUpdate,
            (
                mirror_scene_fade.after(crate::scene_render::fade_pass),
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
    app.add_plugins(super::loading::LoadingPlugin)
        .add_plugins(super::fullscreen_refresh::FullscreenRefreshPlugin)
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
    app.add_systems(PostUpdate, warm_destination
        .after(sky_policy).before(TransformSystems::Propagate));
    Ok(())
}

fn warm_destination(
    mut commands: Commands,
    level: Res<crate::Level>,
    eyes: Query<(&RenderTarget, &Projection, &Msaa), With<GameEye>>,
    warm: Query<Entity, With<DestinationWarmup>>,
) {
    if !super::loading::streaming() || super::loading::destination_ready() {
        for e in &warm { commands.entity(e).despawn(); }
        return;
    }
    if !warm.is_empty() { return; }
    // The destination's play state is set up at the cut (crate::vr::loading): its placed Ratchet instance (class 0)
    // gives the eye, before the hero init's ground snap.
    let m = &level.0.mobys;
    let Some(hero) = m.instances.iter().zip(&m.placed).find(|(i, p)| i.o_class == 0 && p.is_some()).map(|(i, _)| i) else { return };
    let Some((target, projection, msaa)) = eyes.iter().next() else { return };
    let view = Transform::from_translation(crate::tfrag_render::game_to_bevy(hero.position) + Vec3::Y * RATCHET_EYE)
        .with_rotation(Quat::from_rotation_y(hero.rotation[2] - std::f32::consts::FRAC_PI_2));
    super::loading::set_destination_view(view);
    // Prepare the destination's visible material pipelines using the real eye target/format.
    // The flight sky and eye render AFTER this view and replace it completely; none of the
    // unfinished planet is presented. One temporary view is enough for both eyes' pipelines.
    commands.spawn((
        Camera3d::default(),
        Camera { order: 8, output_mode: CameraOutputMode::Skip,
            clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        target.clone(), projection.clone(), *msaa,
        view,
        RenderLayers::default(), Tonemapping::None, DebandDither::Disabled,
        crate::shadow_render::ShadowCamera,
        crate::level_switch::KeepAcrossLevels,
        DestinationWarmup, Name::new("Destination pipeline warmup behind flight"),
    ));
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
        || (cadence.every > 0 && cadence.frame.is_multiple_of(cadence.every))
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
    // Keep the revision moving forward: tracking-space caches keyed by it must not match a recycled number.
    let revision = rig.revision.wrapping_add(1);
    *rig = Rig { revision, ..default() };
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

fn mirror_scene_fade(
    mut commands: Commands,
    loading: Res<super::loading::Hold>,
    travel: Res<crate::travel_render::Travel>,
    main: Query<Option<&crate::scene_render::SceneFade>, With<crate::fly_cam::FlyCam>>,
    eyes: Query<(Entity, Option<&crate::scene_render::SceneFade>), With<GameEye>>,
) {
    let native = main.iter().next().flatten().copied();
    let enabled = native.is_some() || travel.in_transition() || loading.opacity() > 0.0;
    let mut want = native.unwrap_or_default();
    if loading.opacity() > 0.0 {
        let native_black = if want.rgba.truncate() == UVec3::ZERO { want.rgba.w } else { 0 };
        want.rgba = UVec4::new(0, 0, 0, native_black.max((loading.opacity() * 128.0).ceil() as u32));
    }
    // Warm this pass on both eyes during the flight, before replacing the level. Ordinary
    // gameplay keeps the original zero-overhead path when no screen fade is needed.
    for (e, have) in &eyes {
        if enabled {
            if have != Some(&want) { commands.entity(e).insert(want); }
        } else if have.is_some() { commands.entity(e).remove::<crate::scene_render::SceneFade>(); }
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
/// How fast the view follows the hoverboard's heading (the time constant, seconds) and at most (radians / second; the
/// board turns at up to 170° / s).
const BOARD_FOLLOW_SECS: f32 = 0.12;
const BOARD_FOLLOW_RATE: f32 = 4.0;
/// The lean steering on the hoverboard: the neck's sideways offset (metres) below which the board goes straight, and
/// the offset that turns it at full rate.
const BOARD_LEAN_DEAD: f32 = 0.04;
const BOARD_LEAN_FULL: f32 = 0.20;
/// The neck, about which the head turns: looking around moves the eyes but barely the neck, leaning moves both.
fn neck(head: &Transform) -> Vec3 {
    head.translation + head.rotation * Vec3::new(0.0, -0.10, 0.08)
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
    if play.as_ref().is_some_and(|p| matches!(p.game.hero.group, 0xf | rc_game::hero::hoverboard::GROUP)) {
        // Lean around the rail attachment; room-scale walking cannot push the capsule off its spline. On the hoverboard
        // the lean steers (Rig::board_lean) and moves the eye, not the rider.
        rig.previous_head = None;
        return;
    }
    let previous = rig
        .previous_head
        .replace(head.translation)
        .or(rig.rail_center.take());
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
    let delta = rotation * delta * settings.units_per_metre * rig.body_scale();
    play.roomscale_move(coll, super::embodied::game_point(delta));
}

fn rail_direction(game: &rc_game::tick::Game) -> Option<Vec3> {
    let b = &game.hero.boots;
    let path = game.grind_paths.get(b.rail?)?;
    if path.points.len() < 2 {
        return None;
    }
    // Look across several joints instead of copying the current polyline segment's tangent.
    let reach = (b.speed.abs() / rc_game::hero::physics::DT.to_f32() * 0.18).clamp(2.5, 5.0);
    let mut behind = b.cur;
    let mut ahead = b.cur;
    let a = rc_game::spline::advance(&path.points, b.closed, -1.5, &mut behind).0;
    let z = rc_game::spline::advance(&path.points, b.closed, reach, &mut ahead).0;
    let direction = crate::tfrag_render::game_to_bevy(z) - crate::tfrag_render::game_to_bevy(a);
    (direction.length_squared() > 0.01).then(|| direction.normalize())
}

/// [`follow_game_camera`]'s optional state: Ratchet's attachments, the VR options, the departure world of a
/// streaming flight.
type CameraExtras<'w> = (
    Option<Res<'w, crate::moby_attach::MobyAttach>>,
    Option<Res<'w, crate::render_settings::VrOptions>>,
    Option<Res<'w, super::loading::FlightWorld>>,
);

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
    scene: Res<crate::scene_render::ActiveScene>,
    scene_runtime: Res<crate::scene_render::SceneRuntime>,
    fixed: Res<Time<Fixed>>,
    (attach, vr_options, departure): CameraExtras,
) {
    // Until this frame's eye is known, culling uses the native camera (the title, a lost focus).
    super::loading::set_render_eye(None, false);
    // While a flight streams the destination, its play state is only set up at the cut (crate::vr::loading): the
    // voyage runs on the departure's, as before the level change.
    let Some(play) = play.as_deref().or(departure.as_deref().map(|d| &d.play)) else { return };
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
        rig.rail_position = None;
        rig.board_center = None;
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
    let level_id = crate::level_load::level_index();
    if rig.eye_level != level_id {
        rig.eye_level = level_id;
        rig.last_live_eye = None;
        rig.cinematic_root = None;
        rig.cinematic_head = None;
        rig.cinematic_yaw = None;
    }
    let now = time.elapsed_secs_f64();
    let dt = rig
        .rail_time
        .replace(now)
        .map_or(0.0, |old| (now - old).clamp(0.0, 0.05) as f32);
    // The planet-side voyage, or a flown ship's cockpit (at the ship's flight scale).
    let ship = ship_view(play);
    let pilot = travel.pilot_view(play).or(ship.map(|(eye, rotation, _)| (eye, rotation)));
    let view_scale = settings.units_per_metre * ship.map_or(1.0, |(_, _, scale)| scale);
    let actor_eye = scene
        .running
        .then(|| crate::scene_render::ratchet_eye(&scene_runtime))
        .flatten()
        .map(crate::tfrag_render::game_to_bevy);
    let actor_gaze = scene
        .running
        .then(|| crate::scene_render::actor_gaze(&scene_runtime))
        .flatten();
    // A manned turret: the eye at its seat, the view fixed while the turret turns beneath it (super::turret).
    let turret = if scene.running { None } else { super::turret::seat(play) };
    // A scene with no one to see through (a flyover, the infobot's message): a spectator at the scene's camera, its
    // heading taken at the scene's start and at each cut, the head free.
    let spectator = scene
        .camera
        .as_ref()
        .filter(|_| scene.running && pilot.is_none() && !crate::scene_render::has_view_actor(&scene_runtime))
        .map(|c| {
            let forward = crate::tfrag_render::game_to_bevy(c.rows[0]);
            (crate::tfrag_render::game_to_bevy(c.eye), (-forward.x).atan2(-forward.z), c.cut)
        });
    // A pilot view (or a cut) hands over: the next view takes its own anchor and heading.
    if (pilot.is_none() && rig.pilot_rotation.is_some()) || spectator.is_some_and(|(_, _, cut)| cut) {
        rig.cinematic_head = None;
        rig.cinematic_yaw = None;
    }
    let cinematic_eye = pilot
        .map(|(eye, _)| eye)
        .or(actor_eye)
        .or(spectator.map(|(eye, _, _)| eye))
        .or(turret.map(|(eye, _)| eye))
        .or_else(|| {
        // A streamed scene may have no valid Ratchet key on its first frame.
        (scene.running && rig.cinematic_root.is_none()).then(|| {
            rig.last_live_eye.unwrap_or_else(|| rig.body_eye(&play.game))
        })
    });
    if let Some(eye) = cinematic_eye {
        let frame = if let Some((_, target)) = pilot {
            let previous = rig.pilot_rotation.unwrap_or(target);
            let next = previous.slerp(target, 1.0 - (-dt / 0.08).exp());
            rig.pilot_rotation = Some(next);
            next
        } else {
            rig.pilot_rotation = None;
            // A scene starts looking where its actor looks, a turret where it aims: the head's heading at that moment
            // becomes that heading.
            if rig.cinematic_yaw.is_none() {
                let start = turret.map(|(_, aim)| aim).or(spectator.map(|(_, heading, _)| heading)).or_else(|| {
                    actor_gaze.map(|[x, y]| {
                        let gaze = crate::tfrag_render::game_to_bevy([x, y, 0.0]);
                        (-gaze.x).atan2(-gaze.z)
                    })
                });
                if let Some(start) = start {
                    rig.cinematic_yaw = Some(start - heading(head.rotation) + rig.yaw);
                }
            }
            Quat::from_rotation_y(rig.cinematic_yaw.unwrap_or(rig.world_yaw + rig.turn))
        };
        let rotation = frame * Quat::from_rotation_y(-rig.yaw);
        // The room position the head had when the view began, not the recentre point: room-scale walking since the
        // recentre would otherwise put the view that far from the actor's eye.
        let anchor = *rig.cinematic_head.get_or_insert(head.translation);
        let mut center = rig.center.unwrap_or(head.translation);
        center.x = anchor.x;
        center.z = anchor.z;
        // At a turret's seat the head's height when mounting sits at the seat too.
        if turret.is_some() && pilot.is_none() && actor_eye.is_none() {
            center.y = anchor.y;
        }
        rig.cinematic_root = Some(Transform {
            translation: eye - rotation * (center * view_scale),
            rotation,
            scale: Vec3::splat(view_scale),
        });
    } else if !scene.running {
        rig.cinematic_root = None;
        rig.cinematic_head = None;
        rig.cinematic_yaw = None;
        rig.pilot_rotation = None;
    }
    let boots = &play.game.hero.boots;
    let on_rail = play.game.hero.group == 0xf && boots.rail.is_some() && boots.off_rail == 0;
    let direction = on_rail.then(|| rail_direction(&play.game)).flatten();
    let target_pitch = direction.map_or(0.0, |v| v.y.atan2(Vec2::new(v.x, v.z).length()));
    if on_rail {
        let rail = direction.map_or(boots.yaw - std::f32::consts::FRAC_PI_2, |v| {
            (-v.x).atan2(-v.z)
        });
        if let Some(previous) = rig.rail_heading {
            let error = (rail - previous).sin().atan2((rail - previous).cos());
            // Average a span of the path, then reject tiny tangent noise and bound virtual turning.
            let change = (error * (1.0 - (-dt / 0.48).exp())).clamp(-dt * 0.85, dt * 0.85);
            rig.world_yaw += change;
            rig.rail_heading = Some(previous + change);
        } else {
            // Preserve entry gaze: only subsequent gradual path changes rotate the VR origin.
            rig.rail_heading = Some(rail);
            rig.rail_center = Some(head.translation);
        }
    } else {
        // `rail_center` stays until `align_body` turns the lean around the rail into a room-scale step.
        rig.rail_heading = None;
    }
    // The hoverboard: its start and its respawns place it along the course, so the current gaze becomes the board's
    // heading, as a recentre does. While riding, the view turns with the board (it steers about its own heading,
    // `Tracked::board_lean`), so the player keeps facing the board's front without turning in the room.
    let group = play.game.hero.group;
    let on_board = group == rc_game::hero::hoverboard::GROUP;
    let board = Vec3::from_array(play.game.hero.position());
    let placed = rig.board_pos.is_some_and(|old| old.distance(board) > 3.0);
    if on_board && (rig.hero_group != group || placed) {
        rig.world_yaw = play.game.hero.rot[2].to_f32() - std::f32::consts::FRAC_PI_2;
        rig.yaw = heading(head.rotation);
        rig.turn = 0.0;
        rig.board_center = None;
        rig.revision = rig.revision.wrapping_add(1);
        println!("VR_GAME board_heading yaw={:.3} placed={placed}", play.game.hero.rot[2].to_f32());
    } else if on_board {
        let target = play.game.hero.board.yaw - std::f32::consts::FRAC_PI_2;
        let error = (target - rig.world_yaw).sin().atan2((target - rig.world_yaw).cos());
        rig.world_yaw += (error * (1.0 - (-dt / BOARD_FOLLOW_SECS).exp())).clamp(-dt * BOARD_FOLLOW_RATE, dt * BOARD_FOLLOW_RATE);
    }
    if !on_board {
        rig.board_center = None;
    } else if rig.board_center.is_none() {
        rig.board_center = Some((head.translation, neck(&head)));
    }
    rig.hero_group = group;
    rig.board_pos = on_board.then_some(board);
    let pitch_error = target_pitch - rig.rail_pitch;
    rig.rail_pitch += (pitch_error * (1.0 - (-dt / 0.62).exp())).clamp(-dt * 0.30, dt * 0.30);
    let position = crate::tfrag_render::game_to_bevy(play.game.hero.position());
    // On a rail the eye is the body at this frame: the last tick plus its velocity over the overstep (at most a tick
    // ahead), not a smoothed trail, which put the eye, the hands and the wrench's hits a metre behind at grind speed.
    if on_rail {
        let now = (play.game.counter, position);
        let [last, prev] = rig.rail_samples.unwrap_or([now; 2]);
        let samples = if last.0 != now.0 { [now, last] } else { [last, prev] };
        rig.rail_samples = Some(samples);
        let ticks = samples[0].0.saturating_sub(samples[1].0);
        let step = if (1..=4).contains(&ticks) { (samples[0].1 - samples[1].1) / ticks as f32 } else { Vec3::ZERO };
        let step = if step.length() < 1.0 { step } else { Vec3::ZERO };
        rig.rail_offset = step * fixed.overstep_fraction().clamp(0.0, 1.0);
    } else {
        rig.rail_samples = None;
        // Off the rail the prediction fades out (a smooth exit, no speed-dependent lag).
        rig.rail_offset *= (-dt / 0.08).exp();
    }
    rig.rail_position = (on_rail || rig.rail_offset.length() > 0.005).then(|| position + rig.rail_offset);
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
    // Switching body (Clank, Giant Clank, the Hologuise disguise) takes its eye at once, without the posture blend.
    let body = play.game.hero.mode.min(3);
    if rig.body != body {
        rig.body = body;
        rig.eye_height = None;
        rig.revision = rig.revision.wrapping_add(1);
    }
    // Surface swimming uses the native horizontal swimmer's eye height. Blend the posture change;
    // head rotation always remains physically tracked.
    // Hanging with extended arms puts the eyes below the ledge, not alongside the palms.
    let hang = |low: f32, high: f32| {
        (play.game.hero.ledge_blk.top_z - play.game.hero.position()[2] - 0.38).clamp(low, high)
    };
    let target_height = if body == 1 {
        if rc_game::hero::tracked::ledge_hold(play.game.hero.state) {
            hang(0.25, CLANK_EYE)
        } else {
            CLANK_EYE
        }
    } else if body == 2 {
        GIANT_CLANK_EYE
    } else if body == 3 {
        // The disguise (class 634) is Ratchet's rig at Ratchet's height.
        RATCHET_EYE
    } else if matches!(play.game.hero.state, 0x18..=0x1b) {
        hang(0.45, RATCHET_EYE)
    } else if play.game.hero.state == 0x74 {
        // Leave at least 55 cm above the eyes for the cable and the overhead wrench.
        (play.game.hero.boots.cable_pt[2] - play.game.hero.position()[2] - 0.55).clamp(0.55, RATCHET_EYE)
    } else if matches!(play.game.hero.group, 0x11 | 0x12) {
        0.45
    } else if play.game.hero.state == 4
        || (play.game.hero.state == 0x73 && play.game.pad.held & rc_game::pad::button::CROUCH != 0)
    {
        // Crouched: about half the standing eye.
        0.6
    } else {
        RATCHET_EYE
    };
    let height = rig.eye_height.unwrap_or(target_height);
    rig.eye_height = Some(
        height + (target_height - height).clamp(-dt * EYE_BLEND, dt * EYE_BLEND),
    );
    let hero = &play.game.hero;
    rig.climb_drop = if body == 1
        && hero.state == 0x4c
        && rc_game::hero::tracked::ledge_hold(hero.prev_state)
    {
        (hero.position()[2] - hero.ledge_blk.top_z - CLANK_CLIMB_RISE).max(0.0)
    } else {
        // A climb ending above the clamp (a higher ledge caught) rejoins the body smoothly.
        (rig.climb_drop - dt * EYE_BLEND).max(0.0)
    };
    if !eye_parked(&play.game)
        && !play.death_pending
        && !scene.running
        && pilot.is_none()
    {
        rig.last_live_eye = Some(rig.body_eye(&play.game));
    }
    // The flight is drawn on a separate world layer, about its authored camera, not about the
    // hero left on the departure planet. Keep head-driven pitch/roll and show the actual voyage.
    let root = if rig.cinematic_root.is_some() {
        rig.cinematic_root
    } else if travel.blanked() {
        main.iter().next().map(|native| {
            rig_transform(
                *native,
                rig.center.unwrap_or(head.translation),
                rig.yaw,
                settings.units_per_metre,
            )
        })
    } else {
        let flip_on = vr_options.as_ref().is_some_and(|o| o.camera_flip) && pilot.is_none();
        update_flip(&mut rig, play, attach.as_deref(), flip_on, dt, fixed.overstep_fraction());
        rig.view_root_for(&play.game, settings.units_per_metre, head)
    };
    let Some(root) = root else {
        return;
    };
    for mut r in &mut roots {
        *r = root;
    }
    let center = root.mul_transform(head);
    // Level content is culled for the headset's eye, whatever the native camera shows (crate::vr::loading).
    super::loading::set_render_eye(
        Some(center),
        native_panel(play) || crate::visibomb_view::missile_view(Some(play)),
    );
    if mode.free_movement()
        && !native_panel(play)
        && !crate::visibomb_view::missile_view(Some(play))
    {
        for mut t in &mut main {
            // Keep authored camera shots in the original menu/cinematic panel.
            *t = center;
        }
    }
}
fn eye_msaa(
    render: Option<Res<crate::render_settings::RenderSettings>>,
    supported: Option<Res<crate::render_settings::SupportedMsaa>>,
    mut eyes: Query<&mut Msaa, Or<(With<GameEye>, With<EyeSky>)>>,
) {
    let Some(render) = render else { return };
    let samples = supported.map_or(render.msaa, |s| s.clamp(render.msaa));
    for mut msaa in &mut eyes {
        if *msaa != samples {
            *msaa = samples;
        }
    }
}

fn sky_policy(
    play: Option<Res<crate::gameplay::Play>>,
    mode: Res<crate::menu_render::MenuMode>,
    movie: Res<crate::movie_render::MovieState>,
    travel: Res<crate::travel_render::Travel>,
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
    let cinema = movie.busy() || matches!(mode.state.mode,
        rc_game::menus::mode::Mode::Movie | rc_game::menus::mode::Mode::Slideshow);
    let sky_layers = native_sky
        .iter()
        .next()
        .cloned()
        .unwrap_or_else(RenderLayers::none);
    let (world_clear, world_layers) = main
        .iter()
        .next()
        .map(|(c, layers)| (c.clear_color, layers.cloned().unwrap_or_default()))
        .unwrap_or((ClearColorConfig::Default, RenderLayers::default()));
    for (mut c, mut d, mut layers, sky) in &mut cams {
        if sky {
            *layers = if cinema {
                RenderLayers::none()
            } else if play
                .as_deref()
                .is_some_and(|p| crate::visibomb_view::missile_view(Some(p)))
            {
                RenderLayers::layer(1)
            } else {
                sky_layers.clone()
            };
            c.order = 10;
            c.clear_color = if cinema || travel.blanked() {
                ClearColorConfig::Custom(Color::BLACK)
            } else { ClearColorConfig::Default };
            c.output_mode = CameraOutputMode::Skip;
            d.depth_load_op = Camera3dDepthLoadOp::Clear(0.0);
        } else {
            *layers = if cinema {
                RenderLayers::layer(PANEL_LAYER)
            } else {
                world_layers.clone().with(PANEL_LAYER)
            };
            c.order = 11;
            c.clear_color = if cinema { ClearColorConfig::None } else { world_clear };
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
    rig: Res<Rig>,
    mut cinema_anchor: Local<Option<(u64, Transform)>>,
) {
    use rc_game::menus::mode::Mode;
    // Ship/engine movies already exist in the stereo world. Only actual menus and flat video need the composed
    // frame (gadget views have their own panel, super::spatial_menu); a cinematic camera class must not enable it.
    let menu = matches!(mode.state.mode, Mode::Menu | Mode::Freeze | Mode::Vendor | Mode::Movie | Mode::Slideshow);
    let gadget_panel = play.as_deref().is_some_and(native_panel);
    let head = pose(views.as_deref()).unwrap_or_else(|| Transform::from_xyz(0.0, 1.6, 0.0));
    // Mounted in a turret or flying a ship, the HUD hangs along the aim (the turret's script camera behind its barrels,
    // the ship's nose) instead of the gaze: its reticle, drawn at the screen centre, marks where the guns point.
    let aim = play
        .as_deref()
        .filter(|p| p.game.hero.mode == 0 && p.game.hero.state == 0x32)
        .and_then(|p| {
            let root = rig.root_for(&p.game, settings.units_per_metre)?;
            let forward = ship_view(p).map_or_else(
                || crate::tfrag_render::game_to_bevy(p.game.camera.out.rows_f32()[0]),
                |(_, rotation, _)| rotation * Vec3::NEG_Z,
            );
            let forward = (root.rotation.inverse() * forward).normalize_or_zero();
            (forward.length_squared() > 0.5).then_some(forward)
        });
    let cinema = matches!(mode.state.mode, Mode::Movie | Mode::Slideshow);
    if cinema {
        if cinema_anchor.as_ref().is_none_or(|(revision, _)| *revision != rig.revision) {
            *cinema_anchor = Some((rig.revision, Transform::from_translation(head.translation)
                .with_rotation(Quat::from_rotation_y(heading(head.rotation)))));
        }
    } else { *cinema_anchor = None; }
    let spatial_pause = matches!(mode.state.mode, Mode::Menu | Mode::Freeze);
    let missile = play
        .as_deref()
        .is_some_and(|p| crate::visibomb_view::missile_view(Some(p)));
    for (panel, mut t, mut visibility, material) in &mut panels {
        if spatial_pause
            || missile
            || gadget_panel
            || (mode.spatial_vendor && mode.state.mode == Mode::Vendor)
        {
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
        if cinema && panel.0 == 0 {
            // A level screen in a black room, fixed where the viewer was looking at entry.
            // Head motion remains free instead of dragging a picture across the face.
            let anchor = cinema_anchor.as_ref().map_or(head, |(_, anchor)| *anchor);
            *t = anchor.mul_transform(Transform::from_xyz(0.0, 0.0, -3.0));
            t.scale = Vec3::new(1.8, 3.6 * frame.size.y as f32 / frame.size.x.max(1) as f32 / 1.5, 1.0);
        } else if let Some(forward) = aim.filter(|_| panel.0 != 0) {
            *t = Transform::from_translation(head.translation + forward * (2.0 - panel.0 as f32 * 0.002))
                .looking_to(forward, Vec3::Y);
        } else {
            *t = head.mul_transform(Transform::from_xyz(0.0, 0.0, -2.0 + panel.0 as f32 * 0.002));
        }
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
