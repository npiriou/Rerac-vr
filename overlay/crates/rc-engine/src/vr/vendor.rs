//! Original vendor targets on the animated monitor quads, with independent shopping/locomotion input.
use bevy::{
    asset::RenderAssetUsages,
    camera::{visibility::NoFrustumCulling, visibility::RenderLayers},
    math::Affine2,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    transform::TransformSystems,
};
use rc_game::{
    menus::{MenuInput, vendor::screens},
    pad::PadState,
};

pub(super) struct VendorPlugin;
#[derive(Resource, Default)]
struct Surfaces(Vec<(Entity, Handle<StandardMaterial>)>);
impl Plugin for VendorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Surfaces>()
            .add_systems(
                crate::level_switch::LevelUnload,
                crate::level_switch::reset::<Surfaces>,
            )
            .add_systems(
                FixedUpdate,
                input
                    .after(crate::gameplay::GameTick)
                    .before(crate::menu_render::MenuFrameSet),
            )
            .add_systems(
                PostUpdate,
                present
                    .after(crate::screen_canvas::apply)
                    .before(TransformSystems::Propagate),
            );
    }
}
fn input(
    pad: Option<Res<super::game_input::GamePad>>,
    mut vendor: ResMut<crate::interact_render::VendorRt>,
    mut previous: Local<PadState>,
) {
    let Some(pad) = pad.and_then(|p| p.vendor) else {
        *previous = PadState::default();
        vendor.spatial_input = None;
        return;
    };
    previous.update(Some(&pad.bytes()), false);
    vendor.spatial_input = Some(MenuInput::from_pad(&previous, true));
}

// The native canvas images cover 512x416; HUD targets may have the display's extra side margins.
fn uv(rect: [f32; 4], extra: f32) -> Affine2 {
    let [x, y, w, h] = rect;
    let width = 512.0 + extra * 2.0;
    Affine2::from_scale_angle_translation(
        Vec2::new(w / width, h / 416.0),
        0.0,
        Vec2::new((x + extra) / width, y / 416.0),
    )
}

#[allow(clippy::too_many_arguments)]
fn present(
    mut commands: Commands,
    vendor: Res<crate::interact_render::VendorRt>,
    mode: Res<crate::menu_render::MenuMode>,
    settings: Res<super::Settings>,
    hud: Query<
        &MaterialNode<crate::hud_render::HudComposite>,
        With<crate::hud_render::HudCompositeNode>,
    >,
    composites: Res<Assets<crate::hud_render::HudComposite>>,
    static_nodes: Query<&MaterialNode<crate::hud_render::HudStaticComposite>>,
    static_mats: Res<Assets<crate::hud_render::HudStaticComposite>>,
    images: Res<Assets<Image>>,
    mut surfaces: ResMut<Surfaces>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut reported: Local<Option<usize>>,
) {
    let hud = hud
        .iter()
        .next()
        .and_then(|h| composites.get(&h.0))
        .map(|m| m.image.clone());
    let statics = static_nodes
        .iter()
        .next()
        .and_then(|h| static_mats.get(&h.0))
        .map(|m| m.image.clone());
    let active = mode.spatial_vendor && vendor.spatial() && !settings.desktop_preview;
    if !active {
        *reported = None;
    }
    if active && *reported != Some(vendor.draw.screens.len()) {
        println!(
            "VR_GAME vendor_monitors screens={} hud={} static={} canvases={:?} stable_mesh=true",
            vendor.draw.screens.len(),
            hud.is_some(),
            statics.is_some(),
            vendor.spatial_canvases.each_ref().map(Option::is_some)
        );
        for sd in &vendor.draw.screens {
            println!(
                "VR_GAME vendor_monitor index={} rect={:?} corner={:?} edges={:?}/{:?}",
                sd.s, sd.placed.drawn, sd.placed.quad.c, sd.placed.quad.a, sd.placed.quad.b
            );
        }
        *reported = Some(vendor.draw.screens.len());
    }
    if active && surfaces.0.is_empty() {
        // Keep the geometry immutable after upload; only the posed transform and UV crop change.
        let mesh = meshes.add(
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all())
                .with_inserted_attribute(
                    Mesh::ATTRIBUTE_POSITION,
                    vec![
                        [0.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [1.0, 1.0, 0.0],
                    ],
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4])
                .with_inserted_attribute(
                    Mesh::ATTRIBUTE_UV_0,
                    vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
                )
                .with_inserted_indices(Indices::U32(vec![0, 1, 2, 2, 1, 3])),
        );
        for screen in 0..7 {
            for layer in 0..3 {
                let material = materials.add(StandardMaterial {
                    unlit: true,
                    cull_mode: None,
                    alpha_mode: if layer == 0 {
                        AlphaMode::Opaque
                    } else {
                        AlphaMode::Premultiplied
                    },
                    ..default()
                });
                let entity = commands
                    .spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::IDENTITY,
                        Visibility::Hidden,
                        NoFrustumCulling,
                        RenderLayers::layer(super::game::PANEL_LAYER),
                        Name::new(format!("Vendor model screen {screen} layer {layer}")),
                    ))
                    .id();
                surfaces.0.push((entity, material));
            }
        }
    }
    for (index, (entity, material)) in surfaces.0.iter().enumerate() {
        let (screen, layer) = (index / 3, index % 3);
        let sd = vendor
            .draw
            .screens
            .iter()
            .find(|s| s.s == screen)
            .filter(|_| active);
        let Some(sd) = sd else {
            commands.entity(*entity).insert(Visibility::Hidden);
            continue;
        };
        let canvas = match screen {
            screens::ITEM => vendor.spatial_canvases[0].clone(),
            screens::SALESMAN => vendor.spatial_canvases[1].clone(),
            screens::POPUP if vendor.draw.scene.popup_panel.is_some() => {
                vendor.spatial_canvases[2].clone()
            }
            _ => None,
        };
        let image = match layer {
            0 => canvas.clone().or(hud.clone()),
            1 if canvas.is_some() => hud.clone(),
            2 => statics.clone(),
            _ => None,
        };
        let Some(image) = image else {
            commands.entity(*entity).insert(Visibility::Hidden);
            continue;
        };
        let rect = sd.placed.drawn;
        if rect[2].abs() < 0.5 || rect[3].abs() < 0.5 {
            commands.entity(*entity).insert(Visibility::Hidden);
            continue;
        }
        let q = sd.placed.quad;
        let corners = q.corners().map(crate::tfrag_render::game_to_bevy);
        let mut normal = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .normalize_or_zero();
        if let Some(view) = vendor.draw.view {
            let front = crate::tfrag_render::game_to_bevy(view.eye) - corners[0];
            if normal.dot(front) < 0.0 {
                normal = -normal;
            }
        }
        let extra = if layer == 0 && canvas.is_some() {
            0.0
        } else {
            images
                .get(&image)
                .map_or(0.0, |i| (i.width() as f32 - 512.0) * 0.5)
        };
        let a = corners[1] - corners[0];
        let b = corners[2] - corners[0];
        if a.length_squared() < 1e-8 || b.length_squared() < 1e-8 || normal.length_squared() < 0.5 {
            commands.entity(*entity).insert(Visibility::Hidden);
            continue;
        }
        // The authored joints describe a plane INSIDE the monitor. Original sequence-3 mesh faces
        // sit 0.0224..0.0397 game units toward the viewer from it. The old 0.002 offset put all
        // content behind the opaque black face. Clear that face while retaining world depth tests.
        let origin = corners[0] + normal * (0.05 + layer as f32 * 0.001);
        let transform = Transform::from_matrix(Mat4::from_cols(
            a.extend(0.0),
            b.extend(0.0),
            normal.extend(0.0),
            origin.extend(1.0),
        ));
        if let Some(mut material) = materials.get_mut(material) {
            let crop = uv(rect, extra);
            if material.base_color_texture.as_ref() != Some(&image) {
                material.base_color_texture = Some(image);
            }
            if material.uv_transform != crop {
                material.uv_transform = crop;
            }
        }
        commands
            .entity(*entity)
            .insert((transform, Visibility::Inherited));
    }
}
