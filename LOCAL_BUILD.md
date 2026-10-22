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

## First-person tracked wrench

The existing complete Novalis gameplay path is retained. The view is anchored 1.6 game units above the hero's feet, independent of animated bones and the original follow camera. Left stick remains head-relative movement through the original pad decoder, right stick snap-turns by 30 degrees about the headset, and A retains jump/double jump. XR crouch, firearms and teleportation are not bound.

Both tracked hands have provisional glove visuals. Hold right grip to hold the original wrench model; a physical swing delivers native wrench hits. A moving grip release launches the original outgoing/returning wrench flight, whose catch point now follows the tracked hand. Slow releases stow it. Gesture history clears on focus/tracking loss, menus, snap turns and recentering.

Native build with and without VR: completed. SHA-256: `9B13FA47FBD4C186EADC7965A6C8B1D038E9F1ACB243FED1D8C10E35D6F684A0`. Build log: `work/vr/first-person-build.log`. No tests or commits were performed. Normal Novalis headset launch: `work/vr/novalis-headset-20261022-214231.log`. Physical view/alignment/combat/comfort remain pending user feedback. The previous executable and its build record are preserved under `work/vr/before-first-person/`.

## Tracked Bomb Glove

Right trigger press prepares one original class-121 bomb at the tracked right palm, held without a running fuse until release. A tracked release gesture launches it with hand-derived velocity into the original bomb simulation. A slow release or interruption cancels it without ammunition cost; a valid throw consumes one original ammo unit through the native use-ammo/statistics path. The existing wrench grip controls and full Novalis path remain. Prepared bombs have priority over wrench melee.

Native builds with and without VR completed. SHA-256: `5542FEEC6F74A9C1A69D95DD9F556D55D7108B496C24D85DE8392F4B60A9D5B5`. Build log: `work/vr/bomb-glove-build.log`. No tests or commits performed. Normal Novalis headset launch: `work/vr/novalis-headset-20261022-215447.log`. Physical bomb alignment, launch strength, damage and ammo feedback remain pending. The previous executable/build record are retained under `work/vr/before-bomb-glove/`.

## Body/camera and wrench grip alignment

Physical horizontal headset movement now moves Ratchet through the original capsule collision resolver, with the camera above the resolved body. Snap turns no longer accumulate pivot translation. The original wrench attachment-list-0 frame (chain [0, 1], held sequence 1) has a model-space offset of approximately 0.154 game units and a rotated basis; both are compensated to place the grip at the tracked palm. Rendering, melee, launch and native return share this corrected attachment. Novalis, controls and Bomb Glove remain available.

Native builds with and without VR completed. SHA-256: `ABF3076FA72DAB9FE0DDBE2990363BCA6C744E450DA4EFC84C17DEB1010A2C8F`. Build log: `work/vr/body-hand-build.log`. No tests or commits performed. Normal headset launch: `work/vr/novalis-headset-20261022-222647.log`. Stereo attached; physical user feedback remains pending. Prior executable/build record retained under `work/vr/before-body-hand-alignment/`.

## Y quick select and contextual interactions restored

Left Y now passes Triangle during gameplay as well as menus, restoring native contextual interactions and quick select. Holding Y preserves native left-stick wheel coordinates; release commits through the original menu system. Physical wrench equipment is requested once on a fresh grip press instead of continuously overriding native quick-select choices.

Native checks with/without VR and executable build completed. SHA-256: `C9BA508ECAAAD5D6496D268AE6BAB558F5B0F5275C457265A3A10D95EE6C4E67`. Build log: `work/vr/y-quick-select-build.log`; normal headset launch: `work/vr/novalis-headset-20261022-223102.log`. No tests or commits performed. Physical Y/wheel validation awaits user feedback. Prior executable/build record retained under `work/vr/before-y-quick-select/`.

## Controller-forward wrench and left-hand quick select

The wrench grip frame is rotated -90 degrees about controller-local X, preserving the palm anchor while pointing the shaft forward along the controller. Quick select now uses left-hand displacement from the Y-press position in the view plane captured at press time (6 cm activation / 3.5 cm inner dead zone). Release Y confirms. Joystick input is ignored during selection; gestures update menu-only copied axes after physics, so they cannot drive Ratchet. Native contextual Triangle interactions remain available.

Native checks with/without VR and executable build completed. SHA-256: `E9C468F28978DFF90C31794B29DAED4702860B210335F71F953DE7E08182D111`. Build log: `work/vr/forward-wrench-hand-select-build.log`; normal headset launch: `work/vr/novalis-headset-20261022-224045.log`. No tests or commits performed. Physical orientation/gesture feedback remains pending. Prior executable/build record retained under `work/vr/before-forward-wrench-hand-select/`.

## Tracked Blaster and starting inventory

The Blaster (item 15 / class 168) is added through native GiveItem only when missing, with normal starting ammo, vendor ownership and a free quick-select slot. Existing ammo is preserved. Select it with Y plus left-hand movement; the original mesh follows the right controller and its trigger drives native automatic fire. Optional tracked muzzle/forward override native body/camera aim and target correction while retaining original projectiles (305), cadence, ammo/statistics, collisions, damage, effects and sound. Blocked/lost tracking and menus suspend firing. The original animated Bomb Glove cannot prepare a duplicate bomb in tracked mode. A fresh grip recalls the wrench; snap/recenter preserves its physical grip edge.

Native checks with/without VR and executable build completed. SHA-256: `5776AC0946BF33F9C2A66DC206B09EC311AD0896B5FDF4F3C4AD166839076294`. Build log: `work/vr/blaster-build.log`; normal headset launch: `work/vr/novalis-headset-20261022-225256.log`. Starting inventory observation: `{'observed': True, 'owned': 1, 'ammo': 100, 'quick_select': [10, 15, 0, 0, 0, 0, 0, 0]}`. No tests or commits performed. Physical fire/aim/impact validation remains pending. Prior executable/build record retained under `work/vr/before-blaster/`.

## Pyrocitor, original brown hands, gun roll and startup ammunition

Corrected the Blaster's 90-degree left roll while preserving its muzzle/aim. Replaced the procedural hand meshes with original class-0 hand triangles, UVs and textures mounted independently on both controllers. The original Bomb Glove substitutes the right hand when used. Added native Pyrocitor with tracked nozzle/aim, continuous flames, native damage/collisions/audio/particles and ammunition use. Blaster/Pyrocitor are granted when missing; all owned ammo-using weapons are filled once per application launch to their native record capacity.

Native builds/checks with and without VR completed. SHA-256: `385EAEFE19F4838A92D050DCD7B24940EF40FEC55E09A21C86638364DFAF367A`. Build log: `work/vr/pyro-brown-hands-build.log`; normal launch: `work/vr/novalis-headset-20261022-230802.log`. Observed starting ammo: `[(10, 40), (15, 200), (16, 240)]`; quick select: `[10, 15, 16, 0, 0, 0, 0, 0]`; original hand geometry: `[{'side': 0, 'triangles': 582}, {'side': 1, 'triangles': 452}]`. No tests or commits performed. Physical fit/fire feedback remains pending. Previous executable/build record retained under `work/vr/before-pyro-brown-hands/`.
