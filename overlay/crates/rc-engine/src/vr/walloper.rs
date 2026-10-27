//! Physical punches request the original horizontal Walloper lunge.
//! A punch is the right hand thrown forward, relative to the head (the body walking or turning is not a punch), within
//! about 55° of where the player faces and away from the body: an arm folding back, a sideways swing or a backward
//! step never punches. The lunge itself goes where the player faces (rc-game `tracked_lunge_yaw`).
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
    /// The right hand relative to the head (tracking space), and when: its velocity is the arm's alone.
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
    mut punch: Local<Punch>,
    input: Option<Res<super::game_input::HandInput>>,
    views: Option<Res<OxrViews>>,
) {
    let Some(mut play) = play else { return };
    let p = &mut *play;
    // The game takes the punch when it enters the lunge; one still pending is offered again until it expires.
    if p.game.hero.walloper.tracked_punch.is_none() {
        punch.queued = None;
    }
    let root = rig.root_for(&p.game, settings.units_per_metre);
    let head = super::game::pose(views.as_deref());
    let usable = !settings.desktop_preview
        && gameplay(&mode)
        && p.game.hero.mode == 0
        && p.game.hero.health > 0
        && matches!(p.game.hero.group, 0 | 1 | 2 | 4 | 5 | 6)
        && !p.game.hero.tracked.grabbed.iter().any(|&g| g)
        && !p.death_pending
        && !p.game.hero.hand_hidden()
        && p.game.hero.items.slot.id == rc_game::hero::walloper::WALLOPER
        && p.game.hero.items.slot.state == 2
        && p.game.hero.items.tracked_aim.is_some();
    let level = crate::level_load::level_index();
    let (Some(root), Some(head), Some(hand), true) = (root, head, motion.hands[1], usable) else {
        reset(&mut punch, p, &rig, level);
        return;
    };
    if punch.revision != rig.revision || punch.level != level || p.game.counter < punch.last_tick {
        reset(&mut punch, p, &rig, level);
        return;
    }
    punch.last_tick = p.game.counter;
    let now = time.elapsed_secs_f64();
    let relative = hand.translation - head.translation;
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
    // Where the player faces (the heading the stick and the hero follow: the body or the gaze), in tracking space.
    let facing = p
        .game
        .hero
        .tracked
        .navigation
        .map(|v| {
            let f = rc_game::hero::physics::to_f32x3(v.rows[0]);
            root.rotation.inverse() * Vec3::new(f[0], f[2], -f[1])
        })
        .unwrap_or(*head.forward())
        .with_y(0.0)
        .normalize_or_zero();
    if facing == Vec3::ZERO {
        return;
    }
    // A trigger press is the native lunge: no gesture punch on top of it for 0.3 s.
    if input.as_ref().and_then(|i| i.trigger).is_some_and(|v| v > 0.6) {
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
            "VR_GAME walloper_physical_punch speed={speed:.2} along={:.2} away={:.2}",
            along / speed,
            away / speed
        );
    }
    p.game.hero.walloper.tracked_punch = match punch.queued {
        Some((until, direction)) if now < until => Some(rc_game::hero::walloper::TrackedPunch {
            direction: game_point(root.rotation * direction),
            begin: true,
        }),
        _ => {
            punch.queued = None;
            None
        }
    };
}
fn reset(punch: &mut Punch, p: &mut crate::gameplay::Play, rig: &Rig, level: u32) {
    *punch = Punch {
        revision: rig.revision,
        level,
        last_tick: p.game.counter,
        ..default()
    };
    p.game.hero.walloper.tracked_punch = None;
    p.game.hero.walloper.tracked_until = 0;
}
