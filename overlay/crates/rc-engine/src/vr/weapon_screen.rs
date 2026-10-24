//! The original missile camera is sampled only by the Visibomb's articulated monitor.
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::{NoFrustumCulling, RenderLayers},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    transform::TransformSystems,
};
pub(super) struct WeaponScreenPlugin;
#[derive(Resource, Default)]
struct Screen(Option<(Entity, Handle<StandardMaterial>)>);
impl Plugin for WeaponScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Screen>()
            .add_systems(
                crate::level_switch::LevelUnload,
                crate::level_switch::reset::<Screen>,
            )
            .add_systems(
                PostUpdate,
                present
                    .after(super::solid_contact::Presented)
                    .before(TransformSystems::Propagate),
            );
    }
}
#[allow(clippy::too_many_arguments)]
fn present(
    mut commands: Commands,
    play: Option<Res<crate::gameplay::Play>>,
    rig: Res<super::game::Rig>,
    settings: Res<super::Settings>,
    motion: Res<super::embodied::Motion>,
    support: Res<super::blaster::SupportGrip>,
    attach: Option<Res<crate::moby_attach::MobyAttach>>,
    frame: Res<crate::display::GameFrame>,
    mode: Res<crate::menu_render::MenuMode>,
    mut screen: ResMut<Screen>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let active = play.as_ref().is_some_and(|p| {
        p.game.hero.items.slot.id == 13 && crate::visibomb_view::missile_view(Some(p))
    }) && mode.free_movement()
        && !settings.desktop_preview;
    // Any missing piece (tracking, the item, its pose) hides the monitor rather than leaving it at its last pose.
    let transform = play
        .as_ref()
        .filter(|_| active)
        .and_then(|p| monitor_transform(p, &rig, &settings, &motion, &support, attach.as_deref()));
    let Some(transform) = transform else {
        if let Some((e, _)) = &screen.0 {
            commands.entity(*e).insert(Visibility::Hidden);
        }
        return;
    };
    if screen.0.is_none() {
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
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 1.0, 0.7),
            unlit: true,
            cull_mode: None,
            base_color_texture: Some(frame.image.clone()),
            ..default()
        });
        let entity = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                transform,
                NoFrustumCulling,
                RenderLayers::layer(super::game::PANEL_LAYER),
                Name::new("Visibomb live missile monitor"),
            ))
            .id();
        screen.0 = Some((entity, material));
    }
    if let Some((e, material)) = &screen.0 {
        if let Some(mut m) = materials.get_mut(material) {
            if m.base_color_texture.as_ref() != Some(&frame.image) {
                m.base_color_texture = Some(frame.image.clone());
            }
        }
        commands
            .entity(*e)
            .insert((transform, Visibility::Inherited));
    }
}

/// The monitor's quad on the held Visibomb's screen (its decorative screen vertices, skinned by joint 9).
fn monitor_transform(
    play: &crate::gameplay::Play,
    rig: &super::game::Rig,
    settings: &super::Settings,
    motion: &super::embodied::Motion,
    support: &super::blaster::SupportGrip,
    attach: Option<&crate::moby_attach::MobyAttach>,
) -> Option<Transform> {
    let root = rig.root_for(&play.game, settings.units_per_metre)?;
    let hand = motion.hands[1]?;
    let data = play.game.item_data.as_ref()?;
    let mut item = play.game.hero.items.slot.item.clone().filter(|i| i.o_class == 163)?;
    let hand = root.mul_transform(super::blaster::weapon_hand(hand, motion.hands[0], support));
    super::blaster::place(&mut item, hand, data, support.primary);
    if let Some(at) = attach.and_then(|a| a.tracked_position(item.o_class)) {
        item.position = at;
    }
    let class = data.class(163)?;
    let palette = rc_formats::moby_anim::evaluate_with_snapshot(
        &class.anim,
        &item.anim,
        item.snapshot.as_ref(),
    );
    let bone = palette.get(9)?;
    // Original decorative screen vertices, skinned by joint 9. Keep its folding/firing motion.
    let raw = [
        [2080.0, -2179.0, -588.0],
        [2080.0, -2179.0, 183.0],
        [1537.0, -1958.0, -588.0],
        [1537.0, -1958.0, 183.0],
    ];
    let pose = Mat4::from_cols_array_2d(bone);
    let axes = Mat3::from_cols_array_2d(&item.rows.map(|r| {
        [
            f32::from_bits(r[0]),
            f32::from_bits(r[1]),
            f32::from_bits(r[2]),
        ]
    }));
    let corners = raw.map(|v| {
        crate::tfrag_render::game_to_bevy(
            (Vec3::from_array(item.position)
                + axes * (pose.transform_point3(Vec3::from_array(v)) * (item.scale / 1024.0)))
                .to_array(),
        )
    });
    let a = corners[1] - corners[0];
    let b = corners[2] - corners[0];
    let mut normal = a.cross(b).normalize_or_zero();
    if normal.dot(hand.translation - corners[0]) < 0.0 {
        normal = -normal;
    }
    Some(Transform::from_matrix(Mat4::from_cols(
        a.extend(0.0),
        b.extend(0.0),
        normal.extend(0.0),
        (corners[0] + normal * 0.0008).extend(1.0),
    )))
}
