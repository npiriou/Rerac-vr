//! Explicit desktop rendering checks; never presented as tracked VR.
use std::sync::{Arc,atomic::{AtomicU64,Ordering}};
use bevy::{prelude::*,core_pipeline::tonemapping::{Tonemapping,DebandDither},
    render::{Render,RenderApp,RenderSystems,view::Msaa}};
use super::{Settings,from_v3,scene::SceneData};

#[derive(Resource,Clone,Default)]
struct RenderedFrames(Arc<AtomicU64>);
#[derive(Component,Default)]
struct PreviewCamera {yaw:f32,pitch:f32}

pub struct PreviewPlugin;
impl Plugin for PreviewPlugin {
    fn build(&self,app:&mut App) {
        let frames=RenderedFrames::default();
        app.insert_resource(frames.clone())
            .add_systems(Startup,spawn_camera)
            .add_systems(Update,(move_camera,smoke_test,exit_on_escape));
        app.sub_app_mut(RenderApp).insert_resource(frames)
            .add_systems(Render,count_frames.after(RenderSystems::Render));
    }
}
fn count_frames(frames:Res<RenderedFrames>) {frames.0.fetch_add(1,Ordering::Relaxed);}
fn spawn_camera(mut commands:Commands,data:Res<SceneData>,adapter:Res<bevy::render::renderer::RenderAdapterInfo>) {
    commands.spawn((Camera3d::default(),Projection::Perspective(PerspectiveProjection {
        near:0.03,far:2000.0,..default()
    }),Transform::from_translation(from_v3(data.spawn)+Vec3::Y*1.6),
        Msaa::Off,Tonemapping::None,DebandDither::Disabled,PreviewCamera::default(),Name::new("Desktop preview camera")));
    println!("VR_POC DESKTOP PREVIEW: WASD move, Q/E down/up, arrow keys look, R reset, Shift faster, Escape exit. No VR tracking or controller input is simulated.");
    println!("VR_POC desktop GPU={} backend={:?}",adapter.name,adapter.backend);
}
fn move_camera(keys:Res<ButtonInput<KeyCode>>,time:Res<Time>,data:Res<SceneData>,
    mut camera:Query<(&mut Transform,&mut PreviewCamera)>) {
    let Ok((mut t,mut camera))=camera.single_mut() else {return;};
    let dt=time.delta_secs().min(0.1);
    if keys.just_pressed(KeyCode::KeyR) {
        t.translation=from_v3(data.spawn)+Vec3::Y*1.6;
        *camera=PreviewCamera::default();
    }
    let axis=|positive,negative|i32::from(keys.pressed(positive)) as f32-i32::from(keys.pressed(negative)) as f32;
    camera.yaw+=axis(KeyCode::ArrowLeft,KeyCode::ArrowRight)*dt;
    camera.pitch=(camera.pitch+axis(KeyCode::ArrowUp,KeyCode::ArrowDown)*dt).clamp(-1.5,1.5);
    t.rotation=Quat::from_euler(EulerRot::YXZ,camera.yaw,camera.pitch,0.0);
    let movement=Vec3::new(axis(KeyCode::KeyD,KeyCode::KeyA),axis(KeyCode::KeyE,KeyCode::KeyQ),axis(KeyCode::KeyS,KeyCode::KeyW));
    let speed=if keys.pressed(KeyCode::ShiftLeft) {9.0} else {3.0};
    let displacement=t.rotation*movement.normalize_or_zero()*speed*dt;
    t.translation+=displacement;
}
fn smoke_test(settings:Res<Settings>,time:Res<Time<Real>>,frames:Res<RenderedFrames>,
    mut reported:Local<bool>,mut exit:MessageWriter<AppExit>) {
    let Some(seconds)=settings.preview_seconds else {return;};
    if *reported || time.elapsed_secs()<seconds {return;}
    let count=frames.0.load(Ordering::Relaxed);
    if count<30 && time.elapsed_secs()<seconds+30.0 {return;}
    *reported=true;
    if count>=30 {
        println!("VR_POC DESKTOP_SMOKE_PASS scene={} rendered_frames={count} elapsed_seconds={:.2}",settings.scene,time.elapsed_secs());
        exit.write(AppExit::Success);
    } else {
        error!("VR_POC DESKTOP_SMOKE_FAIL: only {count} render frames completed");
        exit.write(AppExit::error());
    }
}
fn exit_on_escape(keys:Res<ButtonInput<KeyCode>>,mut exit:MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {exit.write(AppExit::Success);}
}
