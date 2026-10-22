//! Feature-gated experimental VR terrain explorer and input test range.
//! Nothing in this module is a replacement for the campaign's simulation.
mod actions;
mod core;
mod diagnostics;
mod interaction;
mod scene;

use anyhow::{Context, Result, ensure};
use bevy::{prelude::*,render::pipelined_rendering::PipelinedRenderingPlugin};
use bevy_mod_openxr::{add_xr_plugins,resources::OxrSessionConfig};
use self::core::V3;

#[derive(Resource,Clone,Debug)]
pub struct Settings {
    pub scene:String,
    pub units_per_metre:f32,
    pub spawn_game:Option<[f32;3]>,
}
impl Settings {
    fn from_env()->Result<Self> {
        let scene=std::env::var("RC_VR_SCENE").unwrap_or_else(|_|"calibration".into());
        ensure!(matches!(scene.as_str(),"calibration"|"novalis"),"RC_VR_SCENE must be calibration or novalis");
        let units_per_metre=std::env::var("RC_VR_UNITS_PER_METRE").unwrap_or_else(|_|"1".into())
            .parse::<f32>().context("RC_VR_UNITS_PER_METRE must be a number")?;
        core::game_to_metres([0.0;3],units_per_metre).map_err(anyhow::Error::msg)?;
        let spawn_game=std::env::var("RC_VR_SPAWN_GAME").ok().map(|s|->Result<[f32;3]> {
            let values=s.split(',').map(|n|n.trim().parse::<f32>()).collect::<std::result::Result<Vec<_>,_>>()?;
            ensure!(values.len()==3 && values.iter().all(|n|n.is_finite()),"RC_VR_SPAWN_GAME must be x,y,z in game units");
            Ok([values[0],values[1],values[2]])
        }).transpose()?;
        Ok(Self {scene,units_per_metre,spawn_game})
    }
}
pub fn requested()->bool {std::env::var("RC_VR").is_ok_and(|s|s=="1")}
pub fn from_v3(v:V3)->Vec3 {Vec3::new(v.0,v.1,v.2)}
pub fn to_v3(v:Vec3)->V3 {V3(v.x,v.y,v.z)}

pub fn run()->Result<()> {
    let settings=Settings::from_env()?;
    println!("VR_POC experimental source candidate; upstream=89c87e98f9d8296187df2a6310c67b32144090d3");
    println!("VR_POC scene={} backend=Vulkan required_reference_space=STAGE",settings.scene);
    println!("VR_POC controls: left trigger teleport (release), right trigger diagnostic shot, right stick snap turn, left X/A recenter, Escape exit");
    println!("VR_POC calibration cube edge=1 metre; units_per_metre is provisional, not a canonical Ratchet scale");
    let data=scene::load(&settings)?;
    let background=data.background;
    let mut app=App::new();
    app.add_plugins(add_xr_plugins(DefaultPlugins.build()
        .disable::<PipelinedRenderingPlugin>()
        .set(WindowPlugin {primary_window:Some(Window {title:"ReRAC VR PoC — hardware validation pending".into(),
            resolution:(1000u32,700u32).into(),..default()}),..default()})
        .set(AssetPlugin {file_path:crate::asset_dir(),..default()})))
        .insert_resource(OxrSessionConfig::default())
        .insert_resource(ClearColor(background))
        .insert_resource(settings)
        .insert_resource(data)
        .add_plugins((actions::InputPlugin,interaction::InteractionPlugin,diagnostics::DiagnosticsPlugin))
        .add_systems(Startup,(scene::spawn_geometry,scene::spawn_targets).chain());
    let result=app.run();
    ensure!(matches!(result,AppExit::Success),"VR PoC exited with an error; inspect the timestamped log");
    Ok(())
}
