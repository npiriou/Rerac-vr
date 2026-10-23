//! Headset facing drives native heading, locomotion frame and spatial audio independently of the authored camera.
use super::{Settings, game::Rig, game_input::Focused};
use bevy::prelude::*;
use bevy_mod_openxr::resources::OxrViews;
use rc_game::{follow_camera::CameraView, ps2v::Pf};

pub(super) struct OrientationPlugin;
impl Plugin for OrientationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            sync.after(super::traversal::TraversalUpdate)
                .before(super::bomb_glove::BombUpdate),
        );
    }
}
fn view(mut native: CameraView, pose: Transform) -> CameraView {
    let rows = crate::game_camera::game_rows(&pose)
        .map(|r| [Pf::f(r.x), Pf::f(r.y), Pf::f(r.z), Pf::ZERO]);
    let pos = crate::game_camera::game_eye(&pose);
    native.pos = [Pf::f(pos.x), Pf::f(pos.y), Pf::f(pos.z), Pf::ONE];
    native.rows = rows;
    native.euler = rc_game::moby_update::services::rows_euler(&[
        rows[0],
        rows[1],
        rows[2],
        [Pf::ZERO, Pf::ZERO, Pf::ZERO, Pf::ONE],
    ]);
    native
}
fn sync(
    play: Option<ResMut<crate::gameplay::Play>>,
    settings: Res<Settings>,
    rig: Res<Rig>,
    views: Option<Res<OxrViews>>,
    focus: Res<Focused>,
    mode: Res<crate::menu_render::MenuMode>,
) {
    let Some(mut play) = play else { return };
    let p = &mut *play;
    p.game.hero.tracked.view = None;
    p.game.hero.tracked.navigation = None;
    if settings.desktop_preview
        || !focus.0
        || !mode.free_movement()
        || !p.game.hero.tracked.enabled
        || p.death_pending
        || p.game.hero.health <= 0
        || crate::saves::front_end_active()
    {
        return;
    }
    let Some((head, root)) =
        super::game::pose(views.as_deref()).zip(rig.root_for(&p.game, settings.units_per_metre))
    else {
        return;
    };
    let head = root.mul_transform(head);
    let up = rig.surface_up();
    let forward = *head.forward();
    // Looking at the floor/ceiling must not erase or reverse the walking basis.
    let mut forward = forward - up * forward.dot(up);
    if forward.length_squared() < 0.01 {
        forward = *root.forward();
        forward -= up * forward.dot(up);
    }
    if forward.length_squared() < 0.01 {
        return;
    }
    let mut navigation = Transform::from_translation(head.translation);
    navigation.look_to(forward.normalize(), up);
    let native = p.game.camera.out;
    p.game.hero.tracked.view = Some(view(native, head));
    p.game.hero.tracked.navigation = Some(view(native, navigation));
    p.game.hero.tracked.view_origin = p.game.hero.position();
    // Apply before collision/ground probes too, including a stationary physical turn.
    p.game.hero.tracked_face();
}
