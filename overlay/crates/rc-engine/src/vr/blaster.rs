//! The original Blaster, mounted on the right controller and fired through the native weapon update.
use super::{
    Settings,
    embodied::{Motion, game_point, gameplay, unobstructed},
    game::Rig,
};
use bevy::prelude::*;
use rc_game::hero::{
    blaster::{BLASTER, CLASS},
    items::{HandItem, ItemData},
};

pub(super) struct BlasterPlugin;
impl Plugin for BlasterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (grant, aim)
                .chain()
                .after(super::embodied::WrenchUpdate)
                .before(crate::gameplay::GameTick),
        );
    }
}

fn grant(
    saved: Option<ResMut<crate::gameplay::Persistent>>,
    session: Option<ResMut<crate::gameplay::Session>>,
    mut failed: Local<bool>,
    mut filled: Local<bool>,
) {
    let (Some(mut saved), Some(mut session)) = (saved, session) else {
        return;
    };
    let weapons = [
        BLASTER as usize,
        rc_game::hero::pyrocitor::PYROCITOR as usize,
    ];
    if *failed || (*filled && weapons.iter().all(|&id| saved.0.global.owned[id] != 0)) {
        return;
    }
    let root = crate::level_load::extracted_root();
    let load = || -> anyhow::Result<rc_formats::save_game::ItemTables> {
        let elf = crate::disc_source::read_path(&root, &root.join("boot/SCUS_971.99"))?;
        let overlay =
            crate::disc_source::level_file(&root, crate::level_load::level_index(), "overlay.bin")?;
        Ok(rc_formats::save_game::ItemTables::load(&elf, &overlay)?)
    };
    match load() {
        Ok(tables) => {
            for id in weapons {
                if saved.0.global.owned[id] == 0 {
                    saved.0.give_item(id, false, &tables, &mut session.0);
                }
            }
            if !*filled {
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
                    "VR_GAME starting_max_ammo reserves={reserves:?} quick_select={:?}",
                    saved.0.global.quick_select
                );
            }
        }
        Err(e) => {
            *failed = true;
            error!("VR_GAME could not prepare starting weapons/ammo: {e:#}");
        }
    }
}

/// Place the original mesh using its authored muzzle frame (list 0, local barrel axis -Y).
/// Its muzzle sits 40 cm forward of the palm and 3 cm above it, along controller forward.
pub(super) fn place(item: &mut HandItem, hand: Transform, data: &ItemData) -> Option<Vec3> {
    let class = data.class(item.o_class)?;
    let chain = class.chains.first().filter(|c| !c.is_empty())?;
    let muzzle = *rc_formats::moby_anim::evaluate_chains(
        &class.anim,
        &item.anim,
        item.snapshot.as_ref(),
        &[chain.as_slice()],
    )
    .first()?;
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
) {
    let Some(mut play) = play else { return };
    let p = &mut *play;
    p.game.hero.weapons.blaster.tracked = !settings.desktop_preview;
    p.game.hero.weapons.blaster.tracked_aim = None;
    p.game.hero.weapons.pyro.tracked = !settings.desktop_preview;
    p.game.hero.weapons.pyro.tracked_aim = None;
    if settings.desktop_preview {
        return;
    }
    if !matches!(
        p.game.hero.items.slot.id,
        BLASTER | rc_game::hero::pyrocitor::PYROCITOR
    ) {
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
        return;
    }
    let Some((hand, root, coll)) = motion.hands[1]
        .zip(rig.root_for(&p.game, settings.units_per_metre))
        .zip(level.0.collision.as_ref())
        .map(|((h, r), c)| (h, r, c))
    else {
        return;
    };
    let hand = root.mul_transform(hand);
    let Some(mut item) = p
        .game
        .hero
        .items
        .slot
        .item
        .as_ref()
        .filter(|i| matches!(i.o_class, CLASS | rc_game::hero::pyrocitor::CLASS))
        .cloned()
    else {
        return;
    };
    let Some(data) = p.game.item_data.as_ref() else {
        return;
    };
    let Some(muzzle) = place(&mut item, hand, data) else {
        return;
    };
    let eye = crate::tfrag_render::game_to_bevy(p.game.hero.position()) + Vec3::Y * 1.6;
    if !unobstructed(p, coll, eye, hand.translation)
        || !unobstructed(p, coll, hand.translation, muzzle)
    {
        return;
    }
    p.game.hero.items.tracked_hand = Some((item.position, item.rows));
    let aim = Some((game_point(muzzle), game_point(*hand.forward())));
    if p.game.hero.items.slot.id == BLASTER {
        p.game.hero.weapons.blaster.tracked_aim = aim;
    } else {
        p.game.hero.weapons.pyro.tracked_aim = aim;
    }
}
