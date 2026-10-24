//! Optional tracked inputs enter the original state machine and collision pipeline.
use super::{Hero, physics::*, states::Ctx};
use crate::ps2v::Pf;

#[derive(Clone, Debug, Default)]
pub struct Tracked {
    pub enabled: bool,
    /// Physical wrench socket and the native capsule's angular travel around the bolt this tick.
    pub crank_target: Option<usize>,
    pub crank_turn: f32,
    pub crank_radius: f32,
    /// The last gameplay tick belonged to a short camera cutaway, not a dialogue/ship scene.
    pub cutaway_free: bool,
    pub pack_jump: bool,
    /// Physical tool insertion; separate from the hero's actual ground collision.
    pub utility_target: Option<usize>,
    pub utility_use: bool,
    /// Headset listener and levelled locomotion frames; neither overwrites the native camera.
    pub view: Option<crate::follow_camera::CameraView>,
    pub navigation: Option<crate::follow_camera::CameraView>,
    pub view_origin: [f32; 3],
    pub hands: [Option<[f32; 3]>; 2],
    pub grips: [bool; 2],
    pub hyper_strike: bool,
    pub hyper_buffer: i32,
    pub ledge_jump: bool,
    pub ledge_delta: [f32; 3],
    pub anchors: [Option<[f32; 3]>; 2],
    /// World velocity requested by submerged hand strokes, units / second.
    pub stroke: [f32; 3],
    pub swim_velocity: [f32; 3],
    pub grabbed: [bool; 2],
    /// Accepted grip anchors and native climb launches, consumed as presentation events.
    pub ledge_grabs: [u64; 2],
    pub ledge_climbs: u64,
    pub wrench_hits: u64,
    /// The thrown wrench's way back to the tracked hand (planned when it turns back).
    pub wrench_return: Option<WrenchReturn>,
}

/// The tracked wrench's return: one readable path from where it turned back to the hand that catches it.
/// It starts at rest (the outbound flight decelerates to zero), accelerates, then eases into the hand. The
/// spin keeps the outbound rate, slows near the hand and is phased to end on the held yaw.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WrenchReturn {
    pub from: [f32; 3],
    /// Progress (0..1) at the last game tick, and its gain per tick.
    pub progress: f32,
    pub step: f32,
    /// The spin about world up where it turned back, and its total over the return.
    pub spin_from: f32,
    pub spin: f32,
}

impl WrenchReturn {
    /// The native spin, radians per tick (`0x277380(0, 0, dt·24.43)`).
    const SPIN: f32 = 24.434_608 / 60.0;
    /// The spin rate holds until this progress, then falls linearly to [`Self::SPIN_END`] of it.
    const SPIN_HOLD: f32 = 0.55;
    const SPIN_END: f32 = 0.4;

    /// `hand_yaw`: the held wrench's shaft heading (None: no tracked hand, no phasing).
    pub fn plan(from: [f32; 3], hand: [f32; 3], spin_from: f32, hand_yaw: Option<f32>) -> Self {
        let dist = (0..3).map(|i| (hand[i] - from[i]).powi(2)).sum::<f32>().sqrt();
        // About 0.35 s for a short throw and 0.6 s from the native 8-unit turnaround.
        let ticks = (18.0 + dist * 2.1).clamp(21.0, 45.0);
        let nominal = Self::SPIN * ticks * Self::spin_area(1.0);
        let spin = match hand_yaw {
            Some(yaw) => {
                let tau = std::f32::consts::TAU;
                let gap = (yaw - spin_from - nominal).rem_euclid(tau);
                nominal + if gap > std::f32::consts::PI { gap - tau } else { gap }
            }
            None => nominal,
        };
        WrenchReturn { from, progress: 0.0, step: 1.0 / ticks, spin_from, spin }
    }

    /// The share of the path covered at progress `p`: 2.4p² − 1.4p³ (zero speed at the turn, 1.37× the mean
    /// speed at 57 %, 0.6× the mean speed into the hand).
    pub fn ease(p: f32) -> f32 {
        let p = p.clamp(0.0, 1.0);
        p * p * (2.4 - 1.4 * p)
    }

    /// The integral of the spin rate profile from 0 to `p`.
    fn spin_area(p: f32) -> f32 {
        let p = p.clamp(0.0, 1.0);
        let late = (p - Self::SPIN_HOLD).max(0.0);
        p - (1.0 - Self::SPIN_END) * late * late / (2.0 * (1.0 - Self::SPIN_HOLD))
    }

    /// The position and the spin at progress `p` toward `hand`.
    pub fn at(&self, p: f32, hand: [f32; 3]) -> ([f32; 3], f32) {
        let e = Self::ease(p);
        let pos = std::array::from_fn(|i| self.from[i] + (hand[i] - self.from[i]) * e);
        (pos, self.spin_from + self.spin * Self::spin_area(p) / Self::spin_area(1.0))
    }
}

impl Hero {
    pub(super) fn tracked_free_facing(&self) -> bool {
        self.tracked.enabled
            && self.tracked.navigation.is_some()
            && self.mode == 0
            && self.health > 0
            && self.frozen == 0
            && matches!(self.state, 0..=4 | 6..=0x12 | 0x2d | 0x3f | 0x70 | 0x71 | 0x73)
    }

    /// Keep physical heading on the native body, retaining its current surface normal.
    /// Rail, ledge, scripted interaction and attack poses retain their authored orientation.
    pub fn tracked_face(&mut self) {
        if !self.tracked_free_facing() {
            return;
        }
        let Some(view) = self.tracked.navigation else {
            return;
        };
        let up = to_f32x3(euler_rows(self.rot)[2]);
        let forward = to_f32x3(view.rows[0]);
        let dot = forward.iter().zip(up).map(|(a, b)| a * b).sum::<f32>();
        let mut forward: [f32; 3] = std::array::from_fn(|i| forward[i] - up[i] * dot);
        let len = forward.iter().map(|v| v * v).sum::<f32>().sqrt();
        if len < 0.01 {
            return;
        }
        forward = forward.map(|v| v / len);
        let left = [
            up[1] * forward[2] - up[2] * forward[1],
            up[2] * forward[0] - up[0] * forward[2],
            up[0] * forward[1] - up[1] * forward[0],
        ];
        let rows = [forward, left, up].map(|r| [Pf::f(r[0]), Pf::f(r[1]), Pf::f(r[2]), Pf::ZERO]);
        self.rot = crate::moby_update::services::rows_euler(&[
            rows[0],
            rows[1],
            rows[2],
            [Pf::ZERO, Pf::ZERO, Pf::ZERO, Pf::ONE],
        ]);
        self.rows = [
            rows[0],
            rows[1],
            rows[2],
            [Pf::ZERO, Pf::ZERO, Pf::ZERO, Pf::ONE],
        ];
        self.target_yaw = self.rot[2];
        self.yaw_vel = Pf::ZERO;
        self.yaw_residual = Pf::ZERO;
    }

    /// Run after damage/water checks, before ordinary button transitions.
    pub(super) fn tracked_transitions(&mut self, c: &mut Ctx) -> bool {
        if !self.tracked.enabled || self.mode != 0 || self.health <= 0 || self.frozen != 0 {
            return false;
        }
        if !matches!(self.state, 0x18..=0x1c)
            && matches!(self.prev_state, 0x18..=0x1b)
            && (0..2).any(|s| self.tracked.grabbed[s] && self.tracked.grips[s])
        {
            self.set_state(c, 0x19, false);
        }
        if matches!(self.state, 0x18..=0x1b) {
            let anchor = self.tracked_ledge_anchor();
            for side in 0..2 {
                // A pull moves the controller away from the lip; only grip release unbinds it.
                self.tracked.grabbed[side] &= self.tracked.grips[side];
                if self.tracked.grips[side]
                    && self.tracked.hands[side].is_some_and(|h| {
                        (h[0] - anchor[0]).hypot(h[1] - anchor[1]) < 1.05
                            && (h[2] - anchor[2]).abs() < 0.75
                    })
                {
                    self.tracked.grabbed[side] = true;
                    if self.tracked.anchors[side].is_none() {
                        let hand = self.tracked.hands[side].unwrap();
                        let tangent = [-self.ledge_blk.yaw.sin(), self.ledge_blk.yaw.cos()];
                        let along = ((hand[0] - anchor[0]) * tangent[0]
                            + (hand[1] - anchor[1]) * tangent[1])
                            .clamp(-0.65, 0.65);
                        self.tracked.ledge_grabs[side] =
                            self.tracked.ledge_grabs[side].wrapping_add(1);
                        self.tracked.anchors[side] = Some([
                            anchor[0] + tangent[0] * along,
                            anchor[1] + tangent[1] * along,
                            anchor[2],
                        ]);
                    }
                }
                if !self.tracked.grabbed[side] {
                    self.tracked.anchors[side] = None;
                }
            }
            if !self.tracked.grabbed.iter().any(|&g| g) {
                self.set_state(c, 6, true);
                self.f500 = ticks(20) as i16;
                return true;
            }
            if std::mem::take(&mut self.tracked.ledge_jump) && self.ledge_blk.f844 & 1 == 0 {
                self.tracked.grabbed = [false; 2];
                self.set_state(c, 0x1c, true);
                return true;
            }
        } else {
            self.tracked.grabbed = [false; 2];
            self.tracked.anchors = [None; 2];
            self.tracked.ledge_delta = [0.0; 3];
            self.tracked.ledge_jump = false;
            if self.f838 != 0 {
                let anchor = self.tracked_ledge_anchor();
                for side in 0..2 {
                    self.tracked.grabbed[side] = self.tracked.grips[side]
                        && self.tracked.hands[side].is_some_and(|hand| {
                            let d: [f32; 3] = std::array::from_fn(|k| hand[k] - anchor[k]);
                            // Reach along the edge is wider than reach into/across it.
                            let normal =
                                d[0] * self.ledge_blk.yaw.cos() + d[1] * self.ledge_blk.yaw.sin();
                            d[0].hypot(d[1]) < 1.05 && normal.abs() < 0.65 && d[2].abs() < 0.75
                        });
                }
                if !self.tracked.grabbed.iter().any(|&g| g) {
                    self.f838 = 0;
                }
            }
        }
        if super::walloper::tracked_fire(self, c) {
            return true;
        }
        if std::mem::take(&mut self.tracked.hyper_strike) {
            self.tracked.hyper_buffer = ticks(0x12);
        }
        let age = ticks(0x12) - self.tracked.hyper_buffer;
        if self.tracked.hyper_buffer > 0 && self.try_tracked_jump_attack(c, age) {
            self.tracked.hyper_buffer = 0;
            return true;
        }
        self.tracked.hyper_buffer = (self.tracked.hyper_buffer - 1).max(0);
        if self.grounded() || self.state == 0x14 {
            self.tracked.hyper_buffer = 0;
        }
        let charge = std::mem::take(&mut self.tracked.pack_jump);
        if charge && matches!(self.state, 0 | 2 | 3 | 4 | 0x73) && self.grounded() {
            // The VR left trigger is native R1. Preserve the original side/back flip chooser before the pack jump.
            if self.try_jump(c, ticks(7), true) {
                return true;
            }
            return super::packs::crouch_jump(self, c);
        }
        // Pushing water up with both hands requests a dive; native water states own oxygen and surfacing.
        if self.group == 0x12 && self.tracked.stroke[2] < -0.8 && self.swim.dive_lock == 0 {
            self.set_state(c, super::swim::id::UNDERWATER, true);
            return true;
        }
        false
    }

    pub fn tracked_ledge_anchor(&self) -> [f32; 3] {
        let l = &self.ledge_blk;
        [
            l.point[0] - l.yaw.cos() * 0.435,
            l.point[1] - l.yaw.sin() * 0.435,
            l.top_z,
        ]
    }

    /// Close the hand-anchor error along a validated ledge, without inertia or velocity prediction.
    pub(super) fn tracked_ledge_motion(&mut self, env: &super::Env, anim: &super::anim::AnimView) {
        let delta = std::mem::take(&mut self.tracked.ledge_delta);
        let tangent = [-self.ledge_blk.yaw.sin(), self.ledge_blk.yaw.cos()];
        let error = delta[0] * tangent[0] + delta[1] * tangent[1];
        if !error.is_finite() || error.abs() < 0.008 {
            return;
        }
        let along = (error - error.signum() * 0.008).clamp(-0.045, 0.045);
        let pos = self.pos;
        let old = self.ledge_blk;
        let old_found = self.f838;
        self.pos[0] = self.pos[0] + crate::ps2v::Pf::f(tangent[0] * along);
        self.pos[1] = self.pos[1] + crate::ps2v::Pf::f(tangent[1] * along);
        let there = to_f32x3(self.pos);
        let mut valid = false;
        if super::ledge::probe_c(self, env, there, self.rot[2].to_f32()) {
            super::ledge::tracked_ledge_probe(self, env, anim);
            let next = self.ledge_blk;
            let yaw_change = (next.yaw - old.yaw)
                .sin()
                .atan2((next.yaw - old.yaw).cos())
                .abs();
            valid = self.f838 != 0
                && next.moby == old.moby
                && (next.top_z - old.top_z).abs() < 0.15
                && yaw_change < 0.35;
            if valid {
                // A valid probe establishes wall/top/clearance. Its incidental tangential hit location
                // is replaced by the exact requested body position, so it cannot add another step.
                let drift = (there[0] - next.point[0]) * tangent[0]
                    + (there[1] - next.point[1]) * tangent[1];
                self.ledge_blk.point[0] += tangent[0] * drift;
                self.ledge_blk.point[1] += tangent[1] * drift;
                self.carry.flags &= !1; // Reattach moving platforms at the newly requested location.
            }
        }
        if !valid {
            self.ledge_blk = old;
            self.f838 = old_found;
        }
        self.pos = pos;
    }

    pub(super) fn tracked_swim(&mut self, surface: bool) {
        if !self.tracked.enabled {
            return;
        }
        // Bounded drag gives strokes momentum, without bypassing move_collide.
        let limit = if surface { 3.0 } else { 4.5 };
        let t = &mut self.tracked;
        for k in 0..3 {
            t.swim_velocity[k] = t.swim_velocity[k] * 0.96 + t.stroke[k] * 0.12;
            if surface && k == 2 {
                t.swim_velocity[k] = 0.0;
            }
        }
        if let Some(view) = t.view {
            let mut forward = to_f32x3(view.rows[0]);
            if surface {
                forward[2] = 0.0;
            }
            let len = forward.iter().map(|v| v * v).sum::<f32>().sqrt();
            if len > 0.01 {
                let speed = t.swim_velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
                t.swim_velocity = forward.map(|v| v / len * speed);
            }
        }
        let speed = t.swim_velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
        if speed > limit {
            for v in &mut t.swim_velocity {
                *v *= limit / speed;
            }
        }
        for k in 0..3 {
            self.vel[k] = self.vel[k] + crate::ps2v::Pf::f(t.swim_velocity[k] / 60.0);
        }
    }

    /// Continuous foot sweep before the native ground probe. Thin floors remain visible when a
    /// falling step or capsule correction crosses their plane between two simulation positions.
    pub(super) fn tracked_floor_sweep(&mut self, env: &super::Env, before: V4) {
        use crate::ps2v::Pf;
        if !self.tracked.enabled
            || self.mode != 0
            || self.gravity_mode != 0
            || self.health <= 0
            || self.frozen != 0
            || self.pos[2] >= before[2]
            || !matches!(self.group, 0 | 1 | 2 | 4)
        {
            return;
        }
        let mut a = before;
        let mut b = self.pos;
        a[2] = a[2] + Pf::f(0.035);
        b[2] = b[2] - Pf::f(0.02);
        let Some(hit) = env.line(a, b, 0x24) else {
            return;
        };
        let n = hit.normal;
        let length = n.iter().map(|v| v * v).sum::<f32>().sqrt();
        if length < 1e-6 || n[2] / length < 0.65 || matches!(hit.surface_id(), 0 | 3 | 0xb | 0xd) {
            return;
        }
        if hit.point[2] > before[2].to_f32() + 0.035 {
            return;
        }
        self.pos[2] = Pf::f(hit.point[2] + 0.003);
        if self.vel[2] < Pf::ZERO {
            self.vel[2] = Pf::ZERO;
        }
    }
}
