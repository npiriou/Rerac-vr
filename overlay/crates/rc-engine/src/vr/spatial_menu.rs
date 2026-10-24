//! Native pause primitives and animated models placed in the level, around a fixed opening pose.
use crate::hud_render::{Prim, Tex};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::{NoFrustumCulling, RenderLayers},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    transform::TransformSystems,
};

pub(super) struct SpatialMenuPlugin;
#[derive(Resource, Default)]
pub(crate) struct SpatialMenu {
    pub anchor: Option<Transform>,
    front_pose: Option<Transform>,
    socket: Option<usize>,
}
#[derive(Resource, Default)]
struct Surfaces {
    slots: Vec<(Entity, Handle<Mesh>, Handle<StandardMaterial>)>,
    textures: Vec<(Tex, bool, bool, u64, Handle<Image>)>,
    signature: Option<Vec<(Prim, bool)>>,
    active: usize,
}
pub(crate) const DEPTH: f32 = 2.2;
// Text clears the opaque monitor faces; previews remain in front of this plane.
pub(crate) const CONTENT_DEPTH: f32 = DEPTH - 0.035;
// The Trespasser puzzle: its centre at this fraction of the native hand-to-prong-tip line, raised above the tips so
// the background (±192 px, 0.27 units half-height) clears the tool. Measured on class 615: nothing of the lock lies
// between the eyes and the panel, about a metre away.
const PUZZLE_ALONG: f32 = 0.55;
const PUZZLE_RISE: f32 = 0.42;
const PUZZLE_NEAREST: f32 = 0.6;
pub(crate) fn point(x: f32, y: f32, depth: f32) -> Vec3 {
    Vec3::new(
        (x / 256.0 - 1.0) * DEPTH * 0.63,
        (1.0 - y / 208.0) * DEPTH * 0.63 * 0.775,
        -depth,
    )
}
impl Plugin for SpatialMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpatialMenu>()
            .init_resource::<Surfaces>()
            .add_systems(
                crate::level_switch::LevelUnload,
                (
                    crate::level_switch::reset::<SpatialMenu>,
                    crate::level_switch::reset::<Surfaces>,
                ),
            )
            .add_systems(Update, anchor.before(crate::menu_render::MenuPrims))
            .add_systems(PostUpdate, present.before(TransformSystems::Propagate));
    }
}
fn anchor(
    mode: Res<crate::menu_render::MenuMode>,
    mut menu: ResMut<SpatialMenu>,
    rig: Res<super::game::Rig>,
    play: Option<Res<crate::gameplay::Play>>,
    settings: Res<super::Settings>,
    views: Option<Res<bevy_mod_openxr::resources::OxrViews>>,
    attach: Option<Res<crate::moby_attach::MobyAttach>>,
) {
    if let Some(p) = play.as_ref().filter(|p| {
        mode.state.mode == rc_game::menus::mode::Mode::Gameplay && super::game::native_panel(p)
    }) {
        if let Some(item) = attach.as_ref().and_then(|a| {
            // Locate the puzzle at the final socket, not at the first reach key.
            a.socket_item(
                p,
                p.game.hero.items.slot.item.as_ref()?,
                p.game.hero.tracked.utility_target?,
                true,
            )
        }) {
            // The native hand and the prong tips of the plugged tool. The lock's upright plate stands about three
            // quarters of the way to the tips: the panel goes over the tool, in front of that plate.
            let hand = crate::tfrag_render::game_to_bevy(item.position);
            let tip = p
                .game
                .item_data
                .as_ref()
                .and_then(|data| super::gadget_socket::tool_tip(&item, data))
                .unwrap_or(hand);
            let target = p.game.hero.tracked.utility_target;
            if menu.socket != target || menu.anchor.is_none() {
                let eye = rig
                    .root_for(&p.game, settings.units_per_metre)
                    .zip(super::game::pose(views.as_deref()))
                    .map(|(root, head)| root.transform_point(head.translation))
                    .unwrap_or_else(|| crate::tfrag_render::game_to_bevy(p.game.hero.position()));
                let mut centre = hand.lerp(tip, PUZZLE_ALONG);
                centre.y = tip.y + PUZZLE_RISE;
                let mut forward = centre - eye;
                if forward.length() < PUZZLE_NEAREST {
                    centre = eye + forward.normalize_or(Vec3::NEG_Z) * PUZZLE_NEAREST;
                    forward = centre - eye;
                }
                // Facing the eyes, also when looking down at it.
                let rotation = Transform::IDENTITY
                    .looking_to(forward.normalize_or(Vec3::NEG_Z), Vec3::Y)
                    .rotation;
                menu.anchor = Some(Transform {
                    translation: centre + rotation * Vec3::Z * CONTENT_DEPTH * 0.27,
                    rotation,
                    scale: Vec3::splat(0.27),
                });
                menu.socket = target;
            }
            return;
        }
    }
    if menu.socket.take().is_some() {
        menu.anchor = None;
    }
    let active = matches!(
        mode.state.mode,
        rc_game::menus::mode::Mode::Menu | rc_game::menus::mode::Mode::Freeze
    ) && !settings.desktop_preview;
    if !active {
        menu.anchor = None;
        menu.front_pose = None;
        return;
    }
    if crate::saves::front_end_active() {
        // Keep boot/title UI fixed in physical tracking space even when the authored title
        // scene moves its camera or replaces the level parked behind the boot pictures.
        let tracked = *menu.front_pose.get_or_insert_with(|| {
            super::game::pose(views.as_deref())
                .unwrap_or_else(|| Transform::from_xyz(0.0, 1.6, 0.0))
        });
        menu.anchor = play
            .as_ref()
            .and_then(|p| rig.root_for(&p.game, settings.units_per_metre))
            .map(|root| {
                let mut world = root.mul_transform(tracked);
                world.scale = Vec3::ONE;
                world
            });
        return;
    }
    if menu.front_pose.take().is_some() {
        menu.anchor = None;
    }
    if menu.anchor.is_none() {
        if let Some(root) = play
            .as_ref()
            .and_then(|p| rig.root_for(&p.game, settings.units_per_metre))
        {
            let head = super::game::pose(views.as_deref())
                .unwrap_or_else(|| Transform::from_xyz(0.0, 1.6, 0.0));
            let mut world = root.mul_transform(head);
            // A menu stays put when looking around. Keep its upright direction relative to the current gravity.
            world.scale = Vec3::ONE;
            menu.anchor = Some(world);
        }
    }
}
#[derive(Clone, Copy)]
struct Vertex {
    p: Vec2,
    uv: Vec2,
}
fn clip(mut polygon: Vec<Vertex>, scissor: [i32; 4]) -> Vec<Vertex> {
    for (axis, edge, above) in [
        (0, scissor[0].max(0) as f32, true),
        (0, (scissor[1] + 1).min(512) as f32, false),
        (1, scissor[2].max(0) as f32, true),
        (1, (scissor[3] + 1).min(416) as f32, false),
    ] {
        let mut out = Vec::new();
        if polygon.is_empty() {
            return out;
        }
        let mut last = *polygon.last().unwrap();
        for v in &polygon {
            let inside = |q: Vertex| {
                if above {
                    q.p[axis] >= edge
                } else {
                    q.p[axis] <= edge
                }
            };
            if inside(last) != inside(*v) {
                let t = (edge - last.p[axis]) / (v.p[axis] - last.p[axis]);
                out.push(Vertex {
                    p: last.p.lerp(v.p, t),
                    uv: last.uv.lerp(v.uv, t),
                });
            }
            if inside(*v) {
                out.push(*v);
            }
            last = *v;
        }
        polygon = out;
    }
    polygon
}
#[allow(clippy::too_many_arguments)]
fn present(
    mut commands: Commands,
    menu: Res<SpatialMenu>,
    level: Res<crate::Level>,
    hook: Res<crate::hud_render::Hud2dHook>,
    pictures: Option<Res<crate::hud_images::HudImages>>,
    mut surfaces: ResMut<Surfaces>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    play: Option<Res<crate::gameplay::Play>>,
    mode: Res<crate::menu_render::MenuMode>,
) {
    let Some(anchor) = menu.anchor else {
        for (e, _, _) in &surfaces.slots {
            commands.entity(*e).insert(Visibility::Hidden);
        }
        surfaces.signature = None;
        return;
    };
    let Some(hud) = level.0.hud.as_ref() else {
        return;
    };
    let mut prims: Vec<_> = hook
        .prims
        .iter()
        .copied()
        .map(|p| (p, false))
        .chain(
            hook.statics
                .iter()
                .enumerate()
                .flat_map(|(pass, prims)| prims.iter().copied().map(move |p| (p, pass == 1))),
        )
        .collect();
    if let Some(p) = play.as_ref().filter(|p| {
        mode.state.mode == rc_game::menus::mode::Mode::Gameplay && super::game::native_panel(p)
    }) {
        // The native minigame only: its translucent grey frame, ring bands, nodes and beams (no HUD prompts).
        prims = p
            .svc
            .draw_callbacks
            .rings
            .iter()
            .map(|r| {
                (
                    Prim {
                        tex: Tex::Fx(r.fx),
                        pos: r.pos.map(|p| [p[0].round() as i32, p[1].round() as i32]),
                        uv: r.uv.map(|p| [p[0] as i32, p[1] as i32]),
                        rgba: r.rgba,
                        scissor: [0, 511, 0, 415],
                        repeat: r.repeat,
                        nearest: false,
                        boxed: false,
                        uv16: false,
                    },
                    false,
                )
            })
            .collect();
    }
    let revision = pictures.as_ref().map_or(0, |p| p.loads) ^ ((hud.lang as u64) << 32);
    let changed = surfaces.signature.as_ref() != Some(&prims)
        || surfaces
            .textures
            .iter()
            .any(|(t, _, _, r, _)| matches!(t, Tex::Dyn(_)) && *r != revision);
    if !changed {
        for (i, (e, _, _)) in surfaces.slots.iter().enumerate() {
            commands.entity(*e).insert((
                anchor,
                if i < surfaces.active {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ));
        }
        return;
    }
    // Keep contiguous native draw runs together: an entire line of glyphs is one mesh.
    // Per-primitive depth retains the native compositing order without hundreds of draw calls.
    type Batch = (
        Option<Handle<Image>>,
        bool,
        Vec<[f32; 3]>,
        Vec<[f32; 2]>,
        Vec<[f32; 4]>,
        Vec<u32>,
    );
    let mut batches: Vec<Batch> = Vec::new();
    for (order, (prim, additive)) in prims.iter().enumerate() {
        let rect = prim.rect();
        // Full-frame fades/snapshot darkening have no physical surface in the world.
        if !crate::saves::front_end_active()
            && rect[2] >= 510
            && rect[3] >= 414
            && prim.tex == Tex::None
        {
            continue;
        }
        let mut polygon = Vec::new();
        let mut positions = Vec::<[f32; 3]>::new();
        let mut uvs = Vec::<[f32; 2]>::new();
        let mut indices = Vec::<u32>::new();
        for tri in [[0, 1, 2], [2, 1, 3]] {
            polygon.clear();
            for k in tri {
                polygon.push(Vertex {
                    p: Vec2::from_array(prim.pos[k].map(|v| v as f32)),
                    uv: Vec2::from_array(prim.uv[k].map(|v| v as f32))
                        / if prim.uv16 { 16.0 } else { 1.0 },
                });
            }
            let clipped = clip(polygon.clone(), prim.scissor);
            let base = positions.len() as u32;
            for v in &clipped {
                positions
                    .push(point(v.p.x, v.p.y, CONTENT_DEPTH - order as f32 * 0.00001).to_array());
                uvs.push(v.uv.to_array());
            }
            for i in 1..clipped.len().saturating_sub(1) {
                indices.extend([base, base + i as u32, base + i as u32 + 1]);
            }
        }
        if indices.is_empty() {
            continue;
        }
        let texture = if prim.tex == Tex::None {
            None
        } else {
            let cached = surfaces
                .textures
                .iter()
                .find(|(t, repeat, add, r, _)| {
                    *t == prim.tex
                        && *repeat == prim.repeat
                        && *add == *additive
                        && (if matches!(t, Tex::Dyn(_)) {
                            *r == revision
                        } else {
                            (*r >> 32) == (revision >> 32)
                        })
                })
                .map(|(_, _, _, _, h)| h.clone());
            if let Some(h) = cached {
                Some(h)
            } else {
                let decoded = match prim.tex {
                    Tex::Frame(i) => hud
                        .frames
                        .get(i)
                        .map(|t| (t.width, t.height, t.rgba.clone())),
                    Tex::Fx(i) => hud
                        .fx
                        .get(i)
                        .and_then(Option::as_ref)
                        .map(|t| (t.width, t.height, t.rgba.clone())),
                    Tex::Dyn(i) => pictures
                        .as_ref()
                        .and_then(|p| p.rect(i).zip(images.get(&p.atlas)))
                        .and_then(|([x, y, w, h], a)| {
                            a.data.as_ref().map(|bytes| {
                                let mut out = Vec::new();
                                for row in y..y + h {
                                    let at = ((row * a.width() + x) * 4) as usize;
                                    out.extend_from_slice(&bytes[at..at + (w * 4) as usize]);
                                }
                                (w, h, out)
                            })
                        }),
                    Tex::None => None,
                };
                decoded.map(|(w, h, mut rgba)| {
                    for px in rgba.chunks_exact_mut(4) {
                        // Native noise uses FIX, independently of the sampled alpha.
                        px[3] = if *additive {
                            255
                        } else {
                            px[3].saturating_mul(2)
                        };
                    }
                    let mut image = Image::new(
                        Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                        TextureDimension::D2,
                        rgba,
                        TextureFormat::Rgba8UnormSrgb,
                        RenderAssetUsages::all(),
                    );
                    let mut sampler = bevy::image::ImageSamplerDescriptor::nearest();
                    if prim.repeat {
                        sampler.address_mode_u = bevy::image::ImageAddressMode::Repeat;
                        sampler.address_mode_v = bevy::image::ImageAddressMode::Repeat;
                    }
                    image.sampler = bevy::image::ImageSampler::Descriptor(sampler);
                    let handle = images.add(image);
                    surfaces.textures.retain(|(t, repeat, add, _, _)| {
                        *t != prim.tex || *repeat != prim.repeat || *add != *additive
                    });
                    surfaces.textures.push((
                        prim.tex,
                        prim.repeat,
                        *additive,
                        revision,
                        handle.clone(),
                    ));
                    handle
                })
            }
        };
        if prim.tex != Tex::None && texture.is_none() {
            continue;
        }
        if let Some((w, h)) = texture
            .as_ref()
            .and_then(|h| images.get(h))
            .map(|i| (i.width() as f32, i.height() as f32))
        {
            for uv in &mut uvs {
                uv[0] /= w;
                uv[1] /= h;
            }
        }
        let rgba = prim.rgba.to_le_bytes();
        let rgb = if texture.is_some() {
            rgba[..3]
                .iter()
                .map(|v| *v as f32 / 128.0)
                .collect::<Vec<_>>()
        } else {
            rgba[..3].iter().map(|v| *v as f32 / 255.0).collect()
        };
        if batches
            .last()
            .is_none_or(|b| b.0 != texture || b.1 != *additive)
        {
            batches.push((
                texture,
                *additive,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ));
        }
        let batch = batches.last_mut().unwrap();
        let base = batch.2.len() as u32;
        batch.4.extend(vec![
            [
                rgb[0],
                rgb[1],
                rgb[2],
                (rgba[3] as f32 / 128.0).min(1.0)
            ];
            positions.len()
        ]);
        batch.2.extend(positions);
        batch.3.extend(uvs);
        batch.5.extend(indices.into_iter().map(|i| i + base));
    }
    let used = batches.len();
    for (slot, (texture, additive, positions, uvs, colors, indices)) in
        batches.into_iter().enumerate()
    {
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all())
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_NORMAL,
                vec![[0.0, 0.0, 1.0]; positions.len()],
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
            .with_inserted_indices(Indices::U32(indices));
        if surfaces.slots.len() <= slot {
            let mesh_handle = meshes.add(mesh);
            let material = materials.add(StandardMaterial::default());
            let entity = commands
                .spawn((
                    Mesh3d(mesh_handle.clone()),
                    MeshMaterial3d(material.clone()),
                    anchor,
                    NoFrustumCulling,
                    RenderLayers::layer(super::game::PANEL_LAYER),
                    Name::new("Native spatial menu geometry"),
                ))
                .id();
            surfaces.slots.push((entity, mesh_handle, material));
        } else if let Some(mut old) = meshes.get_mut(&surfaces.slots[slot].1) {
            *old = mesh;
        }
        let (entity, _, material) = &surfaces.slots[slot];
        if let Some(mut m) = materials.get_mut(material) {
            // Transparent meshes sort by their bounds in each eye. Native text, clears and glass
            // require painter order instead; per-vertex depth alone cannot impose that order.
            // Bias spacing exceeds the canvas diagonal, giving both eyes the same stable order.
            let depth_bias = (slot + 1) as f32 * 8.0;
            let alpha_mode = if additive {
                AlphaMode::Add
            } else {
                AlphaMode::Blend
            };
            if m.base_color_texture != texture
                || !m.unlit
                || m.depth_bias != depth_bias
                || m.alpha_mode != alpha_mode
            {
                *m = StandardMaterial {
                    base_color: Color::WHITE,
                    base_color_texture: texture,
                    unlit: true,
                    alpha_mode,
                    depth_bias,
                    cull_mode: None,
                    ..default()
                };
            }
        }
        commands
            .entity(*entity)
            .insert((anchor, Visibility::Inherited));
    }
    for (e, _, _) in surfaces.slots.iter().skip(used) {
        commands.entity(*e).insert(Visibility::Hidden);
    }
    surfaces.active = used;
    surfaces.signature = Some(prims);
}
