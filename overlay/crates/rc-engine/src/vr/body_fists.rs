//! Clank's and Giant Clank's fists in first person: a punch of either tracked hand strikes where that hand actually
//! is, with the body's original attack sphere and damage (`rc_game::hero::bodies`: Clank's kick sphere, Giant Clank's
//! fist, plus his punch's ground sweep toward the strike), like Ratchet's physical wrench. Nothing plays the native
//! punch animation.
use super::{
    Settings,
    embodied::{Motion, game_point, gameplay},
    game::Rig,
};
use bevy::prelude::*;

pub(super) struct BodyFistsPlugin;
impl Plugin for BodyFistsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            strike
                .after(super::traversal::TraversalUpdate)
                .before(crate::gameplay::GameTick),
        );
    }
}

#[derive(Default, Clone, Copy)]
struct Fist {
    previous: Option<(f64, Vec3)>,
    velocity: Vec3,
    /// Where the current stroke started (tracking space, metres): reset whenever the hand is nearly still.
    from: Option<Vec3>,
    direction: Vec3,
    /// Game ticks left in the strike's hit window; the stroke must slow down before another one.
    active: u8,
    spent: bool,
}
#[derive(Default)]
struct Fists {
    hands: [Fist; 2],
    revision: u64,
}

// A deliberate translating punch: fast, a stroke of several centimetres, not a wrist flick (metres, seconds).
const SPEED: f32 = 2.0;
const STROKE: f32 = 0.12;
const STILL: f32 = 0.65;
/// The strike is detected mid-stroke; its window covers the rest of the stroke and the arm held at full extension,
/// where the target is (Giant Clank's native fist keys 7..14, its sweep the whole punch).
const WINDOW: u8 = 10;

fn strike(
    play: Option<ResMut<crate::gameplay::Play>>,
    motion: Res<Motion>,
    rig: Res<Rig>,
    settings: Res<Settings>,
    mode: Res<crate::menu_render::MenuMode>,
    time: Res<Time<Real>>,
    mut fists: Local<Fists>,
) {
    let Some(mut play) = play else { return };
    let p = &mut *play;
    let root = rig.root_for(&p.game, settings.units_per_metre);
    let fighting = !settings.desktop_preview
        && gameplay(&mode)
        && matches!(p.game.hero.mode, 1 | 2)
        && !p.death_pending
        && p.game.hero.tracked.grabbed.iter().all(|&g| !g)
        && !rc_game::hero::tracked::ledge_hold(p.game.hero.state);
    if !fighting || root.is_none() || fists.revision != rig.revision {
        *fists = Fists { revision: rig.revision, ..default() };
        return;
    }
    let root = root.unwrap();
    let now = time.elapsed_secs_f64();
    for side in 0..2 {
        let f = &mut fists.hands[side];
        let Some(hand) = motion.hands[side] else {
            *f = Fist::default();
            continue;
        };
        let at = hand.translation;
        match f.previous {
            Some((t, before)) if now - t >= 0.004 => {
                let dt = (now - t) as f32;
                f.velocity = if dt < 0.1 && at.distance(before) < 0.3 { (at - before) / dt } else { Vec3::ZERO };
                f.previous = Some((now, at));
            }
            Some(_) => {}
            None => f.previous = Some((now, at)),
        }
        let speed = f.velocity.length();
        if speed < STILL {
            f.from = Some(at);
            f.spent = false;
        }
        let travel = f.from.map_or(0.0, |from| from.distance(at));
        if f.active == 0 && !f.spent && speed >= SPEED && travel >= STROKE {
            f.active = WINDOW;
            f.spent = true;
            f.direction = f.velocity / speed;
            p.game.hero.tracked.fist_strikes[side] = p.game.hero.tracked.fist_strikes[side].wrapping_add(1);
            println!("VR_GAME fist_strike side={side} speed={speed:.2} travel={travel:.2}");
        }
        if f.active == 0 {
            continue;
        }
        f.active -= 1;
        // A retracting arm ends the strike: it cannot hit on the way back. Slowing at full extension does not.
        if f.velocity.dot(f.direction) < -STILL {
            f.active = 0;
            continue;
        }
        let centre = game_point(root.transform_point(at));
        let d = game_point(root.rotation * f.direction);
        let flat = Vec2::new(d[0], d[1]).normalize_or(Vec2::X);
        p.game.hero.tracked.fists.push((side, centre, [flat.x, flat.y, 1.0]));
    }
}
