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
impl Plugin for EmbodiedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Motion>()
            .init_resource::<super::game_input::Feedback>()
            .init_resource::<Melee>()
            .add_plugins((
                super::bomb_glove::BombGlovePlugin,
                super::blaster::BlasterPlugin,
                super::gadget_socket::GadgetSocketPlugin,
                super::traversal::TraversalPlugin,
                super::orientation::OrientationPlugin,
                super::walloper::WalloperPlugin,
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
                    .before(crate::moby_attach::upload)
                    .before(TransformSystems::Propagate),
            );
    }
}

#[derive(Resource, Default)]
pub(super) struct Motion {
    pub hands: [Option<Transform>; 2],
    pub bomb: BombGesture,
    held: bool,
    equip_wrench: bool,
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
    pub(super) fn velocity(&self) -> Vec3 {
        self.velocity
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
        let bomb_held = motion.bomb.held;
        *motion = Motion {
            revision: rig.revision,
            level,
            held,
            bomb: BombGesture {
                held: bomb_held,
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
    let grip = input.as_ref().and_then(|i| i.grip);
    let trigger = input.as_ref().and_then(|i| i.trigger);
    let Some(hand) = motion.hands[1].filter(|_| grip.is_some() || trigger.is_some()) else {
        motion.held = grip.is_some_and(|v| v > 0.6 || (v >= 0.35 && motion.held));
        motion.equip_wrench = false;
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
    if value > 0.6 {
        if !motion.held {
            motion.equip_wrench = true;
        }
        motion.held = true;
    }
    if value < 0.35 && motion.held {
        if motion.velocity.length() >= 1.0 {
            motion.release = Some((hand, motion.velocity, now));
        }
        motion.held = false;
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

fn place(item: &mut HandItem, transform: Transform, data: Option<&ItemData>, stretch: f32) {
    if let Some(class) = data.and_then(|d| d.class(item.o_class)) {
        item.scale = class.scale * 1.20;
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
fn crate_class(class: i16) -> bool {
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
) {
    let Some(mut play) = play else { return };
    let p = &mut *play;
    let Some(root) = rig.root_for(&p.game, settings.units_per_metre) else {
        return;
    };
    let ready =
        gameplay(&mode) && p.game.hero.mode == 0 && !p.death_pending && p.game.hero.health > 0;
    if !ready {
        motion.release = None;
        melee.clear_contact();
        melee.previous = None;
        melee.swing_from = None;
        melee.down_from = None;
        p.game.hero.items.tracked_hand = None;
        return;
    }
    // A fresh grip requests the wrench once; native quick-select requests remain authoritative afterward.
    let using_hands = p.game.hero.f838 != 0
        || p.game.hero.tracked.grabbed.iter().any(|&g| g)
        || matches!(p.game.hero.group, 3 | 0x11 | 0x12);
    if using_hands || p.game.hero.items.slot.id == rc_game::hero::walloper::WALLOPER {
        motion.equip_wrench = false;
        motion.release = None;
    }
    if motion.equip_wrench {
        if let Some(s) = session.as_mut() {
            s.0.temp_hand = 8;
            p.game.hero.items.restore_pending = 0;
            motion.equip_wrench = false;
        }
    }
    let Some(hand) = motion.hands[1] else {
        p.game.hero.items.tracked_hand = None;
        melee.clear_contact();
        melee.previous = None;
        melee.swing_from = None;
        melee.down_from = None;
        return;
    };
    let world_hand = root.mul_transform(hand);
    let Some(coll) = level.0.collision.as_ref() else {
        return;
    };
    let eye =
        crate::tfrag_render::game_to_bevy(p.game.hero.position()) + root.rotation * Vec3::Y * 1.6;
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
        melee.clear_contact();
        melee.previous = None;
        return;
    };
    if melee.revision != rig.revision
        || melee.level != crate::level_load::level_index()
        || p.game.counter < melee.last_tick
    {
        *melee = Melee {
            revision: rig.revision,
            level: crate::level_load::level_index(),
            ..default()
        };
    }
    melee.last_tick = p.game.counter;
    let stretch = 1.0 + ((motion.velocity.length() - 0.6) / 3.0).clamp(0.0, 1.0) * 0.15;
    place(&mut item, world_hand, p.game.item_data.as_ref(), stretch);
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
        melee.clear_contact();
        melee.previous = None;
        melee.swing_from = None;
        melee.down_from = None;
        motion.release = None;
        return;
    }
    let release = motion.release.take();
    if item.mstate != 0 || !motion.wrench_held() {
        melee.clear_contact();
        melee.previous = None;
        melee.swing_from = None;
        melee.down_from = None;
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
    if !p.game.hero.grounded()
        && now >= melee.slam_until
        && motion.slam_velocity.y < -1.2
        && melee
            .down_from
            .is_some_and(|v| v.y - hand.translation.y >= 0.05)
    {
        p.game.hero.tracked.hyper_strike = true;
        melee.slam_until = now + 0.5;
        melee.down_from = None;
    }
    // Require a deliberate translating hand stroke; a high-speed tip from tiny wrist twitches is insufficient.
    if motion.velocity.length() < 0.65 || melee.swing_from.is_none() {
        melee.swing_from = Some(hand.translation);
    }
    let travel = melee
        .swing_from
        .map_or(0.0, |start| start.distance(hand.translation));
    let hand_speed = motion.velocity.length();
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
        head + (head - palm).normalize_or_zero() * 0.17
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

fn launch_wrench(game: &mut rc_game::tick::Game, hand: Transform, velocity: Vec3) {
    let h = &mut game.hero;
    if h.items.slot.state != 2 {
        return;
    }
    let Some(it) = h.items.slot.item.as_mut() else {
        return;
    };
    place(it, hand, game.item_data.as_ref(), 1.0);
    let r = it.rows.map(|row| row.map(Pf));
    let rows = [r[0], r[1], r[2], [Pf::ZERO, Pf::ZERO, Pf::ZERO, Pf::ONE]];
    let dir = game_point(velocity.normalize_or_zero());
    it.mstate = rc_game::hero::comet::OUT;
    it.flight = rc_game::hero::comet::Flight {
        dir,
        speed: (velocity.length() * 4.0).clamp(10.0, 23.0) * rc_game::hero::physics::DT.to_f32(),
        accel: 0.0,
        wall_close: false,
        euler: rc_game::hero::comet::launch_euler(&rows),
    };
    h.items.slot.detached = 1;
    h.items.pending_blend = Some((6, 0, 5));
    h.melee.hit = 0;
    h.fx.trails.wrench.start(rc_game::hero::melee::WRENCH_CLASS);
    h.fx.trails.wrench.add(0x30, 3);
    h.fx.trails.wrench.add(0x17, 5);
    println!(
        "VR_GAME tracked_wrench_throw origin={:?} direction={dir:?}",
        it.position
    );
}

fn present(
    mut commands: Commands,
    body: Query<Entity, (With<crate::gameplay::RatchetMesh>, Without<RenderLayers>)>,
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
    time: Res<Time<Real>>,
) {
    for e in &body {
        commands.entity(e).insert(RenderLayers::layer(61));
    }
    let root = play
        .as_ref()
        .and_then(|p| rig.root_for(&p.game, settings.units_per_metre));
    let active = gameplay(&mode)
        && play
            .as_ref()
            .is_some_and(|p| p.game.hero.mode == 0 && p.game.hero.health > 0 && !p.death_pending);
    let bomb_glove = play.as_ref().and_then(|p| {
        let selected = rc_game::hero::items::is_glove(p.game.hero.items.slot.id);
        if !active
            || p.game.hero.hand_hidden()
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
            attach.tracked_hands([None; 2], &play, None, false);
            return;
        }
        let mut item = play.game.hero.items.slot.item.clone();
        if let (Some(it), Some(hand)) = (item.as_mut(), motion.hands[1]) {
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
                    1.0 + ((motion.velocity.length() - 0.6) / 3.0).clamp(0.0, 1.0) * 0.15,
                );
                if motion.wrench_held() && active {
                    if let Some(coll) = level.0.collision.as_ref() {
                        let eye = crate::tfrag_render::game_to_bevy(play.game.hero.position())
                            + root.rotation * Vec3::Y * 1.6;
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
                if motion.wrench_held() && *alignment_report != Some(bucket) {
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
        let blaster = super::blaster::supported(play.game.hero.items.slot.id)
            && !rc_game::hero::items::is_glove(play.game.hero.items.slot.id);
        let wrench = item
            .as_ref()
            .is_some_and(|i| i.o_class == rc_game::hero::melee::WRENCH_CLASS);
        let flying = item
            .as_ref()
            .is_some_and(|i| i.o_class == rc_game::hero::melee::WRENCH_CLASS && i.mstate != 0);
        let mut shown = Vec::with_capacity(2);
        if active && motion.hands[1].is_some() {
            if let Some(glove) = bomb_glove.as_ref() {
                shown.push(glove);
            }
            if !play.game.hero.hand_hidden()
                && (blaster
                    || flying
                    || (wrench
                        && motion.wrench_held()
                        && bomb_glove.is_none()
                        && bomb.id.is_none()))
            {
                if let Some(item) = item.as_ref() {
                    shown.push(item);
                }
            }
        }
        // A flying wrench can remain visible while the Bomb Glove replaces the right-hand mesh.
        attach.tracked_items(&shown);
        let holding_weapon = !play.game.hero.hand_hidden()
            && item.as_ref().is_some_and(|i| {
                if i.o_class == rc_game::hero::melee::WRENCH_CLASS {
                    i.mstate == 0 && motion.wrench_held()
                } else {
                    !rc_game::hero::items::is_glove(play.game.hero.items.slot.id)
                }
            });
        attach.tracked_hands(
            [0, 1].map(|side| {
                if !active
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
            support.held,
        );
    }
}
