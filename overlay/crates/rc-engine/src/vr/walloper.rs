//! Physical punches request the original horizontal Walloper lunge.
use super::{
    Settings,
    embodied::{Motion, game_point, gameplay},
    game::Rig,
};
use bevy::prelude::*;

pub(super) struct WalloperPlugin;
#[derive(Default)]
struct Punch {
    previous: Option<Transform>,
    rest: Option<Vec3>,
    active_until: f64,
    ready: bool,
    cooldown: f64,
    resting_since: Option<f64>,
    revision: u64,
    level: u32,
    last_tick: u64,
    direction: Vec3,
}
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
) {
    let Some(mut play) = play else { return };
    let p = &mut *play;
    p.game.hero.walloper.tracked_punch = None;
    let root = rig.root_for(&p.game, settings.units_per_metre);
    let usable = !settings.desktop_preview
        && gameplay(&mode)
        && p.game.hero.mode == 0
        && p.game.hero.health > 0
        && matches!(p.game.hero.group, 0 | 1 | 2 | 4 | 5 | 6)
        && !p.game.hero.tracked.grabbed.iter().any(|&g|g)
        && !p.death_pending
        && !p.game.hero.hand_hidden()
        && p.game.hero.items.slot.id == rc_game::hero::walloper::WALLOPER
        && p.game.hero.items.slot.state == 2
        && p.game.hero.items.tracked_aim.is_some()
        // A simultaneous trigger press uses the native lunge, without adding gesture damage to it.
        && !input.as_ref().and_then(|i|i.trigger).is_some_and(|v|v>0.6)
        && p.game.hero.state != rc_game::hero::walloper::LUNGE;
    let level = crate::level_load::level_index();
    if !usable
        || root.is_none()
        || motion.hands[1].is_none()
        || punch.revision != rig.revision
        || punch.level != level
        || p.game.counter < punch.last_tick
    {
        *punch = Punch {
            revision: rig.revision,
            level,
            last_tick: p.game.counter,
            ..default()
        };
        p.game.hero.walloper.tracked_until = 0;
        return;
    }
    punch.last_tick = p.game.counter;
    let hand = motion.hands[1].unwrap();
    let root = root.unwrap();
    let Some(previous) = punch.previous.replace(hand) else {
        punch.rest = Some(hand.translation);
        punch.ready = false;
        return;
    };
    if hand.translation.distance(previous.translation) > 0.3 {
        punch.rest = Some(hand.translation);
        punch.ready = false;
        punch.resting_since = None;
        punch.active_until = 0.0;
        return;
    }
    let now = time.elapsed_secs_f64();
    let speed = motion.velocity().length();
    if speed < 0.55 {
        punch.rest = Some(hand.translation);
        let resting_since = *punch.resting_since.get_or_insert(now);
        if now >= punch.cooldown && now - resting_since >= 0.06 {
            punch.ready = true;
        }
        punch.active_until = 0.0;
    } else {
        punch.resting_since = None;
    }
    let distance = punch.rest.map_or(0.0, |p| hand.translation.distance(p));
    let outward = (hand.translation
        - root.compute_affine().inverse().transform_point3(rig.body_eye(&p.game)))
    .normalize_or_zero();
    let velocity = motion.velocity();
    let extending =
        velocity.dot(*hand.forward()) > speed * 0.30 || velocity.dot(outward) > speed * 0.30;
    let begin = punch.ready
        && now >= punch.cooldown
        && speed >= 1.45
        && distance >= 0.07
        && extending
        && hand.translation.distance(previous.translation) >= 0.002;
    if begin {
        punch.ready = false;
        punch.active_until = now + 0.25;
        punch.cooldown = now + 0.35;
        punch.direction = velocity.normalize_or_zero();
        println!(
            "VR_GAME walloper_physical_punch speed={speed:.2} direction={:?}",
            velocity.normalize_or_zero()
        );
    }
    if now >= punch.active_until || speed < 0.55 {
        return;
    }
    if velocity.dot(punch.direction) < speed * 0.25 {
        // Retracting the arm cannot turn the same punch into a second backwards hit.
        punch.active_until = 0.0;
        return;
    }
    if hand.translation.distance(previous.translation) < 0.002 {
        return;
    }
    p.game.hero.walloper.tracked_punch = Some(rc_game::hero::walloper::TrackedPunch {
        direction: game_point(root.rotation * velocity.normalize_or_zero()),
        begin,
    });
}
