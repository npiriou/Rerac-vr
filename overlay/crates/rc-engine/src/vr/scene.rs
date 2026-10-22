//! Static, deliberately simplified terrain presentation for the VR integration PoC.
//! This is NOT the original renderer, gameplay collision or a complete Novalis scene.
//! The loader may decode other assets, but ties, shrubs, mobys, water, sky, animation
//! and the campaign are not presented or simulated by this app.

use std::collections::BTreeMap;
use anyhow::{Context, Result, ensure};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    mesh::PrimitiveTopology,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use super::core::{Bvh, Triangle, V3, game_to_metres};
use super::{Settings, from_v3};

#[derive(Default)]
struct Batch {
    positions: Vec<[f32; 3]>, normals: Vec<[f32; 3]>, uvs: Vec<[f32; 2]>, colours: Vec<[f32; 4]>,
    pixels: Vec<u8>, width: u32, height: u32, clamp_s: bool, clamp_t: bool,
}
impl Batch {
    fn triangle(&mut self, t: Triangle, uv: [[f32; 2]; 3], colours: [[f32; 4]; 3]) {
        let n=t.cross().normalized().unwrap_or(V3::UP).array();
        self.positions.extend([t.a.array(),t.b.array(),t.c.array()]);
        self.normals.extend([n;3]); self.uvs.extend(uv); self.colours.extend(colours);
    }
}
#[derive(Resource)]
pub struct SceneData {
    batches: Vec<Batch>,
    pub geometry: Bvh,
    pub spawn: V3,
    pub background: Color,
}

pub fn load(settings: &Settings) -> Result<SceneData> {
    if settings.scene == "calibration" { return Ok(calibration()); }
    ensure!(settings.scene == "novalis", "RC_VR_SCENE must be calibration or novalis");
    let root=crate::disc_source::startup();
    ensure!(crate::disc_source::game(&root) == crate::disc_source::Game::Rac1,
        "The PoC accepts RAC1 data only, not Going Commando");
    crate::level_load::set_level_index(1);
    let level=crate::level_load::load_level(&root,1).context("Loading Novalis through ReRAC's loader")?;
    let mut batches: BTreeMap<(i32,bool,bool), Batch>=BTreeMap::new();
    let mut triangles=Vec::new();
    for t in &level.tfrags {
        let list=if t.header.base_only!=0 {2} else {0};
        for tri in rc_formats::tfrag::tfrag_triangles(t,list).context("decoding terrain triangle strips")? {
            let ad=&t.ad_gifs[tri.ad_gif as usize];
            let key=(t.texture_index(tri.ad_gif as usize),ad.clamp.data_lo&1!=0,ad.clamp.data_hi&1!=0);
            let vi=[tri.a as usize,tri.b as usize,tri.c as usize];
            let p=vi.map(|i|game_to_metres(t.world_position(i),settings.units_per_metre));
            let t3=Triangle {a:p[0].map_err(anyhow::Error::msg)?,b:p[1].map_err(anyhow::Error::msg)?,c:p[2].map_err(anyhow::Error::msg)?};
            if !t3.valid() {continue;}
            let uv=vi.map(|i| {let v=t.vertex_info[i]; [v.s as f32/4096.0,v.t as f32/4096.0]});
            let colours=vi.map(|i| {
                let position=usize::try_from(t.vertex_info[i].vertex/2).ok();
                position.and_then(|p|t.rgba.get(p)).map_or([1.0;4],|c|
                    [c.r,c.g,c.b,c.a].map(|n|(n as f32/128.0).min(1.0)))
            });
            batches.entry(key).or_default().triangle(t3,uv,colours);
            triangles.push(t3);
        }
    }
    ensure!(!triangles.is_empty(),"Novalis yielded no terrain triangles");
    let geometry=Bvh::new(triangles);
    let spawn=if let Some(p)=settings.spawn_game {
        let p=game_to_metres(p,settings.units_per_metre).map_err(anyhow::Error::msg)?;
        let hit=geometry.ray(p+V3(0.0,0.5,0.0),-V3::UP,8.0)
            .context("RC_VR_SPAWN_GAME has no terrain within 8 metres below it")?;
        ensure!(hit.normal.1>0.85 && geometry.landing_clear(hit.point,1.8),"Requested spawn lacks a clear flat landing");
        hit.point
    } else {
        // A geometric inspection spawn, NOT Ratchet's canonical spawn. Reject obstructed candidates.
        let mut candidates:Vec<_>=geometry.triangles().iter().copied()
            .filter(|t|t.cross().normalized().is_some_and(|n|n.1.abs()>0.97))
            .collect();
        candidates.sort_by(|a,b|b.cross().length().total_cmp(&a.cross().length()));
        candidates.into_iter().map(|t|t.centroid())
            .find(|p|geometry.landing_clear(*p,1.8))
            .context("No clear inspection spawn; set RC_VR_SPAWN_GAME=x,y,z")?
    };
    let mut output=Vec::new();
    for ((index,clamp_s,clamp_t),mut batch) in batches {
        if let Some(tex)=usize::try_from(index).ok().and_then(|i|level.tfrag_lod.mips.get(i))
            .and_then(Option::as_ref).and_then(|mips|mips.first()) {
            batch.width=tex.width; batch.height=tex.height; batch.pixels=tex.rgba.clone();
        } else {
            eprintln!("VR_POC missing terrain texture {index}; using magenta diagnostic material");
            batch.width=1; batch.height=1; batch.pixels=vec![255,0,255,255];
        }
        batch.clamp_s=clamp_s; batch.clamp_t=clamp_t;
        output.push(batch);
    }
    println!("VR_POC scene=novalis terrain_triangles={} batches={} units_per_metre={} spawn_metres={:?}",
        geometry.triangles().len(),output.len(),settings.units_per_metre,spawn.array());
    println!("VR_POC terrain-only snapshot: no original enemies, weapons, scripts, audio, or character physics");
    Ok(SceneData {batches:output,geometry,spawn,background:Color::srgb_u8(level.background[0],level.background[1],level.background[2])})
}

fn box_triangles(centre:V3,size:V3)->Vec<Triangle> {
    let h=size*0.5;
    let v=[V3(-h.0,-h.1,-h.2),V3(h.0,-h.1,-h.2),V3(h.0,h.1,-h.2),V3(-h.0,h.1,-h.2),
        V3(-h.0,-h.1,h.2),V3(h.0,-h.1,h.2),V3(h.0,h.1,h.2),V3(-h.0,h.1,h.2)].map(|v|v+centre);
    [[0,2,1],[0,3,2],[4,5,6],[4,6,7],[0,1,5],[0,5,4],[3,7,6],[3,6,2],
        [0,4,7],[0,7,3],[1,2,6],[1,6,5]].map(|i|Triangle {a:v[i[0]],b:v[i[1]],c:v[i[2]]}).to_vec()
}
fn calibration()->SceneData {
    let mut batches=Vec::new(); let mut triangles=Vec::new();
    for (centre,size,pixels) in [
        (V3(0.0,-0.05,0.0),V3(16.0,0.1,16.0),[85,90,100,255]),
        (V3(-2.0,0.5,-3.0),V3(1.0,1.0,1.0),[40,130,210,255]),
        (V3(0.0,1.0,-7.0),V3(8.0,2.0,0.1),[100,100,110,255]),
    ] {
        let mut batch=Batch {width:1,height:1,pixels:pixels.to_vec(),..Default::default()};
        for t in box_triangles(centre,size) {
            batch.triangle(t,[[0.0,0.0],[1.0,0.0],[1.0,1.0]],[[1.0;4];3]); triangles.push(t);
        }
        batches.push(batch);
    }
    SceneData {batches,geometry:Bvh::new(triangles),spawn:V3::ZERO,background:Color::srgb(0.07,0.08,0.11)}
}

pub fn spawn_geometry(mut commands:Commands,mut data:ResMut<SceneData>,mut meshes:ResMut<Assets<Mesh>>,
    mut images:ResMut<Assets<Image>>,mut materials:ResMut<Assets<StandardMaterial>>) {
    for (i,b) in std::mem::take(&mut data.batches).into_iter().enumerate() {
        let mut image=Image::new_fill(Extent3d {width:b.width,height:b.height,depth_or_array_layers:1},
            TextureDimension::D2,&b.pixels,TextureFormat::Rgba8UnormSrgb,RenderAssetUsages::RENDER_WORLD);
        let wrap=|clamp|if clamp {ImageAddressMode::ClampToEdge} else {ImageAddressMode::Repeat};
        image.sampler=ImageSampler::Descriptor(ImageSamplerDescriptor {address_mode_u:wrap(b.clamp_s),
            address_mode_v:wrap(b.clamp_t),mag_filter:ImageFilterMode::Linear,min_filter:ImageFilterMode::Linear,..default()});
        let mesh=Mesh::new(PrimitiveTopology::TriangleList,RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION,b.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL,b.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0,b.uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR,b.colours);
        commands.spawn((Mesh3d(meshes.add(mesh)),MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture:Some(images.add(image)),unlit:true,cull_mode:None,alpha_mode:AlphaMode::Mask(0.5),..default()
        })),Transform::IDENTITY,NoFrustumCulling,Name::new(format!("VR terrain batch {i}"))));
    }
    println!("VR_POC geometry uploaded; stereo views are owned by OpenXR, not GameFrame");
}

#[derive(Component)]
pub struct Target {pub radius:f32,pub hits:u32}
#[derive(Component)]
pub struct HeadCover;

pub fn spawn_targets(mut commands:Commands,data:Res<SceneData>,mut meshes:ResMut<Assets<Mesh>>,
    mut materials:ResMut<Assets<StandardMaterial>>) {
    let eye=data.spawn+V3(0.0,1.5,0.0);
    for (i,d) in [V3(-0.35,0.0,-1.0),V3(0.0,0.15,-1.0),V3(0.35,-0.05,-1.0)].into_iter().enumerate() {
        let d=d.normalized().unwrap();
        let distance=data.geometry.ray(eye,d,6.0).map_or(4.0,|h|(h.distance-0.5).min(4.0));
        if distance<0.8 {warn!("No clear space for diagnostic target {i}"); continue;}
        commands.spawn((Mesh3d(meshes.add(Sphere::new(0.2))),MeshMaterial3d(materials.add(StandardMaterial {
            base_color:Color::srgb(1.0,0.3,0.06),unlit:true,..default()
        })),Transform::from_translation(from_v3(eye+d*distance)),Target {radius:0.2,hits:0},Name::new(format!("Diagnostic target {i}"))));
    }
    // Inward-visible shell: tracking continues while the scene is hidden; no camera push-back.
    commands.spawn((Mesh3d(meshes.add(Sphere::new(0.3))),MeshMaterial3d(materials.add(StandardMaterial {
        base_color:Color::BLACK,unlit:true,cull_mode:None,..default()
    })),Transform::from_translation(from_v3(eye)),Visibility::Hidden,NoFrustumCulling,HeadCover,Name::new("Head obstruction cover")));
}
