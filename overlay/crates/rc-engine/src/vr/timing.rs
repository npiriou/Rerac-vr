//! Per-stage frame timing of the headset loop, logged every 5 s as `VR_PERF`.
//! Timestamps only: no graphics or gameplay setting is changed. With pipelined rendering the main
//! and render columns run on different threads and overlap; without it they add up.
//! Columns are avg/p50/p95/max in milliseconds:
//! * `interval`: xrWaitFrame call to xrWaitFrame call (the headset frame actually achieved);
//! * `wait`: time blocked in xrWaitFrame (slack the runtime gave back);
//! * `main`: main world after xrWaitFrame (game ticks, visibility, game render preparation);
//! * `handoff`: end of the main world to the next xrWaitFrame (waiting for the render thread,
//!   extraction, window events);
//! * `render`: the render world's Render schedule (preparation, encoding, submit, xrEndFrame);
//! * `xr_end`: xrReleaseSwapchainImage + xrEndFrame inside it (SteamVR waits for the GPU there).
use bevy::{
    prelude::*,
    render::{Render, RenderApp, RenderSystems, pipelined_rendering::PipelinedRenderingPlugin},
};
use bevy_mod_openxr::{
    render::{OxrWaitFrameSystem, end_frame, release_image},
    resources::OxrFrameState,
};
use bevy_mod_xr::session::{XrFirst, XrRenderSystems};
use std::{
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

const REPORT_EVERY: Duration = Duration::from_secs(5);

pub struct FrameTimingPlugin;
impl Plugin for FrameTimingPlugin {
    fn build(&self, app: &mut App) {
        let shared = Shared::default();
        let pipelined = app.is_plugin_added::<PipelinedRenderingPlugin>();
        println!("VR_PERF pipelined_rendering={pipelined} (RC_VR_PIPELINED=0 disables it)");
        app.insert_resource(shared.clone())
            .insert_resource(Clock {
                pipelined,
                report: Instant::now(),
                frame: None,
                awake: None,
                main_end: None,
            })
            .add_systems(XrFirst, wait_start.before(OxrWaitFrameSystem))
            .add_systems(XrFirst, wait_end.after(OxrWaitFrameSystem))
            .add_systems(FixedUpdate, count_tick)
            .add_systems(Last, main_end);
        app.sub_app_mut(RenderApp)
            .insert_resource(shared)
            .init_resource::<RenderClock>()
            .add_systems(Render, render_start.in_set(RenderSystems::ExtractCommands))
            .add_systems(
                Render,
                (
                    xr_end_start.before(release_image),
                    xr_end_done.after(end_frame),
                )
                    .in_set(XrRenderSystems::PostRender),
            )
            .add_systems(Render, render_end.in_set(RenderSystems::PostCleanup));
    }
}

#[derive(Default)]
struct Samples {
    interval: Vec<f32>,
    wait: Vec<f32>,
    main: Vec<f32>,
    handoff: Vec<f32>,
    render: Vec<f32>,
    xr_end: Vec<f32>,
    ticks: u32,
}
#[derive(Resource, Clone, Default)]
struct Shared(Arc<Mutex<Samples>>);
impl Shared {
    fn lock(&self) -> MutexGuard<'_, Samples> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
#[derive(Resource)]
struct Clock {
    pipelined: bool,
    report: Instant,
    frame: Option<Instant>,
    awake: Option<Instant>,
    main_end: Option<Instant>,
}
#[derive(Resource, Default)]
struct RenderClock {
    start: Option<Instant>,
    xr_end: Option<Instant>,
}

fn ms(d: Duration) -> f32 {
    d.as_secs_f32() * 1e3
}

fn wait_start(mut clock: ResMut<Clock>, shared: Res<Shared>) {
    let now = Instant::now();
    let mut s = shared.lock();
    if let Some(end) = clock.main_end.take() {
        s.handoff.push(ms(now - end));
    }
    if let Some(previous) = clock.frame.replace(now) {
        s.interval.push(ms(now - previous));
    }
}
fn wait_end(mut clock: ResMut<Clock>, shared: Res<Shared>) {
    let now = Instant::now();
    if let Some(start) = clock.frame {
        shared.lock().wait.push(ms(now - start));
    }
    clock.awake = Some(now);
}
fn count_tick(shared: Res<Shared>) {
    shared.lock().ticks += 1;
}
fn main_end(mut clock: ResMut<Clock>, shared: Res<Shared>, frame: Option<Res<OxrFrameState>>) {
    let now = Instant::now();
    if let Some(awake) = clock.awake.take() {
        shared.lock().main.push(ms(now - awake));
    }
    clock.main_end = Some(now);
    if now - clock.report < REPORT_EVERY {
        return;
    }
    let elapsed = now - clock.report;
    clock.report = now;
    let mut s = std::mem::take(&mut *shared.lock());
    let frames = s.interval.len();
    let period = frame
        .map(|f| f.predicted_display_period.as_nanos() as f32 * 1e-6)
        .filter(|p| *p > 0.0);
    // Intervals counted in display periods: 1 is the headset's full rate.
    let mut vsyncs = [0u32; 4];
    if let Some(period) = period {
        for i in &s.interval {
            vsyncs[((i / period).round() as usize).clamp(1, 4) - 1] += 1;
        }
    }
    println!(
        "VR_PERF pipelined={} frames={frames} fps={:.1} period_ms={} vsyncs_1/2/3/4+={}/{}/{}/{} ticks_per_frame={:.2} | interval {} | wait {} | main {} | handoff {} | render {} | xr_end {}",
        clock.pipelined,
        frames as f32 / elapsed.as_secs_f32(),
        period.map_or("-".into(), |p| format!("{p:.2}")),
        vsyncs[0],
        vsyncs[1],
        vsyncs[2],
        vsyncs[3],
        s.ticks as f32 / frames.max(1) as f32,
        stats(&mut s.interval),
        stats(&mut s.wait),
        stats(&mut s.main),
        stats(&mut s.handoff),
        stats(&mut s.render),
        stats(&mut s.xr_end),
    );
}

fn render_start(mut clock: ResMut<RenderClock>) {
    clock.start = Some(Instant::now());
}
fn xr_end_start(mut clock: ResMut<RenderClock>) {
    clock.xr_end = Some(Instant::now());
}
fn xr_end_done(mut clock: ResMut<RenderClock>, shared: Res<Shared>) {
    if let Some(start) = clock.xr_end.take() {
        shared.lock().xr_end.push(ms(start.elapsed()));
    }
}
fn render_end(mut clock: ResMut<RenderClock>, shared: Res<Shared>) {
    if let Some(start) = clock.start.take() {
        shared.lock().render.push(ms(start.elapsed()));
    }
}

/// avg/p50/p95/max of one column.
fn stats(v: &mut [f32]) -> String {
    if v.is_empty() {
        return "-".into();
    }
    v.sort_by(f32::total_cmp);
    let avg = v.iter().sum::<f32>() / v.len() as f32;
    let at = |q: f32| v[((v.len() - 1) as f32 * q).round() as usize];
    format!(
        "{avg:.1}/{:.1}/{:.1}/{:.1}",
        at(0.5),
        at(0.95),
        v[v.len() - 1]
    )
}
