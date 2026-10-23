//! Early GPU submission. Bevy submits every camera's command buffers once, after the last camera,
//! so SteamVR's xrEndFrame (which waits for the frame's GPU work) waits for all of it. When a camera
//! starts, what the previous cameras recorded is submitted: the GPU renders the first views while
//! the CPU still records the others. Same commands, same order.
//! RC_VR_EARLY_SUBMIT=0 disables it; RC_VR_EARLY_SUBMIT=ab alternates it every VR_PERF window.
//! The camera pipelines also run on a single-threaded executor (see the plugin).
use bevy::{
    core_pipeline::{Core2d, Core2dSystems, Core3d, Core3dSystems},
    ecs::schedule::SingleThreadedExecutor,
    prelude::*,
    render::{
        RenderApp,
        renderer::{PendingCommandBuffers, RenderQueue},
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Shared by both worlds: `on` is read by the render world, flipped per window when `alternate`.
#[derive(Resource, Clone)]
pub(super) struct EarlySubmit {
    pub on: Arc<AtomicBool>,
    pub alternate: bool,
}

pub struct EarlySubmitPlugin;
impl Plugin for EarlySubmitPlugin {
    fn build(&self, app: &mut App) {
        let mode = std::env::var("RC_VR_EARLY_SUBMIT").unwrap_or_default();
        let state = EarlySubmit {
            on: Arc::new(AtomicBool::new(mode.trim() != "0")),
            alternate: mode.trim() == "ab",
        };
        app.insert_resource(state.clone());
        app.sub_app_mut(RenderApp)
            .insert_resource(state)
            .add_systems(Core3d, submit_recorded.before(Core3dSystems::Prepass))
            .add_systems(Core2d, submit_recorded.before(Core2dSystems::Prepass));
        // Each camera's pipeline is a chain of passes: Bevy's multithreaded executor made the render
        // thread wait for a worker per pass and camera (about 0.7 ms a frame). The passes still
        // encode in parallel internally. RC_VR_CAMERA_EXECUTOR=multi restores Bevy's executor.
        if !std::env::var("RC_VR_CAMERA_EXECUTOR").is_ok_and(|s| s.trim() == "multi") {
            let render = app.sub_app_mut(RenderApp);
            render.edit_schedule(Core3d, |s| {
                s.set_executor(SingleThreadedExecutor::new());
            });
            render.edit_schedule(Core2d, |s| {
                s.set_executor(SingleThreadedExecutor::new());
            });
        }
    }
}

fn submit_recorded(world: &mut World) {
    if !world.resource::<EarlySubmit>().on.load(Ordering::Relaxed) {
        return;
    }
    let buffers = world.resource_mut::<PendingCommandBuffers>().take();
    if !buffers.is_empty() {
        world.resource::<RenderQueue>().submit(buffers);
    }
}
