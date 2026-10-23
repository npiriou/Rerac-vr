//! Controller gestures feed native pack, ledge and swimming states.
use super::{
    Settings,
    embodied::{Motion, game_point, gameplay},
    game::Rig,
    game_input::{Focused, HandInput},
};
use bevy::prelude::*;

pub(super) struct TraversalPlugin;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TraversalUpdate;
#[derive(Resource, Default)]
struct Gestures {
    previous: [Option<(f64, Vec3)>; 2],
    velocity: [Vec3; 2],
    pull_from: [Option<Vec3>; 2],
    shove_at: [Option<f64>; 2],
    revision: u64,
    level: u32,
}
impl Plugin for TraversalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Gestures>()
            .add_systems(PreUpdate, sample.after(super::embodied::capture))
            .add_systems(
                FixedUpdate,
                drive
                    .in_set(TraversalUpdate)
                    .before(super::bomb_glove::BombUpdate),
            );
    }
}
fn sample(
    motion: Res<Motion>,
    rig: Res<Rig>,
    time: Res<Time<Real>>,
    focus: Res<Focused>,
    mode: Res<crate::menu_render::MenuMode>,
    mut strokes: ResMut<Gestures>,
) {
    let level = crate::level_load::level_index();
    if !focus.0 || !gameplay(&mode) || strokes.revision != rig.revision || strokes.level != level {
        *strokes = Gestures {
            revision: rig.revision,
            level,
            ..default()
        };
        return;
    }
    let now = time.elapsed_secs_f64();
    for side in 0..2 {
        strokes.velocity[side] = Vec3::ZERO;
        let Some(hand) = motion.hands[side] else {
            strokes.previous[side] = None;
            strokes.pull_from[side] = None;
            continue;
        };
        if let Some((t, pos)) = strokes.previous[side] {
            let dt = (now - t) as f32;
            let delta = hand.translation - pos;
            if dt >= 0.12 || delta.length() >= 0.3 {
                strokes.pull_from[side] = None;
            }
            if (0.002..0.12).contains(&dt) && delta.length() < 0.3 {
                let v = delta / dt;
                strokes.velocity[side] = if v.length() > 0.25 {
                    v.clamp_length_max(4.0)
                } else {
                    Vec3::ZERO
                };
            }
        }
        strokes.previous[side] = Some((now, hand.translation));
    }
}
fn drive(
    play: Option<ResMut<crate::gameplay::Play>>,
    settings: Res<Settings>,
    rig: Res<Rig>,
    motion: Res<Motion>,
    mut input: ResMut<HandInput>,
    focus: Res<Focused>,
    mode: Res<crate::menu_render::MenuMode>,
    mut gestures: ResMut<Gestures>,
    time: Res<Time<Real>>,
) {
    let Some(mut play) = play else { return };
    let p = &mut *play;
    let root = rig.root_for(&p.game, settings.units_per_metre);
    let ready = !settings.desktop_preview
        && focus.0
        && gameplay(&mode)
        && p.game.hero.health > 0
        && !p.death_pending
        && root.is_some();
    let jump = std::mem::take(&mut input.pack_jump);
    if !ready {
        p.game.hero.tracked = default();
        gestures.pull_from = [None; 2];
        gestures.shove_at = [None; 2];
        return;
    }
    let root = root.unwrap();
    let water = p.game.hero.water_level.to_f32();
    let swimming = matches!(p.game.hero.group, 0x11 | 0x12);
    let t = &mut p.game.hero.tracked;
    t.enabled = true;
    t.pack_jump |= jump;
    t.hands = motion
        .hands
        .map(|hand| hand.map(|h| game_point(root.transform_point(h.translation))));
    t.grips = [input.left_grip, input.grip].map(|v| v.is_some_and(|v| v > 0.5));
    t.stroke = [0.0; 3];
    let mut ledge_motion = Vec3::ZERO;
    let mut grips = 0;
    let now = time.elapsed_secs_f64();
    let mut force = Vec3::ZERO;
    for side in 0..2 {
        if t.grabbed[side] && t.grips[side] {
            if let Some(hand) = motion.hands[side] {
                let start = *gestures.pull_from[side].get_or_insert(hand.translation);
                if let (Some(anchor), Some(actual)) = (t.anchors[side], t.hands[side]) {
                    // Absolute anchor error closes after the body moves. Integrating controller
                    // deltas alone never notices an automatic native hang correction or drift.
                    ledge_motion += crate::tfrag_render::game_to_bevy(anchor)
                        - crate::tfrag_render::game_to_bevy(actual);
                    grips += 1;
                }
                if start.y - hand.translation.y > 0.22 && gestures.velocity[side].y < -2.2 {
                    gestures.shove_at[side] = Some(now);
                }
                if start.y - hand.translation.y < 0.08 {
                    gestures.shove_at[side] = None;
                }
            }
        } else {
            gestures.pull_from[side] = None;
            gestures.shove_at[side] = None;
        }
        if !swimming {
            continue;
        }
        let Some(hand) = motion.hands[side] else {
            continue;
        };
        if root.transform_point(hand.translation).y > water + 0.04 {
            continue;
        }
        let velocity = gestures.velocity[side];
        // A broad palm pushes water; rotating it edge-on allows a recovery stroke.
        let normal = *hand.right();
        let drag = velocity.normalize_or_zero().dot(normal).abs();
        force -= root.rotation * velocity * drag * 0.65 * settings.units_per_metre;
    }
    t.ledge_delta = game_point(if grips > 0 {
        ledge_motion / grips as f32
    } else {
        Vec3::ZERO
    });
    t.ledge_jump |= gestures.shove_at[0]
        .zip(gestures.shove_at[1])
        .is_some_and(|(a, b)| {
            // A small simultaneity window accommodates the two independently sampled hands.
            (a - b).abs() <= 0.10 && now - a < 0.10 && now - b < 0.10
        });
    if t.ledge_jump {
        gestures.shove_at = [None; 2];
    }
    if swimming {
        t.stroke = game_point(force.clamp_length_max(3.5));
    } else {
        t.swim_velocity = [0.0; 3];
    }
}
