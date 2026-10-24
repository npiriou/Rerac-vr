//! Tracked right-controller ray mapped to the original pause menu's pixel coordinates.
use super::{
    Settings,
    embodied::Motion,
    game::Rig,
    game_input::{Focused, HandInput},
    spatial_menu::{CONTENT_DEPTH, DEPTH, SpatialMenu},
};
use bevy::{
    camera::visibility::{NoFrustumCulling, RenderLayers},
    prelude::*,
    transform::TransformSystems,
};
use rc_game::menus::pause::pointer::{Input, Target};

pub(super) struct MenuPointerPlugin;
#[derive(Resource, Default)]
struct Visuals(Option<(Entity, Entity, Handle<StandardMaterial>)>);
impl Plugin for MenuPointerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Visuals>()
            .init_resource::<Focused>()
            .init_resource::<HandInput>()
            .add_systems(
                crate::level_switch::LevelUnload,
                crate::level_switch::reset::<Visuals>,
            )
            .add_systems(
                FixedUpdate,
                interact
                    .after(crate::gameplay::GameTick)
                    .before(crate::menu_render::MenuFrameSet),
            )
            .add_systems(PostUpdate, present.before(TransformSystems::Propagate));
    }
}
struct Ray {
    origin: Vec3,
    end: Vec3,
    point: Option<[f32; 2]>,
}
fn ray(
    menu: &SpatialMenu,
    rig: &Rig,
    play: &crate::gameplay::Play,
    settings: &Settings,
    motion: &Motion,
) -> Option<Ray> {
    let anchor = menu.anchor?;
    let hand = rig
        .root_for(&play.game, settings.units_per_metre)?
        .mul_transform(motion.hands[1]?);
    let origin = hand.translation + *hand.forward() * 0.025;
    let inv = anchor.to_matrix().inverse();
    let local = inv.transform_point3(origin);
    let direction = inv.transform_vector3(*hand.forward());
    let t = (-CONTENT_DEPTH - local.z) / direction.z;
    if direction.z >= -0.0001 || !t.is_finite() || t <= 0.0 || t > 12.0 {
        return Some(Ray {
            origin,
            end: origin + *hand.forward() * 2.0,
            point: None,
        });
    }
    let at = local + direction * t;
    let pixel = [
        (at.x / (DEPTH * 0.63) + 1.0) * 256.0,
        (1.0 - at.y / (DEPTH * 0.63 * 0.775)) * 208.0,
    ];
    let inside = pixel[0] >= 0.0 && pixel[0] < 512.0 && pixel[1] >= 0.0 && pixel[1] < 416.0;
    Some(Ray {
        origin,
        end: anchor.transform_point(Vec3::new(at.x, at.y, -CONTENT_DEPTH + 0.014)),
        point: inside.then_some(pixel),
    })
}
fn target(
    rt: &crate::menu_render::MenuRt,
    saved: &crate::gameplay::Persistent,
    point: Option<[f32; 2]>,
) -> Option<Target> {
    let point = point?;
    let make = |rect: [f32; 4], key| Target {
        page: 0,
        widget: u32::MAX,
        rect,
        action: rc_game::menus::pause::pointer::Action::Key(key),
        enabled: true,
    };
    // Match the actual native dialog labels, so memory-card and confirmation buttons
    // keep their measured positions and original Cross/Circle/Triangle semantics.
    for draw in &rt.draws {
        if let rc_game::menus::MenuDraw::Hud(rc_game::hud::Draw::Text {
            font, x, y, text, ..
        }) = draw
        {
            let key = [
                (0x524a, rc_game::pad::button::CROSS),
                (0x524e, rc_game::pad::button::CROSS),
                (0x524f, rc_game::pad::button::CIRCLE),
                (0x524b, rc_game::pad::button::TRIANGLE),
                (0x4fa8, rc_game::pad::button::TRIANGLE),
                (0x5249, rc_game::pad::button::CROSS),
                (0x5248, rc_game::pad::button::CIRCLE),
            ]
            .into_iter()
            .find(|(id, _)| rt.assets.msg(*id) == text.as_slice())
            .map(|(_, key)| key);
            if let Some(key) = key {
                let t = make(
                    [
                        *x as f32 - 8.0,
                        *y as f32 - 5.0,
                        rt.assets.width(*font, text) as f32 + 16.0,
                        26.0,
                    ],
                    key,
                );
                if t.contains(point) {
                    return Some(t);
                }
            }
        }
    }
    rt.menu
        .as_ref()
        .and_then(|m| {
            m.pointer_targets(&rt.assets, &saved.0)
                .into_iter()
                .rev()
                .find(|t| t.contains(point))
        })
        .or_else(|| {
            (crate::saves::front_end_active() && rt.menu.as_ref().is_none_or(|m| !m.active))
                .then(|| make([0.0, 0.0, 512.0, 416.0], rc_game::pad::button::START))
        })
}
#[allow(clippy::too_many_arguments)]
fn interact(
    mode: Res<crate::menu_render::MenuMode>,
    focus: Res<Focused>,
    settings: Res<Settings>,
    menu: Res<SpatialMenu>,
    rig: Res<Rig>,
    motion: Res<Motion>,
    play: Option<Res<crate::gameplay::Play>>,
    saved: Option<Res<crate::gameplay::Persistent>>,
    mut rt: Option<ResMut<crate::menu_render::MenuRt>>,
    mut input: ResMut<HandInput>,
    mut pad: ResMut<crate::input_map::PadFrame>,
) {
    let pressed = input.menu_click;
    let Some(mut rt) = rt.take() else { return };
    if let Some(m) = rt.menu.as_mut() {
        m.pointer = None;
    }
    if !matches!(
        mode.state.mode,
        rc_game::menus::mode::Mode::Menu | rc_game::menus::mode::Mode::Freeze
    ) || !focus.0
        || settings.desktop_preview
    {
        return;
    }
    input.menu_click = false;
    let (Some(play), Some(saved)) = (play, saved) else {
        return;
    };
    let Some(ray) = ray(&menu, &rig, &play, &settings, &motion) else {
        return;
    };
    let hit = target(&rt, &saved, ray.point);
    if let Some(t) = hit.filter(|t| t.widget == u32::MAX) {
        if pressed {
            if let rc_game::menus::pause::pointer::Action::Key(key) = t.action {
                pad.0.buttons |= key as u16;
            }
        }
        return;
    }
    if let Some(m) = rt.menu.as_mut() {
        m.pointer = Some(Input {
            target: hit,
            point: ray.point.unwrap_or([-1.0, -1.0]),
            pressed,
            held: input.menu_held,
        });
    }
}
#[allow(clippy::too_many_arguments)]
fn present(
    mut commands: Commands,
    mode: Res<crate::menu_render::MenuMode>,
    focus: Res<Focused>,
    settings: Res<Settings>,
    menu: Res<SpatialMenu>,
    rig: Res<Rig>,
    motion: Res<Motion>,
    play: Option<Res<crate::gameplay::Play>>,
    saved: Option<Res<crate::gameplay::Persistent>>,
    rt: Option<Res<crate::menu_render::MenuRt>>,
    mut visuals: ResMut<Visuals>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    vendor: Res<crate::interact_render::VendorRt>,
) {
    let active = (matches!(
        mode.state.mode,
        rc_game::menus::mode::Mode::Menu | rc_game::menus::mode::Mode::Freeze
    ) || mode.spatial_vendor)
        && focus.0
        && !settings.desktop_preview;
    let ray = play.as_ref().filter(|_| active).and_then(|p| {
        if mode.spatial_vendor {
            super::vendor::ray(&vendor, &rig, p, &settings, &motion).map(|(origin, end, hit)| Ray {
                origin,
                end,
                point: hit.map(|(_, uv)| uv.to_array()),
            })
        } else {
            ray(&menu, &rig, p, &settings, &motion)
        }
    });
    let Some(ray) = ray else {
        if let Some((beam, dot, _)) = &visuals.0 {
            commands.entity(*beam).insert(Visibility::Hidden);
            commands.entity(*dot).insert(Visibility::Hidden);
        }
        return;
    };
    let hit = rt
        .as_ref()
        .zip(saved.as_ref())
        .and_then(|(rt, s)| target(rt, s, ray.point));
    let color = if hit.is_some_and(|t| t.enabled)
        || ((mode.spatial_vendor || play.as_ref().is_some_and(|p| super::game::native_panel(p)))
            && ray.point.is_some())
    {
        Color::srgb(0.2, 0.95, 1.0)
    } else {
        Color::srgb(0.5, 0.55, 0.6)
    };
    if visuals.0.is_none() {
        let material = materials.add(StandardMaterial {
            base_color: color,
            unlit: true,
            ..default()
        });
        let beam = commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                MeshMaterial3d(material.clone()),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(super::game::PANEL_LAYER),
                Name::new("Pause controller pointer"),
            ))
            .id();
        let dot = commands
            .spawn((
                Mesh3d(meshes.add(Sphere::new(0.006))),
                MeshMaterial3d(material.clone()),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(super::game::PANEL_LAYER),
                Name::new("Pause pointer contact"),
            ))
            .id();
        visuals.0 = Some((beam, dot, material));
    }
    if let Some((beam, dot, material)) = &visuals.0 {
        let delta = ray.end - ray.origin;
        if delta.length() < 0.001 {
            return;
        }
        if let Some(mut m) = materials.get_mut(material) {
            if m.base_color != color {
                m.base_color = color;
            }
        }
        commands.entity(*beam).insert((
            Transform {
                translation: (ray.origin + ray.end) * 0.5,
                rotation: Quat::from_rotation_arc(Vec3::Z, delta.normalize()),
                scale: Vec3::new(0.0016, 0.0016, delta.length()),
            },
            Visibility::Inherited,
        ));
        commands.entity(*dot).insert((
            Transform::from_translation(ray.end),
            if ray.point.is_some() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
        ));
    }
}
