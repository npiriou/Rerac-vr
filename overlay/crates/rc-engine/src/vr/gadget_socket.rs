//! A physical push of the held tool enters the original water/lock state machines.
use super::{
    Settings,
    blaster::{self, SupportGrip, WeaponAim},
    embodied::{Motion, game_point, gameplay, unobstructed},
    game::Rig,
    game_input::{Focused, HandInput},
};
use bevy::prelude::*;
use rc_game::collision_query::{
    QueryFlags, coll_line_m,
    mobys::{CollMoby, MobyScene, MobySource},
};

pub(super) struct GadgetSocketPlugin;
pub(super) fn docked(
    p: &crate::gameplay::Play,
    attach: &crate::moby_attach::MobyAttach,
) -> Option<rc_game::hero::items::HandItem> {
    let id = p.game.hero.items.slot.id;
    if !matches!(id, 22 | 26) {
        return None;
    }
    let target = p.game.hero.tracked.utility_target?;
    let active = if id == 26 {
        p.game.hero.state == 0x72
    } else {
        matches!(p.game.hero.state, 0x38..=0x3a)
            || p.game
                .hero
                .items
                .slot
                .item
                .as_ref()
                .is_some_and(|i| matches!(i.mstate, 3..=5))
    };
    // Sucking, native Ratchet stays at the pad centre with the gadget at his side, away from the retracted tap
    // the player plugs into: the tool stays in the tracked hand where it was plugged.
    if !active || (id == 22 && sucking(p)) {
        return None;
    }
    attach.socket_item(p, p.game.hero.items.slot.item.as_ref()?, target, !active)
}
/// The Hydrodisplacer use is a suck (Ratchet's 0x46..0x48), or about to be one; `hydro_full` flips mid-use.
fn sucking(p: &crate::gameplay::Play) -> bool {
    let s = &p.ratchet.state;
    let suck = |q: u8| matches!(q, 0x46..=0x48);
    let pour = |q: u8| matches!(q, 0x83..=0x85);
    suck(s.seq_a)
        || suck(s.seq_b)
        || (!pour(s.seq_a) && !pour(s.seq_b) && !p.game.hero.gadgets.hydro_full)
}
/// A pad's tap retracted into the ground (the suck position): the top of its lowered column (Bevy coordinates).
/// Raised, the tap is the native pour socket instead.
fn retracted_tap(
    p: &crate::gameplay::Play,
    level: &crate::level_load::LoadedLevel,
    target: usize,
) -> Option<Vec3> {
    let m = p.game.mobys.mobys.get(target)?;
    let offsets = super::embodied::posed_offsets(m, level);
    let top = offsets.iter().map(|v| v.z).fold(f32::NEG_INFINITY, f32::max);
    // Raised it stands about 1.3 units high; retracted, about 0.27.
    if !top.is_finite() || top > 0.6 {
        return None;
    }
    let cap: Vec<_> = offsets.iter().filter(|v| v.z >= top - 0.03).collect();
    let centre = cap.iter().map(|v| v.truncate()).sum::<Vec2>() / cap.len() as f32;
    Some(crate::tfrag_render::game_to_bevy([
        m.position[0] + centre.x,
        m.position[1] + centre.y,
        m.position[2] + top,
    ]))
}
pub(super) fn tool_tip(
    item: &rc_game::hero::items::HandItem,
    data: &rc_game::hero::items::ItemData,
) -> Option<Vec3> {
    let class = data.class(item.o_class)?;
    let local = if item.o_class == 188 {
        Vec3::new(0.04, -1.0, -0.025)
    } else {
        let chain = class.chains.first()?;
        let p = rc_formats::moby_anim::evaluate_chains(
            &class.anim,
            &item.anim,
            item.snapshot.as_ref(),
            &[chain],
        )
        .into_iter()
        .next()?[3];
        Vec3::new(p[0], p[1], p[2]) * (item.scale / 1024.0)
    };
    let axes = Mat3::from_cols_array_2d(&item.rows.map(|r| {
        [
            f32::from_bits(r[0]),
            f32::from_bits(r[1]),
            f32::from_bits(r[2]),
        ]
    }));
    Some(crate::tfrag_render::game_to_bevy(
        (Vec3::from_array(item.position) + axes * local).to_array(),
    ))
}
impl Plugin for GadgetSocketPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Insertion>().add_systems(
            FixedUpdate,
            engage.after(WeaponAim).before(crate::gameplay::GameTick),
        );
    }
}
#[derive(Clone, Copy)]
struct Contact {
    target: usize,
    point: Vec3,
    normal: Vec3,
    travel: f32,
    used: bool,
    acknowledged: bool,
    requested_at: f64,
}
#[derive(Resource, Default)]
struct Insertion {
    revision: u64,
    level: u32,
    weapon: i32,
    previous: Option<(f64, Vec3)>,
    contact: Option<Contact>,
}
struct SocketTargets<'a> {
    source: &'a dyn MobySource,
    class: i16,
}
impl MobySource for SocketTargets<'_> {
    fn moby(&self, id: usize) -> Option<CollMoby> {
        let mut m = self.source.moby(id)?;
        if m.o_class != self.class || !m.collision {
            return None;
        }
        // SKIP_WORLD line queries require this bit. Only the query copy is changed.
        m.mode |= 0x4000;
        Some(m)
    }
    fn joints(&self, id: usize, count: usize) -> Vec<[u32; 4]> {
        self.source.joints(id, count)
    }
}
fn engage(
    play: Option<ResMut<crate::gameplay::Play>>,
    level: Res<crate::Level>,
    settings: Res<Settings>,
    rig: Res<Rig>,
    motion: Res<Motion>,
    support: Res<SupportGrip>,
    focus: Res<Focused>,
    mut input: ResMut<HandInput>,
    mode: Res<crate::menu_render::MenuMode>,
    time: Res<Time<Real>>,
    mut insertion: ResMut<Insertion>,
    attach: Option<Res<crate::moby_attach::MobyAttach>>,
) {
    let Some(mut play) = play else {
        *insertion = default();
        return;
    };
    let p = &mut *play;
    let now = time.elapsed_secs_f64();
    let id = p.game.hero.items.slot.id;
    let level_id = crate::level_load::level_index();
    // Turning/recentring changes controller coordinates, not the accepted native socket.
    let stale = insertion.level != level_id || insertion.weapon != id;
    if stale {
        *insertion = Insertion {
            revision: rig.revision,
            level: level_id,
            weapon: id,
            ..default()
        };
    }
    if insertion.revision != rig.revision {
        insertion.revision = rig.revision;
        insertion.previous = None;
        if insertion.contact.is_some_and(|c| !c.used) {
            insertion.contact = None;
        }
    }
    let prior_target = p.game.hero.tracked.utility_target;
    p.game.hero.tracked.utility_use = false;
    p.game.hero.tracked.utility_target = None;
    if settings.desktop_preview
        || !matches!(id, 22 | 26)
        || p.game.hero.health <= 0
        || p.death_pending
    {
        insertion.contact = None;
        insertion.previous = None;
        return;
    }
    // Preserve the accepted binding throughout native cutaways and the lock puzzle.
    // They may temporarily capture input or hide the held item.
    let active = if id == 22 {
        matches!(p.game.hero.state, 0x38..=0x3a)
            || p.game
                .hero
                .items
                .slot
                .item
                .as_ref()
                .is_some_and(|it| matches!(it.mstate, 3..=5))
    } else {
        p.game.hero.state == 0x72
    };
    // Circle remains optional, but its native platform activation must dock to the same socket.
    if active && insertion.contact.is_none() {
        let target = prior_target.or(p.game.hero.ground_moby).filter(|&target| {
            p.game
                .mobys
                .mobys
                .get(target)
                .is_some_and(|m| m.o_class == if id == 22 { 341 } else { 615 })
        });
        if let Some(target) = target {
            let socket = attach.as_ref().and_then(|a| {
                a.socket_item(p, p.game.hero.items.slot.item.as_ref()?, target, true)
            });
            let point = socket
                .as_ref()
                .zip(p.game.item_data.as_ref())
                .and_then(|(item, data)| tool_tip(item, data));
            let normal = socket.as_ref().map(|item| {
                crate::tfrag_render::game_to_bevy([
                    f32::from_bits(item.rows[1][0]),
                    f32::from_bits(item.rows[1][1]),
                    f32::from_bits(item.rows[1][2]),
                ])
                .normalize_or_zero()
            });
            insertion.contact = Some(Contact {
                target,
                point: point.unwrap_or(Vec3::ZERO),
                normal: normal.unwrap_or(Vec3::Y),
                travel: 0.0,
                used: true,
                acknowledged: true,
                requested_at: now,
            });
        }
    }
    // The moby loop reads the previous hero item update. Retain a pending request
    // across that boundary until the native action acknowledges it (or times out).
    let cancel = std::mem::take(&mut input.utility_cancel);
    if let Some(contact) = insertion.contact.as_mut() {
        contact.acknowledged |= active || cancel;
    }
    if cancel {
        // Keep the target through this tick so the native cancel/withdraw calls survive
        // the generic cutaway filter. Disarm until the controller leaves the socket.
        p.game.hero.tracked.utility_target = prior_target;
        p.game.hero.gadgets.item_bc = 0;
        insertion.previous = None;
        return;
    }
    if let Some(contact) = insertion
        .contact
        .filter(|c| c.used && (active || (!c.acknowledged && now - c.requested_at < 1.5)))
    {
        if p.game
            .mobys
            .mobys
            .get(contact.target)
            .is_some_and(|m| m.o_class == if id == 22 { 341 } else { 615 })
        {
            p.game.hero.tracked.utility_target = Some(contact.target);
            p.game.hero.tracked.utility_use = !active;
            if let Some(item) = attach.as_ref().and_then(|a| docked(p, a)) {
                p.game.hero.items.tracked_hand = Some((item.position, item.rows));
                p.game.hero.items.tracked_class = item.o_class;
            }
            insertion.previous = None;
            return;
        }
    }
    if !focus.0 || !gameplay(&mode) || !p.game.hero.tracked.enabled || p.game.hero.hand_hidden() {
        insertion.previous = None;
        // A completed insertion stays disarmed until the tool is actually withdrawn.
        if insertion.contact.is_some_and(|c| !c.used) {
            insertion.contact = None;
        }
        return;
    }
    let Some((right, root, coll)) = motion.hands[1]
        .zip(rig.root_for(&p.game, settings.units_per_metre))
        .zip(level.0.collision.as_ref())
        .map(|((h, r), c)| (h, r, c))
    else {
        insertion.previous = None;
        return;
    };
    let hand = root.mul_transform(blaster::weapon_hand(right, motion.hands[0], &support));
    let Some(mut item) = p.game.hero.items.slot.item.clone() else {
        return;
    };
    let Some(data) = p.game.item_data.as_ref() else {
        return;
    };
    let Some(tip) = blaster::place(&mut item, hand, data, support.primary) else {
        return;
    };
    // Count controller translation only: animated prongs, locomotion and a wrist
    // rotation must not manufacture the physical push that starts the gadget.
    let local_hand = right.translation;
    let delta = insertion
        .previous
        .filter(|(t, old)| (0.001..0.15).contains(&(now - *t)) && old.distance(local_hand) < 0.3)
        .map(|(_, old)| root.rotation * (local_hand - old) * settings.units_per_metre);
    insertion.previous = Some((now, local_hand));
    let forward = *hand.forward();
    let units = settings.units_per_metre;
    let eye = crate::tfrag_render::game_to_bevy(p.game.hero.position()) + Vec3::Y * 1.6;
    if !unobstructed(p, coll, eye, hand.translation) {
        insertion.contact = None;
        return;
    }
    if let Some(mut contact) = insertion.contact {
        let offset = tip - contact.point;
        let depth = -offset.dot(contact.normal);
        let lateral = (offset + contact.normal * depth).length();
        let valid = p.game.mobys.mobys.get(contact.target).is_some_and(|m| {
            m.state < 0x80
                && (id != 26 || m.state == 1)
                && crate::tfrag_render::game_to_bevy([m.position[0], m.position[1], m.position[2]])
                    .distance(eye)
                    < 3.0 * units
        });
        if !valid || depth < -0.28 * units || lateral > 0.28 * units || depth > 0.40 * units {
            insertion.contact = None;
        } else if !contact.used && -forward.dot(contact.normal) > 0.45 {
            if let Some(delta) = delta {
                contact.travel = (contact.travel - delta.dot(contact.normal)).max(0.0);
            }
            if (contact.travel >= 0.005 * units || input.trigger.is_some_and(|v| v > 0.6))
                && depth >= -0.12 * units
                && p.game.hero.air_ticks == 0
                && p.game.hero.items.slot.state == 2
                && (id != 22
                    || (p.game.hero.group <= 1
                        && p.game
                            .hero
                            .items
                            .slot
                            .item
                            .as_ref()
                            .is_some_and(|it| it.mstate == 2)))
            {
                // The shaft may now overlap the target. Reject a nearer world/other-moby obstruction.
                let src = p.svc.scene_parts(&p.game.mobys, &*p.classes);
                let scene = p.svc.scene(&src);
                let blocked = coll_line_m(
                    coll,
                    Some(&scene),
                    game_point(hand.translation),
                    game_point(tip),
                    QueryFlags(2 | 0x10),
                    Some(p.game.hero_moby),
                )
                .is_some_and(|h| {
                    // A retracted tap sits in the floor: the nozzle reaches it through the ground around it.
                    let reach = if contact.normal == Vec3::Y { 0.45 } else { 0.05 };
                    h.moby != Some(contact.target)
                        && crate::tfrag_render::game_to_bevy(h.point).distance(contact.point)
                            > reach * units
                });
                if !blocked {
                    p.game.hero.tracked.utility_target = Some(contact.target);
                    p.game.hero.tracked.utility_use = true;
                    contact.used = true;
                    contact.acknowledged = false;
                    contact.requested_at = now;
                    println!("VR_GAME tool_insert item={id} target={}", contact.target);
                }
            }
            insertion.contact = Some(contact);
        }
    }
    if insertion.contact.is_none() {
        if let Some(attach) = attach.as_ref() {
            let found = p
                .game
                .mobys
                .mobys
                .iter()
                .enumerate()
                .filter(|(_, m)| m.o_class == if id == 22 { 341 } else { 615 })
                .filter_map(|(target, m)| {
                    if m.state >= 0x80 || (id == 26 && m.state != 1) {
                        return None;
                    }
                    // Sucking from a retracted tap: push the nozzle down onto it, where it is in the ground.
                    if let Some(tap) = (id == 22 && !p.game.hero.gadgets.hydro_full)
                        .then(|| retracted_tap(p, &level.0, target))
                        .flatten()
                    {
                        let beside = Vec2::new(tip.x - tap.x, tip.z - tap.z).length();
                        if beside < 0.30 * units
                            && (-0.40 * units..0.35 * units).contains(&(tip.y - tap.y))
                            && forward.dot(Vec3::NEG_Y) > 0.35
                            && eye.distance(tap) < 3.0 * units
                        {
                            return Some((target, tap, Vec3::Y, tip.distance(tap)));
                        }
                    }
                    let native = attach.socket_item(p, &item, target, true)?;
                    let point = tool_tip(&native, data)?;
                    let forward_native = -Vec3::new(
                        f32::from_bits(native.rows[1][0]),
                        f32::from_bits(native.rows[1][1]),
                        f32::from_bits(native.rows[1][2]),
                    );
                    let approach = crate::tfrag_render::game_to_bevy(forward_native.to_array())
                        .normalize_or_zero();
                    let distance = tip.distance(point);
                    (distance < 0.30 * units
                        && forward.dot(approach) > 0.35
                        && eye.distance(point) < 3.0 * units)
                        .then_some((target, point, -approach, distance))
                })
                .min_by(|a, b| a.3.total_cmp(&b.3));
            if let Some((target, point, normal, _)) = found {
                insertion.contact = Some(Contact {
                    target,
                    point,
                    normal,
                    travel: 0.0,
                    used: false,
                    acknowledged: false,
                    requested_at: 0.0,
                });
            }
        }
    }
    if insertion.contact.is_none() && attach.is_none() {
        let src = p.svc.scene_parts(&p.game.mobys, &*p.classes);
        let scene = p.svc.scene(&src);
        let targets = SocketTargets {
            source: scene.mobys,
            class: if id == 22 { 341 } else { 615 },
        };
        let sockets = MobyScene {
            mobys: &targets,
            grid: scene.grid,
            classes: scene.classes,
            cache: scene.cache,
        };
        // Start on the approach side, so an already buried tool never activates by proximity alone.
        if let Some(hit) = coll_line_m(
            coll,
            Some(&sockets),
            game_point(tip - forward * 0.06 * units),
            game_point(tip + forward * 0.18 * units),
            // Match the body-collision submask used to stand on these native pads.
            QueryFlags(1 | 4 | 0x10),
            Some(p.game.hero_moby),
        ) {
            if let Some(target) = hit.moby {
                let point = crate::tfrag_render::game_to_bevy(hit.point);
                let mut normal = crate::tfrag_render::game_to_bevy(hit.normal).normalize_or_zero();
                if normal.dot(forward) > 0.0 {
                    normal = -normal;
                }
                if (tip - point).dot(normal) >= 0.0 && -forward.dot(normal) > 0.45 {
                    insertion.contact = Some(Contact {
                        target,
                        point,
                        normal,
                        travel: 0.0,
                        used: false,
                        acknowledged: false,
                        requested_at: 0.0,
                    });
                }
            }
        }
    }
}
