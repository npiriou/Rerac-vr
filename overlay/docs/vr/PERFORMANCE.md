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

## Retired visibility experiment

The first batched-visibility experiment was rejected after the user reported worse
lag. Its original source/build entrypoints were restored. Its source and binaries
are archived locally; visibility.rs was removed from both active source trees.
The original B4250C4E and unchanged-source dev opt3 A3E2431D executables remain
available under target/debug for rollback. The third-party license notice for
the archived adaptation is retained in THIRD_PARTY_NOTICES.md.
