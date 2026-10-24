//! Trigger prepares one native Bomb Glove projectile; a tracked release supplies its launch velocity.
use super::{
    Settings,
    embodied::{BombGesture, Motion, game_point, gameplay, unobstructed},
    game::Rig,
};
use bevy::prelude::*;
use rc_game::{
    hero::{packs::SoundCmd, weapons::BOMB_GLOVE},
    moby_runtime::mode,
    moby_update::{
        classes::{bomb, decoy, doom_canister, mine},
        services::{ClassData, pvar},
    },
};

pub(super) struct BombGlovePlugin;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct BombUpdate;
#[derive(Resource, Default)]
pub(super) struct HeldBomb {
    pub id: Option<usize>,
    pub item: i32,
    level: u32,
    last_tick: u64,
}
#[derive(Default)]
struct Recall {
    weapon: Option<i32>,
    trigger_consumed: bool,
    contact: super::solid_contact::History,
}
impl Plugin for BombGlovePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeldBomb>().add_systems(
            FixedUpdate,
            update.in_set(BombUpdate).before(crate::gameplay::GameTick),
        );
    }
}

pub(super) fn palm_point(hand: Transform, root: Transform, scale: f32) -> Transform {
    let mut world = root.mul_transform(hand);
    // -X is the hollow of the right glove; move 8 cm farther forward from the inner wrist into the palm.
    world.translation += world.rotation * Vec3::new(-0.135, 0.01, -0.125) * scale;
    world
}

fn point(hand: Transform, root: Transform, scale: f32) -> Transform {
    palm_point(hand, root, scale)
}

fn solid_point(
    p: &crate::gameplay::Play,
    coll: &rc_formats::collision::Collision,
    hand: Transform,
    root: Transform,
    scale: f32,
    history: &mut super::solid_contact::History,
) -> Transform {
    let mut pose = point(hand, root, scale);
    let palm = root.mul_transform(hand).translation;
    let eye =
        crate::tfrag_render::game_to_bevy(p.game.hero.position()) + root.rotation * Vec3::Y * super::game::RATCHET_EYE;
    let probes = [(palm, 0.13 * scale), (pose.translation, 0.15 * scale)];
    pose.translation += super::solid_contact::constrain(p, coll, &probes, eye, history);
    pose
}

/// The original Bomb Glove replaces the provisional right hand. Its authored palm (list 0)
/// follows the controller, with fingers forward (-Y in the original model) and palm up.
pub(super) fn appearance(
    hand: Transform,
    data: &rc_game::hero::items::ItemData,
    item: i32,
) -> Option<rc_game::hero::items::HandItem> {
    let class = data.class(glove_class(item))?;
    let chain = class.chains.first().filter(|c| !c.is_empty())?;
    let mut anim = rc_formats::moby_anim::AnimState::spawn(&class.anim);
    // Keep a stable authored hand pose; Ratchet's running/jumping fingers must not drive this hand.
    if class.anim.sequence(1).is_some() {
        anim.seq_a = 1;
        anim.seq_b = 1;
    }
    let palm =
        *rc_formats::moby_anim::evaluate_chains(&class.anim, &anim, None, &[chain.as_slice()])
            .first()?;
    let basis = Mat3::from_cols_array_2d(&[
        palm[0][..3].try_into().ok()?,
        palm[1][..3].try_into().ok()?,
        palm[2][..3].try_into().ok()?,
    ]);
    let determinant = basis.determinant();
    if !determinant.is_finite() || determinant.abs() < 1e-5 {
        return None;
    }
    let frame = crate::game_camera::game_rows(&hand);
    let axes = Mat3::from_cols(frame[2], -frame[0], -frame[1]) * basis.inverse();
    let anchor = Vec3::new(palm[3][0], palm[3][1], palm[3][2]) * (class.scale / 1024.0);
    Some(rc_game::hero::items::HandItem {
        o_class: class.o_class,
        mstate: 0,
        anim,
        snapshot: None,
        scale: class.scale,
        position: (crate::game_camera::game_eye(&hand) - axes * anchor).to_array(),
        rows: [0, 1, 2].map(|i| {
            let r = axes.col(i);
            [r.x.to_bits(), r.y.to_bits(), r.z.to_bits(), 0]
        }),
        hit_timer: 0,
        flight: default(),
    })
}
fn rotation(hand: &Transform) -> [f32; 3] {
    let r = crate::game_camera::game_rows(hand).map(|v| {
        [
            rc_game::ps2v::Pf::f(v.x),
            rc_game::ps2v::Pf::f(v.y),
            rc_game::ps2v::Pf::f(v.z),
            rc_game::ps2v::Pf::ZERO,
        ]
    });
    rc_game::hero::comet::launch_euler(&[
        r[0],
        r[1],
        r[2],
        [
            rc_game::ps2v::Pf::ZERO,
            rc_game::ps2v::Pf::ZERO,
            rc_game::ps2v::Pf::ZERO,
            rc_game::ps2v::Pf::ONE,
        ],
    ])
}
pub(super) fn glove_class(item: i32) -> i16 {
    match item {
        17 => mine::GLOVE_CLASS,
        20 => doom_canister::GLOVE_CLASS,
        25 => decoy::GLOVE_CLASS,
        _ => bomb::GLOVE_CLASS,
    }
}
/// The gold Decoy Glove's decoys are their own class (0x76c), as the native glove's.
fn gold_decoy(p: &crate::gameplay::Play) -> bool {
    p.game.hero.weapons.gold.get(decoy::DECOY_GLOVE as usize).is_some_and(|&g| g != 0)
}
fn object_class(item: i32, gold: bool) -> i16 {
    match item {
        17 => mine::CLASS,
        20 => doom_canister::CLASS,
        25 if gold => decoy::GOLD_CLASS,
        25 => decoy::CLASS,
        _ => bomb::BOMB_CLASS,
    }
}
fn initialise(p: &mut crate::gameplay::Play, id: usize, item: i32, point: [f32; 3]) {
    let scale = p.game.mobys.mobys[id].scale;
    match item {
        17 => {
            mine::init_held(
                &mut p.game.mobys.mobys[id],
                &mut p.game.rng,
                point,
                p.game.counter as i32,
            );
            // Native held growth is suspended: use its settled held scale before release divides by 0.65.
            p.game.mobys.mobys[id].scale = scale * 0.65;
        }
        20 => {
            doom_canister::init_held(&mut p.game.mobys, id, point, false);
            // The original held canister settles at a quarter scale; flight handles its later growth.
            p.game.mobys.mobys[id].scale = scale * 0.25;
        }
        25 => {
            let gold = gold_decoy(p);
            decoy::init_held(&mut p.game.mobys, id, &mut p.game.rng, point, gold, false);
            decoy::cap_count(&mut p.game.mobys, id);
        }
        _ => bomb::init_held(
            &mut p.game.mobys.mobys[id],
            &mut p.game.rng,
            point,
            p.game.hero.rot[2].to_f32(),
        ),
    }
}
fn is_held(p: &crate::gameplay::Play, id: usize) -> bool {
    p.game.mobys.mobys.get(id).is_some_and(|m| {
        matches!(m.o_class, 121 | 74 | 230 | 203 | 1900)
            && m.state == 0
            && m.mode & mode::NO_UPDATE != 0
    })
}
fn cancel(p: &mut crate::gameplay::Play, held: &mut HeldBomb) {
    if let Some(id) = held.id.take().filter(|id| is_held(p, *id)) {
        bomb::delete_from_hero(&mut p.game.mobys, &mut p.svc, id, p.game.counter);
    }
}
fn empty(p: &mut crate::gameplay::Play, position: [f32; 3]) {
    p.game.hero.fx.item_voices.push(SoundCmd::MobySound {
        moby: p.game.hero_moby,
        o_class: bomb::GLOVE_CLASS,
        pos: position,
        index: rc_game::hero::weapons::EMPTY_SOUND,
        flags: 0,
    });
    println!("VR_GAME bomb_glove_empty");
}

fn update(
    play: Option<ResMut<crate::gameplay::Play>>,
    level: Res<crate::Level>,
    settings: Res<Settings>,
    rig: Res<Rig>,
    mut motion: ResMut<Motion>,
    mut held: ResMut<HeldBomb>,
    time: Res<Time<Real>>,
    mode: Res<crate::menu_render::MenuMode>,
    saved: Option<ResMut<crate::gameplay::Persistent>>,
    mut session: Option<ResMut<crate::gameplay::Session>>,
    input: Option<Res<super::game_input::HandInput>>,
    mut pad: ResMut<crate::input_map::PadFrame>,
    mut recall: Local<Recall>,
) {
    let Some(mut play) = play else {
        held.id = None;
        return;
    };
    let p = &mut *play;
    p.game.hero.items.tracked_bomb_glove = !settings.desktop_preview;
    let level_id = crate::level_load::level_index();
    if held.level != level_id || p.game.counter < held.last_tick {
        // Slots belong to the old table: never delete an entity in the newly loaded level by its old index.
        held.id = None;
        held.level = level_id;
    }
    held.last_tick = p.game.counter;
    if held.id.is_some_and(|id| !is_held(p, id)) {
        held.id = None;
    }
    let selected = p.game.hero.items.slot.id;
    let ready = gameplay(&mode)
        && p.game.hero.mode == 0
        && p.game.hero.health > 0
        && !p.death_pending
        && !p.game.hero.hand_hidden();
    if ready && p.game.hero.items.slot.state == 2 && super::blaster::supported(selected) {
        recall.weapon = Some(selected);
    }
    if input
        .as_ref()
        .and_then(|i| i.trigger)
        .is_none_or(|v| v < 0.35)
    {
        recall.trigger_consumed = false;
    }
    let empty_hand = selected == 0 || selected == 8;
    let swapping = (p.game.hero.items.target != 0 && p.game.hero.items.target != selected)
        || session.as_ref().is_some_and(|s| s.0.temp_hand != 0);
    if ready && empty_hand && !swapping && motion.bomb.press.is_some() {
        if let (Some(saved), Some(session)) = (saved.as_ref(), session.as_mut()) {
            let available = |id: i32| {
                super::blaster::supported(id)
                    && saved
                        .0
                        .global
                        .owned
                        .get(id as usize)
                        .is_some_and(|&v| v != 0)
            };
            let last = [saved.0.global.equipped[0], saved.0.global.last_hand_item]
                .into_iter()
                .find(|&id| available(id))
                .or_else(|| recall.weapon.filter(|&id| available(id)));
            if let Some(id) = last {
                session.0.temp_hand = id;
                p.game.hero.items.restore_pending = 0;
                recall.trigger_consumed = true;
                println!("VR_GAME recall_last_weapon item={id}");
            }
        }
    }
    if recall.trigger_consumed {
        // This press equips only. A fresh press after release is required to fire/prepare a throw.
        pad.0.buttons &= !(rc_game::pad::button::CIRCLE as u16);
        pad.0.pressure[5] = 0;
        cancel(p, &mut held);
        motion.bomb.press = None;
        motion.bomb.release = None;
        return;
    }
    if !rc_game::hero::items::is_glove(selected)
        || (super::blaster::supported(p.game.hero.items.target)
            && !rc_game::hero::items::is_glove(p.game.hero.items.target))
    {
        cancel(p, &mut held);
        motion.bomb.press = None;
        motion.bomb.release = None;
        return;
    }
    let item_id = selected;
    if held.id.is_some() && held.item != item_id {
        cancel(p, &mut held);
    }
    let Some((hand, root, coll)) = motion.hands[1]
        .zip(rig.root_for(&p.game, settings.units_per_metre))
        .zip(level.0.collision.as_ref())
        .map(|((hand, root), coll)| (hand, root, coll))
        .filter(|_| ready)
    else {
        cancel(p, &mut held);
        motion.bomb = BombGesture::default();
        return;
    };
    recall.contact.prepare(rig.revision, selected);
    let pose = solid_point(
        p,
        coll,
        hand,
        root,
        settings.units_per_metre,
        &mut recall.contact,
    );
    let eye = rig.body_eye(&p.game);
    if !unobstructed(p, coll, eye, pose.translation) {
        cancel(p, &mut held);
        motion.bomb.press = None;
        motion.bomb.release = None;
        return;
    }
    let now = time.elapsed_secs_f64();
    let press = motion.bomb.press.take().filter(|t| now - t <= 0.12);
    let release = motion
        .bomb
        .release
        .take()
        .filter(|(_, _, t)| now - t <= 0.12);
    let Some(mut saved) = saved else {
        cancel(p, &mut held);
        return;
    };
    let item = item_id as usize;
    let object_class = object_class(item_id, gold_decoy(p));
    if held.id.is_none() && press.is_some() && (motion.bomb.held || release.is_some()) {
        if saved.0.global.owned[item] == 0 || saved.0.global.ammo[item] <= 0 {
            empty(p, game_point(pose.translation));
            return;
        }
        if p.classes.info(object_class).is_none() {
            return;
        }
        if let Some(id) = bomb::create_from_hero(
            &mut p.game.mobys,
            &mut p.svc,
            &*p.classes,
            object_class,
            p.game.counter,
        ) {
            initialise(p, id, item_id, game_point(pose.translation));
            let m = &mut p.game.mobys.mobys[id];
            // While held, the XR hand owns the pose. Suspend the original update (including its glove-slot
            // deletion check); release restores the native flight, fuse, collisions, explosion and damage.
            m.mode |= mode::NO_UPDATE;
            m.coll_disable = 1;
            m.glow = 0xff9f_9f9f;
            held.id = Some(id);
            held.item = item_id;
            println!(
                "VR_GAME throw_glove_prepare item={item_id} moby={id} ammo={}",
                saved.0.global.ammo[item]
            );
        }
    }
    let Some(id) = held.id else { return };
    if let Some((hand, velocity, _)) = release {
        let launch = solid_point(
            p,
            coll,
            hand,
            root,
            settings.units_per_metre,
            &mut recall.contact,
        );
        if !unobstructed(p, coll, eye, launch.translation) {
            cancel(p, &mut held);
            return;
        }
        // The original engine copies saved ammo into the hero before GameTick: update both mirrors now,
        // and let its normal post-tick path persist the use-ammo statistics once.
        let w = &mut p.game.hero.weapons;
        w.ammo[item] = saved.0.global.ammo[item];
        w.uses_ammo[item] = true;
        if saved.0.global.owned[item] == 0 || !w.use_ammo(item_id, 1) {
            cancel(p, &mut held);
            return;
        }
        saved.0.global.ammo[item] = w.ammo[item];
        w.throws += 1;
        // Tracking motion determines direction and strength. Locomotion is inherited only after a valid
        // release gesture, so moving the stick alone can never launch a bomb.
        let v = (root.rotation * velocity * (3.0 * settings.units_per_metre))
            .clamp_length_max(18.0 * settings.units_per_metre)
            / 60.0;
        let inherited = rc_game::hero::physics::to_f32x3(p.game.hero.disp);
        let v = game_point(v);
        let m = &mut p.game.mobys.mobys[id];
        pvar::set_v4f(
            &mut m.pvars,
            if item_id == 25 {
                decoy::pv::VEL
            } else {
                bomb::pv::VEL
            },
            [
                v[0] + inherited[0],
                v[1] + inherited[1],
                v[2] + inherited[2],
                0.0,
            ],
        );
        // The XR gesture supplies the aim; suppress the flat game's automatic landing-preview reticle.
        if item_id == BOMB_GLOVE {
            pvar::set_i16(&mut m.pvars, bomb::pv::FLAG56, 1);
        }
        match item_id {
            17 => mine::release(
                &mut p.game.mobys,
                id,
                &mut p.game.rng,
                game_point(launch.translation),
            ),
            20 => doom_canister::release(
                &mut p.game.mobys.mobys[id],
                game_point(launch.translation),
                None,
            ),
            25 => decoy::release(
                &mut p.game.mobys.mobys[id],
                game_point(launch.translation),
                None,
            ),
            _ => bomb::release(
                &mut p.game.mobys.mobys[id],
                game_point(launch.translation),
                None,
            ),
        }
        let m = &mut p.game.mobys.mobys[id];
        // Native thrown bombs keep their passive collider disabled; fly() performs the explicit
        // projectile sweep and explosion tests. Do not turn the bomb into a solid obstacle.
        m.mode &= !mode::NO_UPDATE;
        p.svc.build_matrix_in(&mut p.game.mobys, &*p.classes, id);
        p.game.hero.fx.item_voices.push(SoundCmd::Voice {
            index: rc_game::hero::weapons::THROW_VOICE,
            flags: 0,
        });
        held.id = None;
        println!(
            "VR_GAME throw_glove_throw item={item_id} moby={id} ammo={} velocity={v:?}",
            saved.0.global.ammo[item]
        );
    } else if motion.bomb.held {
        p.tracked_bomb_pose(id, game_point(pose.translation), rotation(&pose));
    } else {
        // Slow release cancels the preparation; ammunition is consumed only by an actual throw.
        cancel(p, &mut held);
    }
}

/// Follow the latest hand and post-tick hero position, including render frames with no simulation tick.
pub(super) fn follow_held(
    play: Option<ResMut<crate::gameplay::Play>>,
    settings: Res<Settings>,
    rig: Res<Rig>,
    motion: Res<Motion>,
    held: Res<HeldBomb>,
    mode: Res<crate::menu_render::MenuMode>,
    level: Res<crate::Level>,
    contacts: Res<super::solid_contact::Contacts>,
) {
    let (Some(mut p), Some(id), Some(hand)) = (play, held.id, motion.hands[1]) else {
        return;
    };
    if !gameplay(&mode) || held.level != crate::level_load::level_index() || !motion.bomb.held {
        return;
    }
    let Some(root) = rig.root_for(&p.game, settings.units_per_metre) else {
        return;
    };
    let mut pose = point(hand, root, settings.units_per_metre);
    pose.translation += contacts.right_shift;
    let Some(coll) = level.0.collision.as_ref() else {
        return;
    };
    let eye = rig.body_eye(&p.game);
    if unobstructed(&p, coll, eye, pose.translation) {
        p.tracked_bomb_pose(id, game_point(pose.translation), rotation(&pose));
    }
}
