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
//! * `prep`: inside it, the Render schedule's start to the render graph (phases, batches, buffers);
//! * `graph`: inside it, the render graph (every camera's encoding, submission, window present);
//! * `xr_pre`: inside it, xrBeginFrame, swapchain image acquire and xrWaitSwapchainImage (with
//!   a deferred xrEndFrame, the previous frame's xrEndFrame too, see vr/deferred_end.rs);
//! * `xr_end`: inside it, xrReleaseSwapchainImage + xrEndFrame (SteamVR waits for the GPU there);
//! * `spec`: per frame, the 3D views with meshes, their visible meshes, the meshes newly visible in a
//!   view and the meshes Bevy marked changed: the last two are what pipeline specialization redoes;
//! * `eye_draws`: per world eye, the draw calls of the opaque, alpha-mask and transparent phases
//!   (entities batched into them in brackets).
use bevy::{
    core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        camera::DirtySpecializations,
        pipelined_rendering::PipelinedRenderingPlugin,
        render_phase::{
            BinnedPhaseItem, BinnedRenderPhase, PhaseItem, ViewBinnedRenderPhases,
            ViewSortedRenderPhases,
        },
        view::{ExtractedView, RenderVisibleEntities},
    },
};
use bevy_mod_openxr::{
    render::OxrWaitFrameSystem,
    resources::OxrFrameState,
};
use bevy_mod_xr::session::{XrFirst, XrRenderSystems};
use super::deferred_end::DeferredSystems;
use std::{
    sync::{Arc, Mutex, MutexGuard, PoisonError, atomic::Ordering},
    time::{Duration, Instant},
};

const REPORT_EVERY: Duration = Duration::from_secs(5);

/// `xr`: the headset loop (xrWaitFrame/xrEndFrame columns); otherwise the desktop preview, whose
/// frames start in `First` and have no XR columns.
pub struct FrameTimingPlugin {
    pub xr: bool,
}
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
            .add_systems(FixedUpdate, count_tick)
            .add_systems(Last, main_end);
        if self.xr {
            app.add_systems(XrFirst, wait_start.before(OxrWaitFrameSystem))
                .add_systems(XrFirst, wait_end.after(OxrWaitFrameSystem));
        } else {
            app.add_systems(First, (wait_start, wait_end).chain());
        }
        let render = app.sub_app_mut(RenderApp);
        render
            .insert_resource(shared)
            .init_resource::<RenderClock>()
            .add_systems(Render, render_start.in_set(RenderSystems::ExtractCommands))
            .add_systems(Render, specialization_load.in_set(RenderSystems::PrepareMeshes))
            .add_systems(Render, draw_load.in_set(RenderSystems::PrepareBindGroups))
            .add_systems(
                Render,
                (
                    graph_start.before(bevy::render::renderer::render_system),
                    graph_end.after(bevy::render::renderer::render_system),
                )
                    .in_set(RenderSystems::Render),
            )
            .add_systems(Render, render_end.in_set(RenderSystems::PostCleanup));
        if self.xr {
            render.add_systems(
                Render,
                (
                    xr_end_start
                        .after(RenderSystems::Render)
                        .before(DeferredSystems::Post)
                        .before(XrRenderSystems::PostRender),
                    xr_end_done
                        .after(DeferredSystems::Post)
                        .after(XrRenderSystems::PostRender)
                        .before(RenderSystems::Cleanup),
                    xr_pre_start
                        .after(XrRenderSystems::HandleEvents)
                        .before(DeferredSystems::Pre)
                        .before(XrRenderSystems::PreRender),
                    xr_pre_done
                        .after(DeferredSystems::Pre)
                        .after(XrRenderSystems::PreRender)
                        .before(RenderSystems::PrepareViews)
                        .before(RenderSystems::PrepareAssets),
                ),
            );
        }
    }
}

#[derive(Default)]
struct Samples {
    interval: Vec<f32>,
    wait: Vec<f32>,
    main: Vec<f32>,
    handoff: Vec<f32>,
    render: Vec<f32>,
    prep: Vec<f32>,
    graph: Vec<f32>,
    xr_pre: Vec<f32>,
    xr_end: Vec<f32>,
    ticks: u32,
    spec_frames: u32,
    views: u64,
    visible: u64,
    added: u64,
    changed: u64,
    eye_views: u64,
    /// Opaque, alpha-mask, transparent: draw calls and entities.
    eye_draws: [u64; 3],
    eye_entities: [u64; 3],
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
    graph: Option<Instant>,
    xr_pre: Option<Instant>,
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
fn main_end(
    mut clock: ResMut<Clock>,
    shared: Res<Shared>,
    frame: Option<Res<OxrFrameState>>,
    cameras: Query<(&Camera, Option<&Name>, Has<Camera3d>)>,
    early: Option<Res<super::early_submit::EarlySubmit>>,
    deferred: Option<Res<super::deferred_end::DeferredEnd>>,
) {
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
    let per = |n: u64| n as f32 / s.spec_frames.max(1) as f32;
    // This window's early submission state; an A/B run flips it for the next window.
    let early_on = early.as_ref().is_some_and(|e| e.on.load(Ordering::Relaxed));
    if let Some(e) = early.as_ref().filter(|e| e.alternate) {
        e.on.store(!early_on, Ordering::Relaxed);
    }
    let deferred_on = deferred.as_ref().is_some_and(|d| d.on.load(Ordering::Relaxed));
    if let Some(d) = deferred.as_ref().filter(|d| d.alternate) {
        d.on.store(!deferred_on, Ordering::Relaxed);
    }
    let spec = format!(
        "views={:.1} visible={:.0} added={:.0} changed={:.0}",
        per(s.views),
        per(s.visible),
        per(s.added),
        per(s.changed)
    );
    let eye = |n: u64| n as f32 / s.eye_views.max(1) as f32;
    let draws = format!(
        "opaque={:.0} ({:.0}) mask={:.0} ({:.0}) transparent={:.0} ({:.0})",
        eye(s.eye_draws[0]),
        eye(s.eye_entities[0]),
        eye(s.eye_draws[1]),
        eye(s.eye_entities[1]),
        eye(s.eye_draws[2]),
        eye(s.eye_entities[2])
    );
    println!(
        "VR_PERF pipelined={} early_submit={early_on} deferred_end={deferred_on} frames={frames} fps={:.1} period_ms={} vsyncs_1/2/3/4+={}/{}/{}/{} ticks_per_frame={:.2} | interval {} | wait {} | main {} | handoff {} | render {} | prep {} | graph {} | xr_pre {} | xr_end {} | spec {spec} | eye_draws {draws}",
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
        stats(&mut s.prep),
        stats(&mut s.graph),
        stats(&mut s.xr_pre),
        stats(&mut s.xr_end),
    );
    let active: Vec<String> = cameras
        .iter()
        .filter(|(c, _, _)| c.is_active)
        .map(|(c, n, three_d)| {
            format!(
                "{}{}@{}",
                if three_d { "3d:" } else { "2d:" },
                n.map_or("?", |n| n.as_str()),
                c.order
            )
        })
        .collect();
    println!("VR_PERF active_cameras={} [{}]", active.len(), active.join(", "));
}

fn specialization_load(
    dirty: Res<DirtySpecializations>,
    views: Query<&RenderVisibleEntities>,
    shared: Res<Shared>,
) {
    let mut s = shared.lock();
    s.spec_frames += 1;
    s.changed += dirty.changed_renderables.len() as u64;
    for class in views.iter().filter_map(|v| v.get::<Mesh3d>()) {
        s.views += 1;
        s.visible += class.entities_cpu_culling.len() as u64;
        s.added += class.added_entities().len() as u64;
    }
}
/// Draw calls and batched entities of one binned phase (no multidraw: indirect drawing is off).
fn binned<BPI: BinnedPhaseItem>(phase: &BinnedRenderPhase<BPI>) -> (u64, u64) {
    let batchable = phase.batchable_meshes.values().map(|b| b.entities().len() as u64);
    let unbatchable: u64 = phase
        .unbatchable_meshes
        .values()
        .map(|u| u.entities.len() as u64)
        .sum();
    (
        phase.batchable_meshes.len() as u64 + unbatchable + phase.non_mesh_items.len() as u64,
        batchable.sum::<u64>() + unbatchable,
    )
}
fn draw_load(
    eyes: Query<&ExtractedView, With<super::game::GameEye>>,
    opaque: Res<ViewBinnedRenderPhases<Opaque3d>>,
    mask: Res<ViewBinnedRenderPhases<AlphaMask3d>>,
    transparent: Res<ViewSortedRenderPhases<Transparent3d>>,
    shared: Res<Shared>,
) {
    let mut s = shared.lock();
    for view in &eyes {
        let k = view.retained_view_entity;
        s.eye_views += 1;
        for (i, (d, e)) in [
            opaque.get(&k).map(binned),
            mask.get(&k).map(binned),
            transparent.get(&k).map(|p| {
                // As `SortedRenderPhase::render_range`: a batch's first item carries its range and
                // the items it merged are skipped (their own ranges are stale).
                let (mut draws, mut skip) = (0u64, 0usize);
                for item in p.items.values() {
                    let n = item.batch_range().len();
                    if skip > 0 {
                        skip -= 1;
                    } else if n > 0 {
                        draws += 1;
                        skip = n - 1;
                    }
                }
                (draws, p.items.len() as u64)
            }),
        ]
        .into_iter()
        .map(|x| x.unwrap_or_default())
        .enumerate()
        {
            s.eye_draws[i] += d;
            s.eye_entities[i] += e;
        }
    }
}
fn render_start(mut clock: ResMut<RenderClock>) {
    clock.start = Some(Instant::now());
}
fn graph_start(mut clock: ResMut<RenderClock>, shared: Res<Shared>) {
    let now = Instant::now();
    if let Some(start) = clock.start {
        shared.lock().prep.push(ms(now - start));
    }
    clock.graph = Some(now);
}
fn graph_end(mut clock: ResMut<RenderClock>, shared: Res<Shared>) {
    if let Some(start) = clock.graph.take() {
        shared.lock().graph.push(ms(start.elapsed()));
    }
}
fn xr_pre_start(mut clock: ResMut<RenderClock>) {
    clock.xr_pre = Some(Instant::now());
}
fn xr_pre_done(mut clock: ResMut<RenderClock>, shared: Res<Shared>) {
    if let Some(start) = clock.xr_pre.take() {
        shared.lock().xr_pre.push(ms(start.elapsed()));
    }
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
