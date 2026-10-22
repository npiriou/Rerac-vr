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
        classes::bomb,
        services::{ClassData, pvar},
    },
};

pub(super) struct BombGlovePlugin;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct BombUpdate;
#[derive(Resource, Default)]
pub(super) struct HeldBomb {
    pub id: Option<usize>,
    level: u32,
    last_tick: u64,
}
impl Plugin for BombGlovePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeldBomb>().add_systems(
            FixedUpdate,
            update.in_set(BombUpdate).before(crate::gameplay::GameTick),
        );
    }
}

fn point(hand: Transform, root: Transform, scale: f32) -> Transform {
    let mut world = root.mul_transform(hand);
    // Put the original bomb just in front of the palm, rather than inside the glove mesh.
    world.translation += world.rotation * Vec3::new(0.0, 0.03, -0.12) * scale;
    world
}

/// The original Bomb Glove replaces the provisional right hand. Its authored palm (list 0)
/// follows the controller, with fingers forward (-Y in the original model) and palm up.
pub(super) fn appearance(
    hand: Transform,
    data: &rc_game::hero::items::ItemData,
) -> Option<rc_game::hero::items::HandItem> {
    let class = data.class(bomb::GLOVE_CLASS as i16)?;
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
    let axes = Mat3::from_cols(frame[1], -frame[0], frame[2]) * basis.inverse();
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
fn is_held(p: &crate::gameplay::Play, id: usize) -> bool {
    p.game.mobys.mobys.get(id).is_some_and(|m| {
        m.o_class == bomb::BOMB_CLASS && m.state == bomb::HELD && m.mode & mode::NO_UPDATE != 0
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
    if matches!(
        p.game.hero.items.slot.id,
        rc_game::hero::blaster::BLASTER | rc_game::hero::pyrocitor::PYROCITOR
    ) || matches!(
        p.game.hero.items.target,
        rc_game::hero::blaster::BLASTER | rc_game::hero::pyrocitor::PYROCITOR
    ) {
        cancel(p, &mut held);
        // Consume this press without resetting the held edge: changing weapons with the trigger down
        // cannot prepare a bomb until a fresh trigger press.
        motion.bomb.press = None;
        motion.bomb.release = None;
        return;
    }
    let ready = gameplay(&mode)
        && p.game.hero.mode == 0
        && p.game.hero.health > 0
        && !p.death_pending
        && !p.game.hero.hand_hidden();
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
    let pose = point(hand, root, settings.units_per_metre);
    let eye = crate::tfrag_render::game_to_bevy(p.game.hero.position()) + Vec3::Y * 1.6;
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
    let item = BOMB_GLOVE as usize;
    if held.id.is_none() && press.is_some() && (motion.bomb.held || release.is_some()) {
        if saved.0.global.owned[item] == 0 || saved.0.global.ammo[item] <= 0 {
            empty(p, game_point(pose.translation));
            return;
        }
        if p.classes.info(bomb::BOMB_CLASS).is_none() {
            return;
        }
        if let Some(id) = bomb::create_from_hero(
            &mut p.game.mobys,
            &mut p.svc,
            &*p.classes,
            bomb::BOMB_CLASS,
            p.game.counter,
        ) {
            let m = &mut p.game.mobys.mobys[id];
            bomb::init_held(
                m,
                &mut p.game.rng,
                game_point(pose.translation),
                p.game.hero.rot[2].to_f32(),
            );
            // While held, the XR hand owns the pose. Suspend the original update (including its glove-slot
            // deletion check); release restores the native flight, fuse, collisions, explosion and damage.
            m.mode |= mode::NO_UPDATE;
            m.coll_disable = 1;
            m.glow = 0xff9f_9f9f;
            held.id = Some(id);
            println!(
                "VR_GAME bomb_glove_prepare moby={id} ammo={}",
                saved.0.global.ammo[item]
            );
        }
    }
    let Some(id) = held.id else { return };
    if let Some((hand, velocity, _)) = release {
        let launch = point(hand, root, settings.units_per_metre);
        if !unobstructed(p, coll, eye, launch.translation) {
            cancel(p, &mut held);
            return;
        }
        // The original engine copies saved ammo into the hero before GameTick: update both mirrors now,
        // and let its normal post-tick path persist the use-ammo statistics once.
        let w = &mut p.game.hero.weapons;
        w.ammo[item] = saved.0.global.ammo[item];
        w.uses_ammo[item] = true;
        if saved.0.global.owned[item] == 0 || !w.use_ammo(BOMB_GLOVE, 1) {
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
            bomb::pv::VEL,
            [
                v[0] + inherited[0],
                v[1] + inherited[1],
                v[2] + inherited[2],
                0.0,
            ],
        );
        // The XR gesture supplies the aim; suppress the flat game's automatic landing-preview reticle.
        pvar::set_i16(&mut m.pvars, bomb::pv::FLAG56, 1);
        bomb::release(m, game_point(launch.translation), None);
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
            "VR_GAME bomb_glove_throw moby={id} ammo={} velocity={v:?}",
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
    let pose = point(hand, root, settings.units_per_metre);
    let Some(coll) = level.0.collision.as_ref() else {
        return;
    };
    let eye = crate::tfrag_render::game_to_bevy(p.game.hero.position()) + Vec3::Y * 1.6;
    if unobstructed(&p, coll, eye, pose.translation) {
        p.tracked_bomb_pose(id, game_point(pose.translation), rotation(&pose));
    }
}
