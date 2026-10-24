//! Feedback follows native firing, accepted damage, gadget states and puzzle changes.
use super::{
    embodied::Motion,
    game_input::{Actions, Feedback, Focused, HandInput},
};
use bevy::prelude::*;
use bevy_mod_openxr::session::OxrSession;
use openxr::{HapticVibration, Path};

#[derive(Clone, Copy)]
struct Pulse {
    start: f64,
    end: f64,
    amplitude: f32,
    frequency: f32,
    priority: u8,
    owner: i32,
}
#[derive(Clone, Copy)]
struct Sent {
    key: (u64, u32, u32),
    until: f64,
}
#[derive(Clone, PartialEq)]
struct Lock {
    id: usize,
    state: u8,
    ring: i32,
    rotation: [i32; 3],
    powered: usize,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum PackUse {
    HeliLift,
    HeliGlide,
    ThrusterBoost,
    ThrusterGlide,
    ThrusterHover,
    ThrusterStompRise,
    ThrusterStompDive,
    HydroCruise,
    HydroBoost,
}
impl PackUse {
    fn owner(self) -> i32 {
        match self {
            Self::HeliLift | Self::HeliGlide => -102,
            Self::HydroCruise | Self::HydroBoost => -104,
            _ => -103,
        }
    }
    fn start(self) -> (f32, u32, f32) {
        match self {
            Self::HeliLift => (0.32, 38, 165.0),
            Self::HeliGlide => (0.23, 30, 175.0),
            Self::ThrusterBoost | Self::ThrusterStompRise => (0.68, 65, 65.0),
            Self::ThrusterStompDive => (0.52, 60, 50.0),
            Self::ThrusterGlide => (0.38, 45, 80.0),
            Self::ThrusterHover => (0.30, 50, 90.0),
            Self::HydroCruise => (0.27, 70, 50.0),
            Self::HydroBoost => (0.43, 85, 60.0),
        }
    }
}
#[derive(Clone, Copy)]
struct PackSample {
    module: i32,
    phase: Option<PackUse>,
    load: f32,
    state: i32,
    timer: i32,
    stomp_ground: i32,
}
fn pack_sample(h: &rc_game::hero::Hero, pad: u32) -> PackSample {
    use rc_game::hero::{
        packs::{id, item},
        physics::{DT, ticks},
        swim,
    };
    let module = if h.mode == 0 && h.back_slot.clank_hidden == 0 {
        h.back_module()
    } else {
        -1
    };
    let speed = h.speed.to_f32().abs() / DT.to_f32();
    let phase = match module {
        item::HELI_PACK => match h.state {
            id::GLIDE => Some(PackUse::HeliGlide),
            id::HELI_HIGH_JUMP
                if h.jump.landed == 0
                    && h.jump.curve_on != 0
                    && h.timer >= h.jump.curve_from
                    && h.timer < h.jump.curve_to =>
            {
                Some(PackUse::HeliLift)
            }
            id::HELI_LONG_JUMP
                if h.jump.landed == 0 && h.timer >= h.jump.takeoff && h.jump.descending == 0 =>
            {
                Some(PackUse::HeliLift)
            }
            0xe if h.jump.landed == 0 && h.jump.descending == 0 => Some(PackUse::HeliLift),
            _ => None,
        },
        item::THRUSTER_PACK
            if rc_game::moby_update::classes::thruster_flame::thrusting(h, ticks) =>
        {
            Some(match h.state {
                id::GLIDE => PackUse::ThrusterGlide,
                id::HOVER => PackUse::ThrusterHover,
                id::STOMP if h.packs.stomp_down => PackUse::ThrusterStompDive,
                id::STOMP => PackUse::ThrusterStompRise,
                _ => PackUse::ThrusterBoost,
            })
        }
        item::HYDRO_PACK
            if h.state == swim::id::HYDRO
                && speed.max(h.target_speed.to_f32().abs() / DT.to_f32()) > 0.3 =>
        {
            Some(
                if pad & (rc_game::pad::button::R1 | rc_game::pad::button::R2) != 0 {
                    PackUse::HydroBoost
                } else {
                    PackUse::HydroCruise
                },
            )
        }
        _ => None,
    };
    PackSample {
        module,
        phase,
        load: if h.state == id::STOMP {
            (h.vel[2].to_f32().abs() / DT.to_f32() / 20.0).clamp(0.0, 1.0)
        } else {
            (speed / 7.0).clamp(0.0, 1.0)
        },
        state: h.state,
        timer: h.timer,
        stomp_ground: h.packs.stomp_ground,
    }
}
fn pack_owner(owner: i32) -> bool {
    matches!(owner, -102 | -103 | -104)
}
struct Snapshot {
    level: u32,
    tick: u64,
    health: i32,
    body: u8,
    impacts: u64,
    blaster: u64,
    wrench: u32,
    wrench_hits: u64,
    wrench_held: bool,
    hyper_landings: u64,
    ledge_grabs: [u64; 2],
    ledge_climbs: u64,
    suck_shots: u32,
    vacuumed: u32,
    suck_load: i32,
    visibomb: Option<usize>,
    salvo: u8,
    item: i32,
    hero_state: i32,
    walloper_until: u64,
    walloper_hits: u32,
    fist_strikes: [u64; 2],
    fist_hits: [u64; 2],
    morphs: usize,
    hydro_full: bool,
    hooked: bool,
    swing_state: u8,
    detector_dig: i32,
    vendor: bool,
    bolts: i64,
    used: Vec<i64>,
    bomb: Option<(usize, i32)>,
    gravity: i32,
    rail: Option<usize>,
    lock: Option<Lock>,
    pack: PackSample,
}
struct Deployment {
    item: i32,
    anim: rc_formats::moby_anim::AnimState,
    palette: Vec<rc_formats::moby_anim::Rows>,
    speed: f32,
    started: u64,
    last_tick: u64,
    last_pulse: u64,
    clicks: usize,
}
#[derive(Clone, Copy)]
struct DeployProfile {
    strength: f32,
    hz: f32,
    ms: u32,
    accents: &'static [f32],
}
fn deploy_profile(item: i32) -> DeployProfile {
    let (strength, hz, ms, accents): (f32, f32, u32, &'static [f32]) = match item {
        8 => (0.26, 155.0, 20, &[0.55, 1.0]),
        9 => (0.62, 55.0, 48, &[0.4, 0.65, 1.0, 0.8]), // Suction chamber and front jaws.
        10 => (0.28, 130.0, 24, &[0.5, 1.0, 0.65]),
        11 => (0.70, 65.0, 42, &[0.5, 0.85, 0.6, 1.0]), // Launcher rails, then lock.
        12 => (0.30, 175.0, 18, &[0.6, 0.45, 1.0]),
        13 => (0.48, 105.0, 28, &[0.4, 0.7, 0.5, 1.0]), // Scope hinge and screen detent.
        14 => (0.36, 90.0, 30, &[0.5, 1.0, 0.6]),
        15 => (0.34, 185.0, 18, &[0.45, 1.0, 0.65]),
        16 => (0.46, 75.0, 34, &[0.4, 0.75, 1.0]),
        17 => (0.30, 160.0, 20, &[0.35, 1.0, 0.6]),
        18 => (0.66, 50.0, 45, &[0.35, 0.6, 1.0]),
        19 => (0.40, 200.0, 16, &[0.4, 0.7, 0.45, 1.0]),
        20 => (0.38, 120.0, 27, &[0.45, 0.8, 1.0, 0.55]),
        21 => (0.32, 150.0, 24, &[0.5, 0.7, 0.4, 1.0]),
        22 => (0.48, 60.0, 38, &[0.45, 0.7, 1.0]),
        23 => (0.78, 55.0, 40, &[0.3, 0.5, 0.75, 0.6, 1.0]),
        24 => (0.26, 190.0, 18, &[0.4, 1.0, 0.65, 0.5]),
        25 => (0.30, 100.0, 30, &[0.5, 0.8, 1.0]),
        26 => (0.24, 170.0, 17, &[0.4, 0.65, 1.0]),
        27 => (0.28, 140.0, 22, &[0.5, 0.75, 1.0]),
        31 => (0.24, 195.0, 18, &[0.6, 0.4, 1.0]),
        32 => (0.22, 180.0, 16, &[0.5, 1.0, 0.6]),
        _ => (0.25, 140.0, 20, &[0.6, 1.0]),
    };
    DeployProfile {
        strength,
        hz,
        ms,
        accents,
    }
}
#[derive(Resource, Default)]
pub(super) struct State {
    previous: Option<Snapshot>,
    deployment: Option<Deployment>,
    cues: [Vec<Pulse>; 2],
    sent: [Option<Sent>; 2],
    continuous: Option<i32>,
    next: f64,
    error_reported: bool,
    pack_phase: Option<PackUse>,
    pack_next: f64,
    pack_started: f64,
    pack_beat: u64,
    bolt_next: f64,
    rail_next: f64,
}
impl State {
    fn pulse(
        &mut self,
        hand: usize,
        now: f64,
        amplitude: f32,
        ms: u32,
        hz: f32,
        priority: u8,
        owner: i32,
    ) {
        self.cues[hand].push(Pulse {
            start: now,
            end: now + ms as f64 / 1000.0,
            amplitude,
            frequency: hz,
            priority,
            owner,
        });
        if self.cues[hand].len() > 48 {
            self.cues[hand].remove(0);
        }
    }
    fn right(&mut self, now: f64, amplitude: f32, ms: u32, hz: f32, owner: i32) {
        self.pulse(1, now, amplitude, ms, hz, 5, owner);
    }
    fn double(&mut self, now: f64, amp: f32, owner: i32) {
        self.right(now, amp, 35, 140.0, owner);
        self.right(now + 0.075, amp * 0.75, 30, 180.0, owner);
    }
    fn hurt(&mut self, now: f64) {
        self.cues = default();
        for side in 0..2 {
            self.pulse(side, now, 1.0, 135, 55.0, 20, -1);
            self.pulse(side, now + 0.16, 0.65, 60, 35.0, 20, -1);
        }
    }
    fn stop_packs(&mut self) {
        for cues in &mut self.cues {
            cues.retain(|p| !pack_owner(p.owner));
        }
        self.pack_phase = None;
        self.pack_beat = 0;
        self.pack_next = 0.0;
    }
    fn packs(&mut self, now: f64, n: &PackSample, o: &PackSample) {
        if n.module != o.module {
            self.stop_packs();
        }
        let repeated_boost =
            n.phase == Some(PackUse::ThrusterBoost) && (n.state != o.state || n.timer < o.timer);
        if self.pack_phase != n.phase || repeated_boost {
            for cues in &mut self.cues {
                // Keep a native stomp impact ringing after the motor itself has stopped.
                cues.retain(|p| !pack_owner(p.owner) || p.priority >= 8);
            }
            let previous = self.pack_phase;
            self.pack_phase = n.phase;
            self.pack_next = now;
            self.pack_started = now;
            self.pack_beat = 0;
            if let Some(phase) = n.phase {
                let (mut amp, ms, hz) = phase.start();
                if !repeated_boost && previous.is_some_and(|p| p.owner() == phase.owner()) {
                    amp *= 0.65;
                }
                for side in 0..2 {
                    self.pulse(side, now, amp, ms, hz, 4, phase.owner());
                    self.pulse(
                        side,
                        now + ms as f64 / 1000.0 + 0.025,
                        amp * 0.42,
                        28,
                        hz * 1.25,
                        3,
                        phase.owner(),
                    );
                }
            } else if let Some(previous) = previous.filter(|_| n.module == o.module) {
                // Gentle motor release, only after actual operation, never on merely equipping a pack.
                for side in 0..2 {
                    self.pulse(side, now, 0.10, 25, 95.0, 2, previous.owner());
                }
            }
        }
        if n.module == 3 && n.stomp_ground > 0 && o.stomp_ground == 0 {
            for side in 0..2 {
                self.pulse(side, now, 0.92, 105, 50.0, 8, -103);
                self.pulse(side, now + 0.13, 0.32, 45, 85.0, 8, -103);
            }
        }
        let Some(phase) = n.phase else { return };
        if now < self.pack_next {
            return;
        }
        let ramp = ((now - self.pack_started) / 0.18).clamp(0.0, 1.0) as f32;
        let wave = (self.pack_beat as f32 * 1.3).sin() * 0.025;
        let (amp, ms, hz, interval) = match phase {
            PackUse::HeliLift => (0.18 + 0.08 * ramp, 32, 165.0, 0.055),
            PackUse::HeliGlide => (0.12 + 0.04 * ramp, 28, 180.0, 0.065),
            PackUse::ThrusterBoost => (0.33 + n.load * 0.10 + wave, 65, 65.0, 0.085),
            PackUse::ThrusterGlide => (0.23 + wave, 60, 75.0, 0.09),
            PackUse::ThrusterHover => (0.16 + n.load * 0.07 + wave, 85, 85.0, 0.11),
            PackUse::ThrusterStompRise => (0.38 + wave, 55, 65.0, 0.075),
            PackUse::ThrusterStompDive => (0.24 + n.load * 0.24, 60, 50.0, 0.08),
            PackUse::HydroCruise => (0.10 + n.load * 0.14 + wave * 0.4, 100, 50.0, 0.12),
            PackUse::HydroBoost => (0.25 + n.load * 0.16 + wave * 0.4, 110, 65.0, 0.11),
        };
        for side in 0..2 {
            let rotor = if matches!(phase, PackUse::HeliLift | PackUse::HeliGlide)
                && side as u64 != self.pack_beat % 2
            {
                0.60
            } else {
                1.0
            };
            // Independent pack cadence; weapon firing/continuous use and damage remain stronger priorities.
            self.pulse(side, now, amp * rotor, ms, hz, 0, phase.owner());
        }
        self.pack_next = now + interval;
        self.pack_beat = self.pack_beat.wrapping_add(1);
    }
    /// Follow the item's original take-out sequence 0. Pulses occur on mechanical motion changes
    /// and native animation sound markers, rather than on a wall-clock equip timer.
    fn deploy(&mut self, now: f64, p: &crate::gameplay::Play, support: bool) {
        let h = &p.game.hero;
        let Some(item) = h.items.slot.item.as_ref() else {
            self.deployment = None;
            return;
        };
        let Some(class) = p
            .game
            .item_data
            .as_ref()
            .and_then(|d| d.class(item.o_class))
        else {
            return;
        };
        let previous = self.deployment.take();
        let prior = previous.filter(|d| d.item == h.items.slot.id && d.last_tick <= p.game.counter);
        if item.anim.seq_b != 0 {
            if prior.is_some() {
                let profile = deploy_profile(h.items.slot.id);
                self.pulse(
                    1,
                    now,
                    profile.strength * 0.8,
                    profile.ms,
                    profile.hz * 1.15,
                    3,
                    h.items.slot.id,
                );
            }
            return;
        }
        if prior
            .as_ref()
            .is_some_and(|d| d.last_tick == p.game.counter)
        {
            self.deployment = prior;
            return;
        }
        let palette = rc_formats::moby_anim::evaluate_with_snapshot(
            &class.anim,
            &item.anim,
            item.snapshot.as_ref(),
        );
        let dt = prior.as_ref().map_or(1.0 / 60.0, |d| {
            (p.game.counter - d.last_tick).max(1) as f32 / 60.0
        });
        let speed = prior.as_ref().map_or(0.0, |d| {
            palette
                .iter()
                .zip(&d.palette)
                .map(|(a, b)| {
                    let translation =
                        Vec3::from_array([a[3][0] - b[3][0], a[3][1] - b[3][1], a[3][2] - b[3][2]])
                            .length()
                            * class.scale
                            / 1024.0;
                    let rotation = (0..3)
                        .map(|i| (0..3).map(|j| (a[i][j] - b[i][j]).abs()).sum::<f32>())
                        .sum::<f32>()
                        * 0.012;
                    translation + rotation
                })
                .fold(0.0, f32::max)
                / dt
        });
        let jerk = prior.as_ref().map_or(0.0, |d| (speed - d.speed).abs());
        let sound = prior.as_ref().is_some_and(|d| {
            rc_formats::moby_anim::advance_trigger(&d.anim, &item.anim, &class.anim).is_some()
        });
        let ready = prior
            .as_ref()
            .is_none_or(|d| p.game.counter.saturating_sub(d.last_pulse) >= 3);
        let mechanical = !(rc_game::hero::items::is_glove(h.items.slot.id)
            && item.anim.seq_a == rc_formats::moby_anim::SNAPSHOT_SEQ);
        let click = prior.is_none()
            || (ready
                && prior
                    .as_ref()
                    .is_some_and(|d| p.game.counter.saturating_sub(d.started) < 120)
                && mechanical
                && (sound || jerk > 0.12));
        let mut clicks = prior.as_ref().map_or(0, |d| d.clicks);
        if click {
            let profile = deploy_profile(h.items.slot.id);
            let accent = profile.accents[clicks % profile.accents.len()];
            let strength = profile.strength * accent * (0.8 + jerk.min(0.8) * 0.4);
            self.pulse(1, now, strength, profile.ms, profile.hz, 3, h.items.slot.id);
            if support {
                self.pulse(
                    0,
                    now,
                    strength * 0.4,
                    profile.ms,
                    profile.hz,
                    3,
                    h.items.slot.id,
                );
            }
            clicks += 1;
        }
        self.deployment = Some(Deployment {
            item: h.items.slot.id,
            anim: item.anim,
            palette,
            speed,
            started: prior.as_ref().map_or(p.game.counter, |d| d.started),
            last_tick: p.game.counter,
            last_pulse: if click {
                p.game.counter
            } else {
                prior.as_ref().map_or(p.game.counter, |d| d.last_pulse)
            },
            clicks,
        });
    }
    fn repeating(
        &mut self,
        now: f64,
        owner: i32,
        amp: f32,
        ms: u32,
        hz: f32,
        interval: f64,
        support: bool,
    ) {
        if self.continuous != Some(owner) {
            self.next = now;
        }
        self.continuous = Some(owner);
        if now < self.next {
            return;
        }
        self.next = now + interval;
        self.pulse(1, now, amp, ms, hz, 1, owner);
        if support {
            self.pulse(0, now, amp * 0.45, ms, hz, 1, owner);
        }
    }
}

fn lock_snapshot(p: &crate::gameplay::Play, previous: Option<&Lock>) -> Option<Lock> {
    p.game.mobys.mobys.iter().enumerate().find_map(|(id, m)| {
        if m.o_class != 615 || m.pvars.len() < 0x120 {
            return None;
        }
        let on = i16::from_le_bytes(m.pvars[0x12..0x14].try_into().ok()?);
        let finishing = previous.is_some_and(|l| l.id == id && l.state != 4 && m.state == 4);
        if (on == 0 || !matches!(m.state, 1..=3)) && !finishing {
            return None;
        }
        // Native Trespasser lock pvars: chosen ring e0, ring offsets 14/18/1c, outputs b0.
        let read = |off| i32::from_le_bytes(m.pvars[off..off + 4].try_into().unwrap());
        Some(Lock {
            id,
            state: m.state,
            ring: read(0xe0),
            rotation: [read(0x14), read(0x18), read(0x1c)],
            powered: (0..12).filter(|i| read(0xb0 + i * 4) == 2).count(),
        })
    })
}

pub(super) fn update(
    actions: Option<Res<Actions>>,
    session: Option<Res<OxrSession>>,
    focus: Res<Focused>,
    mode: Res<crate::menu_render::MenuMode>,
    play: Option<Res<crate::gameplay::Play>>,
    saved: Option<Res<crate::gameplay::Persistent>>,
    motion: Res<Motion>,
    input: Res<HandInput>,
    bomb: Res<super::bomb_glove::HeldBomb>,
    support: Res<super::blaster::SupportGrip>,
    mut feedback: ResMut<Feedback>,
    mut state: ResMut<State>,
    time: Res<Time<Real>>,
) {
    let physical_wrench = std::mem::take(&mut feedback.wrench_impact);
    let wall_impact = feedback.wrench_wall.take();
    let now = time.elapsed_secs_f64();
    let snapshot = play.as_ref().map(|p| {
        let h = &p.game.hero;
        Snapshot {
            level: crate::level_load::level_index(),
            tick: p.game.counter,
            health: h.health,
            body: h.mode,
            impacts: h.damage.impacts,
            blaster: h.weapons.blaster.shots,
            wrench: h.items.hit_sounds,
            wrench_hits: h.tracked.wrench_hits,
            wrench_held: motion.wrench_held() && h.items.slot.id == 8,
            hyper_landings: h.melee.hyper_landings,
            ledge_grabs: h.tracked.ledge_grabs,
            ledge_climbs: h.tracked.ledge_climbs,
            suck_shots: h.weapons.reactive.suck.fired,
            vacuumed: h.weapons.reactive.suck.vacuumed,
            suck_load: p.svc.creatures.react.held,
            visibomb: h.weapons.visibomb.missile,
            salvo: h.weapons.ryno.count,
            item: h.items.slot.id,
            hero_state: h.state,
            walloper_until: h.walloper.tracked_until,
            walloper_hits: h.walloper.stats[3],
            fist_strikes: h.tracked.fist_strikes,
            fist_hits: h.tracked.fist_hits,
            morphs: h.weapons.reactive.morph.morphs.len(),
            hydro_full: h.gadgets.hydro_full,
            hooked: h.swing.hooked,
            swing_state: h.swing.item.state,
            detector_dig: h.gadgets.detector.w8,
            vendor: mode.spatial_vendor && mode.state.mode == rc_game::menus::mode::Mode::Vendor,
            bolts: saved.as_ref().map_or(0, |s| s.0.global.bolts as i64),
            used: saved.as_ref().map_or_else(Vec::new, |s| {
                s.0.global.ammo_used.iter().map(|&v| v as i64).collect()
            }),
            bomb: bomb.id.map(|id| (id, bomb.item)),
            gravity: h.gravity_mode as i32,
            rail: (h.group == 0xf && h.boots.off_rail == 0)
                .then_some(h.boots.rail)
                .flatten(),
            pack: pack_sample(h, p.game.pad.held),
            lock: if h.items.slot.id == 26 {
                lock_snapshot(p, state.previous.as_ref().and_then(|s| s.lock.as_ref()))
            } else {
                None
            },
        }
    });
    let old = state.previous.take();
    let stable = snapshot
        .as_ref()
        .zip(old.as_ref())
        .is_some_and(|(n, o)| n.level == o.level && n.tick >= o.tick);
    let active = focus.0 && mode.free_movement() && stable;
    if !active {
        state.cues = default();
        state.deployment = None;
        state.continuous = None;
        state.stop_packs();
    } else if let (Some(n), Some(o), Some(p)) = (snapshot.as_ref(), old.as_ref(), play.as_ref()) {
        let h = &p.game.hero;
        let hurt = n.impacts > o.impacts || (n.body == o.body && n.health < o.health);
        if hurt {
            state.hurt(now);
        }
        if h.health > 0 && !p.death_pending {
            if n.tick > o.tick {
                state.packs(now, &n.pack, &o.pack);
            } else {
                state.stop_packs();
            }
            if n.item != o.item {
                for cues in &mut state.cues {
                    cues.retain(|p| p.owner < 0);
                }
                state.continuous = None;
            }
            state.deploy(now, p, support.held);
            for side in 0..2 {
                if n.ledge_grabs[side] > o.ledge_grabs[side] {
                    state.pulse(side, now, 0.23, 25, 175.0, 6, -2);
                }
            }
            if n.ledge_climbs > o.ledge_climbs {
                // Native accepted launch, whether requested by A or a simultaneous arm shove.
                for side in 0..2 {
                    state.pulse(side, now, 0.65, 75, 85.0, 7, -2);
                    state.pulse(side, now + 0.095, 0.24, 30, 145.0, 7, -2);
                }
            }
            if n.hero_state == 0x14 && o.hero_state != 0x14 {
                state.right(now, 0.28, 30, 110.0, 8);
            }
            if n.hyper_landings > o.hyper_landings {
                // A single grounded landing event, not every airborne shockwave sample.
                state.pulse(1, now, 0.95, 105, 55.0, 8, 8);
                state.pulse(0, now, 0.48, 75, 55.0, 8, 8);
                state.pulse(1, now + 0.12, 0.30, 35, 125.0, 8, 8);
            }
            if let Some(force) =
                wall_impact.filter(|_| motion.wrench_held() && n.item == 8 && n.hero_state != 0x14)
            {
                let amplitude = (0.3 + force * 0.085).clamp(0.35, 0.9);
                let duration = (30.0 + force * 7.0).clamp(35.0, 85.0) as u32;
                state.right(now, amplitude, duration, 95.0, 8);
            }
            if n.wrench_held && !o.wrench_held {
                let profile = deploy_profile(8);
                state.pulse(1, now, profile.strength, profile.ms, profile.hz, 3, 8);
            }
            if physical_wrench
                || n.wrench_hits > o.wrench_hits
                || (motion.wrench_held() && n.item == 8 && n.wrench > o.wrench)
            {
                state.pulse(1, now, 0.92, 85, 75.0, 10, 8);
                state.pulse(1, now + 0.09, 0.32, 30, 135.0, 10, 8);
            }
            if n.blaster > o.blaster {
                state.right(now, 0.35, 25, 160.0, 15);
            }
            for side in 0..2 {
                if n.fist_hits[side] > o.fist_hits[side] {
                    state.pulse(side, now, 0.92, 85, 75.0, 10, 8);
                    state.pulse(side, now + 0.09, 0.32, 30, 135.0, 10, 8);
                } else if n.fist_strikes[side] > o.fist_strikes[side] {
                    state.pulse(side, now, 0.22, 25, 160.0, 4, 8);
                }
            }
            // Ammo-used is the native committed-use statistic; pickups/refills and cancelled throws do not increase it.
            for (id, (&used, &before)) in n.used.iter().zip(&o.used).enumerate() {
                if used <= before {
                    continue;
                }
                let pulse = match id {
                    10 => Some((0.48, 60, 90.0)),
                    11 => Some((0.90, 110, 55.0)),
                    13 => Some((0.72, 90, 65.0)),
                    17 => Some((0.36, 35, 150.0)),
                    20 => {
                        state.double(now, 0.50, 20);
                        None
                    }
                    24 => {
                        state.double(now, 0.30, 24);
                        None
                    }
                    25 => Some((0.25, 45, 125.0)),
                    _ => None,
                };
                if let Some((amp, ms, hz)) = pulse {
                    state.right(now, amp, ms, hz, id as i32);
                    if support.held && matches!(id, 11 | 13) {
                        state.pulse(0, now, amp * 0.45, ms, hz, 5, id as i32);
                    }
                }
            }
            if n.item == 23 && n.item == o.item && n.salvo > o.salvo {
                state.right(now, 0.80, 65, 70.0, 23);
                if support.held {
                    state.pulse(0, now, 0.40, 65, 70.0, 5, 23);
                }
            }
            if n.suck_shots > o.suck_shots {
                state.pulse(1, now, 1.0, 145, 45.0, 10, 9);
                state.pulse(1, now + 0.16, 0.45, 55, 90.0, 10, 9);
                if support.held {
                    state.pulse(0, now, 0.65, 110, 45.0, 10, 9);
                }
            }
            if n.suck_load > o.suck_load {
                state.pulse(1, now, 0.95, 115, 60.0, 10, 9);
                state.pulse(1, now + 0.13, 0.55, 65, 100.0, 10, 9);
                if support.held {
                    state.pulse(0, now, 0.5, 90, 60.0, 10, 9);
                }
            } else if n.vacuumed > o.vacuumed {
                state.right(now, 0.20, 20, 140.0, 9);
            }
            if o.visibomb.is_some() && n.visibomb.is_none() && n.item == 13 {
                state.right(now, 0.65, 85, 50.0, 13);
            }
            if n.bomb != o.bomb && n.bomb.is_some() {
                state.right(now, 0.20, 22, 150.0, n.bomb.unwrap().1);
            }
            if (n.walloper_until > o.walloper_until)
                || (n.hero_state == 0x20 && o.hero_state != 0x20)
            {
                state.right(now, 0.50, 65, 75.0, 18);
            }
            if n.walloper_hits > o.walloper_hits {
                state.right(now, 0.85, 85, 50.0, 18);
            }
            if n.morphs > o.morphs {
                state.double(now, 0.55, 21);
            }
            if n.hooked && !o.hooked {
                state.right(now, 0.70, 70, 95.0, 12);
            } else if n.swing_state != o.swing_state && matches!(n.swing_state, 2 | 9) {
                state.right(now, 0.35, 30, 155.0, 12);
            }
            if !n.hooked && o.hooked {
                state.right(now, 0.22, 25, 140.0, 12);
            }
            if n.hydro_full != o.hydro_full {
                state.double(now, 0.48, 22);
            }
            if n.body != o.body && (n.body == 3 || o.body == 3) {
                state.double(now, 0.28, 31);
            }
            if n.vendor && !o.vendor && n.item == 32 {
                state.double(now, 0.22, 32);
            }
            if n.vendor && o.vendor && n.bolts < o.bolts && n.item == 32 {
                state.double(now, 0.40, 32);
            }
            if n.detector_dig == 0x7fff && o.detector_dig != 0x7fff && n.item == 27 {
                state.double(now, 0.50, 27);
            }
            if n.item == 26 {
                if let Some((a, b)) = n
                    .lock
                    .as_ref()
                    .zip(o.lock.as_ref())
                    .filter(|(a, b)| a.id == b.id)
                {
                    if a.ring != b.ring {
                        state.right(now, 0.15, 18, 180.0, 26);
                    }
                    if a.rotation != b.rotation {
                        state.right(now, 0.26, 25, 145.0, 26);
                    }
                    if a.powered > b.powered {
                        state.right(now, 0.40, 35, 190.0, 26);
                    }
                    if a.state == 4 && b.state != 4 {
                        state.double(now, 0.55, 26);
                    }
                }
                if n.hero_state == 0x72 && o.hero_state != 0x72 {
                    state.right(now, 0.30, 40, 120.0, 26);
                }
            }
            // Back/boot gadgets give attachment/launch cues, rather than vibrating while merely equipped.
            if n.bolts > o.bolts && !n.vendor && now >= state.bolt_next {
                state.bolt_next = now + 0.09;
                for side in 0..2 {
                    state.pulse(side, now, 0.10, 16, 185.0, 0, -105);
                }
            }
            if n.rail.is_some() && n.tick > o.tick && now >= state.rail_next {
                state.rail_next = now + 0.085;
                let amplitude = (0.055 + h.boots.speed.abs() * 0.004).clamp(0.055, 0.14);
                for side in 0..2 {
                    state.pulse(side, now, amplitude, 55, 90.0, 0, -106);
                }
            }
            if (n.gravity != 0 && o.gravity == 0) || (n.rail.is_some() && n.rail != o.rail) {
                for side in 0..2 {
                    state.pulse(side, now, 0.35, 60, 80.0, 4, -2);
                }
            }
            let mstate = h.items.slot.item.as_ref().map_or(0, |i| i.mstate);
            let wave = (now as f32 * 21.0).sin() * 0.05;
            let ready = h.items.slot.state == 2 && !n.vendor;
            let continuous = if ready {
                match n.item {
                    16 if mstate == 3 => Some((16, 0.28 + wave, 70, 70.0, 0.08, support.held)),
                    19 if h.weapons.tesla.firing => Some((
                        19,
                        0.36 + wave
                            + h.weapons
                                .tesla
                                .targets
                                .iter()
                                .filter(|t| t.is_some())
                                .count() as f32
                                * 0.06,
                        32,
                        185.0,
                        0.055,
                        false,
                    )),
                    21 if h.weapons.reactive.morph.firing => Some((
                        21,
                        if h.weapons.reactive.morph.target.is_some() {
                            0.30
                        } else {
                            0.18
                        },
                        80,
                        125.0,
                        0.10,
                        false,
                    )),
                    9 if matches!(mstate, 2 | 3) => Some((
                        9,
                        if h.weapons.reactive.suck.pulled.is_empty() {
                            0.22
                        } else {
                            0.36
                        },
                        75,
                        55.0,
                        0.11,
                        support.held,
                    )),
                    14 if h.weapons.reactive.taunter.on_seq > 0 => {
                        Some((14, 0.24, 40, 155.0, 0.16, false))
                    }
                    12 if n.hooked && h.swing.item.rope.is_some() => Some((
                        12,
                        (0.14 + h.swing.rope_vel.abs() * 0.05).clamp(0.14, 0.38),
                        45,
                        65.0,
                        0.15,
                        false,
                    )),
                    22 if matches!(n.hero_state, 0x38 | 0x39) => {
                        Some((22, 0.25 + wave, 90, 55.0, 0.12, false))
                    }
                    27 if p.game.pad.held & rc_game::pad::button::CIRCLE != 0
                        && h.items.slot.ticks_ready > rc_game::hud::scale_ticks(15) =>
                    {
                        p.svc.buried.seen.filter(|(_, d)| *d < 10.0).map(|(_, d)| {
                            let near = 1.0 - (d / 10.0).clamp(0.0, 1.0);
                            (
                                27,
                                0.16 + near * 0.25,
                                25,
                                175.0,
                                0.45 - near as f64 * 0.36,
                                false,
                            )
                        })
                    }
                    _ => None,
                }
            } else {
                None
            };
            if let Some((owner, amp, ms, hz, interval, supported)) = continuous {
                state.repeating(now, owner, amp, ms, hz, interval, supported);
            } else {
                state.continuous = None;
                for cues in &mut state.cues {
                    cues.retain(|p| p.priority != 1);
                }
            }
        } else {
            state.stop_packs();
        }
    }
    state.previous = snapshot;
    let (Some(actions), Some(session)) = (actions, session) else {
        state.cues = default();
        state.sent = default();
        state.stop_packs();
        return;
    };
    for side in 0..2 {
        let controller_active = if side == 0 {
            input.left_grip.is_some()
        } else {
            input.grip.is_some() || input.trigger.is_some()
        };
        let focused = focus.0 && mode.free_movement();
        // Body-worn packs and damage reach connected controllers through optical occlusion.
        // Held weapon/gadget feedback still requires a valid tracked hand.
        state.cues[side].retain(|p| {
            p.end > now
                && focused
                && (motion.hands[side].is_some()
                    || ((p.priority == 20 || pack_owner(p.owner)) && controller_active))
        });
        let damage_pending = state.cues[side].iter().any(|p| p.priority == 20);
        let cue = state.cues[side]
            .iter()
            .filter(|p| p.start <= now && (!damage_pending || p.priority == 20))
            .max_by(|a, b| {
                a.priority
                    .cmp(&b.priority)
                    .then(a.amplitude.total_cmp(&b.amplitude))
            })
            .copied();
        if let Some(pulse) = cue {
            let key = (
                pulse.start.to_bits(),
                pulse.amplitude.to_bits(),
                pulse.frequency.to_bits(),
            );
            if state.sent[side].is_some_and(|s| s.key == key && s.until > now + 0.003) {
                continue;
            }
            let duration = ((pulse.end - now) * 1_000_000_000.0).max(1_000_000.0) as i64;
            let vibration = HapticVibration::new()
                .amplitude(pulse.amplitude.clamp(0.0, 1.0))
                .frequency(pulse.frequency)
                .duration(openxr::Duration::from_nanos(duration));
            match actions.vibration[side].apply_feedback(&session, Path::NULL, &vibration) {
                Ok(()) => {
                    state.sent[side] = Some(Sent {
                        key,
                        until: pulse.end,
                    })
                }
                Err(e) if !state.error_reported => {
                    warn!("VR_GAME haptics unavailable: {e}");
                    state.error_reported = true;
                }
                Err(_) => {}
            }
        } else if state.sent[side].take().is_some() {
            let _ = actions.vibration[side].stop_feedback(&session, Path::NULL);
        }
    }
}
