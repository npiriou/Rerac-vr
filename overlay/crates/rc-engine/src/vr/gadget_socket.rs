//! A physical push of the held tool enters the original water/lock state machines.
use super::{
    Settings,
    blaster::{self, SupportGrip, WeaponAim},
    embodied::{Motion, game_point, gameplay, unobstructed},
    game::Rig,
    game_input::Focused,
};
use bevy::prelude::*;
use rc_game::collision_query::{
    QueryFlags, coll_line_m,
    mobys::{CollMoby, MobyScene, MobySource},
};

pub(super) struct GadgetSocketPlugin;
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
    mode: Res<crate::menu_render::MenuMode>,
    time: Res<Time<Real>>,
    mut insertion: ResMut<Insertion>,
) {
    let Some(mut play) = play else {
        *insertion = default();
        return;
    };
    let p = &mut *play;
    let now = time.elapsed_secs_f64();
    let id = p.game.hero.items.slot.id;
    let level_id = crate::level_load::level_index();
    let stale =
        insertion.revision != rig.revision || insertion.level != level_id || insertion.weapon != id;
    if stale {
        *insertion = Insertion {
            revision: rig.revision,
            level: level_id,
            weapon: id,
            ..default()
        };
    }
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
        p.game
            .hero
            .items
            .slot
            .item
            .as_ref()
            .is_some_and(|it| matches!(it.mstate, 3..=5))
    } else {
        p.game.hero.state == 0x72
    };
    // The moby loop reads the previous hero item update. Retain a pending request
    // across that boundary until the native action acknowledges it (or times out).
    if let Some(contact) = insertion
        .contact
        .filter(|c| c.used && (active || now - c.requested_at < 0.25))
    {
        if p.game
            .mobys
            .mobys
            .get(contact.target)
            .is_some_and(|m| m.o_class == if id == 22 { 341 } else { 615 })
        {
            p.game.hero.tracked.utility_target = Some(contact.target);
            p.game.hero.tracked.utility_use = !active;
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
        if !valid || depth < -0.16 * units || lateral > 0.18 * units || depth > 0.35 * units {
            insertion.contact = None;
        } else if !contact.used && -forward.dot(contact.normal) > 0.45 {
            if let Some(delta) = delta {
                contact.travel = (contact.travel - delta.dot(contact.normal)).max(0.0);
            }
            if contact.travel >= 0.035 * units
                && depth >= 0.012 * units
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
                && delta.is_some_and(|d| -d.dot(contact.normal) > 0.0001 * units)
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
                    h.moby != Some(contact.target)
                        && crate::tfrag_render::game_to_bevy(h.point).distance(contact.point)
                            > 0.05 * units
                });
                if !blocked {
                    p.game.hero.tracked.utility_target = Some(contact.target);
                    p.game.hero.tracked.utility_use = true;
                    contact.used = true;
                    contact.requested_at = now;
                    println!("VR_GAME tool_insert item={id} target={}", contact.target);
                }
            }
            insertion.contact = Some(contact);
        }
    }
    if insertion.contact.is_none() {
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
                        requested_at: 0.0,
                    });
                }
            }
        }
    }
}
