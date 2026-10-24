//! The manned turrets in first person (Gaspar's cannons, Hoven's turret, Batalia's anti-aircraft turret): the turret
//! turns toward where the right controller points (the gaze without it, the stick while it is pushed) at its own
//! speeds (`rc_game::moby_update::creature::turn::aim_speed`), the trigger fires (super::game_input) and the eye sits
//! at its seat, the view fixed while the turret turns beneath it (super::game).
use super::{
    Settings,
    embodied::{Motion, game_point},
    game::Rig,
};
use bevy::prelude::*;
use bevy_mod_openxr::resources::OxrViews;
use rc_game::moby_update::classes::units::{batalia_turret, gaspar_cannon, hoven_turret};

pub(super) struct TurretPlugin;
impl Plugin for TurretPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            aim.after(super::traversal::TraversalUpdate)
                .before(crate::gameplay::GameTick),
        );
    }
}

/// The turret Ratchet mans (hero state 0x32 without a flown ship) and its class.
pub(super) fn manned(p: &crate::gameplay::Play) -> Option<(usize, i16)> {
    let h = &p.game.hero;
    if h.mode != 0 || h.state != 0x32 || p.svc.vehicle.moby.is_some() {
        return None;
    }
    p.game.mobys.mobys.iter().enumerate().find_map(|(i, m)| {
        let manned = match m.o_class {
            c if c == hoven_turret::CLASSES[0] => m.state == 4,
            c if c == gaspar_cannon::CLASSES[0] || c == batalia_turret::CLASSES[0] => m.cmd == 1,
            _ => false,
        };
        manned.then_some((i, m.o_class))
    })
}

/// The eye at the manned turret's seat (Bevy space) and the turret's aim heading (Bevy, `game::heading` convention):
/// Gaspar's script camera target (its pivot), Hoven's (0.45 forward, 3 up on its axis), above Batalia's barrels.
pub(super) fn seat(p: &crate::gameplay::Play) -> Option<(Vec3, f32)> {
    let (id, class) = manned(p)?;
    let m = p.game.mobys.mobys.get(id)?;
    let rz = m.rotation[2];
    let local = |x: f32, y: f32, z: f32| {
        let (s, c) = rz.sin_cos();
        [m.position[0] + x * c - y * s, m.position[1] + x * s + y * c, m.position[2] + z]
    };
    // The turrets' forward: the model's +y for Gaspar's and Hoven's (aim yaw rot.z + π/2), +x for Batalia's.
    let (eye, yaw) = if class == gaspar_cannon::CLASSES[0] {
        (p.game.camera.script.target_pos, rz + std::f32::consts::FRAC_PI_2)
    } else if class == hoven_turret::CLASSES[0] {
        (local(0.0, 0.45, 3.0), rz + std::f32::consts::FRAC_PI_2)
    } else {
        (local(2.0, 0.0, 6.2), rz)
    };
    let forward = crate::tfrag_render::game_to_bevy([yaw.cos(), yaw.sin(), 0.0]);
    eye.iter()
        .all(|v| v.is_finite())
        .then(|| (crate::tfrag_render::game_to_bevy(eye), (-forward.x).atan2(-forward.z)))
}

/// Each tick while a turret is manned: the aim (world yaw, pitch up) the right controller points along, or the gaze.
fn aim(
    play: Option<ResMut<crate::gameplay::Play>>,
    motion: Res<Motion>,
    rig: Res<Rig>,
    settings: Res<Settings>,
    views: Option<Res<OxrViews>>,
) {
    let Some(mut play) = play else { return };
    let p = &mut *play;
    // The stick, pushed, steers natively.
    let stick = p.game.hero.stick.map(|x| x.to_f32());
    let aim = manned(p)
        .filter(|_| stick[0].hypot(stick[1]) < 0.3)
        .and_then(|_| {
            let root = rig.root_for(&p.game, settings.units_per_metre)?;
            let pose = motion.hands[1].or_else(|| super::game::pose(views.as_deref()))?;
            let d = game_point(root.rotation * *pose.forward());
            (d.iter().all(|v| v.is_finite()) && d[0].hypot(d[1]) > 0.05)
                .then(|| [d[1].atan2(d[0]), d[2].atan2(d[0].hypot(d[1]))])
        });
    p.game.hero.tracked.turret_aim = aim;
}
