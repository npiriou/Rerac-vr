//! **VR akimbo: a second hand item, in the player's left hand.** The game has one hand slot (0x1403e0); the off hand
//! keeps a second copy of what the hand-item code reads and writes for the item in hand — the slot and its item moby,
//! the weapons' pvars, the weapon arm's flags 0x1413f8 / 0x1413fa and its timer 0x13f50c, the hand records, the item's
//! sound loops — and [`update`] swaps it into the hero after the hand's own item pass (the tick calls it right after the
//! hand's sounds), runs the same steps on it (creation, the tracked placement, the weapon check's draw, the item's
//! update) and swaps it back. Its sounds are played by [`flush_sounds`] the same way.
//!
//! Ratchet's own animation stays the hand's: what the off-hand update asks of the hero (an arm layer, a stance, a
//! `SetState`) is dropped, and its draw only raises the off hand's arm flag. The engine picks the item ([`OffHand::want`],
//! one of [`ITEMS`]), holds the trigger ([`OffHand::fire`]) and places it ([`OffHand::items`]' tracked hand and aim, as
//! for the hand). Ammo is the hand's: both hands draw on the same reserve.
//!
//! The off hand takes the guns whose whole use is their item update, the Suck Cannon while the hand holds none (the
//! moby side keeps one cannon), and the two items whose use is a hero state, the Swingshot (the pull and the swing)
//! and the Walloper (the lunge): while the left hand uses one, it **leads**: [`lead`] swaps it into the hero before the
//! hero update, which runs the native weapon check, states and item pass on it (○ from the left trigger), and
//! [`unlead`] swaps it back after the item pass. The thrown gloves are thrown by the engine (a held bomb at the left
//! controller), not here; the Visibomb (a camera) and the gadgets stay in the hand.
//!
//! **One hand busy, the other free.** In VR a hero state can keep one hand busy while the other is free: a ledge held by
//! one hand, the bolt crank's wrench (the right hand), the Swingshot's pull and swing, the Walloper's lunge. The native
//! no-item flags of those states (0x1413f7 / 0x1413fc / 0x1413fe) are the busy hand's, and the free hand's item pass
//! runs as for Ratchet standing (group and state 0 for the pass, [`busy`]): the left hand's beside the hand's, and the
//! hand's beside the leading left hand ([`update_hand`]: the native swap rule `UpdateWrenchSelected` for its requests,
//! then the same pass). A ledge held by the left hand alone makes the left hand lead, so that the hand's item stays out.

use super::items::{HeroItems, HitSink, ItemEnv, ItemGlobals};
use super::Hero;
use crate::moby_runtime::{Moby, MobyTable};
use crate::pad::{button, PadState};
use crate::rng::Rng;

/// The items the left hand holds as its item (the thrown gloves are the engine's).
pub const ITEMS: [i32; 10] = [
    super::suck_cannon::SUCK_CANNON,
    super::devastator::DEVASTATOR,
    super::swingshot::SWINGSHOT,
    super::taunter::TAUNTER,
    super::blaster::BLASTER,
    super::pyrocitor::PYROCITOR,
    super::tesla::TESLA,
    super::morph_ray::MORPH,
    super::walloper::WALLOPER,
    super::ryno::RYNO,
];

/// The items whose use is a hero state: the left hand leads while it uses one ([`lead`]).
pub fn hero_item(id: i32) -> bool { id == super::swingshot::SWINGSHOT || id == super::walloper::WALLOPER }

/// Whether item `id` can go in the left hand.
pub fn supported(id: i32) -> bool { ITEMS.contains(&id) }

const N: usize = rc_formats::save_game::ITEM_COUNT;

/// The off hand's copy of the hand-item state (module doc).
#[derive(Clone, Debug)]
pub struct OffHand {
    /// The item the player wants in the left hand (0: none), set by the engine every tick.
    pub want: i32,
    /// The left trigger is held: ○ for the off-hand item.
    pub fire: bool,
    fire_prev: bool,
    /// The left trigger's presses of the last 30 ticks (bit n: n ticks ago): the pad history of ○ while leading.
    presses: u32,
    /// The left hand has the hero's hand slot for this tick's hero update ([`lead`]).
    pub leading: bool,
    /// The slot, its item moby and the tracked placement (the engine writes `tracked`, `tracked_hand`, `tracked_class`
    /// and `tracked_aim`, as for the hand).
    pub items: HeroItems,
    globals: ItemGlobals,
    /// The guns' pvars (the engine writes the Blaster's and the Pyrocitor's tracked aim, as for the hand).
    pub blaster: super::blaster::Blaster,
    pub pyro: super::pyrocitor::Pyro,
    devastator: super::devastator::Devastator,
    tesla: super::tesla::Tesla,
    ryno: super::ryno::Ryno,
    reactive: super::reactive::Reactive,
    /// 0x1413f8 / 0x1413fa / 0x13f50c of the off hand's arm.
    f13f8: u8,
    f13fa: u8,
    f50c: i32,
    /// The hand records and the item moby's modifiers (`super::gadgets::Gadgets`' hand part).
    gadgets: super::gadgets::Gadgets,
    /// The item's sound loops and the sounds its last update made (played by [`flush_sounds`]).
    item_loops: [Option<i32>; 6],
    item_loop_alive: [bool; 6],
    seq_loop: Option<i32>,
    item_sounds: Vec<i32>,
    item_voices: Vec<super::packs::SoundCmd>,
    /// Ammo the off-hand item used, by item (the engine's feedback tells the hands apart with it).
    pub used: [i32; N],
    /// Uses of the off-hand item (a shot, a tick of ammo burnt): the left hand's feedback.
    pub fired: u64,
    /// The last pass found the slot empty and cleared what its item left (its light, its flame): nothing to run until
    /// an item is wanted again.
    clean: bool,
}

impl Default for OffHand {
    fn default() -> Self {
        OffHand {
            want: 0,
            fire: false,
            fire_prev: false,
            presses: 0,
            leading: false,
            items: HeroItems::default(),
            globals: ItemGlobals::default(),
            blaster: Default::default(),
            pyro: Default::default(),
            devastator: Default::default(),
            tesla: Default::default(),
            ryno: Default::default(),
            reactive: Default::default(),
            f13f8: 0,
            f13fa: 0,
            f50c: 0,
            gadgets: Default::default(),
            item_loops: [None; 6],
            item_loop_alive: [false; 6],
            seq_loop: None,
            item_sounds: Vec::new(),
            item_voices: Vec::new(),
            used: [0; N],
            fired: 0,
            clean: true,
        }
    }
}

impl OffHand {
    /// The item moby in the left hand (None: empty, or between a put-away and the next creation).
    pub fn item(&self) -> Option<&super::items::HandItem> { self.items.slot.item.as_ref() }
    /// The item id held, ready (state 2), else 0.
    pub fn held(&self) -> i32 { if self.items.slot.state == 2 && self.items.slot.item.is_some() { self.items.slot.id } else { 0 } }
    /// The hand moby's modifiers as the draw reads them.
    pub fn hand_mods(&self) -> &[rc_formats::moby_anim::JointModifier] { &self.gadgets.hand_mods }
    /// The Tesla Claw's beam of the left hand's item.
    pub fn tesla(&self) -> &super::tesla::Tesla { &self.tesla }
    /// The Suck Cannon's vortex, the Taunter's rings and the Morph-o-Ray's beam of the left hand's item.
    pub fn reactive(&self) -> &super::reactive::Reactive { &self.reactive }
    fn idle(&self) -> bool { self.want == 0 && self.clean && self.items.slot.item.is_none() && self.item_loops.iter().all(Option::is_none) }
}

/// Exchanges the off hand's copy with the hero's own (its own inverse).
fn swap(h: &mut Hero, o: &mut OffHand) {
    use std::mem::swap;
    swap(&mut h.items, &mut o.items);
    let w = &mut h.weapons;
    swap(&mut w.blaster, &mut o.blaster);
    swap(&mut w.pyro, &mut o.pyro);
    swap(&mut w.devastator, &mut o.devastator);
    swap(&mut w.tesla, &mut o.tesla);
    swap(&mut w.ryno, &mut o.ryno);
    swap(&mut w.reactive, &mut o.reactive);
    swap(&mut h.f13f8, &mut o.f13f8);
    swap(&mut h.f13fa, &mut o.f13fa);
    swap(&mut h.f50c, &mut o.f50c);
    let g = &mut h.gadgets;
    swap(&mut g.hand_joints, &mut o.gadgets.hand_joints);
    swap(&mut g.hand_manips, &mut o.gadgets.hand_manips);
    swap(&mut g.hand_mods, &mut o.gadgets.hand_mods);
    swap(&mut g.item_bc, &mut o.gadgets.item_bc);
    swap(&mut g.pending, &mut o.gadgets.pending);
    swap(&mut g.calls, &mut o.gadgets.calls);
    // `HeroItemsCreate`'s clear of the new moby's pvars reaches these.
    swap(&mut g.hydro, &mut o.gadgets.hydro);
    swap(&mut g.detector, &mut o.gadgets.detector);
    let fx = &mut h.fx;
    swap(&mut fx.item_loops, &mut o.item_loops);
    swap(&mut fx.item_loop_alive, &mut o.item_loop_alive);
    swap(&mut fx.seq_loop, &mut o.seq_loop);
    swap(&mut fx.item_sounds, &mut o.item_sounds);
    swap(&mut fx.item_voices, &mut o.item_voices);
}

/// The off hand's pad: the hand's, with ○ from the left trigger and neither □ (the wrench) nor △.
fn pad_of(pad: &PadState, fire: bool, prev: bool) -> PadState {
    let mut p = pad.clone();
    let off = button::CIRCLE | button::SQUARE | button::TRIANGLE;
    let circle = |on: bool| if on { button::CIRCLE } else { 0 };
    p.held = p.held & !off | circle(fire);
    p.pressed = p.pressed & !off | circle(fire && !prev);
    p.released = p.released & !off | circle(!fire && prev);
    p.raw = p.raw & !off | circle(fire);
    p
}

/// The pad for the leading left hand's hero update: [`pad_of`], with ○'s press history the left trigger's.
fn lead_pad(pad: &PadState, fire: bool, prev: bool, presses: u32) -> PadState {
    let mut p = pad_of(pad, fire, prev);
    let off = button::CIRCLE | button::SQUARE | button::TRIANGLE;
    let n = crate::pad::HISTORY as i32;
    for i in 0..n {
        let slot = (p.hist_index as i32 - 1 - i).rem_euclid(n) as usize;
        p.hist_pressed[slot] = p.hist_pressed[slot] & !off | if presses >> i & 1 != 0 { button::CIRCLE } else { 0 };
    }
    p
}

/// Before the hero update: the left hand leads when its Swingshot or Walloper is ready and the left trigger is held, or
/// a punch of the left hand is offered ([`super::walloper::TrackedPunch::left`]), and keeps leading through the pull's
/// and the swing's states (the hook out and back) and the lunge. Leading swaps the left hand's copy and `globals` (its
/// own hand requests: the hand's wait) into the hero and gives the hero update `pad` with ○ from the left trigger;
/// returns the hand's pad for [`unlead`].
pub fn lead(h: &mut Hero, globals: &mut ItemGlobals, pad: &mut PadState) -> Option<PadState> {
    let o = &mut h.off_hand;
    o.presses = o.presses << 1 | (o.fire && !o.fire_prev) as u32;
    let id = o.held();
    let punch = h.walloper.tracked_punch.is_some_and(|p| p.left && p.begin);
    // Not while the hand's no-item flags hold (the water, the swings, the scripted holds, the gold bolt: [`update`]).
    let free = h.f13ff == 0 && h.items.f13fe == 0 && h.items.f13f7 == 0 && h.items.f13fc == 0;
    let start = free && hero_item(id) && id == o.want && (o.fire || (id == super::walloper::WALLOPER && punch));
    let using = match o.items.slot.id {
        super::swingshot::SWINGSHOT => matches!(h.state, 0x24 | 0x25 | 0x26 | 0x2c) || h.swing.item.state > 1,
        super::walloper::WALLOPER => h.state == super::walloper::LUNGE,
        _ => false,
    };
    // A ledge held by the left hand alone: the ledge's no-item flags are its own, the hand's item stays out.
    let hang = h.tracked.grabbed[0] && !h.tracked.grabbed[1];
    o.leading = h.mode == 0 && h.health > 0 && (start || hang || (o.leading && using));
    if !o.leading { return None; }
    let hand = pad.clone();
    *pad = lead_pad(&hand, o.fire, o.fire_prev, o.presses);
    o.fire_prev = o.fire;
    let mut o = std::mem::take(&mut h.off_hand);
    swap(h, &mut o);
    std::mem::swap(globals, &mut o.globals);
    h.off_hand = o;
    Some(hand)
}

/// VR: a hero state that keeps one hand busy (a ledge, the bolt crank's wrench, the Swingshot's pull and swing, the
/// Walloper's lunge), leaving the other free to use its item.
fn busy(h: &Hero) -> bool {
    super::tracked::ledge_hold(h.state)
        || matches!(h.state, super::crank::STATE | 0x24 | 0x25 | 0x26 | 0x2c | 0x2d | super::walloper::LUNGE)
}

/// While the left hand leads, after its item pass: the hand's item, on its own copy, as the left hand's runs beside the
/// hand's ([`run`]: free, its requests through the native swap rule); `env.pad` is the hand's (○ the right trigger).
pub fn update_hand(h: &mut Hero, table: &mut MobyTable, anim: &dyn super::anim::AnimCtl, rng: &mut Rng, env: &ItemEnv, hits: &mut dyn HitSink) {
    let mut o = std::mem::take(&mut h.off_hand);
    // The hand's arm timer (the physics' post-move counts the leading hand's): the Blaster fires past ticks(5) of it.
    o.f50c = if o.f13f8 != 0 { o.f50c + 1 } else { 0 };
    // A weapon a busy state's flags put away (the ledge's entry, before the left hand led: `restore`) comes back to the
    // free hand, unless the hand asks for the wrench.
    if o.globals.request == super::items::item::WRENCH { o.items.restore = 0; }
    if o.items.restore != 0 { o.items.restore_pending = 1; }
    run(h, &mut o, None, true, table, anim, rng, env, hits);
    o.clean = false;
    h.off_hand = o;
}

/// The second hand's pass: `o`'s copy swapped into the hero, [`pass`] (the slot following `want`, or with None the native
/// swap rule's target), the weapon check's draw, and back; Ratchet's animation requests are the first hand's (the
/// second's dropped). `free`: the hero state keeps the other hand busy ([`busy`]): no no-item flags, and the pass sees
/// Ratchet standing (group and state 0, restored after).
#[allow(clippy::too_many_arguments)]
fn run(h: &mut Hero, o: &mut OffHand, want: Option<i32>, free: bool, table: &mut MobyTable, anim: &dyn super::anim::AnimCtl, rng: &mut Rng, env: &ItemEnv, hits: &mut dyn HitSink) {
    let w = &h.weapons;
    let kept = (w.layer_fade, w.layer_seq, w.arm_item, w.pending_draw, w.pending_idle, w.pending_anim, w.deferred);
    h.weapons.pending_draw = false;
    swap(h, o);
    let pose = (h.group, h.state);
    if free {
        (h.group, h.state) = (0, 0);
        (h.items.f13fc, h.items.f13f7, h.items.f13fe, h.items.f13f6) = (0, 0, 0, 0);
    }
    if want.is_none() { super::items::update_hand_selected(h, &mut o.globals, rng, env); }
    pass(h, &mut o.globals, want, table, anim, rng, env, hits);
    if std::mem::take(&mut h.weapons.pending_draw) { draw(h); }
    h.gadgets.calls.clear();
    h.gadgets.pending = None;
    (h.group, h.state) = pose;
    swap(h, o);
    let w = &mut h.weapons;
    (w.layer_fade, w.layer_seq, w.arm_item, w.pending_draw, w.pending_idle, w.pending_anim, w.deferred) = kept;
}

/// After the leading left hand's item pass: the hand's copy, requests and pad back ([`lead`]).
pub fn unlead(h: &mut Hero, globals: &mut ItemGlobals, pad: &mut PadState, hand: PadState) {
    let mut o = std::mem::take(&mut h.off_hand);
    swap(h, &mut o);
    std::mem::swap(globals, &mut o.globals);
    o.clean = false;
    h.off_hand = o;
    *pad = hand;
}

/// The off hand's item pass (module doc), after the hand's own (`items::items_update`, `gadgets::after_items`) on a
/// tick the hero update ran in Ratchet's body.
pub fn update(h: &mut Hero, table: &mut MobyTable, anim: &dyn super::anim::AnimCtl, rng: &mut Rng, env: &ItemEnv, hits: &mut dyn HitSink) {
    if h.off_hand.idle() { return; }
    let mut o = std::mem::take(&mut h.off_hand);
    // The hand's no-item flags hold for both hands (the water, the swings, the scripted holds, the gold bolt), unless
    // the state keeps only the hand busy ([`busy`]: not a ledge the left hand holds).
    let free = busy(h) && !h.tracked.grabbed[0];
    let flags = if free { (0, 0, 0) } else { (h.items.f13fc, h.items.f13f7, h.items.f13fe) };
    (o.items.f13fc, o.items.f13f7, o.items.f13fe, o.items.f13f6) = (flags.0, flags.1, flags.2, 0);
    let blocked = h.mode != 0 || h.health <= 0 || h.f13ff != 0 || flags != (0, 0, 0);
    // One Suck Cannon: the left hand's goes while the hand holds one.
    let cannon = o.want == super::suck_cannon::SUCK_CANNON && h.items.slot.id == super::suck_cannon::SUCK_CANNON;
    let want = if blocked || cannon || !supported(o.want) { 0 } else { o.want };
    let pad = pad_of(env.pad, o.fire && !blocked, o.fire_prev);
    o.fire_prev = o.fire && !blocked;
    let env = ItemEnv { pad: &pad, ..*env };
    // `HeroTickStateTimer`'s count of the arm's timer (`super::physics`' post-move, for the hand).
    o.f50c = if o.f13f8 != 0 { o.f50c + 1 } else { 0 };
    let (used, shots) = (h.weapons.used, o.blaster.shots);
    run(h, &mut o, Some(want), free, table, anim, rng, &env, hits);
    let w = &h.weapons;
    let mut burnt = false;
    for (k, (now, before)) in w.used.iter().zip(used).enumerate() {
        if *now > before {
            o.used[k] += now - before;
            burnt = true;
        }
    }
    if burnt || o.blaster.shots > shots { o.fired += 1; }
    o.clean = o.items.slot.item.is_none();
    h.off_hand = o;
}

/// `0x22ee08` for the off hand: its arm raised (0x1413f8, 0x1413fa = def +0x30), no animation of Ratchet's.
fn draw(h: &mut Hero) {
    h.f13f8 = 1;
    h.f13fa = h.weapons.defs.get(h.items.slot.id.max(0) as usize).map_or(0, |d| d.w30 as u8);
}

/// The hand's create / attach / slot loop (`items::items_update`), on the swapped-in copy, with the swap rule replaced
/// for the left hand: the slot follows `want` (0: empty), never the wrench. With None (the hand's copy, [`update_hand`])
/// the native swap rule has set the target and started the put-away.
#[allow(clippy::too_many_arguments)]
fn pass(h: &mut Hero, g: &mut ItemGlobals, want: Option<i32>, table: &mut MobyTable, anim: &dyn super::anim::AnimCtl, rng: &mut Rng, env: &ItemEnv, hits: &mut dyn HitSink) {
    let id = h.items.slot.id;
    // The slot loop's head: the leftovers of an item that is gone.
    if id != super::pyrocitor::PYROCITOR { super::pyrocitor::item_gone(h, table, env, hits); }
    if id != super::blaster::BLASTER { super::blaster::item_gone(h, hits); }
    if id != super::devastator::DEVASTATOR { super::devastator::item_gone(h, hits); }
    if id != super::tesla::TESLA { super::tesla::item_gone(h, hits); }
    if id != super::morph_ray::MORPH && h.weapons.reactive.morph.beam.drawn.is_some() { super::morph_ray::item_gone(h, hits); }
    if h.items.slot.item.is_none() && h.fx.item_loops[super::fx::LOOP_SEQ].is_some() {
        h.fx.item_voices.push(super::packs::SoundCmd::ItemRelease { n: super::fx::LOOP_SEQ });
        h.fx.seq_loop = None;
    }
    // `HeroItemsCreate`: the wanted item, once the gate frame has passed.
    if h.items.slot.item.is_none() {
        h.items.slot.id = 0;
        if let Some(want) = want {
            if want == 0 { return; }
            (g.request, g.wrench_flag, g.saved) = (0, 0, want);
            h.items.target = want;
        }
        super::items::create_hand(h, g, env);
        if h.items.slot.item.is_none() { return; }
    }
    // `HeroItemsAttach`: only at the tracked hand (the off hand has no joint of Ratchet's to hang from).
    let class = h.items.slot.item.as_ref().map_or(0, |it| it.o_class);
    if h.items.tracked_hand.is_some() && h.items.tracked_class == class { super::items::attach_hand(h, table, anim, env); }
    match h.items.slot.state {
        2 if want.is_some_and(|w| h.items.slot.id != w) => {
            // `UpdateWrenchSelected`'s commit: the arm down, the item's put-away sequence, slot state 3.
            h.items.slot.ticks_ready = 3;
            super::weapons::put_away(h);
            if let Some(m) = h.items.slot.item.as_mut() { super::items::blend_item(m, env.data, 2, 0, 2); }
            h.items.slot.state = 3;
            return;
        }
        2 => {
            h.items.slot.ticks_ready += 1;
            let targets = super::gadgets::item_targets(h, class);
            super::gadgets::hand_records(h, &targets);
            if let Some(it) = h.items.slot.item.as_mut() {
                if it.anim.flags & 2 != 0 && it.anim.seq_b == 0 { super::items::blend_item(it, env.data, 1, 0, 2); }
            }
        }
        3 => {
            h.items.slot.putaway_ticks = h.items.slot.putaway_ticks.wrapping_add(1);
            super::items::delete_hand(h, env.frame + 2);
            return;
        }
        _ => return,
    }
    fire_case(h, env);
    super::gadgets::slot_item_update(h, table, anim, env, hits, rng);
    let targets = super::gadgets::item_targets(h, class);
    super::gadgets::hand_modifiers(h, &targets);
}

/// The weapon check's case of an item that has one (the Blaster's 0xf: `super::blaster::fire`): ○ pressed, or held
/// past 5 ticks of the item ready, with ammo raises the arm; held without ammo, the empty click.
fn fire_case(h: &mut Hero, env: &ItemEnv) {
    // The Swingshot's and the Walloper's weapon checks run while the left hand leads ([`lead`]).
    if hero_item(h.items.slot.id) { return; }
    if super::gadgets::hand_item(h.items.slot.id).and_then(|k| k.fire).is_none() || h.items.tracked_aim.is_none() { return; }
    let g = h.group;
    if !(g <= 1 || g == 0xc || g == 2 || g == 4 || g == 5 || g == 0xf || h.state == 0x1e) { return; }
    let mask = h.items.slot.fire_mask;
    let (held, pressed) = (env.pad.held & mask != 0, env.pad.pressed & mask != 0);
    let id = h.items.slot.id;
    if ((held && super::physics::ticks(5) < h.items.slot.ticks_ready) || pressed) && h.weapons.has_ammo(id) != 0 {
        if h.f13f8 == 0 { draw(h); }
    } else if held && h.weapons.has_ammo(id) == 0 {
        h.fx.item_voices.push(super::packs::SoundCmd::ItemLoop { n: super::fx::LOOP_CLICK, index: 0, flags: 0 });
    }
}

/// The off-hand item's sounds of this tick (`super::gadgets::flush_item_sounds` at its moby), right after [`update`].
pub fn flush_sounds(h: &mut Hero, moby: &Moby, sounds: &mut dyn super::HeroSounds, rng: &mut Rng) {
    if h.off_hand.item_sounds.is_empty() && h.off_hand.item_voices.is_empty() { return; }
    let mut o = std::mem::take(&mut h.off_hand);
    swap(h, &mut o);
    // The hand's Swingshot sounds are the hand's (flushed already): none are added here.
    let swing = std::mem::take(&mut h.swing.item.sounds);
    super::gadgets::flush_item_sounds(h, moby, sounds, rng);
    h.swing.item.sounds = swing;
    swap(h, &mut o);
    h.off_hand = o;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The off hand's ○ is the left trigger's edge; □ and △ never reach it, the hand's other buttons do.
    #[test]
    fn the_left_trigger_is_the_off_hands_circle() {
        let mut pad = PadState::default();
        pad.held = button::SQUARE | button::TRIANGLE | button::L1;
        pad.pressed = button::SQUARE;
        let p = pad_of(&pad, true, false);
        assert_eq!(p.held, button::CIRCLE | button::L1);
        assert_eq!(p.pressed, button::CIRCLE);
        let p = pad_of(&pad, true, true);
        assert_eq!(p.pressed, 0);
        let p = pad_of(&pad, false, true);
        assert_eq!((p.held, p.released), (button::L1, button::CIRCLE));
    }

    /// Swapping twice gives both copies back.
    #[test]
    fn the_swap_is_its_own_inverse() {
        let mut h = Hero::new();
        h.items.slot.id = 15;
        h.f13f8 = 1;
        let mut o = OffHand::default();
        o.items.slot.id = 19;
        swap(&mut h, &mut o);
        assert_eq!((h.items.slot.id, h.f13f8, o.items.slot.id, o.f13f8), (19, 0, 15, 1));
        swap(&mut h, &mut o);
        assert_eq!((h.items.slot.id, h.f13f8, o.items.slot.id, o.f13f8), (15, 1, 19, 0));
    }

    /// The left hand's items: the guns, the Suck Cannon, the Swingshot and the Walloper (not the gloves, the Visibomb or
    /// the gadgets).
    #[test]
    fn the_left_hand_takes_the_guns() {
        for id in [9, 11, 12, 14, 15, 16, 18, 19, 21, 23] { assert!(supported(id), "item {id}"); }
        for id in [0, 8, 10, 13, 17, 20, 22, 24, 25, 26, 27, 31, 32] { assert!(!supported(id), "item {id}"); }
    }

    /// The leading pad's ○ history is the left trigger's.
    #[test]
    fn the_lead_pad_has_the_left_presses() {
        let mut pad = PadState::default();
        pad.hist_count = 30;
        pad.hist_index = 3;
        pad.hist_pressed[2] = button::CIRCLE | button::CROSS;
        let p = lead_pad(&pad, true, false, 0b100);
        assert_eq!(p.hist_pressed[2], button::CROSS);
        assert_eq!(p.hist_pressed[0], button::CIRCLE);
        assert_eq!(p.pressed_within(button::CIRCLE, 3), Some(2));
    }
}
