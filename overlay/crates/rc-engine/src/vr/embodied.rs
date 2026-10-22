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
            .init_resource::<Melee>()
            .add_plugins((
                super::bomb_glove::BombGlovePlugin,
                super::blaster::BlasterPlugin,
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
    revision: u64,
    level: u32,
    serial: u64,
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
}

pub(super) fn gameplay(mode: &crate::menu_render::MenuMode) -> bool {
    mode.state.mode == rc_game::menus::mode::Mode::Gameplay
}

fn capture(
    focus: Option<Res<Focused>>,
    input: Option<Res<HandInput>>,
    rig: Res<Rig>,
    mode: Res<crate::menu_render::MenuMode>,
    time: Res<Time<Real>>,
    tracked: Query<(&Hand, &Transform, &OxrSpaceLocationFlags)>,
    mut motion: ResMut<Motion>,
) {
    if !focus.as_ref().is_some_and(|f| f.0) || !gameplay(&mode) {
        *motion = Motion::default();
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

fn place(item: &mut HandItem, transform: Transform, data: Option<&ItemData>) {
    // The authored grip frame points the shaft up; turn it forward along the controller (-Z).
    // Rotate about the palm, then compensate the authored anchor so the handle stays in the hand.
    let mut grip_transform = transform;
    grip_transform.rotation *= Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let desired = crate::game_camera::game_rows(&grip_transform);
    let desired = Mat3::from_cols(desired[0], desired[1], desired[2]);
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
        melee.previous = None;
        p.game.hero.items.tracked_hand = None;
        return;
    }
    // A fresh grip requests the wrench once; native quick-select requests remain authoritative afterward.
    if motion.equip_wrench {
        if let Some(s) = session.as_mut() {
            s.0.temp_hand = 8;
            p.game.hero.items.restore_pending = 0;
            motion.equip_wrench = false;
        }
    }
    let Some(hand) = motion.hands[1] else {
        p.game.hero.items.tracked_hand = None;
        melee.previous = None;
        return;
    };
    let world_hand = root.mul_transform(hand);
    let Some(coll) = level.0.collision.as_ref() else {
        return;
    };
    let eye = crate::tfrag_render::game_to_bevy(p.game.hero.position()) + Vec3::Y * 1.6;
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
        return;
    };
    place(&mut item, world_hand, p.game.item_data.as_ref());
    // Native attachment and catch target use the corrected model origin; its grip bone sits in the palm.
    p.game.hero.items.tracked_hand = Some((item.position, item.rows));
    if bomb.id.is_some() {
        melee.previous = None;
        motion.release = None;
        return;
    }
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
    let release = motion.release.take();
    if item.mstate != 0 || !motion.held || !clear {
        melee.previous = None;
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
    let previous = melee.previous.replace((hand, now));
    let Some((old, then)) = previous else { return };
    let dt = (now - then) as f32;
    if !(0.004..=0.12).contains(&dt) || old.translation.distance(hand.translation) > 0.5 {
        return;
    }
    let mut current_item = item.clone();
    let mut previous_item = item;
    place(&mut current_item, world_hand, p.game.item_data.as_ref());
    place(
        &mut previous_item,
        root.mul_transform(old),
        p.game.item_data.as_ref(),
    );
    let Some(data) = p.game.item_data.as_ref() else {
        return;
    };
    let end = tip(&current_item, data);
    let start = tip(&previous_item, data);
    let speed = start.distance(end) / (dt * settings.units_per_metre);
    if speed < 0.9 {
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
            Pf::f(0.28),
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
        let sound = rc_game::hero::melee::wrench_hit_sound(&p.game.mobys, Some(id));
        p.game.hero.fx.item_sounds.push(sound);
        println!("VR_GAME tracked_wrench_hit target={id} speed={speed:.2}");
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
    place(it, hand, game.item_data.as_ref());
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
    play: Option<Res<crate::gameplay::Play>>,
    settings: Res<Settings>,
    rig: Res<Rig>,
    motion: Res<Motion>,
    bomb: Res<super::bomb_glove::HeldBomb>,
    mode: Res<crate::menu_render::MenuMode>,
    mut attach: Option<ResMut<crate::moby_attach::MobyAttach>>,
    mut commands: Commands,
    body: Query<Entity, (With<crate::gameplay::RatchetMesh>, Without<RenderLayers>)>,
    mut alignment_report: Local<Option<u64>>,
    mut bomb_glove_report: Local<bool>,
) {
    // Exclude the animated head/body from the eyes without altering Ratchet's simulation or hit box.
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
        let selected =
            p.game.hero.items.slot.item.as_ref().is_some_and(|i| {
                i.o_class == rc_game::moby_update::classes::bomb::GLOVE_CLASS as i16
            });
        if !active || attach.is_none() || !(selected || bomb.id.is_some()) {
            return None;
        }
        let hand = root?.mul_transform(motion.hands[1]?);
        super::bomb_glove::appearance(hand, p.game.item_data.as_ref()?)
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
            attach.tracked_hands([None; 2]);
            return;
        }
        let mut item = play.game.hero.items.slot.item.clone();
        if let (Some(it), Some(hand)) = (item.as_mut(), motion.hands[1]) {
            if matches!(
                it.o_class,
                rc_game::hero::blaster::CLASS | rc_game::hero::pyrocitor::CLASS
            ) {
                if let Some(data) = play.game.item_data.as_ref() {
                    super::blaster::place(it, root.mul_transform(hand), data);
                }
            } else if it.o_class == rc_game::hero::melee::WRENCH_CLASS && it.mstate == 0 {
                place(it, root.mul_transform(hand), play.game.item_data.as_ref());
                let bucket = play.game.counter / 300;
                if motion.held && *alignment_report != Some(bucket) {
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
        let blaster = item.as_ref().is_some_and(|i| {
            matches!(
                i.o_class,
                rc_game::hero::blaster::CLASS | rc_game::hero::pyrocitor::CLASS
            )
        });
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
            if blaster
                || flying
                || (wrench && motion.held && bomb_glove.is_none() && bomb.id.is_none())
            {
                if let Some(item) = item.as_ref() {
                    shown.push(item);
                }
            }
        }
        // A flying wrench can remain visible while the Bomb Glove replaces the right-hand mesh.
        attach.tracked_items(&shown);
        attach.tracked_hands([0, 1].map(|side| {
            if !active || (side == 1 && bomb_glove.is_some()) {
                return None;
            }
            Some(root.mul_transform(motion.hands[side]?))
        }));
    }
}
