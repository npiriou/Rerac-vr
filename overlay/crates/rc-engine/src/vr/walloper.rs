//! Physical punches request the original horizontal Walloper lunge.
//! A punch is the hand holding the Walloper thrown forward, relative to the head (the body walking or turning is not a
//! punch), within about 55° of where the player faces and away from the body: an arm folding back, a sideways swing or
//! a backward step never punches. The lunge itself goes where the player faces (rc-game `tracked_lunge_yaw`). With
//! Akimbo the left hand's Walloper punches too (its lunge led by the left hand: rc_game::hero::off_hand).
use super::{
    Settings,
    embodied::{Motion, game_point, gameplay},
    game::Rig,
};
use bevy::prelude::*;
use bevy_mod_openxr::resources::OxrViews;
use std::collections::VecDeque;

pub(super) struct WalloperPlugin;
#[derive(Default)]
struct Punch {
    /// The hand relative to the head (tracking space), and when: its velocity is the arm's alone.
    samples: VecDeque<(f64, Vec3)>,
    /// Where the current stroke started (relative to the head).
    rest: Option<Vec3>,
    ready: bool,
    cooldown: f64,
    revision: u64,
    level: u32,
    last_tick: u64,
    /// The stroke's top speed (it re-arms when the hand slows below half of it).
    peak: f32,
    /// Since when the trigger is held (its own native lunge).
    trigger: Option<f64>,
    /// A punch not yet taken by the game (until when, its direction): a punch during a lunge starts the next one.
    queued: Option<(f64, Vec3)>,
}
/// The punch's minimum speed (m/s, relative to the head).
const SPEED: f32 = 1.45;
/// cos 55°: the punch's velocity is within this cone around the player's facing.
const FORWARD_CONE: f32 = 0.57;
/// The hand moves away from the spine (behind the eyes) by at least this fraction of its speed.
const AWAY: f32 = 0.2;
/// The spine, behind the eyes (m).
const SPINE: f32 = 0.15;
/// The velocity window (s): short, so a punch registers early in the throw.
const WINDOW: f64 = 0.05;
impl Plugin for WalloperPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            update
                .after(super::blaster::WeaponAim)
                .before(crate::gameplay::GameTick),
        );
    }
}
fn update(
    play: Option<ResMut<crate::gameplay::Play>>,
    motion: Res<Motion>,
    rig: Res<Rig>,
    settings: Res<Settings>,
    mode: Res<crate::menu_render::MenuMode>,
    time: Res<Time<Real>>,
    mut punches: Local<[Punch; 2]>,
    (input, pad): (Option<Res<super::game_input::HandInput>>, Option<Res<super::game_input::GamePad>>),
    views: Option<Res<OxrViews>>,
) {
    let Some(mut play) = play else { return };
    let p = &mut *play;
    // The game takes the punch when it enters the lunge; one still pending is offered again until it expires.
    if p.game.hero.walloper.tracked_punch.is_none() {
        for punch in punches.iter_mut() {
            punch.queued = None;
        }
    }
    p.game.hero.walloper.tracked_until = 0;
    let root = rig.root_for(&p.game, settings.units_per_metre);
    let head = super::game::pose(views.as_deref());
    let hero = &p.game.hero;
    let common = !settings.desktop_preview
        && gameplay(&mode)
        && hero.mode == 0
        && hero.health > 0
        && matches!(hero.group, 0 | 1 | 2 | 4 | 5 | 6)
        && !hero.tracked.grabbed.iter().any(|&g| g)
        && !p.death_pending
        && !hero.hand_hidden();
    let walloper = rc_game::hero::walloper::WALLOPER;
    // The left hand's Walloper (Akimbo), the hand's.
    let usable = [
        common && hero.off_hand.held() == walloper && hero.off_hand.items.tracked_aim.is_some(),
        common && hero.items.slot.id == walloper && hero.items.slot.state == 2 && hero.items.tracked_aim.is_some(),
    ];
    // Where the player faces (the heading the stick and the hero follow: the body or the gaze), in tracking space.
    let facing = root.zip(head).map(|(root, head)| {
        hero.tracked
            .navigation
            .map(|v| {
                let f = rc_game::hero::physics::to_f32x3(v.rows[0]);
                root.rotation.inverse() * Vec3::new(f[0], f[2], -f[1])
            })
            .unwrap_or(*head.forward())
            .with_y(0.0)
            .normalize_or_zero()
    });
    // Each hand's own trigger is its native lunge.
    let triggers = [
        pad.as_ref().is_some_and(|i| i.left_fire),
        input.as_ref().and_then(|i| i.trigger).is_some_and(|v| v > 0.6),
    ];
    let level = crate::level_load::level_index();
    let now = time.elapsed_secs_f64();
    for side in 0..2 {
        let punch = &mut punches[side];
        let (Some(head), Some(hand), Some(facing), true) = (head, motion.hands[side], facing, usable[side]) else {
            reset(punch, p.game.counter, &rig, level);
            continue;
        };
        if punch.revision != rig.revision || punch.level != level || p.game.counter < punch.last_tick {
            reset(punch, p.game.counter, &rig, level);
            continue;
        }
        punch.last_tick = p.game.counter;
        if facing != Vec3::ZERO {
            stroke(punch, side, hand.translation - head.translation, facing, triggers[side], now);
        }
    }
    // The newest punch not yet taken.
    let offer = punches
        .iter()
        .enumerate()
        .filter_map(|(side, punch)| punch.queued.filter(|&(until, _)| now < until).map(|(until, d)| (until, d, side)))
        .max_by(|a, b| a.0.total_cmp(&b.0));
    for punch in punches.iter_mut() {
        if punch.queued.is_some_and(|(until, _)| now >= until) {
            punch.queued = None;
        }
    }
    p.game.hero.walloper.tracked_punch = offer.zip(root).map(|((_, direction, side), root)| {
        rc_game::hero::walloper::TrackedPunch {
            direction: game_point(root.rotation * direction),
            begin: true,
            left: side == 0,
        }
    });
}

/// One tick of a hand's stroke (`relative`: the hand relative to the head, tracking space): a punch is queued.
fn stroke(punch: &mut Punch, side: usize, relative: Vec3, facing: Vec3, trigger: bool, now: f64) {
    // A gap or a jump in the poses (tracking loss, recentring): the stroke starts over.
    if punch
        .samples
        .back()
        .is_some_and(|&(t, p)| now - t > 0.12 || relative.distance(p) > 0.3)
    {
        punch.samples.clear();
        punch.rest = None;
        punch.ready = false;
    }
    // Several fixed ticks in one frame see the same poses.
    if punch.samples.back().is_none_or(|&(t, _)| now > t) {
        punch.samples.push_back((now, relative));
    }
    while punch.samples.len() > 2 && now - punch.samples[0].0 > WINDOW {
        punch.samples.pop_front();
    }
    let velocity = match punch.samples.front() {
        Some(&(t, p)) if now - t > 0.008 => (relative - p) / (now - t) as f32,
        _ => Vec3::ZERO,
    };
    // A trigger press is the native lunge: no gesture punch on top of it for 0.3 s.
    if trigger {
        punch.trigger.get_or_insert(now);
    } else {
        punch.trigger = None;
    }
    let native = punch.trigger.is_some_and(|t| now - t < 0.3);
    let speed = velocity.length();
    let along = velocity.dot(facing);
    punch.peak = punch.peak.max(speed);
    // Re-arm on a pause, on the stroke slowing below half its peak, or on the arm pulling back (jab, pull, jab).
    if speed < 0.6 || speed < 0.5 * punch.peak || along < -0.3 * speed || punch.rest.is_none() {
        punch.rest = Some(relative);
        if now >= punch.cooldown {
            punch.ready = true;
            punch.peak = speed;
        }
    }
    let distance = punch.rest.map_or(0.0, |r| relative.distance(r));
    // Thrown forward and away from the body (the spine behind the eyes): never the arm folding back or across.
    let reach = (relative + facing * SPINE).with_y(0.0).normalize_or_zero();
    let away = velocity.with_y(0.0).dot(reach);
    if punch.ready
        && !native
        && now >= punch.cooldown
        && speed >= SPEED
        && distance >= 0.07
        && along >= FORWARD_CONE * speed
        && away >= AWAY * speed
    {
        punch.ready = false;
        punch.peak = speed;
        punch.cooldown = now + 0.22;
        let direction = velocity.with_y(0.0).normalize_or(facing);
        punch.queued = Some((now + 0.45, direction));
        println!(
            "VR_GAME walloper_physical_punch side={side} speed={speed:.2} along={:.2} away={:.2}",
            along / speed,
            away / speed
        );
    }
}

fn reset(punch: &mut Punch, tick: u64, rig: &Rig, level: u32) {
    *punch = Punch {
        revision: rig.revision,
        level,
        last_tick: tick,
        ..default()
    };
}
