//! Standing/seated PoC controls. Teleport is intentionally not Ratchet locomotion.
use bevy::{prelude::*,core_pipeline::tonemapping::{Tonemapping,DebandDither},render::view::Msaa};
use bevy_mod_openxr::spaces::OxrSpaceLocationFlags;
use bevy_mod_xr::{camera::{XrCamera,XrProjection},session::{XrPreDestroySession,XrTrackingRoot}};
use super::{actions::{Inputs,Tracked},core::{Rig,V3,TriggerGate,SnapGate,Edge,sphere_ray},
    scene::{SceneData,Target,HeadCover},from_v3,to_v3};

#[derive(Resource)]
pub struct InteractionState {
    rig:Rig,calibrated:bool,fire:TriggerGate,teleport:TriggerGate,recenter:TriggerGate,snap:SnapGate,
    last_head:V3,
    pub shots:u64,pub hits:u64,pub head_valid:bool,pub left_valid:bool,pub right_valid:bool,
}
impl Default for InteractionState {
    fn default()->Self {Self {rig:Rig {origin:V3::ZERO,yaw:0.0},calibrated:false,fire:TriggerGate::default(),
        teleport:TriggerGate::default(),recenter:TriggerGate::default(),snap:SnapGate::default(),last_head:V3::ZERO,
        shots:0,hits:0,head_valid:false,left_valid:false,right_valid:false}}
}
pub struct InteractionPlugin;
impl Plugin for InteractionPlugin {
    fn build(&self,app:&mut App) {
        app.init_resource::<InteractionState>()
            .add_systems(Update,(configure_views,interact,hand_visibility,exit_on_escape))
            .add_systems(XrPreDestroySession,reset_session);
    }
}
fn reset_session(mut s:ResMut<InteractionState>) {*s=InteractionState::default();}
fn exit_on_escape(keys:Res<ButtonInput<KeyCode>>,mut exit:MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {exit.write(AppExit::Success);}
}
fn configure_views(mut commands:Commands,mut query:Query<(Entity,&mut Projection),Added<XrCamera>>) {
    for (e,mut p) in &mut query {
        commands.entity(e).insert((Msaa::Off,Tonemapping::None,DebandDither::Disabled));
        if let Projection::Custom(ref mut p)=*p {
            if let Some(xr)=p.get_mut::<XrProjection>() {xr.near=0.03;}
        }
    }
}
fn valid(flags:&OxrSpaceLocationFlags)->bool {
    flags.pos_valid() && flags.rot_valid() && flags.pos_tracked() && flags.rot_tracked()
}
fn hand_visibility(mut query:Query<(&Tracked,&OxrSpaceLocationFlags,&mut Visibility)>) {
    for (kind,flags,mut v) in &mut query {
        if matches!(kind,Tracked::LeftGrip|Tracked::RightGrip) {
            *v=if valid(flags) {Visibility::Inherited} else {Visibility::Hidden};
        }
    }
}
fn aim(rig:Rig,t:&Transform)->(V3,V3) {
    let p=rig.world(to_v3(t.translation));
    let d=to_v3(Quat::from_rotation_y(rig.yaw)*(t.rotation*Vec3::NEG_Z));
    (p,d)
}
fn clear_aim(data:&SceneData,head:V3,hand:V3)->bool {
    if data.geometry.sphere_overlaps(hand,0.03) {return false;}
    let delta=hand-head; let length=delta.length();
    length<0.05 || data.geometry.ray(head,delta,length).is_none_or(|h|h.distance>=length-0.02)
}
fn align_to_spawn(s:&mut InteractionState,head:&Transform,spawn:V3) {
    let forward=head.rotation*Vec3::NEG_Z;
    let yaw=(-forward.x).atan2(-forward.z);
    s.rig.yaw=-yaw;
    s.rig.teleport(spawn,to_v3(head.translation));
    s.calibrated=true;
    s.fire.cancel();s.teleport.cancel();s.recenter.cancel();s.snap=SnapGate::default();
    println!("VR_POC recentered spawn_metres={:?} stage_head_height={:.3}",spawn.array(),head.translation.y);
}

#[allow(clippy::type_complexity,clippy::too_many_arguments)]
fn interact(
    data:Res<SceneData>,inputs:Res<Inputs>,keys:Res<ButtonInput<KeyCode>>,mut s:ResMut<InteractionState>,
    tracked:Query<(&Tracked,&Transform,&OxrSpaceLocationFlags),(Without<XrTrackingRoot>,Without<HeadCover>,Without<Target>)>,
    mut roots:Query<&mut Transform,(With<XrTrackingRoot>,Without<Tracked>,Without<HeadCover>,Without<Target>)>,
    mut covers:Query<(&mut Transform,&mut Visibility),(With<HeadCover>,Without<Tracked>,Without<XrTrackingRoot>,Without<Target>)>,
    mut targets:Query<(Entity,&Transform,&MeshMaterial3d<StandardMaterial>,&mut Target),(Without<Tracked>,Without<XrTrackingRoot>,Without<HeadCover>)>,
    mut materials:ResMut<Assets<StandardMaterial>>,mut gizmos:Gizmos,
) {
    let get=|wanted|tracked.iter().find(|(kind,_,flags)|**kind==wanted && valid(flags)).map(|(_,t,_)|*t);
    let head=get(Tracked::Head);let left=get(Tracked::LeftAim);let right=get(Tracked::RightAim);
    s.head_valid=head.is_some();s.left_valid=left.is_some();s.right_valid=right.is_some();
    let Some(head)=head else {
        s.fire.cancel();s.teleport.cancel();s.recenter.cancel();s.snap.update(None);
        for (mut t,mut v) in &mut covers {
            t.translation=from_v3(s.last_head);*v=if s.calibrated {Visibility::Visible}else{Visibility::Hidden};
        }
        return;
    };
    let Ok(mut root)=roots.single_mut() else {return;};
    let head_local=to_v3(head.translation);
    if !s.calibrated {align_to_spawn(&mut s,&head,data.spawn);}
    let recenter=s.recenter.update(inputs.recenter);
    if recenter==Edge::Pressed || keys.just_pressed(KeyCode::KeyR) {align_to_spawn(&mut s,&head,data.spawn);}
    let angle=s.snap.update(inputs.turn);
    if angle!=0.0 {s.rig.snap(angle,head_local);}
    let mut head_world=s.rig.world(head_local);
    let obstructed=data.geometry.sphere_overlaps(head_world,0.08);
    if obstructed {s.fire.cancel();s.teleport.cancel();}

    // Only a valid release, following a valid press, can execute a teleport.
    if let Some(left)=left.filter(|_|!obstructed) {
        let (origin,direction)=aim(s.rig,&left);
        let permitted=clear_aim(&data,head_world,origin);
        let destination=if permitted {data.geometry.teleport_destination(origin,direction,head_local.1)}else{None};
        let edge=s.teleport.update(if permitted {inputs.teleport}else{None});
        if s.teleport.held() {
            let stop=data.geometry.ray(origin,direction,12.0).map_or(origin+direction*12.0,|h|h.point);
            let colour=if destination.is_some() {Color::srgb(0.1,1.0,0.3)}else{Color::srgb(1.0,0.1,0.05)};
            gizmos.line(from_v3(origin),from_v3(stop),colour);
            if let Some(p)=destination {
                gizmos.line(from_v3(p+V3(-0.2,0.015,0.0)),from_v3(p+V3(0.2,0.015,0.0)),colour);
                gizmos.line(from_v3(p+V3(0.0,0.015,-0.2)),from_v3(p+V3(0.0,0.015,0.2)),colour);
            }
        }
        if edge==Edge::Released {
            if let Some(p)=destination {
                s.rig.teleport(p,head_local);head_world=s.rig.world(head_local);
                println!("VR_POC teleport_metres={:?}",p.array());
            }
        }
    } else {s.teleport.cancel();}

    if let Some(right)=right.filter(|_|!obstructed) {
        let (origin,direction)=aim(s.rig,&right);
        let permitted=clear_aim(&data,head_world,origin);
        let edge=s.fire.update(if permitted {inputs.fire}else{None});
        let mut distance=data.geometry.ray(origin,direction,50.0).map_or(50.0,|h|h.distance);
        let mut hit=None;
        if permitted {
            for (e,t,_,target) in &targets {
                if let Some(d)=sphere_ray(origin,direction,to_v3(t.translation),target.radius,distance) {
                    distance=d;hit=Some(e);
                }
            }
        }
        let end=origin+direction*distance;
        gizmos.line(from_v3(origin),from_v3(end),if permitted {Color::srgb(0.9,0.8,0.15)}else{Color::srgb(1.0,0.05,0.05)});
        if edge==Edge::Pressed {
            s.shots+=1;
            if let Some(e)=hit {
                if let Ok((_,_,material,mut target))=targets.get_mut(e) {
                    target.hits+=1;s.hits+=1;
                    if let Some(mut m)=materials.get_mut(&material.0) {m.base_color=Color::srgb(0.1,1.0,0.3);}
                    println!("VR_POC diagnostic_target_hit entity={e:?} distance_metres={distance:.3} total_hits={}",s.hits);
                }
            } else {println!("VR_POC diagnostic_shot_miss stop_metres={distance:.3}");}
        }
    } else {s.fire.cancel();}
    // No animated head bone, artificial pitch/roll or change to IPD is applied.
    root.translation=from_v3(s.rig.origin);root.rotation=Quat::from_rotation_y(s.rig.yaw);root.scale=Vec3::ONE;
    s.last_head=head_world;
    for (mut t,mut v) in &mut covers {
        t.translation=from_v3(head_world);
        *v=if data.geometry.sphere_overlaps(head_world,0.08) {Visibility::Visible}else{Visibility::Hidden};
    }
}
