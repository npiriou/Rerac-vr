//! The hero's body seen from its eyes (VR Options → Player body, crate::render_settings::VrBody): in scenes and ship
//! sequences, and optionally in play, the body the player sees through is drawn with its head collapsed instead of
//! hidden. A collapsed joint takes a scale of 0 after the moby's own joint modifiers; its children inherit the scale
//! (`moby_anim::evaluate_posed` scales each joint's rows and places its children through them), so the ears, eyes and
//! antenna go with the head while the eye, computed from the uncollapsed chain, stays where it was. In play the
//! forearms collapse too: the VR hands (crate::moby_attach's hand copies, cut at the same joints) are the hands. With
//! the option off, Ratchet's scenes and ship sequences still draw his forearms and hands ([`ratchet_hands_only`]).
use crate::render_settings::{VrBody, VrOptions};
use bevy::prelude::*;
use rc_formats::moby_anim::{self, AnimState, JointModifier, MobyAnimClass, MobyFrame, PoseLayer, Rows};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::Relaxed};

/// The option, mirrored for the draw code that has no access to the resource.
static MODE: AtomicU8 = AtomicU8::new(VrBody::Off as u8);
/// VR Options → Camera flip, mirrored likewise.
static CAMERA_FLIP: AtomicBool = AtomicBool::new(false);

/// The neck's pivot from the eyes in the head's frame (metres): behind them and below them. In play it sits above the
/// hero's position (super::game::neck_pivot): a head turning or tipping in place swings the eyes about it, so looking
/// down pitches the eyes ahead of the body, as a real one does.
pub(crate) const NECK_BACK: f32 = 0.10;
pub(crate) const NECK_DOWN: f32 = 0.15;

pub(crate) fn mode() -> VrBody {
    if !super::game_requested() { return VrBody::Off; }
    VrBody::from_index(MODE.load(Relaxed))
}

/// The body is drawn in scenes and ship sequences.
pub(crate) fn in_scenes() -> bool { mode() >= VrBody::Cutscenes }

/// The body is drawn in play too.
pub(crate) fn in_play() -> bool { mode() == VrBody::Always }

pub(super) fn sync_option(options: Option<Res<VrOptions>>) {
    if let Some(o) = options {
        MODE.store(o.body.index(), Relaxed);
        CAMERA_FLIP.store(o.camera_flip, Relaxed);
    }
}

/// The double jump (`rc_game::hero::jump`, state 0xe) rolls Ratchet's body through sequence 0x16. With the camera flip
/// off the view stays level, so the body is drawn without the roll: it holds the pose it had before the double jump
/// until the roll's frame 35 (where the jump's own landing logic starts), then blends into the live pose over 5 frames.
/// The game's own animation, its triggers (the roll's sound) and the jump's timing are untouched.
const DOUBLE_JUMP: i32 = 0xe;
const ROLL_SEQ: u8 = 0x16;
const ROLL_END: f32 = 35.0;
const ROLL_BLEND: f32 = 5.0;
/// The last key Ratchet was drawn with before a double jump (sequence, frame).
static HELD: Mutex<Option<(u8, u8)>> = Mutex::new(None);

/// Ratchet's pose as drawn (his palette, the items placed on him): `live`, or during the double jump's roll with the
/// camera flip off the pose held from before it.
pub(crate) fn drawn_state(hero_state: i32, live: AnimState) -> AnimState {
    if !super::game_requested() || CAMERA_FLIP.load(Relaxed) { return live; }
    let Ok(mut held) = HELD.lock() else { return live };
    let rolling = hero_state == DOUBLE_JUMP && (live.seq_a == ROLL_SEQ || live.seq_b == ROLL_SEQ);
    if !rolling {
        if hero_state != DOUBLE_JUMP && live.seq_b != moby_anim::SNAPSHOT_SEQ { *held = Some((live.seq_b, live.frame_b)); }
        return live;
    }
    let Some((seq, frame)) = *held else { return live };
    let at = if live.seq_b == ROLL_SEQ { live.frame_b as f32 } else { 0.0 };
    let t = ((at - ROLL_END) / ROLL_BLEND).clamp(0.0, 1.0);
    if t >= 1.0 { return live; }
    let (seq_b, frame_b) = if t > 0.0 { (live.seq_b, live.frame_b) } else { (seq, frame) };
    AnimState { seq_a: seq, frame_a: frame, seq_b, frame_b, t, ..live }
}

/// Ratchet's head joint (the last joint of his head attach list, `rc_game::hero::worn::HEAD_ATTACH`).
pub(crate) const RATCHET_HEAD: u8 = 7;
/// Ratchet's eye joints (left, right): at rest 1.1 above his feet, the gameplay eye's height, and about 0.15 ahead of
/// his head joint, which sits at the base of his skull.
const RATCHET_EYES: [u8; 2] = [19, 20];

/// The joints from the root to `joint`, root first (the chain `moby_anim::evaluate_chains_posed` takes).
fn chain_to(class: &MobyAnimClass, joint: u8) -> Vec<u8> {
    let mut chain = vec![joint];
    while let Some(p) = class.parent(*chain.last().unwrap() as usize) {
        chain.push(p as u8);
    }
    chain.reverse();
    chain
}

/// Ratchet's arms: (forearm, the arm's first joint), left then right. The forearms carry the VR hand copies' cut
/// (crate::moby_attach: 74.. / 54..).
const RATCHET_ARMS: [(u8, u8); 2] = [(74, 72), (54, 52)];

/// Ratchet's stance in sequence `seq` (the hoverboard's ride): how far (radians, counterclockwise about the moby's up)
/// his shoulders are turned from his idle's (sequence 0, where he faces the moby's front). He rides side-on: the view
/// on the board faces where his body does, not where the board goes.
pub(crate) fn stance_yaw(class: &MobyAnimClass, seq: u8) -> Option<f32> {
    let chains = RATCHET_ARMS.map(|(_, shoulder)| chain_to(class, shoulder));
    let refs = [chains[0].as_slice(), chains[1].as_slice()];
    let line = |seq: u8| -> Option<f32> {
        class.sequence(seq)?;
        let s = AnimState { seq_a: seq, frame_a: 0, seq_b: seq, frame_b: 0, t: 0.0, speed: 0.0, rate: 0.0, flags: 0, trigger_count: 0, skip_advance: false };
        let p = moby_anim::evaluate_chains_posed(class, &s, None, &refs, &[], &[]);
        let (l, r) = (p.first()?[3], p.get(1)?[3]);
        let d = [l[0] - r[0], l[1] - r[1]];
        (d[0].hypot(d[1]) > 1e-3 && d.iter().all(|v| v.is_finite())).then(|| d[1].atan2(d[0]))
    };
    let a = line(seq)? - line(0)?;
    Some(a.sin().atan2(a.cos()))
}

/// `joint` is `root` or below it.
fn under(class: &MobyAnimClass, joint: usize, root: u8) -> bool {
    let mut j = Some(joint);
    while let Some(k) = j {
        if k == root as usize { return true; }
        j = class.parent(k);
    }
    false
}

/// Ratchet drawn with only his forearms and hands, as the VR hands show in play (scenes and ship sequences with the
/// option off): every other joint's palette matrix collapses onto an elbow, an arm's joints onto its own elbow so the
/// skin blended across it closes there, the rest of him to nothing.
pub(crate) fn ratchet_hands_only(class: &MobyAnimClass, state: &AnimState, mods: &[JointModifier], palette: &mut [Rows]) {
    let chains = RATCHET_ARMS.map(|(forearm, _)| chain_to(class, forearm));
    let m = moby_anim::evaluate_chains_posed(class, state, None, &[chains[0].as_slice(), chains[1].as_slice()], &[], mods);
    let (Some(l), Some(r)) = (m.first(), m.get(1)) else { return };
    let elbows = [[l[3][0], l[3][1], l[3][2]], [r[3][0], r[3][1], r[3][2]]];
    if elbows.iter().flatten().any(|v| !v.is_finite()) { return; }
    for (j, rows) in palette.iter_mut().enumerate() {
        if RATCHET_ARMS.iter().any(|&(forearm, _)| under(class, j, forearm)) { continue; }
        let e = if under(class, j, RATCHET_ARMS[1].1) { elbows[1] } else { elbows[0] };
        *rows = [[0.0; 4], [0.0; 4], [0.0; 4], [e[0], e[1], e[2], 1.0]];
    }
}

/// The point between Ratchet's eyes in his model space (unscaled, before his rows) for a pose: where a scene or a ship
/// sequence sees through him.
pub(crate) fn ratchet_eyes(class: &MobyAnimClass, state: &AnimState, mods: &[JointModifier]) -> Option<[f32; 3]> {
    let chains = RATCHET_EYES.map(|j| chain_to(class, j));
    let m = moby_anim::evaluate_chains_posed(class, state, None, &[chains[0].as_slice(), chains[1].as_slice()], &[], mods);
    let (a, b) = (m.first()?, m.get(1)?);
    let p: [f32; 3] = std::array::from_fn(|c| (a[3][c] + b[3][c]) * 0.5);
    p.iter().all(|v| v.is_finite()).then_some(p)
}

/// A ship sequence's view (Bevy space: position, rotation) while Ratchet's actor is there: his drawn actor is carried
/// so his drawn eyes and heading are the view's (crate::travel_render's draw), whatever the view follows. None outside
/// them.
static PILOT_VIEW: Mutex<Option<(Vec3, Quat)>> = Mutex::new(None);

pub(crate) fn set_pilot_view(view: Option<(Vec3, Quat)>) {
    if let Ok(mut s) = PILOT_VIEW.lock() { *s = view; }
}

pub(crate) fn pilot_view() -> Option<(Vec3, Quat)> {
    PILOT_VIEW.lock().ok().and_then(|s| *s)
}

/// Ratchet's drawn actor carried so his drawn eyes (`eyes`: Bevy space, as `model` draws them) and heading are the
/// view's: the transform applied before `model`.
pub(crate) fn carry_to_view(view: (Vec3, Quat), eyes: Vec3, heading: Quat) -> Mat4 {
    Mat4::from_translation(view.0) * Mat4::from_quat(view.1 * heading.inverse()) * Mat4::from_translation(-eyes)
}

/// A hero body class's head joint and forearm joints (left, right), measured on the original skeletons: each head joint
/// carries the whole head (Clank's and Giant Clank's from the neck), each forearm the hand copy's cut
/// (crate::moby_attach: Ratchet's 74.. / 54.., the bodies' `BODY_HANDS`).
const BODIES: [(i16, u8, [u8; 2]); 4] = [
    (0, RATCHET_HEAD, [74, 54]),
    (87, 8, [43, 33]),
    (419, 9, [36, 56]),
    (634, 9, [28, 47]),
];

/// The joints collapsed on a hero body of class `o_class` drawn in play: its head and forearms.
pub(crate) fn play_joints(o_class: i16) -> Option<[u8; 3]> {
    BODIES.iter().find(|b| b.0 == o_class).map(|&(_, head, [l, r])| [head, l, r])
}

/// Ratchet's joints collapsed in play: his head and forearms, and on the hoverboard his whole arms from the shoulders
/// (the jumps and tricks swung them through the view). The VR hands, drawn at the controllers, stay.
pub(crate) fn ratchet_play_joints(board: bool) -> [u8; 3] {
    let [head, l, r] = play_joints(0).unwrap_or_default();
    if board { [head, RATCHET_ARMS[0].1, RATCHET_ARMS[1].1] } else { [head, l, r] }
}

/// The horizontal offset (game x / y, world units) from a body's position to its head joint in its current pose. In
/// play the player's neck is above the hero's position while the animations carry the head away from it (a run leans
/// it forward): the drawn body moves back by this so its neck stays at the player's.
#[allow(clippy::too_many_arguments)]
pub(crate) fn head_offset(
    class: &MobyAnimClass,
    state: &AnimState,
    snap: Option<&MobyFrame>,
    layers: &[PoseLayer],
    mods: &[JointModifier],
    head: u8,
    rows: [[f32; 3]; 3],
    scale: f32,
) -> [f32; 2] {
    let chain = chain_to(class, head);
    let Some(h) = moby_anim::evaluate_chains_posed(class, state, snap, &[chain.as_slice()], layers, mods).into_iter().next() else {
        return [0.0; 2];
    };
    let s = scale / 1024.0;
    let off: [f32; 2] = std::array::from_fn(|c| (rows[0][c] * h[3][0] + rows[1][c] * h[3][1] + rows[2][c] * h[3][2]) * s);
    if off.iter().all(|v| v.is_finite()) { off } else { [0.0; 2] }
}

/// `mods` with each of `joints` scaled to 0 after them.
pub(crate) fn collapse(mods: &[JointModifier], joints: &[u8]) -> Vec<JointModifier> {
    let nodes = joints.iter().map(|&j| JointModifier { scale: [0.0; 3], ..JointModifier::compose(j) });
    mods.iter().copied().chain(nodes).collect()
}
