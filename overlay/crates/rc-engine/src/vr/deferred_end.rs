//! Deferred xrEndFrame. SteamVR's Vulkan path blocks in xrEndFrame until the GPU has finished the
//! frame. bevy_mod_openxr calls it at the end of the render schedule, so the render thread, and the
//! main thread waiting to extract the next frame, both stand still for the GPU. Here frame N's
//! xrEndFrame runs at the start of frame N+1's render schedule, right before its xrBeginFrame (the
//! order OpenXR requires): the GPU finishes while the render world is handed back and the next frame
//! is extracted. Same layers, same display time; frame N reaches the compositor once its GPU work is
//! done, as before.
//! RC_VR_DEFERRED_END=1 enables it; RC_VR_DEFERRED_END=ab alternates it every VR_PERF window.
use bevy::{
    prelude::*,
    render::{Render, RenderApp, RenderSystems},
};
use bevy_mod_openxr::{
    environment_blend_mode::OxrEnvironmentBlendModes,
    init::should_run_frame_loop,
    render::{
        begin_frame, insert_texture_views, locate_views, release_image, update_views_render_world,
        wait_image,
    },
    resources::{OxrFrameState, OxrFrameStream, OxrRenderLayers},
};
use bevy_mod_xr::session::{XrPreDestroySession, XrRenderSystems};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Shared by both worlds: `on` is read by the render world, flipped per window when `alternate`.
#[derive(Resource, Clone)]
pub(super) struct DeferredEnd {
    pub on: Arc<AtomicBool>,
    pub alternate: bool,
}

/// Where the deferred frame loop replaces bevy_mod_openxr's PreRender / PostRender systems.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum DeferredSystems {
    Pre,
    Post,
}

/// The rendered frame waiting for its xrEndFrame: display time and whether it rendered.
#[derive(Resource, Default)]
struct PendingEnd(Option<(openxr::Time, bool)>);

pub struct DeferredEndPlugin;
impl Plugin for DeferredEndPlugin {
    fn build(&self, app: &mut App) {
        let mode = std::env::var("RC_VR_DEFERRED_END").unwrap_or_default();
        let state = DeferredEnd {
            on: Arc::new(AtomicBool::new(matches!(mode.trim(), "1" | "ab"))),
            alternate: mode.trim() == "ab",
        };
        app.insert_resource(state.clone());
        app.sub_app_mut(RenderApp)
            .insert_resource(state)
            .init_resource::<PendingEnd>()
            .configure_sets(
                Render,
                (XrRenderSystems::PreRender, XrRenderSystems::PostRender).run_if(not(deferred)),
            )
            .configure_sets(
                Render,
                DeferredSystems::Pre
                    .after(XrRenderSystems::HandleEvents)
                    .before(XrRenderSystems::PreRender)
                    .before(RenderSystems::PrepareViews)
                    .before(RenderSystems::PrepareAssets),
            )
            .configure_sets(
                Render,
                DeferredSystems::Post
                    .after(RenderSystems::Render)
                    .before(XrRenderSystems::PostRender)
                    .before(RenderSystems::Cleanup),
            )
            .add_systems(
                Render,
                (
                    // Also when switching back: the synchronous PreRender then begins the next frame.
                    end_pending,
                    (
                        begin_frame,
                        insert_texture_views,
                        locate_views,
                        update_views_render_world,
                        wait_image,
                    )
                        .chain()
                        .run_if(should_run_frame_loop.and_then(deferred)),
                )
                    .chain()
                    .in_set(DeferredSystems::Pre),
            )
            .add_systems(
                Render,
                (release_image, keep_pending)
                    .chain()
                    .run_if(should_run_frame_loop.and_then(deferred))
                    .in_set(DeferredSystems::Post),
            )
            .add_systems(XrPreDestroySession, end_pending);
    }
}

fn deferred(state: Res<DeferredEnd>) -> bool {
    state.on.load(Ordering::Relaxed)
}

fn keep_pending(frame: Res<OxrFrameState>, mut pending: ResMut<PendingEnd>) {
    pending.0 = Some((frame.predicted_display_time, frame.should_render));
}

/// The previous frame's xrEndFrame, as bevy_mod_openxr's `end_frame`. Its views are still in
/// `OxrViews`: this frame's are located after it.
fn end_pending(world: &mut World) {
    if !world.contains_resource::<OxrFrameStream>() {
        return;
    }
    let Some((display_time, render)) = world.resource_mut::<PendingEnd>().0.take() else {
        return;
    };
    world.resource_scope::<OxrFrameStream, ()>(|world, mut stream| {
        let mut layers = vec![];
        if render {
            for layer in world.resource::<OxrRenderLayers>().iter() {
                if let Some(layer) = layer.get(world) {
                    layers.push(layer);
                }
            }
        }
        let layers: Vec<_> = layers.iter().map(Box::as_ref).collect();
        let blend = world.resource::<OxrEnvironmentBlendModes>().blend_mode();
        if let Err(e) = stream.end(display_time, blend, &layers) {
            error!("Failed to end the deferred frame: {e}");
        }
    });
}
