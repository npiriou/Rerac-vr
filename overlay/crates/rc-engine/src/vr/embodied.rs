//! Tracked hands on the original game: physical melee, native Comet-Strike flight and damage.
use super::{
    Settings,
    game::Rig,
    game_input::{Focused, Hand, HandInput},
};
use bevy::{camera::visibility::RenderLayers, prelude::*, transform::TransformSystems};
use bevy_mod_openxr::spaces::OxrSpaceLocationFlags;
use bevy_mod_xr::spaces::XrSpaceSyncSet;
use rc_game::{
    hero::{
        items::{HandItem, ItemData},
        physics::from_f32x3,
    },
    moby_update::services::{self, HitTemplate},
    ps2v::Pf,
};
use std::collections::{HashMap, HashSet, VecDeque};

pub(super) struct EmbodiedPlugin;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct WrenchUpdate;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct HandPresentation;
impl Plugin for EmbodiedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Motion>()
            .init_resource::<super::game_input::Feedback>()
            .init_resource::<Melee>()
            .add_plugins((
                super::solid_contact::SolidContactPlugin,
                super::body_fists::BodyFistsPlugin,
                super::bomb_glove::BombGlovePlugin,
                super::blaster::BlasterPlugin,
                super::akimbo::AkimboPlugin,
                super::gadget_socket::GadgetSocketPlugin,
                super::traversal::TraversalPlugin,
                super::orientation::OrientationPlugin,
                super::walloper::WalloperPlugin,
                super::turret::TurretPlugin,
            ))
            .add_systems(
                PreUpdate,
                capture
                    .after(XrSpaceSyncSet)
                    .after(super::game_input::sample),
            )
            .add_systems(
                FixedUpdate,
                interact
                    .in_set(WrenchUpdate)
                    .after(super::bomb_glove::BombUpdate)
                    .before(crate::gameplay::GameTick),
            )
            .add_systems(
                PostUpdate,
                present
                    .in_set(HandPresentation)
                    .before(crate::moby_attach::upload)
                    .before(TransformSystems::Propagate),
            )
            .add_systems(
                PostUpdate,
                present_worn
                    .in_set(HandPresentation)
                    .after(present)
                    .before(crate::moby_attach::upload)
                    .before(TransformSystems::Propagate),
            );
    }
}

#[derive(Resource, Default)]
pub(super) struct Motion {
    pub hands: [Option<Transform>; 2],
    pub bomb: BombGesture,
    /// The left trigger's throw (Akimbo: the left hand's glove, super::bomb_glove), with the left hand's own motion.
    pub left_bomb: BombGesture,
    left_samples: VecDeque<(f64, Vec3)>,
    left_velocity: Vec3,
    held: bool,
    grip_down: bool,
    grip_suppressed: bool,
    equip_wrench: bool,
    equip_ready_tick: Option<u64>,
    release: Option<(Transform, Vec3, f64)>,
    samples: VecDeque<(f64, Vec3)>,
    velocity: Vec3,
    slam_velocity: Vec3,
    revision: u64,
    level: u32,
    serial: u64,
}
impl Motion {
    pub(super) fn wrench_held(&self) -> bool {
        self.held
    }
}
#[derive(Default)]
pub(super) struct BombGesture {
    pub held: bool,
    pub press: Option<f64>,
    pub release: Option<(Transform, Vec3, f64)>,
}
#[derive(Resource, Default)]
struct Melee {
    previous: Option<(Transform, f64)>,
    serial: u64,
    revision: u64,
    level: u32,
    last_tick: u64,
    hits: HashMap<usize, u64>,
    down_from: Option<Vec3>,
    swing_from: Option<Vec3>,
    slam_until: f64,
    wall: Option<WrenchWall>,
    safe_head: Option<Vec3>,
    wall_armed: bool,
    wall_until: f64,
    rebound: Option<(f64, Vec3)>,
    crank: Option<usize>,
    crank_previous: Option<Vec3>,
    crank_far_since: Option<f64>,
    /// The swing's whoosh: played this stroke, not before (seconds).
    whooshed: bool,
    whoosh_until: f64,
}

#[derive(Clone, Copy)]
struct WrenchWall {
    point: Vec3,
    normal: Vec3,
    moby: Option<usize>,
}
impl Melee {
    fn clear_contact(&mut self) {
        self.wall = None;
        self.rebound = None;
        self.safe_head = None;
        self.wall_armed = false;
    }
    /// Forget the stroke in progress: its contact, the last sample, and where the swing and the Hyper Strike started.
    fn reset_stroke(&mut self) {
        self.whooshed = false;
        self.clear_contact();
        self.previous = None;
        self.swing_from = None;
        self.down_from = None;
    }
}
// Presentation only: a short displacement opposite the incoming physical stroke.
// Physics history never includes this offset, so the animation cannot manufacture another hit.
fn rebound(item: &mut HandItem, melee: &Melee, root: &Transform, now: f64) -> bool {
    let Some((start, offset)) = melee.rebound else {
        return false;
    };
    let phase = ((now - start) / 0.15) as f32;
    if !(0.0..1.0).contains(&phase) {
        return false;
    }
    let shift = root.rotation * (offset * root.scale.x) * (std::f32::consts::PI * phase).sin();
    item.position = game_point(crate::tfrag_render::game_to_bevy(item.position) + shift);
    true
}

pub(super) fn gameplay(mode: &crate::menu_render::MenuMode) -> bool {
    mode.free_movement()
}

pub(super) fn capture(
    focus: Option<Res<Focused>>,
    input: Option<Res<HandInput>>,
    rig: Res<Rig>,
    mode: Res<crate::menu_render::MenuMode>,
    time: Res<Time<Real>>,
    tracked: Query<(&Hand, &Transform, &OxrSpaceLocationFlags)>,
    mut motion: ResMut<Motion>,
    play: Option<Res<crate::gameplay::Play>>,
    pad: Option<Res<super::game_input::GamePad>>,
) {
    if !focus.as_ref().is_some_and(|f| f.0) {
        *motion = Motion::default();
        return;
    }
    if !gameplay(&mode) {
        // Menus retain optical poses for the pointer, while gestures and weapon edges stay reset.
        *motion = Motion::default();
        if matches!(
            mode.state.mode,
            rc_game::menus::mode::Mode::Menu | rc_game::menus::mode::Mode::Freeze
        ) {
            for (hand, pose, flags) in &tracked {
                if flags.pos_valid()
                    && flags.rot_valid()
                    && flags.pos_tracked()
                    && flags.rot_tracked()
                {
                    motion.hands[hand.0] = Some(*pose);
                }
            }
        }
        return;
    }
    let level = crate::level_load::level_index();
    if motion.revision != rig.revision || motion.level != level {
        // Recentring/snap turning clears motion, not physical button edges or the selected weapon.
        let held = motion.held;
        let grip_down = motion.grip_down;
        let grip_suppressed = motion.grip_suppressed;
        let bomb_held = motion.bomb.held;
        let left_held = motion.left_bomb.held;
        *motion = Motion {
            revision: rig.revision,
            level,
            held,
            grip_down,
            grip_suppressed,
            bomb: BombGesture {
                held: bomb_held,
                ..default()
            },
            left_bomb: BombGesture {
                held: left_held,
                ..default()
            },
            ..default()
        };
    }
    motion.hands = [None; 2];
    for (hand, pose, flags) in &tracked {
        if flags.pos_valid() && flags.rot_valid() && flags.pos_tracked() && flags.rot_tracked() {
            motion.hands[hand.0] = Some(*pose);
        }
    }
    capture_left(&mut motion, pad.as_ref().map(|p| p.left_fire), time.elapsed_secs_f64());
    let grip = input.as_ref().and_then(|i| i.grip);
    let trigger = input.as_ref().and_then(|i| i.trigger);
    let Some(hand) = motion.hands[1].filter(|_| grip.is_some() || trigger.is_some()) else {
        motion.held = false;
        motion.grip_down = grip.is_some_and(|v| v > 0.6);
        motion.grip_suppressed = motion.grip_down;
        motion.equip_wrench = false;
        motion.equip_ready_tick = None;
        motion.release = None;
        motion.samples.clear();
        motion.velocity = Vec3::ZERO;
        motion.slam_velocity = Vec3::ZERO;
        motion.bomb = BombGesture::default();
        return;
    };
    let now = time.elapsed_secs_f64();
    if motion.release.is_some_and(|(_, _, t)| now - t > 0.12) {
        motion.release = None;
    }
    // Only tracking-space motion counts. Rig movement and snap turns cannot throw the wrench.
    if motion
        .samples
        .back()
        .is_some_and(|(t, p)| now - t > 0.12 || hand.translation.distance(*p) > 0.5)
    {
        motion.samples.clear();
        motion.release = None;
        motion.bomb = BombGesture::default();
    }
    motion.samples.push_back((now, hand.translation));
    while motion.samples.len() > 2 && now - motion.samples[0].0 > 0.08 {
        motion.samples.pop_front();
    }
    motion.velocity = motion.samples.front().map_or(Vec3::ZERO, |(t, p)| {
        let dt = (now - t) as f32;
        if dt > 0.008 {
            (hand.translation - *p) / dt
        } else {
            Vec3::ZERO
        }
    });
    // Hyper Strike needs a short preparation window; retain the longer combat/throw history.
    let start = now - 0.028;
    let mut origin = motion.samples.front().copied();
    for i in 1..motion.samples.len() {
        let (a, p) = motion.samples[i - 1];
        let (b, q) = motion.samples[i];
        if a <= start && start <= b && b > a {
            origin = Some((start, p.lerp(q, ((start - a) / (b - a)) as f32)));
            break;
        }
    }
    motion.slam_velocity = origin.map_or(Vec3::ZERO, |(t, p)| {
        if now - t >= 0.008 {
            (hand.translation - p) / (now - t) as f32
        } else {
            Vec3::ZERO
        }
    });
    if grip.is_none() {
        motion.held = false;
        motion.release = None;
    }
    let value = grip.unwrap_or(0.0);
    // Another body (Clank's fists) or a right hand on a ledge: the right grip does not take the wrench.
    // The left hand alone on a ledge, the right away from it: the right grip takes the wrench as usual.
    let free = play.as_ref().is_some_and(|p| right_hand_free(&p.game.hero));
    let no_wrench = play.as_ref().is_some_and(|p| {
        p.game.hero.mode != 0
            || p.game.hero.tracked.grabbed[1]
            || (matches!(p.game.hero.state, 0x18..=0x1b) && !free)
    });
    let reserved = no_wrench
        || (!free
            && (input
                .as_ref()
                .and_then(|i| i.left_grip)
                .is_some_and(|v| v > 0.5)
                || play.as_ref().is_some_and(|p| p.game.hero.f838 != 0)));
    // The wrench (a caught one too) is held only while the grip is squeezed (`held` latched by the press);
    // otherwise it is put away, as after a slow release.
    if value > 0.6 {
        if !motion.grip_down {
            motion.grip_suppressed = reserved;
            motion.held = !reserved;
            motion.equip_wrench = !reserved;
            motion.equip_ready_tick = None;
        }
        motion.grip_down = true;
    }
    if no_wrench {
        motion.grip_suppressed = true;
        motion.held = false;
        motion.equip_wrench = false;
        motion.equip_ready_tick = None;
        motion.release = None;
    }
    if value < 0.35 && motion.grip_down {
        // A throw is a deliberate fling (2.5 m/s over the last 80 ms, about 20 cm): a grip loosened while the hand
        // drifts or follows through puts the wrench away instead.
        let speed = motion.velocity.length();
        if motion.held && !motion.grip_suppressed && speed >= WRENCH_THROW_SPEED {
            motion.release = Some((hand, motion.velocity, now));
        }
        if motion.held && !motion.grip_suppressed && speed >= 1.0 {
            println!("VR_GAME wrench_release speed={speed:.2} thrown={}", speed >= WRENCH_THROW_SPEED);
        }
        motion.held = false;
        motion.grip_down = false;
        motion.grip_suppressed = false;
        motion.equip_wrench = false;
        motion.equip_ready_tick = None;
    }
    if let Some(trigger) = trigger {
        if motion.bomb.press.is_some_and(|t| now - t > 0.12) {
            motion.bomb.press = None;
        }
        if motion.bomb.release.is_some_and(|(_, _, t)| now - t > 0.12) {
            motion.bomb.release = None;
        }
        if trigger > 0.6 && !motion.bomb.held {
            motion.bomb.release = None;
            motion.bomb.held = true;
            motion.bomb.press = Some(now);
        }
        if trigger < 0.35 && motion.bomb.held {
            motion.bomb.held = false;
            if motion.velocity.length() >= 1.0 {
                motion.bomb.release = Some((hand, motion.velocity, now));
            }
        }
    } else {
        motion.bomb = BombGesture::default();
    }
    if motion.bomb.held || motion.bomb.release.is_some() {
        motion.release = None;
    }
    motion.serial = motion.serial.wrapping_add(1);
}

/// The left trigger's throw gesture (`fire`: the left hand's weapon trigger, None without the pad), as the right
/// trigger's in [`capture`]: the press, and the release of a moving hand (1 m/s over the last 80 ms) with its velocity.
fn capture_left(motion: &mut Motion, fire: Option<bool>, now: f64) {
    let Some(hand) = motion.hands[0].filter(|_| fire.is_some()) else {
        motion.left_samples.clear();
        motion.left_velocity = Vec3::ZERO;
        motion.left_bomb = BombGesture::default();
        return;
    };
    if motion
        .left_samples
        .back()
        .is_some_and(|(t, p)| now - t > 0.12 || hand.translation.distance(*p) > 0.5)
    {
        motion.left_samples.clear();
        motion.left_bomb = BombGesture::default();
    }
    motion.left_samples.push_back((now, hand.translation));
    while motion.left_samples.len() > 2 && now - motion.left_samples[0].0 > 0.08 {
        motion.left_samples.pop_front();
    }
    motion.left_velocity = motion.left_samples.front().map_or(Vec3::ZERO, |(t, p)| {
        let dt = (now - t) as f32;
        if dt > 0.008 { (hand.translation - *p) / dt } else { Vec3::ZERO }
    });
    let velocity = motion.left_velocity;
    let g = &mut motion.left_bomb;
    if g.press.is_some_and(|t| now - t > 0.12) {
        g.press = None;
    }
    if g.release.is_some_and(|(_, _, t)| now - t > 0.12) {
        g.release = None;
    }
    if fire == Some(true) && !g.held {
        g.release = None;
        g.held = true;
        g.press = Some(now);
    }
    if fire == Some(false) && g.held {
        g.held = false;
        if velocity.length() >= 1.0 {
            g.release = Some((hand, velocity, now));
        }
    }
}

/// The left hand alone holds a ledge and the right hand is out of its reach (the native grab's reach: 1.05 along, 0.75
/// up or down): the right hand is free, its grip the wrench's (rc_game::hero::off_hand leads the left hand meanwhile).
pub(super) fn right_hand_free(h: &rc_game::hero::Hero) -> bool {
    let t = &h.tracked;
    if !t.grabbed[0] || t.grabbed[1] {
        return false;
    }
    let a = h.tracked_ledge_anchor();
    !t.hands[1].is_some_and(|hand| (hand[0] - a[0]).hypot(hand[1] - a[1]) < 1.05 && (hand[2] - a[2]).abs() < 0.75)
}

fn grip_frame(item: &HandItem, data: &ItemData) -> Option<rc_formats::moby_anim::Rows> {
    let class = data.class(item.o_class)?;
    let chain = class.chains.first().filter(|c| !c.is_empty())?;
    let mut anim = item.anim;
    let mut snapshot = item.snapshot.as_ref();
    if item.mstate != 0 && class.anim.sequence(1).is_some() {
        // The return target is the held pose, not the spinning flight animation.
        anim.seq_a = 1;
        anim.seq_b = 1;
        anim.frame_a = 0;
        anim.frame_b = 0;
        anim.t = 0.0;
        snapshot = None;
    }
    rc_formats::moby_anim::evaluate_chains(&class.anim, &anim, snapshot, &[chain.as_slice()])
        .first()
        .copied()
}

// The original spin pose expands and moves the wrench's joints, then blends them
// back into the held pose before catching. Use the original ready mesh as a rigid
// object in VR: the native flight rows already supply the complete horizontal spin.
// Only the cloned presentation/socket pose changes; native hit joints and flight stay intact.
fn rigid_wrench_pose(item: &mut HandItem, data: Option<&ItemData>) {
    if item.o_class != rc_game::hero::melee::WRENCH_CLASS
        || !(item.mstate != 0 || item.anim.seq_a == 6 || item.anim.seq_b == 6)
        || !data
            .and_then(|d| d.class(item.o_class))
            .is_some_and(|c| c.anim.frame(1, 0).is_some())
    {
        return;
    }
    item.anim.seq_a = 1;
    item.anim.seq_b = 1;
    item.anim.frame_a = 0;
    item.anim.frame_b = 0;
    item.anim.t = 0.0;
    item.snapshot = None;
}

fn place(item: &mut HandItem, transform: Transform, data: Option<&ItemData>, stretch: f32) {
    rigid_wrench_pose(item, data);
    if let Some(class) = data.and_then(|d| d.class(item.o_class)) {
        item.scale = class.scale * WRENCH_SCALE;
    }
    // The authored grip frame points the shaft up; turn it forward along the controller (-Z).
    // Rotate about the palm, then compensate the authored anchor so the handle stays in the hand.
    let mut grip_transform = transform;
    grip_transform.rotation *= Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let desired = crate::game_camera::game_rows(&grip_transform);
    let desired = Mat3::from_cols(desired[0], desired[1], desired[2]);
    let forward = crate::game_camera::game_rows(&transform)[0].normalize_or_zero();
    let elongation = Mat3::IDENTITY
        + Mat3::from_cols(
            forward * forward.x,
            forward * forward.y,
            forward * forward.z,
        ) * (stretch - 1.0);
    let desired = elongation * desired;
    let mut axes = desired;
    let mut position = crate::game_camera::game_eye(&transform);
    if let Some(grip) = data.and_then(|d| grip_frame(item, d)) {
        let basis = Mat3::from_cols_array_2d(&[
            grip[0][..3].try_into().unwrap(),
            grip[1][..3].try_into().unwrap(),
            grip[2][..3].try_into().unwrap(),
        ]);
        let determinant = basis.determinant();
        if determinant.is_finite() && determinant.abs() > 1e-5 {
            axes = desired * basis.inverse();
            let anchor = Vec3::new(grip[3][0], grip[3][1], grip[3][2]) * (item.scale / 1024.0);
            position -= axes * anchor;
        }
    }
    item.position = position.to_array();
    item.rows = [0, 1, 2].map(|i| {
        let r = axes.col(i);
        [r.x.to_bits(), r.y.to_bits(), r.z.to_bits(), 0]
    });
}

fn tip(item: &HandItem, data: &ItemData) -> Vec3 {
    if let Some(class) = data.class(item.o_class) {
        if let Some(chain) = class.chains.get(1).filter(|c| !c.is_empty()) {
            let poses = rc_formats::moby_anim::evaluate_chains(
                &class.anim,
                &item.anim,
                item.snapshot.as_ref(),
                &[chain.as_slice()],
            );
            if let Some(p) = poses.first() {
                let v = p[3].map(f32::to_bits);
                let point =
                    rc_formats::moby_anim::bone_points(&[v], &item.rows, item.position, item.scale)
                        [0];
                return crate::tfrag_render::game_to_bevy(
                    [0, 1, 2].map(|i| f32::from_bits(point[i])),
                );
            }
        }
    }
    crate::tfrag_render::game_to_bevy(item.position)
}

fn obstruction(
    p: &crate::gameplay::Play,
    coll: &rc_formats::collision::Collision,
    a: Vec3,
    b: Vec3,
) -> Option<rc_game::collision_query::CollOutput> {
    let src = p.svc.scene_parts(&p.game.mobys, &*p.classes);
    let scene = p.svc.scene(&src);
    rc_game::collision_query::coll_line_m(
        coll,
        Some(&scene),
        game_point(a),
        game_point(b),
        rc_game::collision_query::QueryFlags(2),
        Some(p.game.hero_moby),
    )
}
/// The native combo swings' whoosh: Ratchet's class sound 27, a marker on his own swing animations (sequences
/// 0x17..0x19, the comet throw 0x1a, the jump attack 0x2b); the wrench's swing sequences carry none.
const SWING_WHOOSH: i32 = 27;
/// The whoosh needs a big fast swing: the hand at least this fast (m/s) over a stroke at least this long (m), the
/// next one only after the hand slowed below the re-arm speed (m/s) and this delay (s).
const WHOOSH_SPEED: f32 = 3.8;
const WHOOSH_TRAVEL: f32 = 0.35;
const WHOOSH_REARM: f32 = 1.0;
const WHOOSH_GAP: f64 = 0.5;

/// The held wrench's size against the native model.
const WRENCH_SCALE: f32 = 1.20;

/// A fast swing lengthens the drawn wrench, as the native swing sequences stretch its head: up to 35 % from 1.0 to
/// 3.5 m/s. `tip` reads the stretched rows, so the hit sweep reaches out with it.
fn swing_stretch(velocity: Vec3) -> f32 {
    1.0 + ((velocity.length() - 1.0) / 2.5).clamp(0.0, 1.0) * 0.35
}

pub(super) fn crate_class(class: i16) -> bool {
    (500..=540).contains(&class)
}

// Crates may have primitive-only colliders. Query them separately so a damageable
// enemy in front cannot hide either the crate or the solid world behind it.
struct WrenchCrates<'a>(&'a dyn rc_game::collision_query::mobys::MobySource);
impl rc_game::collision_query::mobys::MobySource for WrenchCrates<'_> {
    fn moby(&self, id: usize) -> Option<rc_game::collision_query::mobys::CollMoby> {
        self.0
            .moby(id)
            .filter(|m| crate_class(m.o_class) && m.collision)
            .map(|mut m| {
                m.mode |= 0x4000;
                m
            })
    }
    fn joints(&self, id: usize, count: usize) -> Vec<[u32; 4]> {
        self.0.joints(id, count)
    }
}

// Keep the held model on the solid side of world geometry. Damageable mobys still use
// the original melee sweep below; crates also stop the held model before damage is accepted.
fn constrain_wrench(
    p: &crate::gameplay::Play,
    coll: &rc_formats::collision::Collision,
    item: &mut HandItem,
    palm: Vec3,
    eye: Vec3,
    previous: Option<Vec3>,
) -> Option<WrenchWall> {
    use rc_game::collision_query::{
        QueryFlags, coll_line, coll_line_m, coll_sphere, coll_sphere_m,
    };
    let data = p.game.item_data.as_ref()?;
    let src = p.svc.scene_parts(&p.game.mobys, &*p.classes);
    let scene = p.svc.scene(&src);
    let crates = WrenchCrates(scene.mobys);
    let crate_scene = rc_game::collision_query::mobys::MobyScene {
        mobys: &crates,
        grid: scene.grid,
        classes: scene.classes,
        cache: scene.cache,
    };
    let crate_flags = QueryFlags(1 | 0x20); // Crate meshes plus primitives, without the world pass.
    let flags = QueryFlags(2 | 0x20); // Meshes only, excluding the native non-solid surface.
    let solid = |h: &rc_game::collision_query::CollOutput| {
        h.surface_id() != 0
            && h.moby.is_none_or(|id| {
                p.game
                    .mobys
                    .mobys
                    .get(id)
                    .is_some_and(|m| m.mode & 0x4000 == 0 || crate_class(m.o_class))
            })
    };
    let line = |a: Vec3, b: Vec3| {
        if a.distance_squared(b) < 1e-8 {
            return None;
        }
        [
            coll_line(coll, game_point(a), game_point(b), flags),
            coll_line_m(
                coll,
                Some(&scene),
                game_point(a),
                game_point(b),
                flags,
                Some(p.game.hero_moby),
            ),
            coll_line_m(
                coll,
                Some(&crate_scene),
                game_point(a),
                game_point(b),
                crate_flags,
                Some(p.game.hero_moby),
            ),
        ]
        .into_iter()
        .flatten()
        .filter(solid)
        .min_by(|a, b| a.best.total_cmp(&b.best))
    };
    const RADIUS: f32 = 0.12;
    let mut contact = None;
    for _ in 0..2 {
        let head = tip(item, data);
        let mut wall = None;
        // The shaft and eye rays recover safely even when the controller itself crossed a wall.
        for a in [palm, eye] {
            if let Some(hit) = line(a, head) {
                let point = crate::tfrag_render::game_to_bevy(hit.point);
                let mut normal = crate::tfrag_render::game_to_bevy(hit.normal).normalize_or_zero();
                if normal.dot(a - point) < 0.0 {
                    normal = -normal;
                }
                wall = Some(WrenchWall {
                    point,
                    normal,
                    moby: hit.moby,
                });
                break;
            }
        }
        if wall.is_none() {
            if let Some(old) = previous.filter(|old| old.distance_squared(head) > 1e-8) {
                let dir = (head - old).normalize_or_zero();
                let side = dir.any_orthonormal_vector() * RADIUS * 0.85;
                let up = dir.cross(side);
                let mut nearest = None;
                for offset in [Vec3::ZERO, side, -side, up, -up] {
                    if let Some(hit) = line(old + offset, head + offset) {
                        if nearest.as_ref().is_none_or(
                            |h: &rc_game::collision_query::CollOutput| hit.best < h.best,
                        ) {
                            nearest = Some(hit);
                        }
                    }
                }
                if let Some(hit) = nearest {
                    let point = crate::tfrag_render::game_to_bevy(hit.point);
                    let mut normal =
                        crate::tfrag_render::game_to_bevy(hit.normal).normalize_or_zero();
                    if normal.dot(old - point) < 0.0 {
                        normal = -normal;
                    }
                    wall = Some(WrenchWall {
                        point,
                        normal,
                        moby: hit.moby,
                    });
                }
            }
        }
        if wall.is_none() {
            // Sphere overlap catches grazing contacts that do not cross a triangle plane.
            let hit = [
                coll_sphere(coll, game_point(head), RADIUS, flags),
                coll_sphere_m(
                    coll,
                    Some(&scene),
                    game_point(head),
                    RADIUS,
                    flags,
                    Some(p.game.hero_moby),
                ),
                coll_sphere_m(
                    coll,
                    Some(&crate_scene),
                    game_point(head),
                    RADIUS,
                    crate_flags,
                    Some(p.game.hero_moby),
                ),
            ]
            .into_iter()
            .flatten()
            .filter(solid)
            .min_by(|a, b| a.best.total_cmp(&b.best));
            if let Some(hit) = hit {
                let point = crate::tfrag_render::game_to_bevy(hit.point);
                let normal = (head - point).try_normalize().unwrap_or_else(|| {
                    crate::tfrag_render::game_to_bevy(hit.normal).normalize_or_zero()
                });
                wall = Some(WrenchWall {
                    point,
                    normal,
                    moby: hit.moby,
                });
            }
        }
        let Some(wall) = wall.filter(|w| w.normal.length_squared() > 0.5) else {
            break;
        };
        let penetration = RADIUS - wall.normal.dot(head - wall.point);
        if penetration < -0.005 {
            break;
        }
        let shift = wall.normal * penetration.max(0.0);
        let origin = crate::tfrag_render::game_to_bevy(item.position) + shift;
        item.position = game_point(origin);
        contact.get_or_insert(wall);
    }
    contact
}

pub(super) fn unobstructed(
    p: &crate::gameplay::Play,
    coll: &rc_formats::collision::Collision,
    a: Vec3,
    b: Vec3,
) -> bool {
    obstruction(p, coll, a, b)
        .is_none_or(|h| Vec3::from(game_point(b)).distance(Vec3::from(h.point)) < 0.03)
}
pub(super) fn game_point(p: Vec3) -> [f32; 3] {
    [p.x, -p.z, p.y]
}

fn interact(
    play: Option<ResMut<crate::gameplay::Play>>,
    level: Res<crate::Level>,
    settings: Res<Settings>,
    rig: Res<Rig>,
    mut motion: ResMut<Motion>,
    mut melee: ResMut<Melee>,
    mut feedback: ResMut<super::game_input::Feedback>,
    time: Res<Time<Real>>,
    mode: Res<crate::menu_render::MenuMode>,
    mut session: Option<ResMut<crate::gameplay::Session>>,
    bomb: Res<super::bomb_glove::HeldBomb>,
    input: Res<HandInput>,
) {
    let Some(mut play) = play else { return };
    let p = &mut *play;
    p.game.hero.tracked.crank_target = None;
    p.game.hero.tracked.crank_turn = 0.0;
    let Some(root) = rig.root_for(&p.game, settings.units_per_metre) else {
        return;
    };
    let ready =
        gameplay(&mode)
            && p.game.hero.mode == 0
            && !p.death_pending
            && p.game.hero.health > 0
            && super::game::ship_view(p).is_none()
            && super::turret::manned(p).is_none();
    if !ready {
        motion.release = None;
        melee.reset_stroke();
        melee.crank = None;
        melee.crank_previous = None;
        melee.crank_far_since = None;
        p.game.hero.items.tracked_hand = None;
        return;
    }
    // A fresh grip requests the wrench once; native quick-select requests remain authoritative afterward.
    let free = right_hand_free(&p.game.hero);
    let using_hands = !free
        && (p.game.hero.f838 != 0
            || p.game.hero.tracked.grabbed.iter().any(|&g| g)
            || matches!(p.game.hero.group, 3 | 0x11 | 0x12));
    if using_hands {
        if motion.equip_wrench {
            motion.held = false;
            motion.grip_suppressed = motion.grip_down;
        }
        motion.equip_wrench = false;
        motion.equip_ready_tick = None;
        motion.release = None;
    }
    // With the Walloper, a grip takes the wrench back; a fist closed to punch does not (the hand still, no lunge, for
    // 9 ticks).
    let walloper = p.game.hero.items.slot.id == rc_game::hero::walloper::WALLOPER;
    if walloper
        && motion.equip_wrench
        && (p.game.hero.state == rc_game::hero::walloper::LUNGE || motion.velocity.length() > 0.9)
    {
        motion.held = false;
        motion.grip_suppressed = motion.grip_down;
        motion.equip_wrench = false;
        motion.equip_ready_tick = None;
    }
    if input.left_grip.is_some_and(|v| v > 0.5) && !free {
        motion.equip_wrench = false;
        motion.equip_ready_tick = None;
    }
    if p.game.hero.tracked.grabbed[1] || (matches!(p.game.hero.state, 0x18..=0x1b) && !free) {
        motion.held = false;
        motion.grip_suppressed = motion.grip_down;
    }
    if motion.equip_wrench {
        // Let one native collision/probe tick discover a ledge before a fresh squeeze equips a wrench.
        let ready_tick = *motion.equip_ready_tick.get_or_insert(p.game.counter + if walloper { 9 } else { 1 });
        if let Some(s) = session.as_mut().filter(|_| p.game.counter >= ready_tick) {
            s.0.temp_hand = 8;
            p.game.hero.items.restore_pending = 0;
            motion.equip_wrench = false;
            motion.equip_ready_tick = None;
        }
    }
    let Some(hand) = motion.hands[1] else {
        p.game.hero.items.tracked_hand = None;
        melee.reset_stroke();
        return;
    };
    let world_hand = root.mul_transform(hand);
    let Some(coll) = level.0.collision.as_ref() else {
        return;
    };
    let eye = rig.body_eye(&p.game);
    let clear = unobstructed(p, coll, eye, world_hand.translation);
    let Some(mut item) = p
        .game
        .hero
        .items
        .slot
        .item
        .as_ref()
        .filter(|i| i.o_class == rc_game::hero::melee::WRENCH_CLASS)
        .cloned()
    else {
        p.game.hero.items.tracked_hand = None;
        melee.reset_stroke();
        return;
    };
    if melee.revision != rig.revision
        || melee.level != crate::level_load::level_index()
        || p.game.counter < melee.last_tick
    {
        // A snap turn changes the view, not the wrench's attachment to the bolt nor the hit cooldowns.
        let same = melee.level == crate::level_load::level_index() && p.game.counter >= melee.last_tick;
        let crank = same.then_some(melee.crank).flatten();
        let (hits, slam_until) = if same { (std::mem::take(&mut melee.hits), melee.slam_until) } else { default() };
        *melee = Melee {
            revision: rig.revision,
            level: crate::level_load::level_index(),
            crank,
            hits,
            slam_until,
            ..default()
        };
    }
    melee.last_tick = p.game.counter;
    let stretch = if item.mstate == 0 { swing_stretch(motion.velocity) } else { 1.0 };
    place(&mut item, world_hand, p.game.item_data.as_ref(), stretch);
    if crank_wrench(
        p,
        &mut item,
        &mut motion,
        &mut melee,
        hand,
        &root,
        time.elapsed_secs_f64(),
        &level.0,
    ) {
        p.game.hero.items.tracked_hand = Some((item.position, item.rows));
        p.game.hero.items.tracked_class = item.o_class;
        melee.reset_stroke();
        return;
    }
    let old_safe = melee.safe_head.map(|v| root.transform_point(v));
    let raw_head = p.game.item_data.as_ref().map(|data| tip(&item, data));
    let wall = if item.mstate == 0 && motion.wrench_held() && !using_hands && bomb.id.is_none() {
        constrain_wrench(p, coll, &mut item, world_hand.translation, eye, old_safe)
    } else {
        None
    };
    let safe_head = p.game.item_data.as_ref().map(|data| tip(&item, data));
    // The native gripping hand follows the stopped model, including its original grip pose.
    let mut shown = item.clone();
    if rebound(&mut shown, &melee, &root, time.elapsed_secs_f64()) {
        constrain_wrench(p, coll, &mut shown, world_hand.translation, eye, old_safe);
    }
    p.game.hero.items.tracked_hand = Some((shown.position, shown.rows));
    p.game.hero.items.tracked_class = item.o_class;
    if let Some(native) = p.game.hero.items.slot.item.as_mut() {
        native.scale = item.scale;
    }
    if bomb.id.is_some() || using_hands {
        melee.reset_stroke();
        motion.release = None;
        return;
    }
    let release = motion.release.take();
    if item.mstate != 0 || !motion.wrench_held() {
        melee.reset_stroke();
        if item.mstate == 0 && clear {
            if let Some((pose, velocity, _)) = release {
                let launch = root.mul_transform(pose);
                if unobstructed(p, coll, eye, launch.translation) {
                    launch_wrench(&mut p.game, launch, root.rotation * velocity);
                }
            }
        }
        return;
    }
    if melee.serial == motion.serial {
        return;
    }
    melee.serial = motion.serial;
    let now = time.elapsed_secs_f64();
    melee.safe_head = safe_head.map(|v| root.compute_affine().inverse().transform_point3(v));
    if let Some(wall) = wall {
        if let (Some((old, then)), Some(raw_head), Some(data)) =
            (melee.previous, raw_head, p.game.item_data.as_ref())
        {
            let dt = (now - then) as f32;
            if (0.004..=0.12).contains(&dt) && old.translation.distance(hand.translation) < 0.5 {
                let mut old_item = item.clone();
                place(&mut old_item, root.mul_transform(old), Some(data), stretch);
                let velocity = (raw_head - tip(&old_item, data)) / (dt * settings.units_per_metre);
                let force = (-velocity.dot(wall.normal)).max(0.0);
                if melee.wall_armed
                    && now >= melee.wall_until
                    && force >= 1.3
                    && motion.velocity.length() >= 0.65
                    && p.game.hero.state != 0x14
                {
                    feedback.wrench_wall = Some(feedback.wrench_wall.unwrap_or(0.0).max(force));
                    let crate_contact = wall.moby.is_some_and(|id| {
                        p.game
                            .mobys
                            .mobys
                            .get(id)
                            .is_some_and(|m| crate_class(m.o_class))
                    });
                    if crate_contact {
                        p.game
                            .hero
                            .fx
                            .item_sounds
                            .push(rc_game::hero::melee::wrench_hit_sound(
                                &p.game.mobys,
                                wall.moby,
                            ));
                    } else {
                        p.game.hero.fx.item_sounds.push(2);
                        rc_game::hero::fx::wrench_burst(
                            &mut p.game.hero,
                            &mut p.game.rng,
                            game_point(wall.point),
                            game_point(eye),
                        );
                    }
                    melee.wall_until = now + 0.22;
                    let direction = root.rotation.inverse() * -velocity.normalize_or_zero();
                    melee.rebound =
                        Some((now, direction * (0.025 + force * 0.003).clamp(0.03, 0.055)));
                }
            }
        }
        // A new impact requires a real withdrawal, never repeated pressure against the wall.
        melee.wall_armed = false;
        melee.wall = Some(wall);
    } else if melee.wall.is_none_or(|wall| {
        raw_head.is_some_and(|head| {
            wall.normal.dot(head - wall.point) > 0.16 || head.distance(wall.point) > 0.35
        })
    }) {
        melee.wall_armed = true;
        melee.wall = None;
    }
    if !clear {
        melee.previous = Some((hand, now));
        melee.swing_from = None;
        melee.down_from = None;
        return;
    }
    if motion.slam_velocity.y > -0.25 {
        melee.down_from = Some(hand.translation);
    }
    // A Hyper Strike: a mostly downward stroke of 8 cm at 1.6 m/s (lowering the arm after a jump is not one).
    let down = -motion.slam_velocity.y;
    if !p.game.hero.grounded()
        && now >= melee.slam_until
        && down > 1.6
        && down > 0.8 * Vec2::new(motion.slam_velocity.x, motion.slam_velocity.z).length()
        && melee
            .down_from
            .is_some_and(|v| v.y - hand.translation.y >= 0.08)
    {
        p.game.hero.tracked.hyper_strike = true;
        melee.slam_until = now + 0.3;
        melee.down_from = None;
        // The strike hits through the native sphere; this frame's swing hit would land on top of it.
        melee.previous = Some((hand, now));
        return;
    }
    // Require a deliberate translating hand stroke; a high-speed tip from tiny wrist twitches is insufficient.
    if motion.velocity.length() < 0.65 || melee.swing_from.is_none() {
        melee.swing_from = Some(hand.translation);
    }
    let travel = melee
        .swing_from
        .map_or(0.0, |start| start.distance(hand.translation));
    let hand_speed = motion.velocity.length();
    // The native combo swings whoosh (Ratchet's swing animations play it): once per big fast stroke, hit or miss,
    // re-armed when the hand slows down.
    if hand_speed < WHOOSH_REARM {
        melee.whooshed = false;
    } else if !melee.whooshed
        && hand_speed >= WHOOSH_SPEED
        && travel >= WHOOSH_TRAVEL
        && now >= melee.whoosh_until
        && p.game.hero.state != 0x14
    {
        p.game.hero.fx.item_voices.push(rc_game::hero::packs::SoundCmd::Voice { index: SWING_WHOOSH, flags: 0 });
        melee.whooshed = true;
        melee.whoosh_until = now + WHOOSH_GAP;
    }
    let previous = melee.previous.replace((hand, now));
    if p.game.hero.state == 0x14 {
        return;
    }
    let Some((old, then)) = previous else { return };
    let dt = (now - then) as f32;
    if !(0.004..=0.12).contains(&dt) || old.translation.distance(hand.translation) > 0.5 {
        return;
    }
    let current_item = item.clone();
    let mut previous_item = item;
    place(
        &mut previous_item,
        root.mul_transform(old),
        p.game.item_data.as_ref(),
        stretch,
    );
    let Some(data) = p.game.item_data.as_ref() else {
        return;
    };
    let extend = |item: &HandItem, palm: Vec3| {
        let head = tip(item, data);
        head + (head - palm).normalize_or_zero() * (0.17 * swing_stretch(motion.velocity))
    };
    let old_raw = tip(&previous_item, data);
    let old_palm = root.mul_transform(old).translation;
    let end = extend(
        &current_item,
        world_hand.translation + safe_head.unwrap_or_default() - raw_head.unwrap_or_default(),
    );
    let previous_shift = old_safe.unwrap_or(old_raw) - old_raw;
    previous_item.position =
        game_point(crate::tfrag_render::game_to_bevy(previous_item.position) + previous_shift);
    let start = extend(&previous_item, old_palm + previous_shift);
    let speed = old_raw.distance(raw_head.unwrap_or(old_raw)) / (dt * settings.units_per_metre);
    // Kinetic demand rises with speed squared: these minima are substantially above
    // the old 4.0/1.8 m/s gate, and still reject a tiny wrist-only flick.
    if speed < 5.5 || hand_speed < 2.4 || travel < 0.14 {
        return;
    }
    let push = Vec3::from(game_point((end - start).normalize_or_zero()));
    let tmpl = HitTemplate {
        dir: [Pf::f(push.x), Pf::f(push.y), Pf::ONE, Pf::b(0x45af_df66)],
        attacker: None,
        flags: 0x1_0000,
        b18: 0,
        b19: 1,
        h1a: rc_game::hero::melee::WRENCH_CLASS as u16,
        damage: Pf::ONE,
        w20: 1,
    };
    let mut candidates = HashSet::new();
    candidates.extend(wall.and_then(|contact| contact.moby));
    // Sweep the actual model's head, with small steps so fast movements cannot skip a crate.
    let steps = ((end.distance(start) / 0.12).ceil() as usize).clamp(1, 24);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let mut point = start.lerp(end, t);
        let grip = root
            .mul_transform(old)
            .translation
            .lerp(world_hand.translation, t);
        if let Some(h) = obstruction(p, coll, grip, point) {
            if let Some(id) = h.moby {
                candidates.insert(id);
            }
            point = crate::tfrag_render::game_to_bevy(h.point);
        }
        let ids = services::sphere_mobys_in(
            &mut p.game.mobys,
            &mut p.svc,
            &*p.classes,
            Pf::f(0.35),
            from_f32x3(game_point(point)),
            0,
            Some(p.game.hero_moby),
            None,
        );
        candidates.extend(ids);
    }
    let mut hit = None;
    for id in candidates {
        if melee
            .hits
            .get(&id)
            .is_some_and(|t| p.game.counter.saturating_sub(*t) < 18)
        {
            continue;
        }
        let Some(m) = p.game.mobys.mobys.get(id).filter(|m| m.mode & 0x4000 != 0) else {
            continue;
        };
        let crate_hit = crate_class(m.o_class);
        if crate_hit
            && (speed < 6.5
                || hand_speed < 2.8
                || travel < 0.16
                || wall.and_then(|contact| contact.moby) != Some(id))
        {
            continue;
        }
        let target =
            crate::tfrag_render::game_to_bevy([m.position[0], m.position[1], m.position[2] + 0.3]);
        if obstruction(p, coll, world_hand.translation, target).is_some_and(|h| h.moby != Some(id))
        {
            continue;
        }
        let slot = p.svc.hits.next;
        services::deliver_hit_in(&mut p.game.mobys, &mut p.svc.hits, id, &tmpl);
        if p.svc.hits.next != slot {
            p.svc.hits.records[slot as usize].wrench = true;
            melee.hits.insert(id, p.game.counter);
            hit = Some(id);
        }
    }
    melee
        .hits
        .retain(|_, t| p.game.counter.saturating_sub(*t) < 120);
    if let Some(id) = hit {
        p.game.hero.tracked.wrench_hits = p.game.hero.tracked.wrench_hits.wrapping_add(1);
        feedback.wrench_impact = true;
        let sound = rc_game::hero::melee::wrench_hit_sound(&p.game.mobys, Some(id));
        if wall.and_then(|contact| contact.moby) != Some(id) || feedback.wrench_wall.is_none() {
            p.game.hero.fx.item_sounds.push(sound);
        }
        println!(
            "VR_GAME tracked_wrench_hit target={id} tip_speed={speed:.2} hand_speed={hand_speed:.2} travel={travel:.2}"
        );
    }
}

/// A level moby's high-LOD vertices in its current pose, relative to its origin (game coordinates).
pub(super) fn posed_offsets(
    m: &rc_game::moby_runtime::Moby,
    level: &crate::level_load::LoadedLevel,
) -> Vec<Vec3> {
    let Some(ci) = level.mobys.classes.iter().position(|c| c.o_class == m.o_class as i32) else {
        return Vec::new();
    };
    let class = &level.mobys.classes[ci].class;
    let palette = rc_formats::moby_anim::evaluate_with_snapshot(&level.mobys.anim[ci], &m.anim, None);
    let axes = Mat3::from_cols_array_2d(&[0, 1, 2].map(|i| {
        let r = m.rows[i];
        [r[0], r[1], r[2]]
    }));
    class
        .high_lod
        .iter()
        .flat_map(|p| p.vertices.iter())
        .map(|v| {
            let raw = Vec3::new(v.x as f32, v.y as f32, v.z as f32);
            let posed = if v.skin.count == 0 {
                raw
            } else {
                (0..v.skin.count as usize)
                    .map(|k| {
                        palette.get(v.skin.joints[k] as usize).map_or(raw, |p| {
                            Mat4::from_cols_array_2d(p).transform_point3(raw)
                        }) * (v.skin.weights[k] as f32 / 256.0)
                    })
                    .sum()
            };
            axes * posed * (m.scale / 1024.0)
        })
        .collect()
}
/// A class-280 bolt's hex head: the bolt axis at its mid-height, and its top and bottom (Bevy coordinates).
#[derive(Clone, Copy)]
struct CrankSocket {
    nut: Vec3,
    top: f32,
    bottom: f32,
}
fn crank_socket(
    play: &crate::gameplay::Play,
    id: usize,
    level: &crate::level_load::LoadedLevel,
) -> Option<CrankSocket> {
    let m = play.game.mobys.mobys.get(id)?;
    if !(m.o_class == 280 && matches!(m.state, 1 | 3) && m.pvars.len() >= 0x40) {
        return None;
    }
    let offsets = posed_offsets(m, level);
    // The hex head is the widest part of the bolt: everything beyond 75 % of its largest radius.
    let widest = offsets.iter().map(|v| v.truncate().length()).fold(0.0, f32::max);
    let head = offsets.iter().filter(|v| v.truncate().length() >= widest * 0.75);
    let top = offsets.iter().map(|v| v.z).fold(f32::NEG_INFINITY, f32::max);
    let bottom = head.map(|v| v.z).fold(f32::INFINITY, f32::min);
    let (top, bottom) = if top.is_finite() && bottom.is_finite() {
        (top, bottom.min(top))
    } else {
        (1.0, 0.8)
    };
    let z = m.position[2];
    Some(CrankSocket {
        nut: crate::tfrag_render::game_to_bevy([
            m.position[0],
            m.position[1],
            z + (top + bottom) * 0.5,
        ]),
        top: z + top,
        bottom: z + bottom,
    })
}
// The original crank pose straddles the bolt's hex head between the two jaws that sequence 0xb opens.
// This point of the wrench model (raw units, sequence 0xb frame 2) lies on the bolt axis, at the jaws'
// mid-thickness, in Ratchet's native 0x3f/0x40/0x41 poses on his 1.05 ring (measured from the original
// assets); the grip (joint list 0) is at x = -90 on the same shaft.
const CRANK_JAWS: Vec3 = Vec3::new(10670.0, -930.0, -351.0);
const CRANK_GRIP_X: f32 = -90.0;
fn seat_wrench(item: &mut HandItem, data: &ItemData, socket: CrankSocket, hand: Transform) {
    let Some(class) = data.class(item.o_class) else {
        return;
    };
    // Original bolt-crank pose, including the wrench's authored extension.
    if class.anim.frame(0xb, 2).is_some() {
        item.anim.seq_a = 0xb;
        item.anim.seq_b = 0xb;
        item.anim.frame_a = 2;
        item.anim.frame_b = 2;
        item.anim.t = 0.0;
        item.snapshot = None;
    }
    item.scale = class.scale * WRENCH_SCALE;
    let k = item.scale / 1024.0;
    let nut = Vec3::from_array(game_point(socket.nut));
    let palm = Vec3::from_array(game_point(hand.translation));
    // The shaft heads from the hand to the bolt; the jaws stay nearly level around the hex head.
    let to = nut - palm;
    let flat = to.truncate();
    let heading = if flat.length() > 0.05 {
        flat.normalize()
    } else {
        let f = crate::game_camera::game_rows(&hand)[0].truncate();
        if f.length() > 0.05 { f.normalize() } else { Vec2::X }
    };
    let pitch = to.z.atan2(flat.length()).clamp(-0.35, 0.35);
    let x = (heading * pitch.cos()).extend(pitch.sin());
    let z = (Vec3::Z - x * x.z).normalize();
    let y = z.cross(x);
    // Lengthen or shorten the shaft about the jaws so the handle meets the palm; the head keeps its width.
    let length = (CRANK_JAWS.x - CRANK_GRIP_X) * k;
    let ratio = (to.dot(x) / length).clamp(0.55, 2.5);
    let rows = [x * ratio, y, z];
    let jaws = rows[0] * CRANK_JAWS.x * k + rows[1] * CRANK_JAWS.y * k + rows[2] * CRANK_JAWS.z * k;
    item.position = (nut - jaws).to_array();
    item.rows = rows.map(|r| [r.x.to_bits(), r.y.to_bits(), r.z.to_bits(), 0]);
}
fn crank_wrench(
    play: &mut crate::gameplay::Play,
    item: &mut HandItem,
    motion: &mut Motion,
    melee: &mut Melee,
    hand: Transform,
    root: &Transform,
    now: f64,
    level: &crate::level_load::LoadedLevel,
) -> bool {
    let was_attached = melee.crank.is_some();
    let data = play.game.item_data.as_ref();
    let ready = motion.wrench_held() && item.mstate == 0 && data.is_some();
    if !ready
        || melee
            .crank
            .is_some_and(|id| crank_socket(play, id, level).is_none())
    {
        melee.crank = None;
        melee.crank_previous = None;
        melee.crank_far_since = None;
        if was_attached {
            motion.release = None;
        }
        return was_attached;
    }
    let data = data.unwrap();
    if melee.crank.is_none() {
        let head = tip(item, data);
        melee.crank = play
            .game
            .mobys
            .mobys
            .iter()
            .enumerate()
            .filter_map(|(id, _)| {
                let socket = crank_socket(play, id, level)?;
                let eye = crate::tfrag_render::game_to_bevy(play.game.hero.position());
                // Bring the wrench head to the hex head: beside it, on it or just above it.
                let beside = Vec2::new(head.x - socket.nut.x, head.z - socket.nut.z).length();
                (socket.nut.distance(eye) < 3.2
                    && beside < 0.55
                    && (socket.bottom - 0.45..socket.top + 0.45).contains(&head.y))
                    .then_some((id, socket.nut.distance_squared(head)))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id);
    }
    let Some(id) = melee.crank else { return false };
    let socket = crank_socket(play, id, level).unwrap();
    let nut = socket.nut;
    // Withdraw from the bolt to release; walking around it keeps the wrench head seated.
    let body = crate::tfrag_render::game_to_bevy(play.game.hero.position());
    if !was_attached {
        play.game.hero.tracked.crank_radius = Vec2::new(body.x - nut.x, body.z - nut.z)
            .length()
            .clamp(0.8, 2.5);
    }
    if root.mul_transform(hand).translation.distance(nut) > 2.40 || body.distance(nut) > 3.40
    {
        let since = *melee.crank_far_since.get_or_insert(now);
        if now - since >= 0.25 {
            melee.crank = None;
            melee.crank_previous = None;
            melee.crank_far_since = None;
            motion.release = None;
            return true;
        }
    } else {
        melee.crank_far_since = None;
    }
    let mut angle = 0.0;
    if let Some(previous) = melee.crank_previous {
        let before = Vec2::new(previous.x - nut.x, previous.z - nut.z);
        let after = Vec2::new(body.x - nut.x, body.z - nut.z);
        // Observe native capsule positions after collision: stick movement and room-scale walking
        // count equally. Wrist rotation, head turns and moving just the hand cannot turn the bolt.
        if body.distance(previous) < 0.30
            && before.length() >= 0.60
            && after.length() >= 0.60
            && after.length() < 3.0
        {
            let orbit = before.perp_dot(after).atan2(before.dot(after));
            if orbit.abs() <= 0.30 {
                angle = orbit;
            }
        }
    }
    melee.crank_previous = Some(body);
    play.game.hero.tracked.crank_target = Some(id);
    play.game.hero.tracked.crank_turn = angle;
    seat_wrench(item, data, socket, root.mul_transform(hand));
    motion.release = None;
    true
}

/// The middle of the ready-pose wrench (sequence 1 frame 0, raw model units: its bounds' centre, on the shaft line). The
/// thrown wrench's flight position is this point: the original spin pose is centred on its origin, while the rigid
/// ready mesh's origin is at the grip, which swept the head through a metre-wide circle past the eyes.
const WRENCH_MIDDLE: Vec3 = Vec3::new(1513.0, -1200.0, -350.0);
fn wrench_middle(item: &HandItem, rows: Mat3) -> Vec3 {
    rows * WRENCH_MIDDLE * (item.scale / 1024.0)
}
fn item_axes(item: &HandItem) -> Mat3 {
    Mat3::from_cols_array_2d(&item.rows.map(|r| [f32::from_bits(r[0]), f32::from_bits(r[1]), f32::from_bits(r[2])]))
}

/// The hand speed at the grip's release that throws the wrench (metres / second).
const WRENCH_THROW_SPEED: f32 = 2.5;

fn launch_wrench(game: &mut rc_game::tick::Game, hand: Transform, velocity: Vec3) {
    let h = &mut game.hero;
    if h.items.slot.state != 2 {
        return;
    }
    let Some(it) = h.items.slot.item.as_mut() else {
        return;
    };
    place(it, hand, game.item_data.as_ref(), 1.0);
    // The flight carries the wrench's middle, from where it is in the hand.
    it.position = (Vec3::from_array(it.position) + wrench_middle(it, item_axes(it))).to_array();
    let dir = game_point(velocity.normalize_or_zero());
    it.mstate = rc_game::hero::comet::OUT;
    it.flight = rc_game::hero::comet::Flight {
        dir,
        speed: (velocity.length() * 4.0).clamp(10.0, 23.0) * rc_game::hero::physics::DT.to_f32(),
        accel: 0.0,
        wall_close: false,
        // Keep the native spinning plane horizontal, regardless of release wrist pitch/roll.
        euler: [0.0, 0.0, dir[1].atan2(dir[0])],
    };
    h.items.slot.detached = 1;
    h.items.pending_blend = Some((6, 0, 5));
    h.melee.hit = 0;
    h.tracked.wrench_return = None;
    // Native screen-space additive ghosts can cross the stereo near plane on release.
    h.fx.trails.wrench.kill();
    println!(
        "VR_GAME tracked_wrench_throw origin={:?} direction={dir:?}",
        it.position
    );
}

/// Diagnostics of the thrown wrench around its turnaround: what each rendered frame draws against the 60 Hz game.
#[derive(Default)]
struct WrenchTrace {
    /// `RC_VR_WRENCH_TRACE=1` (read once).
    enabled: Option<bool>,
    stage: u8,
    since: f64,
    frames: u32,
    counter: u64,
}
impl WrenchTrace {
    fn frame(&mut self, it: &HandItem, game: &rc_game::tick::Game, now: f64, overstep: f32) {
        let enabled = *self.enabled.get_or_insert_with(|| std::env::var("RC_VR_WRENCH_TRACE").is_ok_and(|v| v == "1"));
        if !enabled || it.o_class != rc_game::hero::melee::WRENCH_CLASS {
            return;
        }
        if it.mstate != self.stage {
            (self.stage, self.since, self.frames) = (it.mstate, now, 0);
        }
        let ticks = game.counter.wrapping_sub(self.counter);
        self.counter = game.counter;
        let sim = game.hero.items.slot.item.as_ref().map_or([0.0; 3], |i| i.position);
        // The last slow outbound frames and the first half second back.
        let slow = it.mstate == rc_game::hero::comet::OUT && it.flight.speed < 4.0 / 60.0;
        if (slow || it.mstate == rc_game::hero::comet::BACK) && self.frames < 36 {
            self.frames += 1;
            println!(
                "VR_GAME wrench_frame stage={} t={:.1}ms ticks={ticks} overstep={overstep:.2} drawn={:?} sim={:?} spin={:.2}",
                it.mstate,
                (now - self.since) * 1000.0,
                it.position,
                sim,
                f32::from_bits(it.rows[0][1]).atan2(f32::from_bits(it.rows[0][0])),
            );
        }
    }
}

/// The worn items in first person (the VR Options page, crate::render_settings::VrOptions): the helmet the hero wears
/// (in play, in his scenes, in the ship sequences) on the player's head, the eye behind its visor; the worn boots at
/// the feet.
fn present_worn(
    views: Option<Res<bevy_mod_openxr::resources::OxrViews>>,
    settings: Res<Settings>,
    rig: Res<Rig>,
    options: Option<Res<crate::render_settings::VrOptions>>,
    play: Option<Res<crate::gameplay::Play>>,
    (scene, scene_rt): (Res<crate::scene_render::ActiveScene>, Res<crate::scene_render::SceneRuntime>),
    attach: Option<ResMut<crate::moby_attach::MobyAttach>>,
) {
    let (Some(play), Some(mut attach)) = (play, attach) else { return };
    let (helmet, boots) = options.map_or((true, true), |o| (o.helmet, o.boots));
    let head = super::game::pose(views.as_deref())
        .or_else(|| settings.desktop_preview.then(|| Transform::from_xyz(0.0, 1.6, 0.0)));
    // The root is the cinematic one in scenes and ship sequences: the player's head in the world in every case.
    let world = head.and_then(|h| rig.view_root_for(&play.game, settings.units_per_metre, h).map(|root| root.mul_transform(h)));
    let class = if scene.running {
        crate::scene_render::worn_head_class(&scene_rt)
    } else if play.svc.game_mode == 6 {
        // The ship sequences' actor wears its own helmet moby (rc_game::travel::space): the same class.
        play.game.mobys.mobys.iter().find(|m| m.state < 0x80 && matches!(m.o_class, 0x509 | 0x50a)).map(|m| m.o_class)
    } else if play.game.hero.mode == 0 && !play.death_pending {
        crate::moby_attach::worn_head_class(&play)
    } else {
        None
    };
    attach.tracked_head(world.filter(|_| helmet), class);
    attach.tracked_feet(boots);
}

fn present(
    mut commands: Commands,
    body: Query<(Entity, Has<RenderLayers>), With<crate::gameplay::RatchetMesh>>,
    play: Option<Res<crate::gameplay::Play>>,
    settings: Res<Settings>,
    rig: Res<Rig>,
    motion: Res<Motion>,
    bomb: Res<super::bomb_glove::HeldBomb>,
    mode: Res<crate::menu_render::MenuMode>,
    mut attach: Option<ResMut<crate::moby_attach::MobyAttach>>,
    mut alignment_report: Local<Option<u64>>,
    mut bomb_glove_report: Local<bool>,
    support: Res<super::blaster::SupportGrip>,
    level: Res<crate::Level>,
    melee: Res<Melee>,
    (time, fixed, mut trace, input, akimbo, left_bomb): (
        Res<Time<Real>>,
        Res<Time<Fixed>>,
        Local<WrenchTrace>,
        Res<HandInput>,
        Res<super::akimbo::Akimbo>,
        Res<super::bomb_glove::LeftBomb>,
    ),
    mut flight_report: Local<Option<u8>>,
) {
    // Ratchet's body is on a layer no eye renders, unless the Player body option draws it in play (its head and
    // forearms collapsed, super::body).
    let body_drawn = super::body::in_play();
    for (e, layered) in &body {
        if !body_drawn && !layered {
            commands.entity(e).insert(RenderLayers::layer(61));
        } else if body_drawn && layered {
            commands.entity(e).remove::<RenderLayers>();
        }
    }
    let root = play
        .as_ref()
        .and_then(|p| rig.root_for(&p.game, settings.units_per_metre));
    let active = gameplay(&mode)
        && play
            .as_ref()
            .is_some_and(|p| {
                p.game.hero.mode == 0
                    && p.game.hero.health > 0
                    && !p.death_pending
                    && super::game::ship_view(p).is_none()
            });
    let bomb_glove = play.as_ref().and_then(|p| {
        let selected = rc_game::hero::items::is_glove(p.game.hero.items.slot.id);
        if !active
            || p.game.hero.hand_hidden()
            || p.game.hero.tracked.grabbed[1]
            || attach.is_none()
            || !(selected || bomb.id.is_some())
        {
            return None;
        }
        let hand = root?.mul_transform(motion.hands[1]?);
        super::bomb_glove::appearance(
            hand,
            p.game.item_data.as_ref()?,
            if bomb.id.is_some() {
                bomb.item
            } else {
                p.game.hero.items.slot.id
            },
        )
    });
    if *bomb_glove_report != bomb_glove.is_some() {
        *bomb_glove_report = bomb_glove.is_some();
        println!(
            "VR_GAME bomb_glove_appearance active={}",
            *bomb_glove_report
        );
    }
    if let (Some(play), Some(root), Some(attach)) = (play, root, attach.as_mut()) {
        if !gameplay(&mode) {
            attach.tracked_items(&[]);
            attach.tracked_hands([None; 2], &play, None, false, [0.0; 2]);
            attach.tracked_body_hands([None; 2], 0, [0.0; 2], &play.game.hero);
            return;
        }
        let mut item = play.game.hero.items.slot.item.clone();
        let holding_wrench = motion.wrench_held();
        if let Some(it) = item.as_mut() {
            let native_anim = (it.anim.seq_a, it.anim.seq_b, it.anim.t);
            rigid_wrench_pose(it, play.game.item_data.as_ref());
            if it.o_class == rc_game::hero::melee::WRENCH_CLASS {
                if *flight_report != Some(it.mstate) && (it.mstate != 0 || flight_report.is_some())
                {
                    println!(
                        "VR_GAME wrench_model stage={} native_anim={native_anim:?} render_anim=({}, {}) pos={:?} scale={}",
                        it.mstate, it.anim.seq_a, it.anim.seq_b, it.position, it.scale
                    );
                }
                *flight_report = Some(it.mstate);
            } else {
                *flight_report = None;
            }
        } else {
            *flight_report = None;
        }
        if let (Some(it), Some(hand)) = (item.as_mut(), motion.hands[1]) {
            if let Some(back) = play.game.hero.tracked.wrench_return.filter(|_| {
                it.o_class == rc_game::hero::melee::WRENCH_CLASS
                    && it.mstate == rc_game::hero::comet::BACK
            }) {
                let mut caught = it.clone();
                place(
                    &mut caught,
                    root.mul_transform(hand),
                    play.game.item_data.as_ref(),
                    1.0,
                );
                // The game's planned path, sampled between its 60 Hz ticks and aimed at the hand as it is now.
                let p = (back.progress + back.step * fixed.overstep_fraction()).min(1.0);
                let held = item_axes(&caught);
                let target = Vec3::from_array(caught.position) + wrench_middle(&caught, held);
                let (at, spin) = back.at(p, target.to_array());
                // The spin is phased onto the held heading; the last fifth only settles the hand's tilt,
                // so the blend never chases a fast spin through the opposite orientation.
                let settle = ((p - 0.8) / 0.2).clamp(0.0, 1.0);
                let settle = settle * settle * (3.0 - 2.0 * settle);
                let rotation = Quat::from_rotation_z(spin)
                    .slerp(Quat::from_mat3(&held).normalize(), settle);
                let rows = Mat3::from_quat(rotation);
                // The path carries the middle; the model's origin is the grip end.
                it.position = (Vec3::from(at) - wrench_middle(it, rows)).to_array();
                let rows = rows.to_cols_array_2d();
                it.rows = rows.map(|r| [r[0].to_bits(), r[1].to_bits(), r[2].to_bits(), 0]);
            } else if it.o_class == rc_game::hero::melee::WRENCH_CLASS && it.mstate != 0 {
                // Outbound (and the turnaround tick before the return is planned): the flight carries the middle.
                it.position = (Vec3::from_array(it.position) - wrench_middle(it, item_axes(it))).to_array();
            }
            trace.frame(it, &play.game, time.elapsed_secs_f64(), fixed.overstep_fraction());
            if super::blaster::supported(play.game.hero.items.slot.id)
                && !rc_game::hero::items::is_glove(play.game.hero.items.slot.id)
            {
                if let Some(data) = play.game.item_data.as_ref() {
                    super::blaster::place(
                        it,
                        root.mul_transform(super::blaster::weapon_hand(
                            hand,
                            motion.hands[0],
                            &support,
                        )),
                        data,
                        support.primary,
                    );
                }
            } else if it.o_class == rc_game::hero::melee::WRENCH_CLASS && it.mstate == 0 {
                place(
                    it,
                    root.mul_transform(hand),
                    play.game.item_data.as_ref(),
                    swing_stretch(motion.velocity),
                );
                if let Some((data, socket)) = play
                    .game
                    .item_data
                    .as_ref()
                    .zip(melee.crank.and_then(|id| crank_socket(&play, id, &level.0)))
                {
                    seat_wrench(it, data, socket, root.mul_transform(hand));
                } else if holding_wrench && active {
                    if let Some(coll) = level.0.collision.as_ref() {
                        let eye = rig.body_eye(&play.game);
                        let previous = (melee.revision == rig.revision
                            && melee.level == crate::level_load::level_index())
                        .then_some(melee.safe_head)
                        .flatten()
                        .map(|v| root.transform_point(v));
                        constrain_wrench(
                            &play,
                            coll,
                            it,
                            root.mul_transform(hand).translation,
                            eye,
                            previous,
                        );
                        if rebound(it, &melee, &root, time.elapsed_secs_f64()) {
                            constrain_wrench(
                                &play,
                                coll,
                                it,
                                root.mul_transform(hand).translation,
                                eye,
                                previous,
                            );
                        }
                    }
                }
                let bucket = play.game.counter / 300;
                if holding_wrench && *alignment_report != Some(bucket) {
                    if let Some(grip) = play.game.item_data.as_ref().and_then(|d| grip_frame(it, d))
                    {
                        let at = rc_formats::moby_anim::bone_points(
                            &[grip[3].map(f32::to_bits)],
                            &it.rows,
                            it.position,
                            it.scale,
                        )[0];
                        let at = Vec3::new(
                            f32::from_bits(at[0]),
                            f32::from_bits(at[1]),
                            f32::from_bits(at[2]),
                        );
                        let palm = crate::game_camera::game_eye(&root.mul_transform(hand));
                        println!(
                            "VR_GAME wrench_alignment grip_error={:.5} origin_offset={:.3}",
                            at.distance(palm),
                            Vec3::from(it.position).distance(palm)
                        );
                        *alignment_report = Some(bucket);
                    }
                }
            }
        }
        // The right controller untracked: a thrown wrench flies on along the game's own path.
        if motion.hands[1].is_none() {
            if let Some(it) = item.as_mut().filter(|i| i.o_class == rc_game::hero::melee::WRENCH_CLASS && i.mstate != 0) {
                it.position = (Vec3::from_array(it.position) - wrench_middle(it, item_axes(it))).to_array();
            }
        }
        let mut grips = [input.left_grip, input.grip].map(|g| g.unwrap_or(0.0));
        let docked = super::gadget_socket::docked(&play, attach);
        let plugged = docked.is_some();
        if let Some(docked) = docked {
            item = Some(docked);
        }
        let blaster = super::blaster::supported(play.game.hero.items.slot.id)
            && !rc_game::hero::items::is_glove(play.game.hero.items.slot.id);
        let wrench = item
            .as_ref()
            .is_some_and(|i| i.o_class == rc_game::hero::melee::WRENCH_CLASS);
        let flying = item
            .as_ref()
            .is_some_and(|i| i.o_class == rc_game::hero::melee::WRENCH_CLASS && i.mstate != 0);
        let mut shown = Vec::with_capacity(2);
        if active {
            let tracked = motion.hands[1].is_some();
            if let Some(glove) = bomb_glove.as_ref().filter(|_| tracked) {
                shown.push(glove);
            }
            // A thrown wrench flies on while the right controller is not tracked.
            if (tracked || flying)
                && (!play.game.hero.hand_hidden() || plugged)
                && (flying || !play.game.hero.tracked.grabbed[1])
                && (blaster
                    || flying
                    || (wrench && holding_wrench && bomb_glove.is_none() && bomb.id.is_none()))
            {
                if let Some(item) = item.as_ref() {
                    shown.push(item);
                }
            }
        }
        // A flying wrench can remain visible while the Bomb Glove replaces the right-hand mesh.
        attach.tracked_items(&shown);
        // Akimbo: the left hand's item at the left controller (super::akimbo), placed again for this frame; the left
        // hand closes on it. Its glove to throw (super::bomb_glove's left-hand throw) replaces the left hand, as the
        // Walloper does, mirrored as the hand's glove.
        let off = &play.game.hero.off_hand;
        let left_hand = motion.hands[0]
            .filter(|_| active && !play.game.hero.tracked.grabbed[0])
            .map(|h| root.mul_transform(h));
        let glove = if left_bomb.0.id.is_some() { left_bomb.0.item } else { akimbo.held };
        let off_item = left_hand.zip(play.game.item_data.as_ref()).and_then(|(hand, data)| {
            if rc_game::hero::items::is_glove(glove) {
                let mut it = super::bomb_glove::appearance(hand, data, glove)?;
                super::akimbo::mirror(&mut it, hand);
                return Some(it);
            }
            let mut it = off.item().filter(|_| off.items.tracked_hand.is_some())?.clone();
            super::akimbo::place_left(&mut it, hand, data)?;
            Some(it)
        });
        let left_replaced = off_item.as_ref().is_some_and(|it| {
            rc_game::hero::items::is_glove(glove) || it.o_class == rc_game::hero::walloper::CLASS
        });
        if off_item.is_some() {
            grips[0] = 1.0;
        }
        attach.tracked_off_hand(off_item.as_ref(), off.hand_mods());
        let holding_weapon = (!play.game.hero.hand_hidden() || plugged)
            && !play.game.hero.tracked.grabbed[1]
            && item.as_ref().is_some_and(|i| {
                if i.o_class == rc_game::hero::melee::WRENCH_CLASS {
                    i.mstate == 0 && holding_wrench
                } else {
                    !rc_game::hero::items::is_glove(play.game.hero.items.slot.id)
                }
            });
        attach.tracked_hands(
            [0, 1].map(|side| {
                if !active
                    || (side == 0 && left_replaced)
                    || (side == 1
                        && (bomb_glove.is_some()
                            || (holding_weapon
                                && play.game.hero.items.slot.id
                                    == rc_game::hero::walloper::WALLOPER)))
                {
                    return None;
                }
                let pose = if side == 0 {
                    if support.held {
                        super::blaster::support_hand(motion.hands[1]?, motion.hands[0], &support)?
                    } else {
                        motion.hands[0]?
                    }
                } else {
                    motion.hands[side]?
                };
                let mut world = root.mul_transform(pose);
                if play.game.hero.tracked.grabbed[side] {
                    if let Some(anchor) = play.game.hero.tracked.anchors[side] {
                        world.translation = crate::tfrag_render::game_to_bevy(anchor);
                    }
                }
                Some(world)
            }),
            &play,
            item.as_ref().filter(|_| holding_weapon),
            support.held || plugged,
            grips,
        );
        // Clank, Giant Clank or the disguise in first person: the body's own hands on the controllers (the body is not
        // drawn). After the item pass, which hides every hand-slot item it is not given.
        let body = play.game.hero.mode;
        let first_person = matches!(body, 1..=3) && !play.death_pending;
        attach.tracked_body_hands(
            [0, 1].map(|side| motion.hands[side].filter(|_| first_person).map(|h| root.mul_transform(h))),
            body,
            grips,
            &play.game.hero,
        );
    }
}
