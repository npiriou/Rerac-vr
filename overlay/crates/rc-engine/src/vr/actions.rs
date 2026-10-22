//! OpenXR action setup follows the pinned bevy_oxr raw_actions example.
//! PICO-over-SteamVR is a test target, not an asserted native controller profile.
use std::ops::Deref;
use anyhow::Result;
use bevy::prelude::*;
use bevy_mod_openxr::{
    action_binding::{OxrSendActionBindings,OxrSuggestActionBinding},
    action_set_attaching::OxrAttachActionSet,
    action_set_syncing::{OxrActionSetSyncSet,OxrSyncActionSet},
    openxr_session_running,
    resources::OxrInstance,
    session::OxrSession,
};
use bevy_mod_xr::session::{XrPreDestroySession,XrSessionCreated,XrState,session_available};
use openxr::{Action,ActionSet,Path,Posef,Vector2f};

pub struct InputPlugin;
impl Plugin for InputPlugin {
    fn build(&self,app:&mut App) {
        app.init_resource::<Inputs>()
            .add_systems(Startup,create_actions.run_if(session_available))
            .add_systems(OxrSendActionBindings,suggest_bindings)
            .add_systems(XrSessionCreated,(attach_set,spawn_trackers))
            .add_systems(PreUpdate,request_sync.before(OxrActionSetSyncSet).run_if(openxr_session_running))
            .add_systems(PreUpdate,sample.after(OxrActionSetSyncSet))
            .add_systems(XrPreDestroySession,cleanup);
    }
}
#[derive(Resource)]
struct ControllerActions {
    set:ActionSet,left_grip:Action<Posef>,right_grip:Action<Posef>,left_aim:Action<Posef>,right_aim:Action<Posef>,
    fire:Action<f32>,teleport:Action<f32>,fire_click:Action<bool>,teleport_click:Action<bool>,
    turn:Action<Vector2f>,recenter:Action<bool>,
}
#[derive(Resource,Default,Debug)]
pub struct Inputs {
    pub fire:Option<f32>,pub teleport:Option<f32>,pub turn:Option<f32>,pub recenter:Option<f32>,
}
#[derive(Component,Clone,Copy,PartialEq,Eq,Debug)]
pub enum Tracked {Head,LeftGrip,RightGrip,LeftAim,RightAim}

fn make_actions(instance:&OxrInstance)->Result<ControllerActions> {
    let set=instance.create_action_set("rerac_vr_poc","ReRAC VR PoC",0)?;
    Ok(ControllerActions {
        left_grip:set.create_action("left_grip","Left grip pose",&[])?,
        right_grip:set.create_action("right_grip","Right grip pose",&[])?,
        left_aim:set.create_action("left_aim","Left aim pose",&[])?,
        right_aim:set.create_action("right_aim","Right aim pose",&[])?,
        fire:set.create_action("fire","Diagnostic shot",&[])?,
        teleport:set.create_action("teleport","Teleport aim",&[])?,
        fire_click:set.create_action("fire_click","Diagnostic shot select",&[])?,
        teleport_click:set.create_action("teleport_click","Teleport select",&[])?,
        turn:set.create_action("turn","Snap turn",&[])?,
        recenter:set.create_action("recenter","Reset to spawn",&[])?,set,
    })
}
fn fail(message:String,exit:&mut MessageWriter<AppExit>) {
    error!("VR_POC {message}");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(1).expect("nonzero constant")));
}
fn create_actions(instance:Res<OxrInstance>,mut commands:Commands,mut exit:MessageWriter<AppExit>) {
    match make_actions(&instance) {Ok(a)=>{commands.insert_resource(a);},Err(e)=>fail(format!("action creation failed: {e:#}"),&mut exit)}
}
fn attach_set(a:Option<Res<ControllerActions>>,mut messages:MessageWriter<OxrAttachActionSet>) {
    if let Some(a)=a {messages.write(OxrAttachActionSet(a.set.clone()));}
}
fn request_sync(a:Option<Res<ControllerActions>>,mut messages:MessageWriter<OxrSyncActionSet>) {
    if let Some(a)=a {messages.write(OxrSyncActionSet(a.set.clone()));}
}
fn suggest_bindings(a:Option<Res<ControllerActions>>,mut out:MessageWriter<OxrSuggestActionBinding>) {
    let Some(a)=a else {return;};
    let mut bind=|action,profile:&str,path:&str|{out.write(OxrSuggestActionBinding {
        action,interaction_profile:profile.into(),bindings:vec![path.into()],
    });};
    for profile in ["/interaction_profiles/oculus/touch_controller","/interaction_profiles/valve/index_controller",
        "/interaction_profiles/microsoft/motion_controller","/interaction_profiles/khr/simple_controller"] {
        bind(a.left_grip.as_raw(),profile,"/user/hand/left/input/grip/pose");
        bind(a.right_grip.as_raw(),profile,"/user/hand/right/input/grip/pose");
        bind(a.left_aim.as_raw(),profile,"/user/hand/left/input/aim/pose");
        bind(a.right_aim.as_raw(),profile,"/user/hand/right/input/aim/pose");
        if profile.ends_with("simple_controller") {
            bind(a.fire_click.as_raw(),profile,"/user/hand/right/input/select/click");
            bind(a.teleport_click.as_raw(),profile,"/user/hand/left/input/select/click");
        } else {
            bind(a.fire.as_raw(),profile,"/user/hand/right/input/trigger/value");
            bind(a.teleport.as_raw(),profile,"/user/hand/left/input/trigger/value");
            bind(a.turn.as_raw(),profile,"/user/hand/right/input/thumbstick");
            let recenter=if profile.ends_with("touch_controller") {"/user/hand/left/input/x/click"}
                else if profile.ends_with("index_controller") {"/user/hand/left/input/a/click"}
                else {"/user/hand/left/input/menu/click"};
            bind(a.recenter.as_raw(),profile,recenter);
        }
    }
}
fn spawn_trackers(a:Option<Res<ControllerActions>>,session:Res<OxrSession>,mut commands:Commands,
    mut meshes:ResMut<Assets<Mesh>>,mut materials:ResMut<Assets<StandardMaterial>>,mut exit:MessageWriter<AppExit>) {
    let Some(a)=a else {fail("No input actions at session creation".into(),&mut exit);return;};
    // XrSpace's required XrTracker component parents each entity to the backend's tracking root.
    for (marker,action) in [(Tracked::LeftGrip,&a.left_grip),(Tracked::RightGrip,&a.right_grip),
        (Tracked::LeftAim,&a.left_aim),(Tracked::RightAim,&a.right_aim)] {
        match session.create_action_space(action,Path::NULL,Isometry3d::IDENTITY) {
            Ok(space)=>{
                let mut e=commands.spawn((marker,space,Name::new(format!("VR {marker:?}"))));
                if matches!(marker,Tracked::LeftGrip|Tracked::RightGrip) {
                    e.insert((Mesh3d(meshes.add(Cuboid::new(0.07,0.09,0.14))),
                        MeshMaterial3d(materials.add(StandardMaterial {base_color:if marker==Tracked::LeftGrip {
                            Color::srgb(0.15,0.55,1.0)}else{Color::srgb(1.0,0.5,0.15)},unlit:true,..default()}))));
                }
            },
            Err(e)=>fail(format!("Cannot create {marker:?} space: {e}"),&mut exit),
        }
    }
    match session.create_reference_space(openxr::ReferenceSpaceType::VIEW,Isometry3d::IDENTITY) {
        Ok(space)=>{commands.spawn((Tracked::Head,space.0,Name::new("VR tracked head")));},
        Err(e)=>fail(format!("Cannot create head VIEW space: {e}"),&mut exit),
    }
}
fn cleanup(mut commands:Commands,query:Query<Entity,With<Tracked>>,mut inputs:ResMut<Inputs>) {
    for e in &query {commands.entity(e).despawn();}
    *inputs=Inputs::default();
}
fn sample(a:Option<Res<ControllerActions>>,session:Option<Res<OxrSession>>,state:Option<Res<XrState>>,
    instance:Option<Res<OxrInstance>>,mut inputs:ResMut<Inputs>,mut profiles:Local<[String;2]>) {
    *inputs=Inputs::default();
    if !matches!(state.as_deref(),Some(XrState::Running)) {return;}
    let (Some(a),Some(session))=(a,session) else {return;};
    let session=session.deref().deref();
    let float=|action:&Action<f32>|action.state(session,Path::NULL).ok().filter(|s|s.is_active).map(|s|s.current_state);
    let boolean=|action:&Action<bool>|action.state(session,Path::NULL).ok().filter(|s|s.is_active).map(|s|if s.current_state {1.0}else{0.0});
    let combine=|a:Option<f32>,b:Option<f32>|match (a,b) {(Some(a),Some(b))=>Some(a.max(b)),(a,b)=>a.or(b)};
    inputs.fire=combine(float(&a.fire),boolean(&a.fire_click));
    inputs.teleport=combine(float(&a.teleport),boolean(&a.teleport_click));
    inputs.turn=a.turn.state(session,Path::NULL).ok().filter(|s|s.is_active).map(|s|s.current_state.x);
    inputs.recenter=boolean(&a.recenter);
    if let Some(instance)=instance {
        for (i,hand) in ["/user/hand/left","/user/hand/right"].into_iter().enumerate() {
            if let Ok(path)=instance.string_to_path(hand) {
                if let Ok(profile)=session.current_interaction_profile(path) {
                    let text=instance.path_to_string(profile).unwrap_or_else(|_|"unbound".into());
                    if profiles[i]!=text {println!("VR_POC interaction_profile {hand}={text}");profiles[i]=text;}
                }
            }
        }
    }
}
