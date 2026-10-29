//! Akimbo (the VR Options' Akimbo row): a weapon in each hand. The left stick's click opens the left hand's ring
//! (crate::menu_render::OffHandRing, on the left; the hand's moves to the right edge), the same pages as the hand's;
//! picking an item the left hand can hold ([`takes`]) puts it there, picking the one it holds empties it, the Drone
//! Device launches its drones, the others do nothing. The left trigger uses it (super::game_input; the other buttons
//! are the same with the option off). The game runs the left hand's item on its own copy of the hand slot
//! (rc_game::hero::off_hand, which also leads the hero's states for the Swingshot and the Walloper): here it is told
//! which item, the trigger, and where the left controller holds it, as super::blaster does for the hand. The gloves
//! are thrown as the hand's are (super::bomb_glove's left-hand throw), the Walloper punches with the left hand
//! (super::walloper). The models made for the right hand are mirrored in the left ([`place_left`]). The wrench stays in
//! the right hand.
use super::{
    Settings,
    embodied::{Motion, game_point, gameplay, unobstructed},
    game::Rig,
    game_input::GamePad,
};
use bevy::prelude::*;
use rc_game::hero::items::{HandItem, ItemData};

pub(super) struct AkimboPlugin;
/// The left hand's pick (kept across levels and deaths; empty at launch).
#[derive(Resource, Default)]
pub(super) struct Akimbo {
    /// The item the player picked for the left hand (0: empty).
    pub left: i32,
    /// The left ring's page (it opens on its first one, as the hand's).
    page: usize,
    /// The item the left hand holds this tick: the pick, when Akimbo is on, the item owned and Ratchet off the board
    /// (else 0).
    pub held: i32,
}
impl Plugin for AkimboPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Akimbo>()
            .init_resource::<crate::menu_render::OffHandRing>()
            .add_systems(
                FixedUpdate,
                ring.after(crate::gameplay::GameTick)
                    .before(crate::menu_render::MenuFrameSet),
            )
            .add_systems(
                FixedUpdate,
                aim.in_set(super::blaster::WeaponAim)
                    .after(super::embodied::WrenchUpdate)
                    .before(crate::gameplay::GameTick),
            );
    }
}

/// The items the left hand takes: its own item (rc_game::hero::off_hand::ITEMS) or a glove to throw.
pub(super) fn takes(id: i32) -> bool {
    rc_game::hero::off_hand::supported(id) || rc_game::hero::items::is_glove(id)
}

/// The left hand's grip on a gun: the hand's ([`super::blaster::grip`]) mirrored across the gun's lateral axis.
pub(super) fn left_grip(class: i16) -> Option<Vec3> {
    super::blaster::grip(class).map(|g| Vec3::new(g.x, g.y, -g.z))
}

/// The models made for the right hand: the gloves, the Walloper's fist and the Swingshot's wrist housing.
fn right_handed(class: i16) -> bool {
    use rc_game::moby_update::classes::{bomb, decoy, doom_canister, mine};
    [
        rc_game::hero::walloper::CLASS,
        rc_game::hero::swingshot::SWINGSHOT_CLASS,
        bomb::GLOVE_CLASS,
        mine::GLOVE_CLASS,
        decoy::GLOVE_CLASS,
        doom_canister::GLOVE_CLASS,
    ]
    .contains(&class)
}

/// The reflection across the left controller's lateral plane (`hand` in the world), in game coordinates.
fn reflect(v: Vec3, normal: Vec3) -> Vec3 {
    v - 2.0 * v.dot(normal) * normal
}

/// A model placed for the right hand at `hand` (the world), turned into its left-hand mirror image there.
pub(super) fn mirror(item: &mut HandItem, hand: Transform) {
    let normal = Vec3::from(game_point(hand.rotation * Vec3::X)).normalize_or_zero();
    let origin = Vec3::from(game_point(hand.translation));
    item.position = (origin + reflect(Vec3::from(item.position) - origin, normal)).to_array();
    item.rows = item.rows.map(|r| {
        let v = reflect(Vec3::new(f32::from_bits(r[0]), f32::from_bits(r[1]), f32::from_bits(r[2])), normal);
        [v.x.to_bits(), v.y.to_bits(), v.z.to_bits(), r[3]]
    });
}

/// The left hand's item at the left controller `hand` (the world), as super::blaster::place does for the hand: a gun
/// by its mirrored grip, a right-handed model by its own grip and then mirrored. Returns its muzzle (the world).
pub(super) fn place_left(item: &mut HandItem, hand: Transform, data: &ItemData) -> Option<Vec3> {
    if !right_handed(item.o_class) {
        return super::blaster::place(item, hand, data, left_grip(item.o_class));
    }
    let muzzle = super::blaster::place(item, hand, data, None)?;
    mirror(item, hand);
    let normal = (hand.rotation * Vec3::X).normalize_or_zero();
    Some(hand.translation + reflect(muzzle - hand.translation, normal))
}

/// The left hand's ring for the menu frame: open while the left stick's click is held, its pages the hand's ring's
/// (the owned items of super::blaster::SELECTABLE, 8 a page, the left trigger turning them); the item it picked (on its
/// release) becomes the left hand's, or empties it when it held that one; the Drone Device launches its drones.
fn ring(
    input: Option<Res<GamePad>>,
    hand: Option<ResMut<super::game_input::HandInput>>,
    options: Option<Res<crate::render_settings::VrOptions>>,
    saved: Option<Res<crate::gameplay::Persistent>>,
    session: Option<ResMut<crate::gameplay::Session>>,
    mut ring: ResMut<crate::menu_render::OffHandRing>,
    mut akimbo: ResMut<Akimbo>,
) {
    ring.akimbo = options.is_some_and(|o| o.akimbo);
    let select = input.as_ref().and_then(|i| i.left_select).filter(|_| ring.akimbo);
    if select.is_some() && !ring.held {
        akimbo.page = 0;
    }
    ring.held = select.is_some();
    ring.direction = select.unwrap_or_default();
    let owned: Vec<i32> = super::blaster::SELECTABLE
        .into_iter()
        .filter(|&id| saved.as_ref().is_some_and(|s| s.0.global.owned[id as usize] != 0))
        .collect();
    let pages = owned.len().div_ceil(8).max(1);
    if hand.is_some_and(|mut h| std::mem::take(&mut h.left_page)) && ring.held {
        akimbo.page = (akimbo.page + 1) % pages;
    }
    akimbo.page = akimbo.page.min(pages - 1);
    ring.items = std::array::from_fn(|slot| owned.get(akimbo.page * 8 + slot).copied().unwrap_or(0));
    // The items the left hand cannot take are greyed (the Drone Device launches its drones).
    ring.greyed = ring.items.map(|id| id != 0 && !takes(id) && id != rc_game::moby_update::classes::drone::DRONE_DEVICE);
    // As the hand's ring's pick (rc_game::menus::quick_select): the next tick launches them.
    if std::mem::take(&mut ring.drone) {
        if let Some(mut s) = session {
            s.0.drone = true;
            println!("VR_GAME akimbo drone_device launch");
        }
    }
    if let Some(id) = ring.picked.take() {
        if takes(id) {
            akimbo.left = if akimbo.left == id { 0 } else { id };
            println!("VR_GAME akimbo left_hand={}", akimbo.left);
        } else {
            println!("VR_GAME akimbo left_hand={} (item {id} stays in the right hand)", akimbo.left);
        }
    }
}

/// The left hand's item for this tick: the pick (when owned), the trigger, and its tracked placement at
/// the left controller — none while its ring is open, out of play, or with the hand or the muzzle behind a wall. A
/// glove is the engine's (super::bomb_glove): the game's left hand stays empty.
fn aim(
    play: Option<ResMut<crate::gameplay::Play>>,
    level: Res<crate::Level>,
    settings: Res<Settings>,
    rig: Res<Rig>,
    motion: Res<Motion>,
    mode: Res<crate::menu_render::MenuMode>,
    input: Option<Res<GamePad>>,
    (options, saved): (Option<Res<crate::render_settings::VrOptions>>, Option<Res<crate::gameplay::Persistent>>),
    mut akimbo: ResMut<Akimbo>,
) {
    let Some(mut play) = play else {
        akimbo.held = 0;
        return;
    };
    let p = &mut *play;
    let on = options.is_some_and(|o| o.akimbo) && !settings.desktop_preview;
    let owned = saved
        .as_ref()
        .is_some_and(|s| s.0.global.owned.get(akimbo.left.max(0) as usize).is_some_and(|&o| o != 0));
    // On the hoverboard the left trigger boosts: the left hand's weapon is put away.
    let board = p.game.hero.group == rc_game::hero::hoverboard::GROUP;
    akimbo.held = if on && owned && !board { akimbo.left } else { 0 };
    let off = &mut p.game.hero.off_hand;
    off.want = if rc_game::hero::off_hand::supported(akimbo.held) { akimbo.held } else { 0 };
    off.fire = input.as_ref().is_some_and(|i| i.left_fire);
    off.items.tracked = !settings.desktop_preview;
    off.items.tracked_hand = None;
    off.items.tracked_aim = None;
    off.blaster.tracked = off.items.tracked;
    off.blaster.tracked_aim = None;
    off.pyro.tracked = off.items.tracked;
    off.pyro.tracked_aim = None;
    if off.want == 0
        || !gameplay(&mode)
        || input.as_ref().is_some_and(|i| i.left_select.is_some())
        || p.game.hero.mode != 0
        || p.game.hero.health <= 0
        || p.death_pending
        || p.game.hero.tracked.grabbed[0]
    {
        return;
    }
    let Some(mut item) = p.game.hero.off_hand.item().cloned() else { return };
    let Some(((hand, root), coll)) = motion.hands[0]
        .zip(rig.root_for(&p.game, settings.units_per_metre))
        .zip(level.0.collision.as_ref())
    else {
        return;
    };
    let hand = root.mul_transform(hand);
    let Some(muzzle) = p.game.item_data.as_ref().and_then(|data| place_left(&mut item, hand, data)) else {
        return;
    };
    let eye = rig.body_eye(&p.game);
    // The Walloper's fist is meant to touch what it hits, as the hand's (super::blaster).
    if !unobstructed(p, coll, eye, hand.translation)
        || (item.o_class != rc_game::hero::walloper::CLASS && !unobstructed(p, coll, hand.translation, muzzle))
    {
        return;
    }
    let off = &mut p.game.hero.off_hand;
    off.items.tracked_hand = Some((item.position, item.rows));
    off.items.tracked_class = item.o_class;
    off.items.tracked_aim = Some(rc_game::hero::items::TrackedAim {
        class: item.o_class,
        muzzle: game_point(muzzle),
        forward: game_point(*hand.forward()),
        up: game_point(*hand.up()),
    });
    let aim = Some((game_point(muzzle), game_point(*hand.forward())));
    match off.items.slot.id {
        rc_game::hero::blaster::BLASTER => off.blaster.tracked_aim = aim,
        rc_game::hero::pyrocitor::PYROCITOR => off.pyro.tracked_aim = aim,
        _ => {}
    }
}
