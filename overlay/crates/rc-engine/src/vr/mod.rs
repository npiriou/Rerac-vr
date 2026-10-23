//! OpenXR integration into the original game. Calibration is an explicit diagnostic mode.
mod blaster;
mod bomb_glove;
mod embodied;
mod gadget_socket;
mod game;
mod game_input;
mod haptics;
mod menu_pointer;
mod orientation;
pub(crate) mod spatial_menu;
mod traversal;
mod vendor;
mod walloper;
mod weapon_screen;
pub use game::{game_plugins, install_game, prepare_game};
pub use game_input::{GamePad, merge_pad};
mod actions;
mod core;
mod diagnostics;
mod interaction;
mod preview;
mod scene;
mod timing;

use self::core::V3;
use anyhow::{Context, Result, ensure};
use bevy::{prelude::*, render::pipelined_rendering::PipelinedRenderingPlugin};
use bevy_mod_openxr::{add_xr_plugins, resources::OxrSessionConfig};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchMode {
    Normal,
    Debug,
    DebugGold,
}
impl LaunchMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Debug => "debug",
            Self::DebugGold => "debug-gold",
        }
    }
    pub fn debug(self) -> bool {
        self != Self::Normal
    }
}

#[derive(Resource, Clone, Debug)]
pub struct Settings {
    pub mode: LaunchMode,
    pub scene: String,
    pub units_per_metre: f32,
    pub spawn_game: Option<[f32; 3]>,
    pub desktop_preview: bool,
    pub preview_seconds: Option<f32>,
}
impl Settings {
    fn from_env() -> Result<Self> {
        let mode = match std::env::var("RC_VR_MODE")
            .unwrap_or_else(|_| "normal".into())
            .as_str()
        {
            "normal" => LaunchMode::Normal,
            "debug" => LaunchMode::Debug,
            "debug-gold" => LaunchMode::DebugGold,
            value => anyhow::bail!("RC_VR_MODE must be normal, debug or debug-gold (got {value})"),
        };
        let scene = std::env::var("RC_VR_SCENE").unwrap_or_else(|_| "kerwan".into());
        ensure!(
            matches!(scene.as_str(), "calibration" | "novalis" | "kerwan"),
            "RC_VR_SCENE must be calibration, novalis or kerwan"
        );
        let units_per_metre = std::env::var("RC_VR_UNITS_PER_METRE")
            .unwrap_or_else(|_| "1".into())
            .parse::<f32>()
            .context("RC_VR_UNITS_PER_METRE must be a number")?;
        core::game_to_metres([0.0; 3], units_per_metre).map_err(anyhow::Error::msg)?;
        let spawn_game = std::env::var("RC_VR_SPAWN_GAME")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|s| -> Result<[f32; 3]> {
                let values = s
                    .split(',')
                    .map(|n| n.trim().parse::<f32>())
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                ensure!(
                    values.len() == 3 && values.iter().all(|n| n.is_finite()),
                    "RC_VR_SPAWN_GAME must be x,y,z in game units"
                );
                Ok([values[0], values[1], values[2]])
            })
            .transpose()?;
        let desktop_preview = std::env::var("RC_VR_DESKTOP_PREVIEW").is_ok_and(|s| s == "1");
        let preview_seconds = std::env::var("RC_VR_PREVIEW_SECONDS")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.parse::<f32>())
            .transpose()
            .context("RC_VR_PREVIEW_SECONDS must be a number")?;
        if let Some(seconds) = preview_seconds {
            ensure!(
                desktop_preview && seconds.is_finite() && (1.0..=600.0).contains(&seconds),
                "Preview smoke tests require desktop preview and 1..=600 seconds"
            );
        }
        Ok(Self {
            mode,
            scene,
            units_per_metre,
            spawn_game,
            desktop_preview,
            preview_seconds,
        })
    }
}
pub fn requested() -> bool {
    std::env::var("RC_VR").is_ok_and(|s| s == "1")
}
pub fn diagnostic_requested() -> bool {
    requested() && std::env::var("RC_VR_SCENE").is_ok_and(|s| s == "calibration")
}
pub fn game_requested() -> bool {
    requested() && !diagnostic_requested()
}
pub fn from_v3(v: V3) -> Vec3 {
    Vec3::new(v.0, v.1, v.2)
}
pub fn to_v3(v: Vec3) -> V3 {
    V3(v.x, v.y, v.z)
}

pub fn run() -> Result<()> {
    let settings = Settings::from_env()?;
    println!(
        "VR_POC experimental source candidate; upstream=89c87e98f9d8296187df2a6310c67b32144090d3"
    );
    println!(
        "VR_POC scene={} desktop_preview={} required_reference_space=STAGE",
        settings.scene, settings.desktop_preview
    );
    println!(
        "VR_POC controls: left trigger teleport (release), right trigger diagnostic shot, right stick snap turn, left X/A recenter, Escape exit"
    );
    println!(
        "VR_POC calibration cube edge=1 metre; units_per_metre is provisional, not a canonical Ratchet scale"
    );
    let data = scene::load(&settings)?;
    let background = data.background;
    let mut app = App::new();
    let plugins = DefaultPlugins
        .build()
        .disable::<PipelinedRenderingPlugin>()
        .set(AssetPlugin {
            file_path: crate::asset_dir(),
            ..default()
        });
    let preview = settings.desktop_preview;
    let plugins = if preview {
        plugins.set(bevy::render::RenderPlugin {
            render_creation: bevy::render::settings::RenderCreation::Automatic(Box::new(
                bevy::render::settings::WgpuSettings {
                    backends: Some(bevy::render::settings::Backends::VULKAN),
                    ..default()
                },
            )),
            ..default()
        })
    } else {
        add_xr_plugins(plugins)
    };
    app.add_plugins(plugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: if preview {
                "ReRAC VR - Desktop Preview (no headset)".into()
            } else {
                "ReRAC VR - Headset".into()
            },
            resolution: (1000u32, 700u32).into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(background))
    .insert_resource(settings)
    .insert_resource(data)
    .add_systems(
        Startup,
        (scene::spawn_geometry, scene::spawn_targets).chain(),
    );
    if preview {
        app.add_plugins(preview::PreviewPlugin);
    } else {
        app.insert_resource(OxrSessionConfig::default())
            .add_plugins((
                actions::InputPlugin,
                interaction::InteractionPlugin,
                diagnostics::DiagnosticsPlugin,
            ))
            .add_systems(Startup, require_headset);
    }
    let result = app.run();
    ensure!(
        matches!(result, AppExit::Success),
        "VR PoC exited with an error; inspect the timestamped log"
    );
    Ok(())
}

fn require_headset(
    state: Option<Res<bevy_mod_xr::session::XrState>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !state.is_some_and(|s| *s != bevy_mod_xr::session::XrState::Unavailable) {
        error!(
            "VR_POC no OpenXR headset is available. Connect the headset and start SteamVR, or run Run.ps1 -DesktopPreview."
        );
        exit.write(AppExit::error());
    }
}
