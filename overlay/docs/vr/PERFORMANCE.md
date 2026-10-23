# PC performance configuration

Native Release is built and launched. Final headset timing remains pending: the headset was inactive when this record was written.

The active build uses the original VR engine sources, compiled in Release with
`RUSTFLAGS=-C target-cpu=native` on the local Ryzen 5 5600. This enables the
processor features reported by rustc and removes development assertions. It is
a local CPU build; the same flags rebuild for the host processor on another PC.

The measured PC has a dedicated NVIDIA profile for rerac.exe with Power management
mode set to Prefer maximum performance. GPU P0 / about 1,785 MHz was verified
after relaunch; earlier samples were about 255-630 MHz. This power policy alone
does not establish a frame-rate gain. Windows High performance, all 12 logical
processors and an uncapped original VR render loop were already active.

The local performance launcher uses Release, default Bevy thread pools and these
Vulkan settings: WGPU_DEBUG=0, WGPU_VALIDATION=0, WGPU_GPU_BASED_VALIDATION=0,
WGPU_VALIDATION_INDIRECT_CALL=0. Bevy 0.19.1 removes indirect argument validation
for production Vulkan in bevy_render/src/settings.rs; the OpenXR initializer
accepts the corresponding wgpu environment settings. These are debugging and
validation options, not resolution, texture, detail, MSAA or effects presets.

Reproduce the build and launch from the applied source tree:

```powershell
$env:RUSTFLAGS='-C target-cpu=native'
.\scripts\vr\Build.ps1 -Release
$env:WGPU_DEBUG='0'
$env:WGPU_VALIDATION='0'
$env:WGPU_GPU_BASED_VALIDATION='0'
$env:WGPU_VALIDATION_INDIRECT_CALL='0'
.\scripts\vr\Run.ps1 -Release -Mode debug-gold -DataDir 'X:\dev\rerac\extracted'
```

No automated tests, smoke tests or gameplay scripts were run. Read-only OpenVR
background observations collected live headset timing during user free play.
The scenes changed, so these observations are not a controlled before/after
benchmark. SteamVR GPU spans can include scheduling gaps and other work; CPU
elapsed intervals include waiting. Low global GPU utilization is not by itself
proof of an artificial utilization limit. No 100% load or FPS gain is guaranteed.

See validation.json / pc_performance_configuration for the active binary hash,
launch log and measured intervals. Raw captures and the reversible NVIDIA profile
record remain under .local-build on the measured PC. No overclock, global GPU
power-limit change, graphics downgrade or new gameplay/visibility logic was used.

## Serial frame loop and pipelined rendering

The earlier perf-20261023-* captures measured target\debug\rerac.exe, not the
Release binary. Their ETW main-thread timeline accounts for 44.9 ms per frame:
19.0 ms main world (2-3 fixed game ticks, visibility, game render preparation),
19.3 ms render world (extract, preparation, encoding, submit) and 6.5 ms inside
xrEndFrame, where SteamVR's Vulkan path waits through vrclient/D3D11 for the GPU.
xrWaitFrame never blocked: every frame was already late. At 72 Hz each frame was
shown for 2-4 display periods. CPU and GPU utilisation stayed low because the two
worlds and the GPU ran one after another (about one core busy at a time).

The VR game had disabled Bevy's PipelinedRenderingPlugin, which the desktop game
keeps. It is enabled again so the next frame's simulation overlaps this frame's
render and its xrEndFrame wait; bevy_mod_openxr predicts poses one display period
further in this mode. RC_VR_PIPELINED=0 restores the previous serial loop.

The headset log now prints a VR_PERF line every 5 s (timestamps only): achieved
fps, frame intervals counted in display periods, game ticks per frame, and
avg/p50/p95/max of the frame interval, xrWaitFrame wait, main world, main-thread
handoff, render world and xrEndFrame (vr/timing.rs).

First pipelined Release session (Kerwan, debug-gold, 72 Hz, free play,
work/vr/debug-gold-kerwan-headset-20261023-192507.log): 30-37 fps against 25-30
fps for the serial Release loop (debug-gold-kerwan-headset-20261023-180424.log).
Typical 5 s window: interval 28.2 ms avg (25.0 p50), main world 14.0 ms, render
world 26.5 ms including 5.3 ms in xrEndFrame, xrWaitFrame wait 1.4 ms, main-thread
handoff 13.0 ms, 1.7 game ticks per frame. The render world is now the critical
path; SteamVR's reported display period also varies (13.9-55.6 ms) as it throttles.

Steam on the measured PC runs as administrator, so SteamVR does too. A game
started without elevation is refused access to vrserver ("Unable to open process
... Access is denied" in Steam\logs\xrclient_rerac.txt), hangs in OpenXR instance
creation, and SteamVR's shared-resource namespace server shuts down until SteamVR
is restarted. Launch the game elevated while Steam is elevated.

## Render-thread reductions (identical image)

Measured without a headset in the desktop preview (`RC_VR_DESKTOP_PREVIEW=1`, same
four eye cameras, Kerwan debug-gold), frame-exact (`RC_DETERMINISTIC=1`, so frame N
is the same scene in every run) and with the window's vsync off (`RC_NOVSYNC=1`).
The eye size does not change the CPU cost: 640 and 1920 pixels give the same
timings (`RC_VR_PREVIEW_SIZE`). Configurations were run alternately, each in its
own process.

| Change | fps | render thread avg / p95 (ms) |
|---|---|---|
| Pipelined, window world view refreshed every 4 frames | 56-59 | 16.5 / 31 |
| Window world view not refreshed during VR gameplay | 74-77 | 12.3 / 14.5 |
| + camera pipelines on a single-threaded executor | 80-82 | 11.4 / 13.6 |
| + one transparent-order sort per view, in parallel | 87-89 | 10.4 / 12.8 |

* **Window world view.** Re-activating the window's cameras every 4th frame made
  Bevy drop and rebuild their pipeline specializations and render phases from
  scratch (about 2,500 entities re-specialized per frame on average) and doubled
  those frames' render time. `RC_VR_MIRROR_EVERY` now defaults to 0: the window's
  world view stays frozen during VR gameplay; menus and native panels keep their
  full frame. SteamVR's VR View mirrors the headset. Any value 1-120 restores a
  refresh cadence.
* **Camera pipelines.** Core3d/Core2d run each camera's passes in order; a
  single-threaded executor removes a worker wake-up per pass and camera. The passes
  still encode in parallel internally. `RC_VR_CAMERA_EXECUTOR=multi` restores Bevy's.
* **Transparent order.** Bevy's sort, crate::determinism's tie-break sort and
  crate::pre_shadow's game-order sort ran one after another over every view, and the
  last two were not ordered with each other. One sort per view with the composite key
  (pre-shadow group, game order, distance, entity), the views in parallel, gives the
  order the three produced; pre-shadow `restore` inserts the few remaining items into
  the stashed ones instead of the reverse. Frame 600 captured with the previous and
  the new ordering (both without tracing) is identical pixel for pixel.
Per world eye: about 508 opaque draws (5,330 entities), 133 alpha-mask draws (4,012)
and 218 transparent draws (4,105 items): instancing already merges most entities.
Bevy's indirect drawing (`RC_INDIRECT=1`) was slower (about 40 fps) and the two sky
eye views cost no measurable time, so neither was changed.

**Cinematic crash fixed** (`vr/fullscreen_refresh.rs`). Bevy's FullscreenMaterial
keeps each view's bind group until the view's texture changes, but the bind group
also holds the material's uniform buffer, which is reallocated when more views
carry the material. VR copies the window camera's scene fade and tints to both eyes;
when a cinematic re-activated the window camera, an eye drew with the third view's
offset on the old one-entry buffer (wgpu validation error, game closed). Those bind
groups are now dropped whenever their buffer changes. A cinematic then played
through in the headset.

## Headset results (Kerwan, debug-gold, 72 Hz, Release with everything above)

| Situation | fps | main world | render thread (prep / graph / xr_pre / xr_end) |
|---|---|---|---|
| Near the spawn | 51-59 | 9-10 ms | 17 ms (4.2 / 5.9 / 2.1 / 4.7) |
| Moving across the level | 36-47 | 12-16 ms | 20-24 ms (6-8 / 7-10 / 1.8 / 5.5) |
| Sustained Blaster fire | 14-25 | 30-50 ms | 34-55 ms |

Morning baseline: 25-30 fps. Prep and graph cost the same as in the desktop preview;
SteamVR adds about 2 ms before rendering (xrBeginFrame, image acquire and wait) and
about 5 ms in xrEndFrame every frame. Under sustained Blaster fire the game logic
gets slower per tick and the 60 Hz catch-up then runs 3-4 ticks per frame, which
slows the next frame further.

Tried in the headset and removed:
* submitting each camera's command buffers when the next camera starts (A/B per
  5 s window): no measurable change;
* calling frame N's xrEndFrame at the start of frame N+1, before its xrBeginFrame:
  about 4 fps slower, the wait only moved to the start of the next frame.
bevy_mod_openxr at the pinned revision has its D3D12 path disabled, so SteamVR's
Vulkan interop cannot be avoided there.

VR_PERF reports `prep` (render schedule up to the graph), `graph` (camera encoding,
submission, present), `xr_pre` and `xr_end`; `RC_VR_PERF=detail` adds `spec`
(views, visible and re-specialized meshes), `eye_draws` and the active cameras.

## Retired visibility experiment

The first batched-visibility experiment was rejected after the user reported worse
lag. Its original source/build entrypoints were restored. Its source and binaries
are archived locally; visibility.rs was removed from both active source trees.
The original B4250C4E and unchanged-source dev opt3 A3E2431D executables remain
available under target/debug for rollback. The third-party license notice for
the archived adaptation is retained in THIRD_PARTY_NOTICES.md.
