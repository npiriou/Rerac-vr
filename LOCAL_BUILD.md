# Native Windows build — 2026-10-22

The original desktop checkout is `X:\dev\rerac`, at upstream commit
`89c87e98f9d8296187df2a6310c67b32144090d3`. Its source remains unchanged.
VR integration lives in `X:\codex\worktrees\rerac-vr-build`, a separate Git
worktree at the same upstream revision with this package applied.
Its ignored `extracted` junction points to the original checkout's extracted
data. The ISO and compressed archive were not copied or added to Git.

Toolchain: Rust/Cargo 1.97.1, `x86_64-pc-windows-msvc`, CMake 4.4.2,
Visual Studio 2019 C++ tools (14.29). VS 2022 is installed without the C++
workload, so the build script selects VS 2019 through `vswhere`.

## Commands and evidence

| Command | Checkout | Result |
| --- | --- | --- |
| `cargo check --locked -p rc-engine --features dev` | original | PASS, exit 0 |
| `cargo xtask test-quick` | original | FAIL, exit 1: upstream PINE imports Unix sockets on Windows |
| `rustc --edition=2021 --test crates/rc-engine/src/vr/core.rs -o work/vr/vr-core-tests.exe` | VR | PASS, exit 0 |
| `work/vr/vr-core-tests.exe --nocapture` | VR | PASS, 15 tests, exit 0 |
| `cargo xtask test-quick` | VR | PASS, 1,070 tests, 3 ignored, exit 0 |
| `cargo check -p rc-engine --features vr` | VR, after fixes | PASS, exit 0 |
| `scripts/vr/Build.ps1` | VR | PASS, exit 0; debug executable and SHA-256 record created |
| `cargo check-all --locked` | VR | PASS, exit 0 |
| `target/debug/rerac.exe --version-json` | VR | PASS, exit 0 |

The original baseline logs are under this package's ignored `.local-build/`.
Integration and build logs are under the VR checkout's ignored `work/vr/`.
The build script writes `work/vr/last-build-debug.json` only after every step
passes and records the executable hash.

The three upstream ignored tests are `moby_lod::tests::novalis_metal_classes`
(print-only survey), `export::level_test::export_level_01_fully` (writes a large
export), and `cameras::tests::survey_other_classes` (survey). No ignored tests
were enabled or removed to obtain the passing result.

## Fixes included in the package

- Give OpenXR action binding string literals the required static lifetime.
- Bind Bevy's mutable material asset guard with `mut`.
- Select a Visual Studio installation that actually includes C++ tools.
- Use PCSX2's loopback TCP transport for PINE on Windows; preserve both
  framing/batching tests using connected sockets on each platform, matching
  [PCSX2's PINE server](https://github.com/PCSX2/pcsx2/blob/v2.8.2/pcsx2/PINE.cpp).
- Apply `resolved-Cargo.lock` after verifying the original upstream lockfile.

A fresh overlay application was executed and its lockfile and changed Rust
sources were compared with the integration checkout: all matched.

## Startup readiness without a headset

The launcher was executed on 2026-10-22. An empty optional spawn argument was
fixed in both the launcher and settings parser. Normal headset mode now reports
that no OpenXR headset is available and exits instead of leaving a blank window.

An explicit `-DesktopPreview` mode renders the calibration room or the same Novalis
terrain through Vulkan without creating an XR session. Both scenes passed an
eight-second smoke test with over 1,200 render frames and clean process exit.
Novalis loaded 61,919 triangles in 86 batches. The detected GPU was the NVIDIA
GeForce RTX 3090. There were no renderer errors or panics in the smoke-test logs.
No Windows capture was used for these checks.

SteamVR is installed and registered as the active OpenXR runtime; its runtime DLL
is present. PICO Connect is installed. Head tracking, stereo views, controller
bindings, and headset frame timing remain pending until hardware is available.

```powershell
& 'X:\codex\worktrees\rerac-vr-build\scripts\vr\Run.ps1' -Scene calibration -DesktopPreview
& 'X:\codex\worktrees\rerac-vr-build\scripts\vr\Run.ps1' -Scene novalis -DesktopPreview -DataDir 'X:\dev\rerac\extracted'
```

Use WASD to move, Q/E down/up, arrow keys to look, Shift for faster movement,
R to reset, and Escape to exit. Add `-SmokeTestSeconds 8` for automatic exit after
the rendering check. Logs are in `work/vr/readiness-*.log` in the VR checkout.

## Rebuild and launch with a headset

```powershell
Set-Location 'X:\codex\worktrees\rerac-vr-build'
.\scripts\vr\Build.ps1
.\scripts\vr\Run.ps1 -Scene calibration
# After calibration has passed on the headset:
.\scripts\vr\Run.ps1 -Scene novalis -DataDir 'X:\dev\rerac\extracted'
```

Launch scripts require a completed build record. Headset stereo, controller
bindings, comfort, and GPU/compositor timing remain unverified. Compiling the
terrain PoC does not establish that it is a playable game conversion.

## Novalis spawn correction

The original geometric inspection point placed the VR floor at game height 38 and
the user reported appearing under the terrain. The launcher now defaults to the
original Novalis class-0 Ratchet instance: game position
`162.530319,136.393478,60.5`. Its downward floor query finds game height 60, giving
VR floor position `[162.53032,60,-136.39348]` at the default scale. The landing
clearance and desktop smoke check passed. The headset scene was relaunched there.

The agreed future objective is a playable Ratchet & Clank VR conversion using
ReRAC's original character, movement, and gameplay. This spawn fix does not
implement those later gameplay systems.

## Original ReRAC gameplay integration

The earlier terrain-only implementation is superseded for Novalis. `RC_VR=1` now enters the original application and plugin chain; only explicit calibration uses the isolated diagnostic app. The original hero and all game systems run once, with XR views and original-pad input added to them.

Original integration Windows executable (before the controller remap below): `X:/codex/worktrees/rerac-vr-build/target/debug/rerac.exe`

SHA-256: `FD9FA600715B98A6A0CDEC487BA4CDA97A86A6957FD5D47BEE1A0F62E44FC5B0`

- Native VR build and non-VR check: PASS (`work/vr/original-game-ready-build.log`).
- Full workspace/all-targets check with dev + VR: PASS.
- Workspace unit suite including VR: 1,089 passed, 3 ignored. Diagnostic standalone suite: 15 passed.
- Package checks: 21 passed. Guarded overlay apply checked on a clean worktree under X:/codex/worktrees.
- Original-game desktop/stereo smoke: 487 ticks, 327 render frames, 14 Ratchet meshes, two world views and two sky views; minimum 701 world draws and 5 sky draws per eye. No render errors.
- Scripted movement/jump/wrench/fire went through the original pad/tick. Position changed from [162.5303, 136.3935, 60.0] to [159.8904, 142.3601, 60.0]; maximum game z 61.2268, then returned to z 60.
- Headset launch: stereo_rendering_and_basic_controller_play_confirmed. Current log: `work/vr/novalis-headset-20261022-205230.log`. The user confirmed Ratchet/scenery visibility and left-stick movement in the headset. Broader combat/menu and comfort checks remain pending.

The original main camera drives the third-person VR rig. Shared culling/LOD/lighting use that camera with a wider HMD envelope; PS2 single-view occlusion masks are disabled only for VR. World/sky eyes use runtime projections, and original fade/underwater/tint effects are copied to them. HUD/menu panels reuse original output. The PC view refreshes every four headset frames during gameplay; original menu camera activation and full-rate menu output are preserved.

PowerShell 7/.NET on this host preserves an empty native environment entry when `[Environment]::SetEnvironmentVariable(name, $null)` is used. The launcher now removes absent optional variables through the Env provider, including empty capture variables, so the original capture-and-exit mode is not accidentally requested.

Start with `scripts/vr/Run.ps1 -Scene novalis -DataDir X:/dev/rerac/extracted`. SteamVR is elevated on this machine, so the local headset launcher uses the normal Windows RunAs/UAC flow to match it. No SteamVR security settings were changed.

This does not implement tracked-hand aiming, new hero physics, or missing upstream ports. Full campaign completion, GPU/compositor budget and comfort are not certified by a startup smoke test.

## Controller remap

A sends Cross, B sends Square, X sends Circle, Y sends Triangle. Left/right stick clicks send Select/Start exclusively. Recenter is on keyboard F8; right trigger remains an additional weapon binding.

Rebuilt native executable SHA-256: `E8D58CD360640BECD389EEC0190182CE7D847F1760771D1D47A1470B936B3011`. Build, non-VR and VR checks passed. Workspace unit suite: 1,090 passed, 3 ignored; standalone diagnostic suite: 15 passed. The new regression checks each physical binding through the original pad decoder and verifies clean releases. Package checks: 21 passed.

Relaunched Novalis in SteamVR with the new binary. Stereo cameras attached; runtime states: IDLE, READY, SYNCHRONIZED. Log: `work/vr/novalis-headset-20261022-210644.log`. Physical verification of the six remapped controls is pending; the earlier user confirmation covers the prior integration binary.

## Camera and movement correction

Initial headset recenter now waits for focus. The tracking rig keeps a level horizon and follows the original camera's yaw and position; physical head motion supplies pitch and roll. Left-stick movement follows horizontal headset heading before reaching the original pad decoder. Menus and the quick-select wheel retain their ordinary directions. Right-stick horizontal orbit is slower, with vertical look on the head. Virtual-gamepad input is ignored while XR input is active to prevent duplicate button/R3 presses.

Native executable SHA-256: `79902F8CEDA780E8B4EEBEDE229F5E74C9F053176CF1CFF80DD16A7657924EDB`. Compilation completed with and without VR. No tests were run, as required by AGENTS.md; the build script now requires explicit `-RunTests` to execute tests. Earlier unit/package and headset evidence remains historical. The new executable was relaunched in SteamVR; log: `work/vr/novalis-headset-20261022-211402.log`. Ratchet visibility and control comfort await user feedback. Live logs now include visible Ratchet mesh count and camera-relative hero position.
