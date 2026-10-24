//! Solid tracked models; physical controller samples remain untouched for gesture detection.
use super::{
    Settings,
    blaster::SupportGrip,
    embodied::{Motion, game_point, gameplay},
    game::Rig,
};
use bevy::{prelude::*, transform::TransformSystems};
use rc_game::collision_query::{
    self as query, CollOutput, QueryFlags,
    mobys::{CollMoby, MobyScene, MobySource},
};

pub(super) struct SolidContactPlugin;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Presented;
#[derive(Default)]
pub(super) struct History {
    key: Option<(u64, u32, i32)>,
    points: Vec<(Vec3, f32)>,
}
impl History {
    pub fn reset(&mut self) {
        *self = default();
    }
    pub fn prepare(&mut self, revision: u64, item: i32) {
        let key = (revision, crate::level_load::level_index(), item);
        if self.key != Some(key) {
            self.reset();
            self.key = Some(key);
        }
    }
}
#[derive(Resource, Default)]
pub(super) struct Contacts {
    pub right_shift: Vec3,
    history: [History; 2],
}
impl Plugin for SolidContactPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Contacts>().add_systems(
            PostUpdate,
            present
                .in_set(Presented)
                .after(super::embodied::HandPresentation)
                .before(crate::moby_attach::upload)
                .before(TransformSystems::Propagate),
        );
    }
}
struct Solids<'a>(&'a dyn MobySource);
impl MobySource for Solids<'_> {
    fn moby(&self, id: usize) -> Option<CollMoby> {
        let mut m = self.0.moby(id)?;
        // Preserve native destroyed/disabled collision flags. Enemies remain damage targets,
        // while crate primitive-only colliders must participate in the flagged-only pass.
        if !m.collision || (m.mode & 0x4000 != 0 && !super::embodied::crate_class(m.o_class)) {
            return None;
        }
        m.mode |= 0x4000;
        Some(m)
    }
    fn joints(&self, id: usize, count: usize) -> Vec<[u32; 4]> {
        self.0.joints(id, count)
    }
}
fn bevy_point(p: [f32; 3]) -> Vec3 {
    crate::tfrag_render::game_to_bevy(p)
}

/// Translation-only contact solve: both grips stay on the original weapon and aim stays physical.
/// Eye recovery prevents a controller already behind a wall from dragging its mesh through it.
pub(super) fn constrain(
    p: &crate::gameplay::Play,
    coll: &rc_formats::collision::Collision,
    probes: &[(Vec3, f32)],
    eye: Vec3,
    history: &mut History,
) -> Vec3 {
    if probes.is_empty() {
        history.reset();
        return Vec3::ZERO;
    }
    // Deployment can change the number of mesh slices. Never sweep unrelated slices together.
    if history.points.len() != probes.len() {
        history.points.clear();
    }
    let parts = p.svc.scene_parts(&p.game.mobys, &*p.classes);
    let native = p.svc.scene(&parts);
    let solids = Solids(native.mobys);
    let scene = MobyScene {
        mobys: &solids,
        grid: native.grid,
        classes: native.classes,
        cache: native.cache,
    };
    let flags = QueryFlags(0x20 | 0x10);
    let moby_flags = QueryFlags(flags.0 | 1);
    let line = |a: Vec3, b: Vec3| -> Option<CollOutput> {
        if a.distance_squared(b) < 1e-8 {
            return None;
        }
        [
            query::coll_line(coll, game_point(a), game_point(b), flags),
            query::coll_line_m(
                coll,
                Some(&scene),
                game_point(a),
                game_point(b),
                moby_flags,
                Some(p.game.hero_moby),
            ),
        ]
        .into_iter()
        .flatten()
        .min_by(|a, b| a.best.total_cmp(&b.best))
    };
    let mut delta = Vec3::ZERO;
    for iteration in 0..5 {
        let mut changed = false;
        for (i, &(desired, radius)) in probes.iter().enumerate() {
            let at = desired + delta;
            let old = history
                .points
                .get(i)
                .filter(|(v, _)| v.distance(desired) < 2.5)
                .map(|v| v.0);
            let mut correction = None;
            // On later iterations only local overlap is needed; the recovery/sweep already ran.
            if iteration == 0 {
                if let Some(hit) = old.is_none().then(|| line(eye, at)).flatten() {
                    let point = bevy_point(hit.point);
                    let mut normal = bevy_point(hit.normal).normalize_or_zero();
                    if normal.dot(eye - point) < 0.0 {
                        normal = -normal;
                    }
                    correction = Some(normal * (radius - normal.dot(at - point)).max(0.0));
                }
                if correction.is_none() {
                    if let Some(old) = old.filter(|v| v.distance_squared(at) > 0.000025) {
                        let dir = (at - old).normalize_or_zero();
                        let side = dir.any_orthonormal_vector() * radius * 0.85;
                        let up = dir.cross(side);
                        let hit = [Vec3::ZERO, side, -side, up, -up]
                            .into_iter()
                            .filter_map(|off| line(old + off, at + off))
                            .min_by(|a, b| a.best.total_cmp(&b.best));
                        if let Some(hit) = hit {
                            let point = bevy_point(hit.point);
                            let mut normal = bevy_point(hit.normal).normalize_or_zero();
                            if normal.dot(old - point) < 0.0 {
                                normal = -normal;
                            }
                            correction = Some(normal * (radius - normal.dot(at - point)).max(0.0));
                        }
                    }
                }
            }
            if correction.is_none() {
                let hit = [
                    query::coll_sphere(coll, game_point(at), radius, flags),
                    query::coll_sphere_m(
                        coll,
                        Some(&scene),
                        game_point(at),
                        radius,
                        moby_flags,
                        Some(p.game.hero_moby),
                    ),
                ]
                .into_iter()
                .flatten()
                .min_by(|a, b| a.best.total_cmp(&b.best));
                if let Some(hit) = hit {
                    // Native pushed_centre also handles the interior of primitive crates correctly.
                    let d = hit
                        .pushed_centre
                        .map(bevy_point)
                        .map(|v| v - at)
                        .unwrap_or_else(|| {
                            let point = bevy_point(hit.point);
                            let n = (at - point)
                                .try_normalize()
                                .unwrap_or_else(|| bevy_point(hit.normal).normalize_or_zero());
                            n * (radius - at.distance(point)).max(0.0)
                        });
                    correction = Some(d);
                }
            }
            if let Some(d) = correction.filter(|d| d.is_finite() && d.length_squared() > 0.000001) {
                delta += d;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    history.points = probes.iter().map(|&(v, r)| (v + delta, r)).collect();
    delta
}

#[allow(clippy::too_many_arguments)]
fn present(
    play: Option<Res<crate::gameplay::Play>>,
    level: Res<crate::Level>,
    settings: Res<Settings>,
    rig: Res<Rig>,
    motion: Res<Motion>,
    mode: Res<crate::menu_render::MenuMode>,
    support: Res<SupportGrip>,
    held: Res<super::bomb_glove::HeldBomb>,
    mut attach: Option<ResMut<crate::moby_attach::MobyAttach>>,
    mut contacts: ResMut<Contacts>,
) {
    contacts.right_shift = Vec3::ZERO;
    let Some((p, coll, a)) = play
        .as_ref()
        .zip(level.0.collision.as_ref())
        .zip(attach.as_mut())
        .map(|((p, c), a)| (p, c, a))
    else {
        return;
    };
    if !gameplay(&mode) || settings.desktop_preview || p.game.hero.health <= 0 || p.death_pending {
        contacts.history.iter_mut().for_each(History::reset);
        return;
    }
    let Some(root) = rig.root_for(&p.game, settings.units_per_metre) else {
        return;
    };
    let eye = rig.body_eye(&p.game);
    let shapes = a.tracked_contact_shapes();
    let flying = p
        .game
        .hero
        .items
        .slot
        .item
        .as_ref()
        .is_some_and(|i| i.o_class == rc_game::hero::melee::WRENCH_CLASS && i.mstate != 0);
    let weapon = p.game.hero.items.slot.id;
    let plugged = super::gadget_socket::docked(p, a).is_some();
    // A weapon, its native grip hand and the accepted support hand are one rigid group.
    for side in [1, 0] {
        if motion.hands[side].is_none()
            || p.game.hero.tracked.grabbed[side]
            || (side == 1 && p.game.hero.tracked.crank_target.is_some())
            || (side == 0 && support.held)
        {
            contacts.history[side].reset();
            continue;
        }
        let mut classes = vec![-30000 - side as i16];
        let mut probes = Vec::new();
        for (class, points) in &shapes {
            if *class == classes[0]
                || (side == 1
                    && *class >= 0
                    && !plugged
                    && !(*class == rc_game::hero::melee::WRENCH_CLASS && flying))
                || (side == 1
                    && support.held
                    && *class == -30000
                    && !p.game.hero.tracked.grabbed[0])
            {
                classes.push(*class);
                probes.extend_from_slice(points);
            }
        }
        if side == 1 && held.id.is_some() {
            if let Some(hand) = motion.hands[1] {
                let point = super::bomb_glove::palm_point(hand, root, settings.units_per_metre);
                probes.push((point.translation, 0.15 * settings.units_per_metre));
            }
        }
        contacts.history[side].prepare(rig.revision, weapon);
        let delta = constrain(p, coll, &probes, eye, &mut contacts.history[side]);
        a.tracked_shift(&classes, delta);
        if side == 1 {
            contacts.right_shift = delta;
        }
    }
}
