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

## Arsenal and gesture traversal expansion

Baseline snapshot committed as `3ee729e`. Native arsenal/gadgets and controller aim, original hand/glove corrections, thrown Mine/Doom/Decoy support, optional foregrips, quick-select pages, long-A pack jumps, native ledge grip/pull, arm-stroke swim and continuous floor support sweep are implemented. Native script-camera interactions use the headset panel.

VR/non-VR compilation and executable build passed; no tests run. SHA-256: `58A8C1A5867D59429B0C39DB62F9AE1874C63DA80B8C169F591C81112D1A3F6F`. Build output: `work/vr/arsenal-traversal-final-build.log`. Normal runtime: `work/vr/novalis-headset-20261022-235646.log`. Stereo attached and starting reserves applied. Physical feedback remains pending as itemized in `overlay/docs/vr/IMPLEMENTATION_STATUS.md`. No additional commit created.

## Arsenal and gesture traversal expansion

Baseline snapshot committed as `3ee729e`. Native arsenal/gadgets and controller aim, original hand/glove corrections, thrown Mine/Doom/Decoy support, optional foregrips, quick-select pages, long-A pack jumps, native ledge grip/pull, arm-stroke swim and continuous floor support sweep are implemented. Native script-camera interactions use the headset panel.

VR/non-VR compilation and executable build passed; no tests run. SHA-256: `4EAF5042B8A435AD63ABB0DA46F67FA6C367ADB1F6A613987BAA2C9FA6169CB2`. Build output: `work/vr/arsenal-traversal-final-build.log`. Normal runtime: `work/vr/novalis-headset-20261023-000025.log`. Stereo attached and starting reserves applied. Physical feedback remains pending as itemized in `overlay/docs/vr/IMPLEMENTATION_STATUS.md`. No additional commit created.

## Palm-side ammunition, weapon grips, Morph aim and physical Walloper

Read-only native geometry inspection identified separate rear grips and muzzle/sight lists. Per-weapon grip mounts replace muzzle anchoring for the affected weapons; foregrips include lateral offsets. Four glove objects move to controller-local -X. The Morph ray and its locks follow tracked aim. Walloper physical punches sweep the native fist with directional push, native damage/effects and per-stroke deduplication; original trigger lunge remains available.

VR/non-VR compilation and build passed; no automated tests run and no commit created. SHA-256 `CF098A4872B3726BCEC9826C0CE0D69E510862705B52966076B5ADEC30F571BC`. Build log `work/vr/grips-morph-walloper-build.log`; normal runtime `work/vr/novalis-headset-20261023-084006.log`. Startup/stereo/inventory initialized; physical feedback remains pending.

## Immediate A and trigger+A pack controls

Removed the long-press charge and release-triggered ordinary jump. A now sends Cross immediately. Right index trigger held + a fresh ground A press queues the equipped pack's native high/long jump; Cross remains held for native jump height/glide. The combo reserves trigger input until release, preventing simultaneous gunfire or glove release.

VR/non-VR compilation and VR executable build passed; no automated tests run. SHA-256 `6D05FF8DB17738687C634133369199AE024F23A318EDDA9E80A006BD02ECABE3`. Build log `work/vr/immediate-jump-build.log`; normal Novalis runtime `work/vr/novalis-headset-20261023-091058.log`. Stereo/inventory initialized; control feel awaits headset feedback.

## Gadget grip and orientation correction

Original Novalis gadget meshes, skinning, attachment lists and item-class definitions were inspected read-only. Added independent grip mounts for Swingshot, Trespasser, Hydrodisplacer and Metal Detector; upright Hologuise/Drone Device remotes and holder-facing PDA. Taunter and actual Morph-o-Ray class 185 also receive measured grips. Trespasser prong tip is explicit; listless PDA has a held-body use point. Presentation and native tracked aim share the same placement.

VR/non-VR compilation and VR executable build passed; no automated tests. SHA-256 `8577B85E7255ACD48E14422E7CD6DF6494339226C43EC592BC2EE649BFA842B1`. Build log `work/vr/gadget-grips-build.log`; normal Novalis startup `work/vr/novalis-headset-20261023-091615.log` initialized stereo and inventory. Physical fit/readability and native contextual interactions await headset feedback.

## Spatial vendor with live locomotion

Native vendor screen targets and item/salesman/popup canvases are mapped to original animated monitor quads. Headset panels are hidden during shopping; canvas cameras stay independent of the headset. Tracked headset vendors retain hero/world updates, stick/room movement and A jump, bypass the native state-100 hold, hiding, fades/music pause, exit teleport/camera cut and weapon demo. Separate shopping pad/history uses right stick, fresh right trigger and Y. Physical vendors close on distance/death; PDA vendors use the same spatial presentation. Desktop/non-VR and gold upgrades retain native behavior.

VR/non-VR compilation and VR executable build passed; no tests, scripted interactions or smoke tests. SHA-256 `43505A14D60401593AB9A12C1DE83FAC95E81C4F3BFF5854BCC4FE309339137F`. Build log `work/vr/spatial-vendor-build.log`; normal Novalis startup `work/vr/novalis-headset-20261023-093010.log` initialized stereo and inventory. Vendor-specific rendering, controls and purchases await headset feedback; startup alone does not verify them.

## Kerwan default start and all destinations unlocked

Default VR scene is Kerwan / Metropolis (native level 3). Run.ps1 supports kerwan, novalis and calibration; gameplay scenes receive the data directory. All native level IDs 0..18 are unlocked through the existing RC_UNLOCK_PLANETS / GameState::unlock_planet hook, including native galaxy-map order, without marking missions completed. Direct VR startup uses the same default when environment overrides are absent. All 19 extracted level cores are present, and the existing loader/VR integration applies to native level changes; full per-planet VR playthroughs remain unverified.

VR/non-VR compilation and VR executable build passed. Normal Kerwan launch initialized stereo, starting inventory/ammo, native unlocks 0..18 and live level-3 ticks. No tests, scripted gameplay or all-level tour. SHA-256 `81DB59FAB86440C4C3C6328B27A9D0153283B3032DF9C7448DA4C24DF52D7B44`. Build log `work/vr/kerwan-unlocked-build.log`; runtime `work/vr/kerwan-headset-20261023-093425.log`.

## Physical Walloper launches Ratchet; glove point advanced into palm

A fresh physical Walloper punch supplies a bounded 18 game-unit/s launch for 10 simulation ticks along its initial 3-D tracked velocity, with gravity. Velocity is applied after native state physics and before original capsule integration/world/moby collision. Existing per-stroke hit/effect logic, rearm/cooldown and native trigger lunge remain. Invalid tracking/equipment/movement cancels the launch. Four throwing gloves now share controller-local point (-0.135,+0.01,-0.125), advanced 8 cm toward fingers after the user identified the previous point as the inner wrist. Hold/render/release use the same point.

VR/non-VR compilation and VR executable build passed; no tests or scripted gameplay. SHA-256 `A3EF3F4A7877CA5307B5A120B5F449FF73B338650CAFD040E267B3F2E12EE217`. Build log `work/vr/walloper-dash-palm-build.log`; normal Kerwan runtime `work/vr/kerwan-headset-20261023-094222.log` initialized stereo, inventory and level-3 ticks. Physical launch/impacts and palm fit await headset feedback.

## Left trigger as R1; native side/back flips; stable vendor screen meshes

Left index trigger now maps to original R1 with independent hysteresis. The queued ground combo runs the native side/back flip chooser first, then the equipped pack jump; native airborne/chained R1 rules remain. Right trigger is independent for weapons. Headset pose does not follow flip/crouch animation. The user's prior vendor session (kerwan-headset-20261023-093425.log) recorded 21 GPU slab unallocated-key errors at opening, matching initially empty monitor meshes rewritten after upload. All monitor layers now share one complete immutable unit quad with pose and UV crop in transform/material data, preserving native targets and free locomotion. Sparse opening telemetry supports physical feedback.

VR/non-VR compilation and VR executable build passed; no tests, scripted vendor input or jump sequence. SHA-256 `95B7EF0A7718B09C36544F3552F6F9C8EE68FE01EC75FE800A80F3633783A0A0`. Build log `work/vr/left-r1-vendor-build.log`; normal Kerwan runtime `work/vr/kerwan-headset-20261023-095603.log` initialized stereo, inventory and level-3 ticks. Physical monitor contents and left-trigger flips remain to be confirmed.

## Ship list, monitor occlusion, cinematic panels, recall and Walloper sensitivity

All 19 unlock flags were set in the prior user session, but the map order began `[3,0,1,2,...]`: zero truncates the native list. The repaired order has 18 contiguous destinations before its terminator (intro 0 remains unlocked; Veldin revisit is 18). Original opened vendor geometry was inspected read-only: opaque front faces sit +0.0224..0.0397 game units ahead of joint reference planes; content formerly at +0.002 was occluded. Vendor and PDA content now clears the faces at +0.05. Mode Cutscene/Ship omit the extra composed-frame panel. Empty-hand trigger recalls the last weapon through native selection, including latched wrench; firing requires release/new press and pending swaps are preserved. Walloper threshold is now 11 cm / 2.1 m/s with 0.10 s quiet rearm and 0.45 s cooldown.

VR/non-VR compilation and executable build passed. SHA-256 `DDAC7B6392FC9AFB76112BACB4FA305BF6CB430E327702B58AE0B6E5AF153607`; build log `work/vr/planets-monitor-recall-build.log`; normal launch `work/vr/kerwan-headset-20261023-102654.log` initialized stereo, max starting reserves and live Kerwan ticks, with startup map order `[3, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 0, 0]`. No tests, commits or interaction sequences. Headset presentation and gesture behavior await user feedback.

## Ship-menu joystick navigation

The user reported that neither joystick changed planets. Read-only Kerwan overlay parsing found focused list flags 0x139 (BUTTONS_ONLY): the original list reads raw_pressed, excluding analog-derived directional bits. XR now encodes the dominant direction of either joystick into D-pad buttons in page/freeze menus at 0.55 threshold, retaining analog axes for native widgets. Up/down selects; centre between steps; A confirms, Y returns.

VR/non-VR compilation and executable build passed; no tests or scripted/manual interaction sequence. SHA-256 `C1E7C158EE45F285F22FDAA74085CC7A0D4A1416FCC68FDCB5B61C770DBEA8F6`. Build `work/vr/ship-menu-input-build.log`; normal Kerwan launch `work/vr/kerwan-headset-20261023-103426.log` initialized stereo and live ticks without startup errors. Physical navigation remains to be confirmed.

## Planet transition: effect target mismatch and missing VR travel view

The user log `work/vr/kerwan-headset-20261023-103426.log` confirms destination 2 loaded and level 03-to-02 swapped, then GPU validation rejected an Rgba8Unorm effect pipeline in an Rgba8UnormSrgb pass. Effects now route according to ready pipeline attachment format as well as marker fallback during compilation, covering retained draws after old-level entity deletion. The exact offending entity was not identified by the log. Native sky initialization no longer overrides the retained XR cameras. VR now mirrors native world/sky layers and follows the authored camera during the blanked travel stage, preserving head-driven pitch/roll and native cards/HUD, with no duplicate cinematic frame.

VR/non-VR compilation and executable build passed. SHA-256 `C1EC307E3D4852BFEB91C9147E07E320BC7FC4CAB24471F092DB66E7B20B944C`. Build `work/vr/planet-transition-render-build.log`; normal Kerwan startup `work/vr/kerwan-headset-20261023-104714.log` initialized stereo and live gameplay without startup errors. No tests, commits or interplanetary interaction sequence. Full travel/arrival in headset remains pending user feedback.

## Combat, Hyper Strike, magnetic tracks, physical ledges and haptics

Walloper restores its original broad three-sphere hit delivery, damage 3 and effects; the obsolete tiny fist-only check and aim-target filter are removed. Gesture sensitivity is 7 cm / 1.45 m/s with quiet rearm and cooldown. Gesture propulsion is removed; tracked trigger attacks suppress horizontal launch. Wrench is excluded from trigger recall, while tracked grip preserves the native Circle saved weapon. The wrench is 20% larger, elongates by up to 15% with a swing and has native reach extension/radius; crates demand a faster intentional strike. Downward airborne gestures request native Hyper Strike state 0x14, preserving native gates, accelerated fall, damage 2 and shockwave. Native melee uses the tracked wrench grip. Gold-bolt scripted cameras omit the duplicate full-frame panel. Magnetic tracks transport the camera/hand frame with native body up and collision-resolved room movement. Fixed ledge hand anchors and opposite hand displacement drive native probes/hang physics; a paired downward arm shove requests the same climb as A. Right-controller output haptics cover actual wrench hits and Blaster spawns.

VR/non-VR compilation and executable build passed. SHA-256 `2B5A210AB7B1B4CE643D6B8433F4672DB82DF028CC24717E26C109D6FAE569A0`. Build `work/vr/combat-traversal-build.log`; normal Kerwan startup `work/vr/kerwan-headset-20261023-113506.log` initialized stereo and live gameplay without startup errors. No tests, commits or gameplay interaction sequences were run. Physical gesture thresholds, impacts, magnetic tracks, ledge behavior and haptics await user feedback.

## Arsenal/gadget haptics, bilateral damage and cinematic skip

Both OpenXR controller outputs are bound. `vr/haptics.rs` derives distinct recoil/stream/capture/attachment/puzzle/scan patterns from native committed ammo-use, actual Blaster spawns, missile counts and gadget states. Heavy support grips get reduced left-hand recoil. A native accepted-damage counter survives same-tick healing; a same-body health-drop fallback handles direct lethal/environmental damage. Damage applies a strong two-hand priority pulse with a softer tail, including active controllers during optical occlusion. Focus/menu loss clears queues; reload/respawn rebase counters.

Fresh right-stick click during scenes/movies queues the native Start+shoulder skip combination through the opening window. Players retain normal fades, actor/dialogue cleanup and progression. Explicit VR skip also permits normally unskippable clips for the current movie only; global replay state is retained. The held click is suppressed through release after returning to gameplay. Ship travel/loading is not fast-forwarded.

VR/non-VR compilation and executable build passed. SHA-256 `B388E2CD1E14965819A52ED24094C054FE6EB0514BD869DCA2180C89711CDDC7`. Build `work/vr/arsenal-haptics-skip-build.log`; normal Kerwan startup `work/vr/kerwan-headset-20261023-121719.log` initialized stereo and live gameplay without startup errors. No tests, commits or gameplay interaction sequence were run. Actual haptic feel and cinematic skipping await user feedback.

## Native planar Walloper lunge, strong strikes/ledge shove, dorsal Swingshot and visible body

Physical Walloper punches enter original state 0x20 instead of delivering a separate instant hit. Original readiness/grounding gates, windup, horizontal dash/ramp and three-sphere damage window run. Tracked dash direction is planar; vertical movement stays with native gravity. Wrench enemy hits now need 4 m/s tip, 1.8 m/s hand and 10 cm travel; crates need 5 m/s tip, 2.2 m/s hand and 12 cm travel, with no wrist-only bypass. Ledge translation ignores sticks in XR and native input after d-pad fallback; climbing by gesture needs both arms down 22 cm at 2.2 m/s within 0.10 s. Slow/single-hand climbing is removed; native A and grip-drop remain. Swingshot class 208 rolls 90 degrees from wrist right to wrist top about its anchored grip and forward axis. Original Ratchet body meshes are visible for inspection, retaining native animation.

VR/non-VR compilation and executable build passed. SHA-256 `28C4E689EF1F0BFDFCA214CC5E1712CA5D6567BF9513A54C1F21A624F9F50DF6`. Build `work/vr/body-planar-dash-strikes-build.log`; normal Kerwan startup `work/vr/kerwan-headset-20261023-122601.log` initialized stereo and live gameplay without startup errors. No tests, commits or gameplay interaction sequences were run. Physical force/shove feel, dash behavior, Swingshot placement and visible-body presentation await user feedback.

## Native glove/grip poses, deployment haptics, spatial pause and Visibomb monitor

Gameplay RatchetMesh excluded from headset layers again; simulation/animation retained.

Original complete brown glove/cuff geometry: left joints 74..91, right 54..71. Same native layered Ratchet palette; free palm controller placement; exact held-hand pose through native item attachment. Item scale propagated to tracked render presentation.

Native b18=2 determines two-hand holds, including Hydrodisplacer. Authored primary/support palm centres determine controller mount/latch and support-hand orientation/fingers/cuff, replacing approximate foregrip positions.

Distinct selectable-item amplitude, duration, frequency and accent profiles. Pulses follow native take-out sequence mechanical motion changes and animation sound markers; ready transition detent, static glove/wrench appearance cue. Focus/tracking/menu cancellation, bounded opening, firing/use/damage priorities retained.

Original clipped HUD primitives and textures batched into world meshes, native animated frame geometry and true stereo menu preview models. Opening anchor remains fixed in world; no composed frame panel. Native navigation/pause retained.

Native missile camera GameFrame refreshed every frame, sampled only on original articulated class-163 joint-9 monitor face. Headset panels/scanline mirroring suppressed; normal hero-relative HMD sky/fog retained, native steering/boost/detonation controls preserved. Decorative screen returns after remote flight.

Left trigger remains native R1, visible blended crouch eye height 0.85 units, immediate ground A/flip queue independent of equipped pack. Native directional flip timing, pack gates and airborne behaviour retained. Wrench grip/release throw unchanged.

VR/non-VR checks and executable build passed. SHA-256 `7E00E415C4913953EBB9E62B4F147C7D9DF0448F6233256228C9F2AD34060670`. Build `work/vr/spatial-equipment-build.log`; normal launch `work/vr/kerwan-headset-20261023-125408.log`. NOT RUN per AGENTS.md. No gameplay interaction scripts or sequences; no commit. Pending user headset feedback on glove cuffs/grip alignment, support handles, deployment rhythms, spatial pause layout/occlusion/readability, missile inset and crouch comfort.

## Right-controller pause pointer and trigger confirmation

Tracked right-controller forward ray intersects original spatial menu canvas; pixel coordinates match native 512x416 layout. Headset-only beam/contact dot with cyan enabled targets.

Hover updates native widget focus/cursors through original leave/enter behavior. List rows, equipment/gadget grids, toggles, camera/Port Options, sound controls, help icons and quit choices use clipped original layout bounds. Fresh trigger confirms via native actions. Port arrows scroll; trigger-held pointing adjusts volume sliders.

One click per trigger press/native tick; release required after opening on a held trigger. Transitions reject old page targets; inactive/lost hand or focus clears queued clicks. Menu trigger is consumed through gameplay return until release, preventing unintended shots. Native buttons/stick fallback retained.

VR/non-VR checks and executable build passed. SHA-256 `73DC66D3EDC93D1F5016D51592D3FD36D36A4774E73311C9D3BE248C23EBFC63`. Build `work/vr/pause-pointer-build.log`; normal launch `work/vr/kerwan-headset-20261023-133613.log`. NOT RUN per AGENTS.md. No gameplay interaction scripts or sequences; no commit. Pending headset feedback on ray alignment, hover/confirm behavior, page changes, audio slider targeting and release handling.

## Wrench wall collision and Hyper Strike impact feedback

Native once-per-grounded-slam landing counter drives strong right-hand impact, softer left-hand shock and a short tail; light initiation cue. Damage haptics retain priority.

Held native wrench/head and original gripping hand are constrained by solid level geometry using previous safe head sweep, radius overlap and grip/eye rays. Native sparks/sound and force-scaled haptics require 1.3 m/s normal head approach plus 0.65 m/s physical hand motion. Cooldown 0.22 s; withdrawal required to rearm. Enemy/crate swing gates and throwing input retained.

Paused capture preserves fully valid tracked hand poses for the spatial pointer while resetting combat gestures and button edges.

VR/non-VR checks and executable build passed. SHA-256 `F36903BCD795BEA778FCA58D03B902E916289F65361D6D0FFE278B7B64F37D49`. Build `work/vr/wrench-wall-hyper-build.log`; normal launch `work/vr/kerwan-headset-20261023-134947.log`. NOT RUN per AGENTS.md. No gameplay interaction scripts or sequences; no commit. Pending headset feedback on wall clamping, contact rearming, Hyper Strike landing timing/intensity and paused pointer interaction.

## Wrench rebound, stable spatial pause content and anchored ledge correction

Held wrench/native hand recoil 3-5.5 cm opposite the measured incoming stroke over 150 ms, collision constrained and excluded from physical hit history. One recoil per accepted wall impact.

Explicit painter-order transparent batch bias in both eyes; repeat samplers for monitor effects; additive FIX-style noise alpha. Content and pointer plane moved 0.035 units ahead of nominal monitor faces; native 3D frames/previews retained.

Actual world-space hand-anchor error replaces integrated controller deltas. Held ledge targets reject automatic velocity-predicted reprobes. Validated steps limited to 0.045 units/tick with 0.008-unit dead zone; full ledge/found-flag rollback on failure. Catch tangent preserved; distant grip tracking released; anchors follow moving carriers. Native joystick ledge movement remains disabled; A/two-arm shove retained.

VR/non-VR checks and executable build passed. SHA-256 `E8EC2F747E67B88887C31F48B92E7E9260A5EA03139BFBB7BC23915CC7C7AE2F`. Build `work/vr/rebound-menu-ledge-build.log`; normal launch `work/vr/kerwan-headset-20261023-141817.log`. NOT RUN per AGENTS.md. No gameplay interaction scripts or sequences; no commit. Pending headset feedback on wrench rebound, consistent pause text/monitor visibility and stable anchored hand-over-hand ledge traversal.

## Ledge grip/climb haptics, stronger wrench strikes and crate collision

Per-hand native accepted anchor counters trigger a light latch pulse (0.23/25ms/175Hz). Accepted native ledge-climb launches by A or two-arm shove trigger both hands (0.65/75ms/85Hz plus 0.24/30ms/145Hz tail after 95ms). Rejected climbs and held grips cannot repeat feedback; damage retains priority.

Enemies require tip 5.5m/s, hand 2.4m/s and 14cm stroke. Crates require tip 6.5m/s, hand 2.8m/s and 16cm stroke plus actual contact. Relative to previous tip-speed minima, kinetic energy demand for equal effective mass rises about 89%/69%.

Separate crate-only native scene query includes meshes and primitive colliders, preserving independent world blocking. Intact crates stop the held model/native gripping hand with existing contact recoil/haptics. Crate damage requires actual collision contact and the stronger stroke; native destruction disables the original collider normally.

VR/non-VR checks and executable build passed. SHA-256 `FE8172DCAF34B0184D4A8ABDD294722A03C5588A06F2E49108DE97E07FE310DA`. Build `work/vr/ledge-haptics-crate-contact-build.log`; normal launch `work/vr/kerwan-headset-20261023-143632.log`. NOT RUN per AGENTS.md. No gameplay interaction scripts or sequences. Pending headset feedback on ledge grip/climb haptics, stronger wrench strikes, intact crate blocking and destruction contact.

## Normal/debug/debug-gold launch modes and shared gold VR settings

Default native boot/logos/Press Start/main menu, New Game/Load Game with native progression. Forced debug level, story skip, grants, full-ammo refill, planet unlock, beat-game and skip-logo overrides disabled. Separate native memory card per launch preset.

Debug names the previous direct-Kerwan/all-items/all-planets/full-initial-ammo launch. Debug-gold uses the same setup plus native gold IDs [19,10,11,16,17,15,21,20,25,9], installed in boot state and debug inventory.

Gold flags preserve the ordinary weapon item IDs/model classes. Existing placement, grip/muzzle orientation, native primary/support-hand poses, glove palm placement, two-hand aim and deployment/use haptic profiles are shared automatically; native gold appearance and upgraded effects remain.

Native title camera anchors the XR origin, frontend UI follows a fixed tracking-space pose through authored scene camera changes, full-frame boot fades retained and title layers mirrored in order. Existing spatial gameplay pause remains.

VR/non-VR checks and executable build passed. SHA-256 `B6281BD655CD058040C1909078FD76382EC90DF5FFC92B8968B963D9B19F2278`. Build `work/vr/launch-modes-build.log`; debug-gold launch `work/vr/debug-gold-kerwan-headset-20261023-144710.log`. NOT RUN per AGENTS.md. No gameplay interaction scripts or sequences. Pending normal-mode boot/New Game/Load Game interaction and physical frontend/gold-weapon headset feedback.

## Walloper hand, ship pointer destination and physical tool insertion

Hide only native primary-hand mesh while holding the Walloper; preserve the weapon housing and secondary hand.

Pointer hover updates actual PageMenu.dest as well as native planet-list cursor and mission state before confirmation. Galaxy dots for known planets are also pointer targets. Native directional repeats are suppressed while pointing.

Hydrodisplacer/Trespasser tips engage their original target colliders: inward alignment, 3.5 cm tracked controller translation, 1.2 cm penetration, native readiness, grounded nearby hero and unobstructed approach. No locomotion/animation/wrist-only stroke. Latched activation requires withdrawal before reuse; preserve target during native cutaways/puzzle. No ground-moby spoof. Original water/lock logic and use haptics remain; suppress native body pull for physical insertion. Native trigger fallback retained.

VR/non-VR checks and executable build passed. SHA-256 `4B8EAB6DEBB659DD38238A55CB6CA0EC5EDDF8343168BC3EC2C7692F1A1607D8`. Build `work/vr/gadget-insertion-build.log`; debug-gold launch `work/vr/debug-gold-kerwan-headset-20261023-152359.log`. NOT RUN per AGENTS.md. No gameplay interaction scripts or sequences. Pending headset feedback on hidden Walloper fingers, pointed travel destination and tool insertion alignment/feel on native Hydrodisplacer pads and Trespasser locks.

## Headset heading, spatial audio, magnetic tracks and grind camera

Native free-movement body heading follows levelled headset orientation, including stationary turns and snap turns. Local surface normal retained; movement direction independent of facing for back/strafe. Rail/ledge/attack/scripted poses retain authored body orientation.

Native hero input receives the headset locomotion frame, raw VR stick axes are no longer transformed through the desktop camera, and magnetic planar velocity follows the surface-projected requested stick direction.

Headset position and full forward/left/up basis feed sound allocation and per-tick mixing; post-move body translation applied to sampled eye. Native authored camera and desktop listener fallback remain independent.

Grind yaw follows the native rail tangent instead of Ratchet's sideways stance; grade follows with pitch blending capped at 1.8 rad/s, including level recovery after exit. Physical head look-around/lean retained; entry uses current tracking head position. Rail lean cannot displace native capsule; exit offset goes through ordinary collision-constrained room-scale movement. Pause retains rail reference and pitch blend uses elapsed time once per rendered frame.

VR/non-VR checks and executable build passed. SHA-256 `B4250C4ED75823CD4509327FC4149F479CD5CCC4BFB1713B3D58E9B7AF7C871B`. Build `work/vr/head-facing-rails-build.log`; debug-gold launch `work/vr/debug-gold-kerwan-headset-20261023-162116.log`. NOT RUN per AGENTS.md. No gameplay interaction scripts or sequences. Pending headset feedback on stationary body turns, stereo channel direction, forward/strafe/back movement on magnetic walls/ceilings, and grind rail entry/curves/grades/exit comfort.


## First CPU visibility optimisation

The live captures motivated a single parallel entity traversal for every active view, replacing only Bevy's CPU culler in original-game VR. Hidden entities are rejected once; transformed bounds and retained per-thread queues are reused. Per-eye frustum/layer/range predicates, sorted outputs, visibility state tracking and the original game's LOD/culling remain. Minimum parallel batch: 2,048. Range/gizmo ordering preserved. The VR dev build now also optimises the game crates at level 3 while retaining debug symbols. Graphics settings remain unchanged. Details and rollback: [PERFORMANCE.md](overlay/docs/vr/PERFORMANCE.md).

Non-VR check, VR check and opt-level-3 executable build passed. SHA-256 `AB8B1445DEE514FE7890CB995E9A7CF1FBD1B19018DBCAA8D3DDCB594410B042`. Successful build: `work/vr/build-20261023-165443.log`. New module formatting, PowerShell syntax and source/script payload equality also checked.

The first build generated the new executable but Cargo could not replace the running image. The original executable (B4250C4E) was preserved as `target/debug/rerac-running-B4250C4E.exe`, leaving PID 258700 running. Rebuilding then completed successfully and installed the new binary at `target/debug/rerac.exe`. No session was closed or restarted.

NOT RUN per AGENTS.md: unit tests, smoke tests, scripted gameplay and new headset benchmark. The binary is built but has not been launched. Performance benefit and visual equivalence still require matched headset feedback; no FPS gain is claimed. No commit created.

## Optimized startup recovery

User-requested launch exposed two independent startup issues. The main thread first waited in SteamVR vrclient_x64.dll / WaitNamedPipeW; gracefully closing SteamVR and reopening it restored the namespace connection. Startup then exposed a Bevy resource_scope assertion in the new visibility installer: PostUpdate system initialization recreated the temporarily extracted Schedules resource. The installer now temporarily extracts only PostUpdate and keeps Schedules available.

Non-VR/VR cargo checks and optimized executable build passed. SHA-256 `D3914197C4FEE54FEF186719CAC0184AC43787732047DFBD2C3F250690947CED`; build `work/vr/build-20261023-170529.log`. Ordinary debug-gold Kerwan headset startup observed in `work/vr/debug-gold-kerwan-headset-20261023-170736.log`. No unit tests, smoke tests, scripted play, performance benchmark or commit. Physical visual feedback and matched-scene FPS comparison remain pending.

## Performance experiment rollback after headset feedback

The user confirmed headset access after PICO reconnection but reported worse lag than before the changes. The new visibility wall time alone does not establish the cause. Both changes were rolled back together: original game.rs/mod.rs and Build.ps1 restored byte-for-byte from package HEAD; the previous B4250C4E executable restored after closing the experimental game cleanly. Experimental source is retained but unregistered/uncompiled, and the rejected D3914197 executable/build record are preserved locally. Graphics settings were unchanged; SteamVR was not restarted during rollback. The previous executable was relaunched in debug-gold Kerwan. No new compilation, tests, scripted interactions, benchmark or commit were performed for this rollback. Post-rollback headset feedback remains pending.

## Native Release PC configuration

Built original VR engine sources in Release with RUSTFLAGS=-C target-cpu=native and four concurrent build jobs. Non-VR and VR compilation checks and native executable build passed. SHA-256 `9796AF47120BBF3043AE33D56087E12ED6368BB10D6EFBFF530C81925B91F8E3`. Source game.rs/mod.rs and Build.ps1 match package HEAD; rejected visibility.rs removed from both active trees after matching its preserved archive. No engine source rewrite or graphics preset changes.

Dedicated NVIDIA rerac.exe maximum-performance profile activated after relaunch: GPU P0 at about 1,785 MHz rather than earlier 255-630 MHz samples. Default thread pools retained after no clear gain from RC_THREADS=8. The local performance launcher selects Release and production Vulkan WGPU debug/validation flags. Runtime `work/vr/debug-gold-kerwan-headset-20261023-180424.log`. pc_performance_configuration records the active Release capture or explicit pending status; earlier scenes varied, no controlled benchmark or guaranteed FPS improvement. No unit tests, smoke tests, scripted gameplay or commit.

## Rail/ledge comfort, gold support grip and immediate VR Hyper Strike

Direction sampled over 1.5 units behind and 2.5-5 ahead; yaw response 350 ms/dead zone 0.006 rad/max 1.05 rad/s; pitch 500 ms/dead zone 0.008 rad/max 0.35 rad/s; translation 80 ms; entry gaze retained, headset look tracking direct.

Neutral eye about 38 cm below palms; capture 80 cm horizontal, 50 cm vertical, 48 cm initial wall-normal reach. Two gripped hands push down 12 cm at 1.15 m/s within 180 ms. Lateral traversal logic unchanged.

Measured original model side-handle anchor (0.125,-0.415,-0.270) shared by gold support capture/aim and rendered palm. Native finger pose retained; ordinary Devastator untouched.

Shared native eligibility/input-window/height/cable code; native flip gates after frames 18/20 retained. Physical preparation 5 cm down at 1.2 m/s with separate interpolated 28 ms velocity history. Buffer uses native timing until eligible. VR dives immediately in native falling phase, retaining native acceleration, terminal speed, shockwave, damage and landing haptics. Normal/button windup and ordinary swing thresholds unchanged.

VR/non-VR cargo checks and Windows executable build passed. SHA-256 `79C3B1173214D4A15247ECC08A02CE59EE4EB691337D0CC5E6358FF3F41EE5D3`; build `work/vr/rail-ledge-hyper-comfort-build.log`. NOT RUN: user explicitly requested no game launch/restart. NOT RUN per AGENTS.md. No gameplay interaction scripts or sequences. Pending headset feedback on rail stability/comfort, lower hanging camera, easier grip/pull-up, gold Devastator handle and immediate physical Hyper Strike.
