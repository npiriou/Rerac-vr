//! The original Blaster, mounted on the right controller and fired through the native weapon update.
use super::{
    Settings,
    embodied::{Motion, game_point, gameplay, unobstructed},
    game::Rig,
};
use bevy::prelude::*;
use rc_game::hero::{
    blaster::BLASTER,
    items::{HandItem, ItemData},
};

pub(super) struct BlasterPlugin;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct WeaponAim;
pub(super) const SELECTABLE: [i32; 22] = [
    8, 10, 15, 16, 11, 13, 9, 18, 19, 20, 17, 21, 23, 14, 24, 25, 12, 22, 26, 27, 31, 32,
];
/// The hand items held and fired in the right hand: the weapons and gadgets, and Kalebo III's board weapon (the
/// Devastator's model).
pub(super) fn supported(id: i32) -> bool {
    matches!(id, 9..=27 | 31 | 32) || id == rc_game::hero::hoverboard::WEAPON_ITEM
}
#[derive(Resource, Default)]
pub(super) struct SupportGrip {
    pub held: bool,
    weapon: i32,
    pub primary: Option<Vec3>,
    offset: Option<Vec3>,
    rotation: Option<Quat>,
    /// A two-handed weapon held in one hand: its lagging position and its offset from the controller (tracking space).
    position: Option<Vec3>,
    lag: Vec3,
    two_handed: bool,
    revision: u64,
}
impl SupportGrip {
    /// Forget the one-handed weapon's inertia.
    fn release(&mut self) {
        self.rotation = None;
        self.position = None;
        self.lag = Vec3::ZERO;
    }
}

/// A two-handed weapon held in one hand is heavy: its turn and position follow the controller with a lag (seconds),
/// within limits (radians, metres), and it droops (radians of pitch down) and sags (metres).
const TICK: f32 = 1.0 / 60.0;
const HEAVY_TURN: f32 = 0.22;
const HEAVY_FOLLOW: f32 = 0.12;
const HEAVY_MAX_TURN: f32 = 0.70;
const HEAVY_MAX_LAG: f32 = 0.10;
const HEAVY_DROOP: f32 = 0.10;
const HEAVY_SAG: f32 = 0.015;
/// Authored model coordinates in game units, measured on the original held meshes.
/// Local X is top, -Y is forward, Z is lateral for gun-shaped items.
/// Handheld panels have their own orientation below. Grip placement never follows animated output joints.
fn grip(class: i16) -> Option<Vec3> {
    Some(match class {
        157 => Vec3::new(0.0, -0.04, 0.0), // Devastator: rear trigger hold, side handle stays available.
        163 => Vec3::new(0.0, -0.18, 0.0),
        168 => Vec3::new(-0.065, -0.13, 0.0),
        175 => Vec3::new(-0.035, -0.22, 0.0), // Taunter handle, behind the speaker.
        176 => Vec3::new(-0.045, -0.14, 0.0),
        177 => Vec3::new(-0.115, -0.17, 0.0),
        180 => Vec3::new(0.0, -0.16, 0.0),
        185 => Vec3::new(-0.07, -0.22, -0.04), // Morph-o-Ray (177 is the Tesla Claw).
        188 => Vec3::new(-0.015, -0.145, 0.0), // Trespasser rear D-handle, not the prongs.
        208 => Vec3::new(0.0, -0.075, 0.0),    // Swingshot fist, not the offset hook launcher.
        454 => Vec3::new(-0.08, -0.15, 0.0),
        483 => Vec3::new(-0.06, -0.22, -0.05), // Hologuise / Drone Device remote handle.
        585 => Vec3::new(0.0, -0.14, -0.03), // Metal Detector upper handle; disc stays ahead/below.
        619 => Vec3::new(0.02, -0.17, -0.045), // PDA lower body, screen facing the player.
        849 => Vec3::new(0.0, -0.14, 0.0),
        1251 => Vec3::new(0.0, -0.15, 0.0), // Hydrodisplacer rear handle.
        _ => return None,
    })
}
fn foregrip(id: i32) -> Vec3 {
    // In controller coordinates, relative to the rear grip (right/up/back).
    match id {
        11 => Vec3::new(-0.27, 0.09, -0.36),
        13 => Vec3::new(-0.27, 0.05, -0.22),
        16 => Vec3::new(0.0, -0.10, -0.49),
        23 => Vec3::new(0.0, -0.06, -0.35),
        9 => Vec3::new(0.0, -0.01, -0.40),
        _ => Vec3::new(0.0, 0.0, -0.32),
    }
}
fn support_point(right: Transform, support: &SupportGrip) -> Vec3 {
    right.transform_point(support.offset.unwrap_or_else(|| foregrip(support.weapon)))
}
fn physical_weapon_hand(
    mut right: Transform,
    left: Option<Transform>,
    support: &SupportGrip,
) -> Transform {
    if support.held {
        if let Some(left) = left {
            let direction = left.translation - right.translation;
            if direction.length() > 0.12 {
                let current = right.rotation
                    * support
                        .offset
                        .unwrap_or_else(|| foregrip(support.weapon))
                        .normalize();
                right.rotation =
                    Quat::from_rotation_arc(current, direction.normalize()) * right.rotation;
            }
        }
    }
    right
}
pub(super) fn weapon_hand(
    right: Transform,
    left: Option<Transform>,
    support: &SupportGrip,
) -> Transform {
    let mut pose = physical_weapon_hand(right, left, support);
    if support.two_handed && !support.held {
        if let Some(rotation) = support.rotation {
            pose.rotation = rotation;
        }
        pose.translation += support.lag;
    }
    pose
}
pub(super) fn support_hand(
    right: Transform,
    left: Option<Transform>,
    support: &SupportGrip,
) -> Option<Transform> {
    let mut left = left?;
    if !support.held {
        return Some(left);
    }
    let right = weapon_hand(right, Some(left), support);
    left.translation = support_point(right, support);
    Some(left)
}
impl Plugin for BlasterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SupportGrip>().add_systems(
            FixedUpdate,
            (grant, aim)
                .chain()
                .in_set(WeaponAim)
                .after(super::embodied::WrenchUpdate)
                .before(crate::gameplay::GameTick),
        );
    }
}

fn grant(
    settings: Res<Settings>,
    saved: Option<ResMut<crate::gameplay::Persistent>>,
    session: Option<ResMut<crate::gameplay::Session>>,
    mut failed: Local<bool>,
    mut filled: Local<bool>,
) {
    if !settings.mode.debug() {
        return;
    }
    let (Some(mut saved), Some(mut session)) = (saved, session) else {
        return;
    };
    let gold = rc_game::moby_update::classes::item_offer::GOLD_ITEMS;
    let gold_ready = gold.iter().all(|&id| {
        saved.0.global.gold_weapons[id as usize]
            == (settings.mode == super::LaunchMode::DebugGold) as u8
    });
    let weapons: Vec<usize> = (1..=35).collect();
    if *failed || (*filled && gold_ready && weapons.iter().all(|&id| saved.0.global.owned[id] != 0))
    {
        return;
    }
    // The level's item tables, read once per level (crate::gameplay).
    match crate::gameplay::item_tables_for(crate::level_load::level_index()) {
        Some(tables) => {
            for id in gold {
                saved.0.global.gold_weapons[id as usize] =
                    (settings.mode == super::LaunchMode::DebugGold) as u8;
            }
            for id in weapons {
                if saved.0.global.owned[id] == 0 {
                    saved.0.give_item(id, false, tables, &mut session.0);
                }
            }
            if !*filled {
                // Match page zero of the VR wheel, preserving immediate access to the original loadout.
                saved
                    .0
                    .global
                    .quick_select
                    .copy_from_slice(&SELECTABLE[..8]);
                let mut reserves = Vec::new();
                for (id, record) in tables.records.iter().enumerate() {
                    if saved.0.global.owned[id] == 0 || !record.has_ammo() {
                        continue;
                    }
                    let cap = u16::from_le_bytes([record.0[0x0e], record.0[0x0f]]) as i32;
                    if cap > 0 {
                        saved.0.global.ammo[id] = cap;
                        reserves.push((id, cap));
                    }
                }
                *filled = true;
                println!(
                    "VR_GAME debug_inventory mode={} gold_items={:?} starting_max_ammo={reserves:?} quick_select={:?}",
                    settings.mode.name(),
                    if settings.mode == super::LaunchMode::DebugGold {
                        gold.to_vec()
                    } else {
                        Vec::new()
                    },
                    saved.0.global.quick_select
                );
            }
        }
        None => {
            *failed = true;
            error!("VR_GAME could not prepare starting weapons/ammo: no item tables for this level");
        }
    }
}

/// Mount weapons and gadgets by their grip, then derive their animated output independently.
/// Unknown classes retain the original fallback.
pub(super) fn place(
    item: &mut HandItem,
    hand: Transform,
    data: &ItemData,
    primary: Option<Vec3>,
) -> Option<Vec3> {
    let class = data.class(item.o_class)?;
    // The Swingshot housing belongs on the dorsal wrist. Rotate right-side housing up,
    // about controller forward, so the grip and forward aim remain fixed.
    let hand = if item.o_class == 208 {
        Transform {
            rotation: hand.rotation * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            ..hand
        }
    } else {
        hand
    };
    if let Some(grip) = primary.or_else(|| grip(item.o_class)) {
        let frame = crate::game_camera::game_rows(&hand);
        let axes = if matches!(item.o_class, 483 | 619) {
            // These upright remotes are wide along Y; their -Z face looks back toward the holder.
            Mat3::from_cols(frame[2], -frame[1], frame[0])
        } else {
            Mat3::from_cols(frame[2], -frame[0], -frame[1])
        };
        let at = crate::game_camera::game_eye(&hand);
        item.position = (at - axes * grip).to_array();
        item.rows = [0, 1, 2].map(|i| {
            let r = axes.col(i);
            [r.x.to_bits(), r.y.to_bits(), r.z.to_bits(), 0]
        });
        let muzzle = match item.o_class {
            // Visibomb list 0 is on the sight, not the barrel opening.
            163 => Vec3::new(0.04, -0.82, 0.0),
            // Trespasser has no output joint list; use its prong tip rather than the model origin.
            188 => Vec3::new(0.04, -1.0, -0.025),
            // Remotes have no projectile: their use point stays at the held body, including listless PDA.
            483 | 619 => grip,
            _ => {
                let chain = class.chains.first().filter(|c| !c.is_empty())?;
                let m = rc_formats::moby_anim::evaluate_chains(
                    &class.anim,
                    &item.anim,
                    item.snapshot.as_ref(),
                    &[chain.as_slice()],
                );
                Vec3::new(m[0][3][0], m[0][3][1], m[0][3][2]) * (item.scale / 1024.0)
            }
        };
        return Some(crate::tfrag_render::game_to_bevy(
            (Vec3::from(item.position) + axes * muzzle).to_array(),
        ));
    }
    let muzzle = if let Some(chain) = class.chains.first().filter(|c| !c.is_empty()) {
        *rc_formats::moby_anim::evaluate_chains(
            &class.anim,
            &item.anim,
            item.snapshot.as_ref(),
            &[chain.as_slice()],
        )
        .first()?
    } else {
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    };
    let basis = Mat3::from_cols_array_2d(&[
        muzzle[0][..3].try_into().ok()?,
        muzzle[1][..3].try_into().ok()?,
        muzzle[2][..3].try_into().ok()?,
    ]);
    let determinant = basis.determinant();
    if !determinant.is_finite() || determinant.abs() < 1e-5 {
        return None;
    }
    let rows = crate::game_camera::game_rows(&hand);
    // Gun top is authored along +X; preserve barrel direction and correct the previous 90-degree left roll.
    let axes = Mat3::from_cols(rows[2], -rows[0], -rows[1]) * basis.inverse();
    let reach = if item.o_class == rc_game::hero::pyrocitor::CLASS {
        0.65
    } else if matches!(item.o_class, 157 | 163 | 454 | 849) {
        0.70
    } else {
        0.40
    };
    let at = hand.translation + (*hand.forward() * reach + *hand.up() * 0.03) * hand.scale.x;
    let anchor = Vec3::new(muzzle[3][0], muzzle[3][1], muzzle[3][2]) * (item.scale / 1024.0);
    item.position =
        (crate::game_camera::game_eye(&Transform::from_translation(at)) - axes * anchor).to_array();
    item.rows = [0, 1, 2].map(|i| {
        let r = axes.col(i);
        [r.x.to_bits(), r.y.to_bits(), r.z.to_bits(), 0]
    });
    Some(at)
}

fn aim(
    play: Option<ResMut<crate::gameplay::Play>>,
    level: Res<crate::Level>,
    settings: Res<Settings>,
    rig: Res<Rig>,
    motion: Res<Motion>,
    mode: Res<crate::menu_render::MenuMode>,
    input: Option<Res<super::game_input::GamePad>>,
    hand_input: Option<Res<super::game_input::HandInput>>,
    mut support: ResMut<SupportGrip>,
    attach: Option<Res<crate::moby_attach::MobyAttach>>,
    mut contact: Local<super::solid_contact::History>,
) {
    let Some(mut play) = play else { return };
    let p = &mut *play;
    p.game.hero.items.tracked = !settings.desktop_preview;
    p.game.hero.items.tracked_aim = None;
    p.game.hero.weapons.blaster.tracked = !settings.desktop_preview;
    p.game.hero.weapons.blaster.tracked_aim = None;
    p.game.hero.weapons.pyro.tracked = !settings.desktop_preview;
    p.game.hero.weapons.pyro.tracked_aim = None;
    if settings.desktop_preview {
        return;
    }
    let id = p.game.hero.items.slot.id;
    if !supported(id) || rc_game::hero::items::is_glove(id) {
        support.held = false;
        support.release();
        support.two_handed = false;
        return;
    }
    p.game.hero.items.tracked_hand = None;
    if !gameplay(&mode)
        || input.as_ref().is_some_and(|i| i.quick_select.is_some())
        || p.game.hero.mode != 0
        || p.game.hero.health <= 0
        || p.death_pending
        || p.game.hero.hand_hidden()
    {
        support.held = false;
        return;
    }
    let Some((hand, root, coll)) = motion.hands[1]
        .zip(rig.root_for(&p.game, settings.units_per_metre))
        .zip(level.0.collision.as_ref())
        .map(|((h, r), c)| (h, r, c))
    else {
        support.held = false;
        return;
    };
    let authored = attach.as_ref().and_then(|a| a.tracked_grips(p));
    if let Some(item) = attach
        .as_ref()
        .and_then(|a| super::gadget_socket::docked(p, a))
    {
        support.held = false;
        support.release();
        p.game.hero.items.tracked_hand = Some((item.position, item.rows));
        p.game.hero.items.tracked_class = item.o_class;
        return;
    }
    // The native item definition selects one or two holding arm layers. Hydrodisplacer included.
    support.primary = authored.map(|g| g[0]);
    support.offset = authored.map(|g| {
        let d = g[1] - g[0];
        // Gun axes relative to controller: X up, Y back, Z right.
        Vec3::new(d.z, d.x, d.y) / settings.units_per_metre
    });
    let two_handed = p
        .game
        .item_data
        .as_ref()
        .is_some_and(|d| d.def(id).b18 == 2);
    let left = motion.hands[0];
    let grip = hand_input.as_ref().and_then(|i| i.left_grip).unwrap_or(0.0);
    if !two_handed || support.weapon != id || grip < 0.35 || left.is_none() {
        if support.weapon != id {
            support.release();
        }
        support.held = false;
    }
    support.weapon = id;
    if two_handed
        && grip > 0.6
        && left.is_some_and(|l| l.translation.distance(support_point(hand, &support)) < 0.25)
    {
        support.held = true;
    }
    support.two_handed = two_handed;
    // Only the native heavy / two-arm inventory items are heavy with a single grip: they lag in turn and position,
    // droop and sag; with both hands they follow exactly.
    // (Not the Hydrodisplacer: plugging it into a tap wants the hand's exact pose.)
    let one_handed = two_handed && !support.held && id != 22;
    let physical = physical_weapon_hand(hand, left, &support);
    let (desired, target) = if one_handed {
        (physical.rotation * Quat::from_rotation_x(-HEAVY_DROOP), physical.translation - Vec3::Y * HEAVY_SAG)
    } else {
        (physical.rotation, physical.translation)
    };
    let fresh = support.revision != rig.revision;
    support.revision = rig.revision;
    let (old, previous) = if fresh {
        (desired, target)
    } else {
        (support.rotation.unwrap_or(desired), support.position.unwrap_or(target))
    };
    let (turn, follow) = if one_handed {
        (1.0 - (-TICK / HEAVY_TURN).exp(), 1.0 - (-TICK / HEAVY_FOLLOW).exp())
    } else {
        (1.0, 1.0)
    };
    let mut weighted = old.slerp(desired, turn);
    let lag = weighted.angle_between(desired);
    if lag > HEAVY_MAX_TURN {
        weighted = weighted.slerp(desired, 1.0 - HEAVY_MAX_TURN / lag);
    }
    support.rotation = Some(weighted);
    let position = target + (previous.lerp(target, follow) - target).clamp_length_max(HEAVY_MAX_LAG);
    support.position = Some(position);
    support.lag = position - physical.translation;
    let mut hand = root.mul_transform(weapon_hand(hand, left, &support));
    let Some(mut item) = p.game.hero.items.slot.item.as_ref().cloned() else {
        return;
    };
    let Some(data) = p.game.item_data.as_ref() else {
        return;
    };
    let Some(mut muzzle) = place(&mut item, hand, data, support.primary) else {
        return;
    };
    let eye = rig.body_eye(&p.game);
    if let Some(attach) = attach.as_ref() {
        let mut probes = attach.tracked_item_shape(&item);
        probes.push((hand.translation, 0.11 * settings.units_per_metre));
        if support.held {
            if let Some(left) = support_hand(motion.hands[1].unwrap(), left, &support) {
                probes.push((
                    root.transform_point(left.translation),
                    0.11 * settings.units_per_metre,
                ));
            }
        }
        contact.prepare(rig.revision, id);
        let shift = super::solid_contact::constrain(p, coll, &probes, eye, &mut contact);
        item.position = game_point(crate::tfrag_render::game_to_bevy(item.position) + shift);
        hand.translation += shift;
        muzzle += shift;
    }
    // Utility tools are meant to touch their pad/lock: a nozzle against the floor keeps its aim (losing it
    // would suspend the item and reset it out of its ready state just as it is plugged).
    if !unobstructed(p, coll, eye, hand.translation)
        || (!matches!(id, rc_game::hero::walloper::WALLOPER | 22 | 26)
            && !unobstructed(p, coll, hand.translation, muzzle))
    {
        return;
    }
    p.game.hero.items.tracked_hand = Some((item.position, item.rows));
    p.game.hero.items.tracked_class = item.o_class;
    p.game.hero.items.tracked_aim = Some(rc_game::hero::items::TrackedAim {
        class: item.o_class,
        muzzle: game_point(muzzle),
        forward: game_point(*hand.forward()),
        up: game_point(*hand.up()),
    });
    let aim = Some((game_point(muzzle), game_point(*hand.forward())));
    if p.game.hero.items.slot.id == BLASTER {
        p.game.hero.weapons.blaster.tracked_aim = aim;
    } else if p.game.hero.items.slot.id == rc_game::hero::pyrocitor::PYROCITOR {
        p.game.hero.weapons.pyro.tracked_aim = aim;
    }
}
