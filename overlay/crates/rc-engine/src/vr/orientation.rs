//! Headset facing drives native heading, locomotion frame and spatial audio independently of the authored camera.
//! The heading and the stick's frame follow the body (the head and both controllers, the default) or the gaze
//! (VrOptions::direction_body).
use super::{Settings, embodied::Motion, game::Rig, game_input::Focused};
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
/// The body's heading, estimated from the head and both controllers (tracking space): a weighted mean of the head's
/// heading, the shoulder line (perpendicular to left → right), the hands' midpoint ahead of the head and where the
/// controllers point; it follows past 15° and settles within 2° (a glance does not swing it), and the head never looks
/// more than 90° from it.
#[derive(Default)]
pub(super) struct BodyHeading {
    yaw: Option<f32>,
    turning: bool,
}
const HEAD_W: f32 = 0.75;
const SHOULDER_W: f32 = 2.0;
const REACH_W: f32 = 1.5;
const POINT_W: f32 = 0.5;
const START: f32 = 0.26;
const SETTLE: f32 = 0.035;
const FOLLOW_SECS: f32 = 0.15;
const NECK: f32 = std::f32::consts::FRAC_PI_2;
fn yaw_of(v: Vec3) -> Option<f32> {
    (v.x * v.x + v.z * v.z > 1e-6).then(|| (-v.x).atan2(-v.z))
}
fn dir_of(yaw: f32) -> Vec3 {
    Vec3::new(-yaw.sin(), 0.0, -yaw.cos())
}
fn wrap(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (a + PI).rem_euclid(TAU) - PI
}
fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
impl BodyHeading {
    /// The body's horizontal forward (tracking space) from the head and the controllers (tracking space).
    fn update(&mut self, head: Transform, hands: [Option<Transform>; 2], dt: f32) -> Vec3 {
        // A pitch-proof head heading: perpendicular to the head's right axis.
        let mut h = Vec3::Y.cross(*head.right());
        if h.length_squared() < 0.04 {
            h = head.forward().with_y(0.0);
        }
        let h = h.normalize_or(Vec3::NEG_Z);
        let head_yaw = yaw_of(h).unwrap_or(0.0);
        let target = match hands {
            [Some(l), Some(r)] => {
                let mut sum = h * HEAD_W;
                // The shoulder line: forward is perpendicular to left → right (not when the hands cross).
                let across = (r.translation - l.translation).with_y(0.0);
                let shoulder = Vec3::Y.cross(across).normalize_or_zero();
                if shoulder.dot(h) > -0.3 {
                    sum += shoulder * SHOULDER_W * smooth(0.2, 0.4, across.length());
                }
                // The hands' midpoint ahead of the head.
                let reach = ((l.translation + r.translation) * 0.5 - head.translation).with_y(0.0);
                if reach.normalize_or_zero().dot(h) > 0.0 {
                    sum += reach.normalize_or_zero() * REACH_W * smooth(0.08, 0.25, reach.length());
                }
                // Where the controllers point (a near-vertical controller adds little).
                for c in [l, r] {
                    sum += c.forward().with_y(0.0) * POINT_W;
                }
                yaw_of(sum).unwrap_or(head_yaw)
            }
            // A controller untracked: the head.
            _ => head_yaw,
        };
        let yaw = self.yaw.get_or_insert(target);
        let gap = wrap(target - *yaw);
        if gap.abs() > START {
            self.turning = true;
        } else if gap.abs() < SETTLE {
            self.turning = false;
        }
        if self.turning {
            *yaw = wrap(*yaw + gap * (1.0 - (-dt / FOLLOW_SECS).exp()));
        }
        let neck = wrap(head_yaw - *yaw);
        if neck.abs() > NECK {
            *yaw = wrap(head_yaw - neck.signum() * NECK);
        }
        dir_of(*yaw)
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
    (motion, options, time, mut body): (
        Res<Motion>,
        Option<Res<crate::render_settings::VrOptions>>,
        Res<Time>,
        Local<BodyHeading>,
    ),
) {
    let Some(mut play) = play else { return };
    let p = &mut *play;
    p.game.hero.tracked.view = None;
    p.game.hero.tracked.navigation = None;
    if settings.desktop_preview
        || !focus.0
        || (!(mode.free_movement() && p.game.hero.tracked.enabled) && !rig.cinematic_active())
        || p.death_pending
        || p.game.hero.health <= 0
        || crate::saves::front_end_active()
    {
        *body = default();
        return;
    }
    let Some((head_local, root)) =
        super::game::pose(views.as_deref()).zip(rig.root_for(&p.game, settings.units_per_metre))
    else {
        return;
    };
    let head = root.mul_transform(head_local);
    let native = p.game.camera.out;
    p.game.hero.tracked.view = Some(view(native, head));
    p.game.hero.tracked.view_origin = p.game.hero.position();
    if rig.cinematic_active() {
        // The listener rides with the pilot/scene actor; the authored actor keeps its own body pose.
        return;
    }
    let up = rig.surface_up();
    let forward = if options.as_ref().is_none_or(|o| o.direction_body) {
        root.rotation * body.update(head_local, motion.hands, time.delta_secs())
    } else {
        *body = default();
        *head.forward()
    };
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
    p.game.hero.tracked.navigation = Some(view(native, navigation));
    // Apply before collision/ground probes too, including a stationary physical turn.
    p.game.hero.tracked_face();
}
