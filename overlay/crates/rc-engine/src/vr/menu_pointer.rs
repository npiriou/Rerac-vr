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
    rt.menu
        .as_ref()?
        .pointer_targets(&rt.assets, &saved.0)
        .into_iter()
        .rev()
        .find(|t| t.contains(point))
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
) {
    let pressed = std::mem::take(&mut input.menu_click);
    let Some(mut rt) = rt.take() else { return };
    if let Some(m) = rt.menu.as_mut() {
        m.pointer = None;
    }
    if mode.state.mode != rc_game::menus::mode::Mode::Menu || !focus.0 || settings.desktop_preview {
        return;
    }
    let (Some(play), Some(saved)) = (play, saved) else {
        return;
    };
    let Some(ray) = ray(&menu, &rig, &play, &settings, &motion) else {
        return;
    };
    let hit = target(&rt, &saved, ray.point);
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
) {
    let active =
        mode.state.mode == rc_game::menus::mode::Mode::Menu && focus.0 && !settings.desktop_preview;
    let ray = play
        .as_ref()
        .filter(|_| active)
        .and_then(|p| ray(&menu, &rig, p, &settings, &motion));
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
    let color = if hit.is_some_and(|t| t.enabled) {
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
