//! CPU-side application-loop intervals only. These are NOT GPU/compositor frame timings.
use bevy::prelude::*;
use bevy_mod_openxr::resources::{OxrFrameState,OxrCurrentSessionConfig};
use bevy_mod_xr::camera::XrCamera;
use std::time::Instant;
use super::{actions::Inputs,interaction::InteractionState};

pub struct DiagnosticsPlugin;
impl Plugin for DiagnosticsPlugin {
    fn build(&self,app:&mut App) {app.init_resource::<SampleWindow>().add_systems(Last,report);}
}
#[derive(Resource)]
struct SampleWindow {last:Instant,start:Instant,intervals:Vec<f64>,warned_views:bool}
impl Default for SampleWindow {
    fn default()->Self {let now=Instant::now();Self {last:now,start:now,intervals:Vec::new(),warned_views:false}}
}
fn percentile(sorted:&[f64],p:f64)->f64 {
    if sorted.is_empty() {return 0.0;}
    sorted[((sorted.len()-1) as f64*p).round() as usize]
}
fn report(mut samples:ResMut<SampleWindow>,xr:Option<Res<OxrFrameState>>,config:Option<Res<OxrCurrentSessionConfig>>,
    views:Query<&XrCamera>,state:Res<InteractionState>,inputs:Res<Inputs>) {
    let now=Instant::now();let elapsed=(now-samples.last).as_secs_f64()*1000.0;samples.last=now;
    samples.intervals.push(elapsed);
    if now.duration_since(samples.start).as_secs_f64()<5.0 {return;}
    samples.intervals.sort_by(f64::total_cmp);
    let hz=xr.as_ref().map(|f|f.predicted_display_period.as_nanos()).filter(|n|*n>0).map(|n|1e9/n as f64);
    let over_budget=hz.map(|h|samples.intervals.iter().filter(|t|**t>1000.0/h).count());
    println!("VR_POC metrics views={} runtime_hz={hz:?} eye_resolution={:?} loop_ms_p50={:.3} loop_ms_p95={:.3} loop_ms_p99={:.3} loop_intervals_over_budget={over_budget:?} samples={} head={} left={} right={} fire_bound={} teleport_bound={} turn_bound={} shots={} hits={}",
        views.iter().count(),config.as_ref().map(|c|c.resolution),percentile(&samples.intervals,0.5),
        percentile(&samples.intervals,0.95),percentile(&samples.intervals,0.99),samples.intervals.len(),
        state.head_valid,state.left_valid,state.right_valid,inputs.fire.is_some(),inputs.teleport.is_some(),inputs.turn.is_some(),state.shots,state.hits);
    if views.iter().count()!=2 && !samples.warned_views {
        warn!("VR_POC stereo not established: expected 2 XR views. This is not a successful headset test.");samples.warned_views=true;
    }
    samples.intervals.clear();samples.start=now;
}
