//! Prepare the destination underneath the native flight, then cut to its ready landing.
//! Keep the departure simulation alive for flight actors while the destination uploads and warms
//! its pipelines. Non-flight level changes use a black-frame handshake before replacing the world.
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        mesh::RenderMesh, render_asset::RenderAssets,
        render_resource::PipelineCache, texture::GpuImage,
    },
};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering}};

static BLOCKED: AtomicBool = AtomicBool::new(false);
pub(crate) fn blocked() -> bool { BLOCKED.load(Ordering::Acquire) }
static STREAMING: AtomicBool = AtomicBool::new(false);
static DESTINATION_READY: AtomicBool = AtomicBool::new(false);
static FINISH_FLIGHT: AtomicBool = AtomicBool::new(false);
static DESTINATION_VIEW: Mutex<Option<Transform>> = Mutex::new(None);
pub(crate) fn streaming() -> bool { STREAMING.load(Ordering::Acquire) }
pub(crate) fn destination_ready() -> bool { DESTINATION_READY.load(Ordering::Acquire) }
pub(crate) fn finish_flight() { FINISH_FLIGHT.store(true, Ordering::Release); }
pub(super) fn set_destination_view(view: Transform) {
    *DESTINATION_VIEW.lock().unwrap_or_else(|e| e.into_inner()) = Some(view);
}
/// Native world LOD/occlusion is shared across views. During preparation it must use the new
/// planet's eye, not the unrelated space camera; flight actors have their own render records.
pub(crate) fn culling_camera(native: Transform) -> Transform {
    if !streaming() { return native; }
    DESTINATION_VIEW.lock().unwrap_or_else(|e| e.into_inner()).unwrap_or(native)
}

static RENDER_EYE: Mutex<Option<Transform>> = Mutex::new(None);
static RENDER_NATIVE_TOO: AtomicBool = AtomicBool::new(false);
/// The headset's eye in the world this frame (None outside the game's views), and whether the native camera's image
/// is shown too (the Visibomb's monitor, a gadget's panel).
pub(super) fn set_render_eye(eye: Option<Transform>, native_too: bool) {
    *RENDER_EYE.lock().unwrap_or_else(|e| e.into_inner()) = eye;
    RENDER_NATIVE_TOO.store(native_too && eye.is_some(), Ordering::Release);
}
/// The views level content is culled and LOD-picked for, for drawing only: the headset's eye in every mode (in movies,
/// ship sequences and the Visibomb's view the native camera, which game logic keeps reading, is elsewhere), plus the
/// native camera while its image is shown too; the destination's eye while a flight streams it.
pub(crate) fn render_cameras(native: Transform) -> (Transform, Option<Transform>) {
    if streaming() {
        return (culling_camera(native), None);
    }
    match *RENDER_EYE.lock().unwrap_or_else(|e| e.into_inner()) {
        Some(eye) => (eye, (RENDER_NATIVE_TOO.load(Ordering::Acquire) && eye != native).then_some(native)),
        None => (native, None),
    }
}

/// The flight's native actor IDs belong to the departure simulation, not the new planet's table.
#[derive(Resource)]
pub(crate) struct FlightWorld {
    pub play: crate::gameplay::Play,
    pub level: crate::Level,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    FadeOut,
    AwaitBlack,
    FlightWorld,
    FlightReady,
    World,
    FirstPose,
    Landing,
    FadeIn,
}
#[derive(Resource, Clone, Default, ExtractResource)]
pub(super) struct Hold {
    phase: Phase,
    alpha: f32,
    epoch: u64,
    ready: Arc<AtomicU64>,
    black_frames: Arc<AtomicU32>,
    meshes: Vec<AssetId<Mesh>>,
    images: Vec<AssetId<Image>>,
}
impl Hold {
    pub(super) fn opacity(&self) -> f32 { self.alpha }
    fn warmup(&mut self, phase: Phase) {
        self.phase = phase;
        self.epoch = self.epoch.wrapping_add(1);
        self.ready.store(0, Ordering::Release);
        BLOCKED.store(true, Ordering::Release);
    }
}
pub(super) struct LoadingPlugin;
impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hold>()
            .add_plugins(ExtractResourcePlugin::<Hold>::default())
            .add_systems(Update, prepare);
        app.sub_app_mut(RenderApp)
            .add_systems(Render, ready.in_set(RenderSystems::Cleanup));
    }
}

/// Last runs before extraction. Arming in LevelUnload was too late: that call already stalls.
pub(crate) fn before_level_swap(world: &mut World) -> bool {
    let Some(mut h) = world.get_resource_mut::<Hold>() else { return true };
    if h.phase == Phase::Idle {
        // The destination starts building DURING the animated flight. Keep the departure actor
        // simulation separate while the new planet's lazy setup and GPU resources are prepared.
        // The caller selects this path only after the flight has started presenting.
        let flight = world.resource::<crate::travel_render::Travel>().space_loading();
        let mut h = world.resource_mut::<Hold>();
        if flight {
            STREAMING.store(true, Ordering::Release);
            DESTINATION_READY.store(false, Ordering::Release);
            FINISH_FLIGHT.store(false, Ordering::Release);
            *DESTINATION_VIEW.lock().unwrap_or_else(|e| e.into_inner()) = None;
            h.warmup(Phase::FlightWorld);
            println!("VR_GAME flight_preload begin; preparing destination during voyage");
            return true;
        }
        h.phase = Phase::FadeOut;
        h.alpha = 0.0;
        h.black_frames.store(0, Ordering::Release);
        BLOCKED.store(true, Ordering::Release);
        println!("VR_GAME loading_fade begin; waiting for submitted black stereo frames");
        return false;
    }
    if h.phase == Phase::AwaitBlack && h.black_frames.load(Ordering::Acquire) >= 3 {
        h.warmup(Phase::World);
        println!("VR_GAME loading_fade black submitted; swapping level");
        return true;
    }
    false
}
fn prepare(
    mut commands: Commands,
    mut h: ResMut<Hold>,
    time: Res<Time<Real>>,
    mut travel: ResMut<crate::travel_render::Travel>,
    mut flight: ResMut<crate::flight_render::FlightRender>,
    mut level: ResMut<crate::Level>,
    audio: Option<ResMut<crate::audio_out::AudioOut>>,
    images: Res<Assets<Image>>,
    meshes: Query<&Mesh3d>,
    play: Option<Res<crate::gameplay::Play>>,
) {
    // An upload stall must not consume the entire reveal fade in one update.
    let dt = time.delta_secs().min(0.05);
    match h.phase {
        Phase::Idle => return,
        Phase::FlightWorld if h.ready.load(Ordering::Acquire) == h.epoch && play.is_some() => {
            h.phase = Phase::FlightReady;
            DESTINATION_READY.store(true, Ordering::Release);
            println!("VR_GAME flight_preload destination GPU ready; voyage may finish");
        }
        Phase::FlightReady if FINISH_FLIGHT.swap(false, Ordering::AcqRel) => {
            h.phase = Phase::FadeOut;
            h.black_frames.store(0, Ordering::Release);
        }
        Phase::FadeOut => {
            h.alpha = (h.alpha + dt / 0.20).min(1.0);
            if h.alpha >= 1.0 { h.phase = Phase::AwaitBlack; }
        }
        Phase::AwaitBlack if streaming() && h.black_frames.load(Ordering::Acquire) >= 3 => {
            // Only hand off simulations at the cut, after all destination setup happened in flight.
            STREAMING.store(false, Ordering::Release);
            commands.remove_resource::<FlightWorld>();
            travel.enter_preloaded(&mut commands);
            flight.active = false;
            flight.draw = None;
            if let (Some(data), Some(mut audio)) = (level.0.audio.take(), audio) {
                audio.swap_level(data);
            }
            h.warmup(Phase::World);
        }
        Phase::World if h.ready.load(Ordering::Acquire) == h.epoch => {
            // Let the native landing create its first posed ship and camera under black.
            h.phase = Phase::FirstPose;
            BLOCKED.store(false, Ordering::Release);
        }
        // Landing movie frames can advance while the gameplay counter is stopped. Waiting for
        // a gameplay tick here hides the beginning of the landing behind a seemingly stuck screen.
        Phase::FirstPose if travel.presentation_ready() => {
            h.warmup(Phase::Landing);
        }
        Phase::Landing if h.ready.load(Ordering::Acquire) == h.epoch => {
            h.phase = Phase::FadeIn;
            BLOCKED.store(false, Ordering::Release);
            println!("VR_GAME loading_fade landing ready; revealing level");
        }
        Phase::FadeIn => {
            h.alpha = (h.alpha - dt / 0.35).max(0.0);
            if h.alpha <= 0.0 {
                h.phase = Phase::Idle;
                h.meshes.clear();
                h.images.clear();
            }
        }
        _ => {}
    }
    if matches!(h.phase, Phase::FlightWorld | Phase::World | Phase::Landing) {
        h.meshes = meshes.iter().map(|m| m.0.id())
            .collect::<std::collections::HashSet<_>>().into_iter().collect();
        h.images = images.iter()
            .filter(|(_, i)| i.asset_usage.contains(RenderAssetUsages::RENDER_WORLD))
            .map(|(id, _)| id).collect();
    }
}
fn ready(
    h: Option<Res<Hold>>,
    images: Res<RenderAssets<GpuImage>>,
    meshes: Res<RenderAssets<RenderMesh>>,
    pipelines: Res<PipelineCache>,
    eyes: Query<&crate::scene_render::SceneFade, With<super::game::GameEye>>,
    mut quiet: Local<(u64, u32)>,
) {
    let Some(h) = h else { return };
    let pipelines_ready = pipelines.waiting_pipelines().next().is_none();
    if h.phase == Phase::AwaitBlack {
        // The eye fade is warmed during flight. Acknowledge extracted opaque frames after
        // rendering, not merely the main world's request to turn black.
        if pipelines_ready && eyes.iter().count() >= 2
            && eyes.iter().all(|f| f.rgba == UVec4::new(0, 0, 0, 128)) {
            h.black_frames.fetch_add(1, Ordering::Release);
        }
    }
    if matches!(h.phase, Phase::FlightWorld | Phase::World | Phase::Landing) {
        if quiet.0 != h.epoch { *quiet = (h.epoch, 0); }
        let uploaded = !h.meshes.is_empty()
            && h.meshes.iter().all(|id| meshes.get(*id).is_some())
            && h.images.iter().all(|id| images.get(*id).is_some())
            && pipelines_ready;
        quiet.1 = if uploaded { quiet.1 + 1 } else { 0 };
        if quiet.1 >= 3 { h.ready.store(h.epoch, Ordering::Release); }
    }
}
