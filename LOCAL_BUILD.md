# Native Windows build — 2026-10-22

The original desktop checkout (the upstream repository) is at upstream commit
`89c87e98f9d8296187df2a6310c67b32144090d3`. Its source remains unchanged.
VR integration lives in a separate build checkout (`rerac-vr-build`), a Git
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
& '.\scripts\vr\Run.ps1' -Scene calibration -DesktopPreview
& '.\scripts\vr\Run.ps1' -Scene novalis -DesktopPreview -DataDir '<extracted RAC1 data>'
```

Use WASD to move, Q/E down/up, arrow keys to look, Shift for faster movement,
R to reset, and Escape to exit. Add `-SmokeTestSeconds 8` for automatic exit after
the rendering check. Logs are in `work/vr/readiness-*.log` in the VR checkout.

## Rebuild and launch with a headset

```powershell
Set-Location '<build checkout>'
.\scripts\vr\Build.ps1
.\scripts\vr\Run.ps1 -Scene calibration
# After calibration has passed on the headset:
.\scripts\vr\Run.ps1 -Scene novalis -DataDir '<extracted RAC1 data>'
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

Original integration Windows executable (before the controller remap below): `target/debug/rerac.exe`

SHA-256: `FD9FA600715B98A6A0CDEC487BA4CDA97A86A6957FD5D47BEE1A0F62E44FC5B0`

- Native VR build and non-VR check: PASS (`work/vr/original-game-ready-build.log`).
- Full workspace/all-targets check with dev + VR: PASS.
- Workspace unit suite including VR: 1,089 passed, 3 ignored. Diagnostic standalone suite: 15 passed.
- Package checks: 21 passed. Guarded overlay apply checked on a clean worktree.
- Original-game desktop/stereo smoke: 487 ticks, 327 render frames, 14 Ratchet meshes, two world views and two sky views; minimum 701 world draws and 5 sky draws per eye. No render errors.
- Scripted movement/jump/wrench/fire went through the original pad/tick. Position changed from [162.5303, 136.3935, 60.0] to [159.8904, 142.3601, 60.0]; maximum game z 61.2268, then returned to z 60.
- Headset launch: stereo_rendering_and_basic_controller_play_confirmed. Current log: `work/vr/novalis-headset-20261022-205230.log`. The user confirmed Ratchet/scenery visibility and left-stick movement in the headset. Broader combat/menu and comfort checks remain pending.

The original main camera drives the third-person VR rig. Shared culling/LOD/lighting use that camera with a wider HMD envelope; PS2 single-view occlusion masks are disabled only for VR. World/sky eyes use runtime projections, and original fade/underwater/tint effects are copied to them. HUD/menu panels reuse original output. The PC view refreshes every four headset frames during gameplay; original menu camera activation and full-rate menu output are preserved.

PowerShell 7/.NET on this host preserves an empty native environment entry when `[Environment]::SetEnvironmentVariable(name, $null)` is used. The launcher now removes absent optional variables through the Env provider, including empty capture variables, so the original capture-and-exit mode is not accidentally requested.

Start with `scripts/vr/Run.ps1 -Scene novalis -DataDir <extracted RAC1 data>`. SteamVR is elevated on this machine, so the local headset launcher uses the normal Windows RunAs/UAC flow to match it. No SteamVR security settings were changed.

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

## Native ledge hands, grip priority, swapped Start/Triangle and High defaults

Accepted hands use Ratchet's native reach/hang palette without weapon layers and retain original wall-facing wrist/finger pose, depth and height at their individual edge anchors. Other hands retain ordinary controller presentation. One hand suffices for hang/sideways traversal; two-hand pull-up gesture retained.

Left grip, available ledge candidate and attached/hanging right hand block a new wrench request. Fresh request waits one native collision/probe tick. Suppressed squeezes require release/new press; a ledge release cannot throw. Accepted right anchors hide held models in the same presentation frame. Ordinary throws outside ledge use retained.

Y (Index left B/Microsoft left menu) is native Start/pause/frontend/queued cinematic skip. Right-stick click is native Triangle/context/vendor cancel/held quick select; left-hand displacement chooses the sector. Held right-stick click plus horizontal stick changes page. Left-stick click remains Select/map.

Port Options High preset requests 8x MSAA (GPU capability clamp), Sharp anisotropic textures, Maximum model detail and SharpPixels HUD. First VR startup adopts High plus enabled shadows, including existing settings, with a one-time marker; later user changes persist. Forced VR RC_MSAA=0 removed; world/sky headset cameras follow native RenderSettings MSAA. Original desktop defaults and display geometry retained.

VR/non-VR cargo checks and Windows executable build passed. SHA-256 `2DC908457014F7A405CC68C3F412DD8C65FA85C164B8B2AF7A818AFB2BBA1260`; build `work/vr/ledge-hands-controls-high-build.log`. NOT RUN: prior user request not to launch/restart the game retained. NOT RUN per AGENTS.md. Existing binding expectations updated; no tests or gameplay interaction scripts run. Pending headset feedback on original hooked ledge fingers, single-hand retention and wrench suppression, new Start/Triangle mapping, and High/MSAA rendering and performance.

## Physical cranks and first-person cinematics

Held wrench head seats within 0.50 units of class-280 socket; right-grip hold and optical quaternion twist/tangential hand travel advance the original angle/progress and native consumers/save/mission completion. Withdrawal and release disengage without throwing. Tracking discontinuities do not turn the crank. Native desktop crank path retained.

Right-hand tracking displacement chooses sectors while right-stick click (Triangle) is held. Release confirms; right-stick horizontal edges change pages. Y remains Start/pause/skip.

Native event timelines and progression retained; VR suppresses letterbox primitives, gameplay director holds/poses/teleports/animation overrides and blocking cutaway entry fades. Collision, movement, jumping and weapon input continue. Mode-2 actor scenes, mode-6 travel and puzzle controls retain their native state machines.

Engine/dialogue scene eye follows the rendered class-0 Ratchet head, using the already-evaluated native palette. Head rotation remains tracked; duplicate Ratchet scene mesh hidden. Listener follows the VR eye. Ship takeoff/landing uses the animated ship origin plus native per-ship Ratchet seat and 0.85-unit seated-eye offset, with bounded/smoothed heading. Real video remains video.

Cable-slide state 0x74 targets eye height 0.55 units below the cable, capped to 0.55..1.15 units above feet, through native VR posture blending.

Outbound and returning wrench models stay level and spin around world up, independent of release wrist pitch/roll. Native trajectories, hit tests, bounce and catch retained.

Non-VR cargo check, VR cargo check and Windows executable build passed. Guarded patch regeneration and git diff --check passed. SHA-256 `E092B0F431794EB2987087DCCB53F1324A8BB7916D8E16B20DA6185D77CBBBA6`; build `work/vr/cranks-cinematics-complete-build.log`. NOT RUN per AGENTS.md; no smoke tests, headset launch or gameplay scripts run for this change. Existing debug executable was closed gracefully to allow rebuilding; game not restarted. Pending headset confirmation of crank socket geometry/turning, short-cutaway movement during all trigger variants, head/seat positions on all three ships, cable clearance and horizontal wrench spin.

## Crank progress from walking around the bolt

After seating the held wrench, progress comes exclusively from the actual native capsule orbit around the bolt. Left-stick locomotion and collision-resolved room-scale walking both count. Rotating the wrist, moving only the hand or turning the head contributes no progress. One full body orbit supplies one native turn, without discarding accepted angular movement through an extra speed cap.

Right grip keeps the wrench head on the socket; snap turns retain the attachment and rebase the body-position history. Release/withdrawal still disengages without throwing. Teleports, large per-tick displacement and crossing the bolt center do not supply progress.

Original progress consumers, turn counts, unwind, save, mission and completion paths retained. Bolt visual rotation uses the original crank::bolt_follow helper driven by Ratchet angle.

Non-VR cargo check, VR cargo check and Windows executable build passed; patch regeneration and git diff --check passed. SHA-256 `52AC4C99081499CEFB91BEAF1F2728DF51350BD6B24DE12E9883CDA343DEEFBD`; log `work/vr/crank-body-orbit-build.log`. NOT RUN per AGENTS.md; no game launch or gameplay scripts. Pending headset confirmation of attachment and walking around native Novalis/Eudora/Batalia cranks, using both stick and room-scale locomotion. Previous cinematic and ship-seat validation remains pending.

## Ship boarding camera, raised cockpit and longer crank reach

Planet-side takeoff and landing follow the actual streamed Ratchet actor head attachment (native list 4), including boarding, sitting and climbing out. Headset rotation remains free, ship yaw follows gradually. Raised seated fallback from native seat +0.85 to +1.45 game units, used when the actor is absent. Animated head attachment uses current joint modifiers and adds 0.15 units for the eye.

SUB_FLIGHT is excluded from pilot_view, restoring the pre-existing native external loading camera in space. Planet-side pilot/actor camera remains for takeoff, landing and flyaway.

Normal engine scenes also resolve the native head attachment chain instead of confusing manipulator list 0x18 with bone index 0x18.

Attached wrench uses the original crank sequence 0xb frame 2. Bounded axial stretching connects the controller grip to the bolt socket and keeps the head exactly seated; the holding glove remains rigid. This presentation cannot generate melee damage.

Hand reach increased from 1.35 to 2.40 units, with 250 ms grace beyond hand reach or body distance 3.40. Grip release remains immediate and cannot throw from the socket. Orbit progress remains based on collision-resolved native capsule movement and accepts radius up to 3.0, preserving turns, saves, missions and native completion.

Non-VR cargo check, VR cargo check and Windows executable build passed; patch regeneration and git diff --check passed. SHA-256 `CC59A3F3ED4E8C0E60A82FECBFE167E34A6E84E2C5F33ED962F533E6A6865E46`; log `work/vr/ship-boarding-crank-reach-build.log`. NOT RUN per AGENTS.md; no game launch or gameplay scripts for this revision. Previous debug game closed gracefully for compilation; not restarted. Pending headset confirmation of cockpit height, boarding/disembarking eye alignment on all three ships, original space loading view, crank pose/shaft extension/glove alignment and longer retention.

## Individual Clank pack haptics

Bilateral 165-180 Hz fine alternating rotor beats, native accepted long-jump takeoff/high-jump acceleration-curve/double-jump lift windows, stronger lift and softer glide, spin-up and release cues. No mere-button/equipped-idle rumble.

Original thruster_flame::thrusting predicate drives accepted boost/flips/double-jump and ledge flame windows. 50-90 Hz ignition, stronger boost, quieter glide/hover, separate stomp rise/dive with speed-dependent descent rumble. First native stomp_ground edge gives 0.92 / 105 ms / 50 Hz to both hands plus a softer tail.

Native Hydro swimming and ready/shown module gate feedback; no idle-on-land vibration. 50-65 Hz smooth propulsion load follows native speed; R1/R2 underwater acceleration selects stronger boost with its own onset.

Independent pack phase/cadence and negative owner IDs preserve pack rumble across hand-weapon selection. Background pack priority 0 is below all weapon feedback; ignition uses priority 4, stomp impact 8 and accepted damage remains 20. Replaces old generic pack launch/glide/Hydro cues. Pack feedback reaches connected controllers through brief optical occlusion.

Pause/focus loss, stopped native tick, hidden Clank/body changes, pack swaps, death and lost OpenXR session clear pack-owned cues. Motor stop has a gentle release only after actual operation. No game-physics or native control changes.

Non-VR cargo check, VR cargo check and Windows executable build passed; git diff --check passed. SHA-256 `F357BD57636A0EC4B2421AC3598B73FB0D0A45EB0FEC92FC0BD040C7A8D4AF13`; log `work/vr/clank-pack-haptics-build.log`. NOT RUN per AGENTS.md; no game launch or gameplay scripts. Pending headset confirmation of all three pack intensities/rhythms, boost/hover/glide/stomp timing, underwater acceleration, cancellation and mixing with weapon/damage feedback.

## Solid VR hands and held weapons

Conservative spheres from slices of the actual posed mesh triangles, including original brown glove cuffs, articulated/deployed weapon parts and gold counterparts. Bounds use the same animation palette, scale and affine model rows as rendering.

Native solid world/moby meshes and live crate meshes/primitives; excludes enemy bodies and disabled/destroyed colliders. Previous safe positions and offset sweeps catch fast crossings; initial eye recovery handles tracking starts beyond a wall. Iterated overlaps permit sliding. Raw controller samples and gesture velocities remain physical.

Held weapon plus its native gripping hand and accepted supporting hand translate as one group. Free hands resolve independently. Native ledge anchors and engaged crank wrench remain intentional contact exceptions; thrown wrench keeps native flight collisions.

Gun/gadget native tracked pose and muzzle are corrected before firing; render-time Visibomb live monitor follows the stopped weapon. Held glove ammunition uses safe preparation/release positions and follows the render group before native dynamic upload. Wrench crate query copies enable primitive-only flagged line queries.

Non-VR cargo check, VR cargo check and Windows debug executable build passed; git diff --check passed. SHA-256 `FB4FAE618B038C30F1B84917836958720B4FF994780F04A70F02431CE4D1CBB5`; log `work/vr/solid-hands-weapons-build.log`. NOT RUN per AGENTS.md; no game launch or gameplay scripts. Pending headset checks of wall/corner contact, fast strokes, breakable crates, all weapon/gadget sizes, two-hand grips, gloved ammunition and Visibomb monitor alignment; frame cost and conservative clearance need physical confirmation.

## VR interaction and presentation polish — 2026-10-24

Suppress VR wrench after-images and near-eye sparkles/bursts; persistent accepted hit events drive stronger wrench feedback. Suck Cannon committed captures and creature shots have heavy, distinct recoil. Bolt collection and actual grind rails have low-priority feedback.

Green cockpit seat moved forward; full authored ship attitude and boarding eye path; Ratchet and duplicate riders hidden during takeoff/landing. Rail heading/pitch and interpolated fixed-step translation are smoothed.

GPU-copy final unfaded space frame across level unload. Cover waits for meshes/images and queued pipelines, permits the first native landing world frame behind it, then waits for a second GPU readiness pass before revealing the destination.

Point/trigger native page widgets, frontend Press Start, actual dialog labels and visible native directional/confirm/back controls. Actual animated vendor/PDA monitor rays drive selection, Buy, Close, popup Yes/No and ammo quantity.

Grip hysteresis and native hang/probe guards retain accepted ledge anchors. Falling hand-height and higher-rung probes preserve native collision validation. Swimming and Hydro propulsion use headset gaze.

Native Trespasser/Hydro attachment poses dock on their original target frames, preserving native action logic. Trespasser has a fixed small world puzzle above the tool, hands free. Physical insertion activates Hydro without requiring trigger. Crank socket uses animated bolt top, level wrench and forward-stick orbital movement.

Native two-hand items gain bounded rotational inertia with one hand; accepted support grip stabilizes the same rendered/firing pose, including gold variants.

Non-VR cargo check, VR cargo check and Windows debug executable build passed; git diff --check passed. SHA-256 `DE5A220B37FA1CF716ED372BA5AD7463874198263B307CA95A020D2F9AF0C08E`; log `work/vr/vr-polish-batch-build.log`. NOT RUN per AGENTS.md. No scripted gameplay checks. Debug relaunch requested; startup verification pending. Pending headset confirmation of each row in VR_POLISH_REVIEW.md, especially actual ladder/rung geometry, all tool sockets, green/purple cockpit fit, menu flows, destination transitions, rail comfort and weapon inertia. Compilation does not establish physical fit or GPU frame cost.

Debug relaunch completed on Kerwan with the matching final executable. OpenXR produced two head views and destination level 3 continued ticking with zero queued pipelines; process responding. Headset was not focused when inspected. Log: `work/vr/debug-kerwan-headset-20261024-113028.log`. Startup Bevy time/camera-order warnings remain; no panic/fatal error observed. No gameplay automation or tests run.

## Wrench flash follow-up at maximum range

The user still observed intense white/colored flashes, localized near the maximum throw range, after the prior after-image fix. OUT-to-BACK has no intentional flash. Read-only original wrench spin-asset inspection showed stable mesh bounds. These findings do not establish the precise observed cause.

VR wrench wall bursts use at most four small unsplit sparks instead of 20 additive ribbons and ten splitting sparks. Actual tracked headset eye supplies effect clearance. Per-eye shader guards reject zero/view-aligned ribbons and near/behind-eye endpoints before projection/normalization. Stationary or non-finite flight collision segments cannot emit a bounce. Turnaround and bounce events now have separate diagnostic positions.

Non-VR cargo check, VR cargo check and debug executable build passed; git diff --check passed. No automated tests run per AGENTS.md. Binary SHA-256 `52B6B56196A8CC8C389558D44DABA49CFD7EB836457EB15559213544415D2553`. Debug startup verified; log `work/vr/debug-kerwan-headset-20261024-114337.log`. Pending a physical throw into empty space at maximum range; the previous after-image change alone was insufficient.

## Rigid VR wrench presentation through return and catch

No improvement from the particle attempt; user identifies the wrench model during return. Captured throw logs contain finite natural turnarounds without bounces. Original asset inspection found finite spin/return-blend matrices and ready versus spin width about 1.02 versus 2.01 game units at VR scale. This is source/asset evidence, not a reproduced headset image or conclusive root-cause proof.

Cloned VR wrench uses original ready pose (sequence 1/frame 0, no snapshot) during OUT, BACK and remaining catch blend. Native flight rows provide horizontal spin. Ready attachment pose matches the displayed mesh. Detached renderer clears held-item spring modifiers. Original distant impact bursts restored; per-eye numerical particle guards retained.

Non-VR cargo check, VR cargo check and Windows debug executable build passed. git diff --check passed. No automated tests run per AGENTS.md. SHA-256 `7F3F8BCA471D22261CAC6E4EC3BC7EE5484C894684055A34757913292A27CDE2`. Debug startup verified: `work/vr/debug-kerwan-headset-20261024-115307.log`. Pending: verify the entire outgoing/return path and catch in the headset. Prior after-image and particle attempts were reported insufficient.

## Wrench return, gadget lifecycle and actor eye corrections

White flashes resolved; unreadable wrench return, broken utility insertion/water transfer, unwanted puzzle text, low cinematic/ship-exit camera and swallowing camera launch.

- Preserve outgoing wrench flight; bound return homing speed, remove return ground correction, settle orientation into receiving hand, restore ready pose on catch and keep caught tool visible.
- Preserve native gadget animation/state calls through VR cutaway filtering, acknowledge and retain accepted socket requests, relax physical push tolerances and retain optional trigger insertion.
- Show native reach/use/withdraw gadget animation with both hands attached; use a canonical socket only for detection and fixed puzzle placement.
- Remove puzzle footer text/buttons, help prompt, decorative background and controller pointer; retain native puzzle elements. Stick directions control rings; Y/right-stick-click/fresh trigger cancel via queued input consumed until release.
- Evaluate cinematic eye from the complete head chain. Use actual streamed boarding actor scale/valid frames and ignore hidden actors. Hold last living camera position during capture/death until respawn.

Non-VR cargo check, VR cargo check and Windows debug executable build passed; git diff --check passed. No tests or gameplay run. Build log: `work/vr/build-20261024-121824.log`. SHA-256 `02237B9F69D84E24F9AC32D5BF77437501C8CE86647CCA115F7285CED4ECC54E`. Not launched. User closed the previous game to allow executable replacement. User headset confirmation: wrench full return/catch, manual and trigger insertion, puzzle controls/cancellation/reinsertion, water fill/empty, Batalia commando scene, ship exits and swallowing attacks.


## Planet preparation during space flight and cinema presentation

The disk worker was already asynchronous, but the engine did not publish the destination ECS world or prepare GPU resources until the flight ended. Destination publication now starts during flight. Flight-only entities, the departure simulation (whose actor IDs must not address the destination moby table), its level data and audio remain available until handoff. The destination simulation stays paused. A hidden eye-format view warms destination materials while the visible flight remains intact; shared LOD and occlusion use the destination view for the new world. Native flight completion waits for GPU readiness and can loop when needed, including first arrivals. The short blackout is now the cut to an already prepared planet, not the primary loading screen. Landing reveal waits for an actual native actor pose, not a gameplay tick that may stay stopped during the cinematic.

Flat movies and slideshows use a black surround with the level sky/world excluded. Their screen is level, larger, preserves the composed frame aspect and stays at the viewer's entry pose rather than following every head movement. Recenter repositions it. No interface text was added.

ECS publication and LevelStartup remain synchronous and may still produce a brief hitch during flight; this change does not claim fully asynchronous scene construction or measured hitch-free travel. The transition behavior and movie presentation require user headset confirmation. No tests or scripted gameplay were run.

Build: `work/vr/build-20261024-133506.log`. SHA-256: `17C40F0EFE9F54DCF82E484ABF6ED1C8E1A3C42F9DDCE316DF81295860034C90`.


## Wrench return, utility gadgets and crank jaws

The thrown wrench now returns along one planned path from where it turned back to the tracked hand as it moves. It starts at rest, accelerates, then eases into the catch in roughly 0.35-0.75 s depending on distance. The native spin continues, slows near the hand and is phased so that it ends on the held heading; only the last fifth of the path blends in the hand's tilt, so the model no longer flips while a fast spin is blended toward the hand. The headset draws the same path between the 60 Hz game ticks. Hits, bolt collection, the catch and the outbound flight are unchanged.

The Trespasser lock puzzle again draws the original translucent grey background and frame with its rings, nodes and beams; HUD prompts stay hidden. The panel is placed just above the plugged prong tip, facing the viewer, rather than 0.55 units above the hand.

The Hydrodisplacer's own update is the native water-transfer sequence. While plugged it has no controller aim, and the tracked-weapon suspension reset it to the reach pose every tick, so the transfer never started. A docked Hydrodisplacer now keeps its native update: reach, the fill/empty command, the cutaway and the withdraw. During the native water cutaway, the headset follows the authored camera glide, kept level, between its two black fades.

On bolt cranks, the open jaws of the original crank pose (sequence 0xb) now straddle the hex head, centred on the bolt axis at the head's mid-height. This is the placement in Ratchet's native crank poses on his 1.05 ring, measured from the original assets. Previously the hub of the head was planted at the top centre of the bolt. The shaft points toward the hand with at most a 20 degree tilt and lengthens or shortens about the jaws.

These changes require headset confirmation. No tests or gameplay scripts were run.

Build: `work/vr/build-20261024-145246.log`. SHA-256: `B97452996E0B308FC577712B71D77D1E75BB6D32F5E2B13B3D33E2AFF57B5F04`.


## Trespasser panel, Hydrodisplacer pads and menu buttons

The Trespasser panel had moved behind the lock's upright plate and disappeared. Measured on the class-615 lock, it now sits over the plugged tool, at 55 % of the native hand-to-prong line and 0.42 units above the tips, facing the eyes about a metre away, with nothing of the lock in between.

VR skips the Hydrodisplacer's native water cutaway: no fades and no camera change; the transfer runs as on a pad without cutaway cameras. When the pad's tap is retracted into the ground (suck), a downward push of the nozzle onto it now plugs the tool, and the tool stays in the hand during the suck instead of jumping to the native pose at the pad centre. Utility tools keep their aim while their nozzle touches a surface, so plugging no longer resets the gadget out of its ready state.

The French VR button bar (Retour, arrows, Valider) drawn at the bottom of the pause and front-end menus is removed with its hit areas. Native pointing and the controller buttons remain.

No second wrench model is drawn by the engine at the turnaround. Per-frame diagnostics (`VR_GAME wrench_frame`) now log what each rendered frame draws around it. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261024-153506.log`. SHA-256: `DEB47E3DB168EFA299971D131A066DD0A175FE6F12E4B627DB040C43CA537F4F`.


## Ledges, loading, ship opening, caught wrench and the Clank bodies

From a hang, A and the two-arm shove now act exactly as native ✕. A VR-only rule refused the climb whenever geometry stood above the lip, which blocked every rung and every higher ledge; natively the climb jump then catches the higher ledge, lands on an open top or regrabs the same one (reproduced with the native hero code at the logged Aridia hangs). The shove is a buffered ✕, measured from each hand's recent top rather than its grab point, keeps its reference through brief tracking loss and needs 0.9 m/s.

The freeze during level changes came from per-level tables derived from the level overlay, built lazily on the main thread after the swap (about 7 s on a first visit, 1 s on a repeat). The loader now builds the destination's tables during the flight, and its spawn, emitter and GS-map reads use the destination level instead of the departing one.

During the canopy-opening landing shot the authored actor sinks Ratchet 2.5 units under the pad; the eye keeps the native cockpit seat until the cut that ends that shot. A caught wrench is held only while the grip is squeezed.

Clank and Giant Clank are first person: native eye heights 0.9 and 9.5, Giant Clank's tracking space scaled ×5.9, the body, rotor and antenna glow not drawn, their own hands on the controllers, physical punches pressing the native □ attack, the trigger holding Giant Clank's missiles, free facing and room-scale walking.

The reported wrench duplicate at maximum range is not explained: per-frame diagnostics show one smooth model without pose change or teleport. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261024-165439.log`. SHA-256: `2D81CBF75A213CD86732E67A6B72D9D91961B9F421F4DAAE8525E3F466F606CA`.


## Clank fixes and the wrench's spin centre

Clank's repeated launches came from room-scale walking: the capsule resolver ignored Ratchet's hidden moby instead of the body, so Clank collided with his own moby and was pushed up. It now ignores the body, as the native tick does. Clank's and Giant Clank's hands are placed after the item pass that hid them.

The thrown wrench now spins about its middle, like the original's centred spin pose, instead of the ready mesh's grip origin, whose head swept a metre-wide circle through the eyes when thrown upward. The flight carries the middle from its place in the hand, and the return ends at the held middle; the first tick back no longer doubles the spin step. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261024-172001.log`. SHA-256: `F4D8D32257A47684C188891D681EEAC2790CB1857321E23F545F770E393931C5`.


## Clank's physical fists, his VR ledges and closing hands

Clank's and Giant Clank's punches are physical, like Ratchet's wrench. Each tracked hand is a fist: a deliberate stroke (2 m/s, 12 cm, extending) strikes where that hand is, for a few ticks, with the body's original sphere: Clank's (radius 0.25, damage 1) or Giant Clank's fist (radius 2.0, damage 4). The native punch animation no longer plays and B no longer punches. The striking hand gets a light pulse, and a strong one only when the native sphere reports a hit.

Clank's ledges use Ratchet's VR ledge system on his own states and dimensions: a gripping hand near the edge is needed to grab, the anchored hands move him along the edge through the native probes (the stick is ignored), A or the two-arm shove climbs as a buffered native ✕, releasing both grips drops. His hands are drawn hooked on the edge in his native hang pose, and the eye drops below the lip while hanging.

Ratchet's and Clank's free hands close into a fist with the analog grip: the finger joints curl about their measured local axes toward the palm and the thumb folds across. A hand holding a weapon or a ledge keeps its pose. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261024-174511.log`. SHA-256: `C2129830169A41A33F61D91453FADFF3B6B4D760818411CEE056DE0CE78775FD`.


## Cutscene eye, Clank's scenes and climb, the wrench's spin, Umbris mines and Rilgar

Cutscenes are seen at the gameplay eye height: the eye sits 0.68 above Ratchet's head attach, where it used to be about 0.53 m lower than in play. A scene without Ratchet is seen through Clank's eyes (his scene body and antenna glow hidden), instead of a viewpoint frozen where gameplay stopped. Clank's ledge climb keeps its native arc, but the eye rises at most 0.3 above the lip.

The thrown wrench's tracked spin is wrapped to ±π: on long throws it grew past the range of the game's VU0 sin/cos, which turned the wrench into a huge white horizontal plane and moved its hit sphere away. The Umbris floating and chain mines read the attacker's class through `attacker_class`, so wrench hits detonate them (an upstream port bug, also without VR).

On Rilgar, a lurker holding the hidden Ratchet 20 above itself parks the eye at the last living viewpoint. The hoverboard turns the current gaze to the board's heading at its start and at each respawn, so the race no longer starts backwards. Room-scale steps stop while a carrier (hover car, Kerwan transports, ferries, path platforms) holds Ratchet with its edge brake, so physical steps no longer push him off a taxi. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261024-184404.log`. SHA-256: `B11E5A02CACEA9FAC7CAE04E8E9A89C6C1F3604899956E83E947B551CCE28B84`.


## Ratchet's eye at 1.1 m

Ratchet's VR eye sits 1.1 above his feet, his own eye height, instead of the 1.6 of the native first-person camera, in gameplay and in cutscenes (0.18 above his head attach). The hanging and cable eyes are capped at it, the crouched eye is 0.6, and the line-of-sight checks from the eye to the hands use the same height. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261024-185513.log`. SHA-256: `839AE147FEEB1579632EBF0C4142C6CA0A1C049B6D8F47C6351F086ED2F346C8`.


## Clank's eye height, the two-hand pull-up, a firmer wrench throw and the taxis

Clank's VR eye sits 0.63 above his feet, where his model's eyes are in the idle pose (measured the same way, Ratchet's are at 1.09-1.10), in gameplay and in his scenes. A ledge pull-up, for Ratchet and for Clank, needs both hands on the ledge and a settled hang (30 ticks, as the native grab animation must end before ✕ climbs); shoves made earlier are dropped. The wrench is thrown only by a release at 2.5 m/s or more.

Carriers no longer eject Ratchet: the VR thin-floor sweep is skipped on the ticks a carrier moves him. It swept from the carrier's old floor and left him a few centimetres above the carried one on descents, an airborne tick that detached the carry. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261024-192636.log`. SHA-256: `C2B6D5DFBE6F7866A586AA0349F00C70BFC84A6E0305EE3109777EA253117B8A`.


## Review fixes and cleanup

A review of the VR layer fixed what was wrong without a headset session. The eye-height blend ran twice per frame and now uses the frame's dt; a sloped rail pitches about its own side axis instead of rolling the horizon for a player recentred facing elsewhere; leaving a rail turns the lean into a room-scale step instead of a jump of the view. One body-eye helper now serves the root, the death and cinematic fallbacks and every eye-to-hand line-of-sight test. A snap turn no longer fires when a stick still pushed leaves quick select, the shop or a menu.

Clank's grip no longer arms Ratchet's wrench, death releases the bolt crank, a thrown wrench keeps flying while the right controller is untracked, the Visibomb monitor hides when its pose is missing, and the gold Decoy Glove throws gold decoys. Native-path exactness was restored for the Walloper's glow point and the flight sky. 137 comment lines corrupted by a cp1252 re-save were restored, and duplicated constants and dead code were removed. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261024-195121.log`. SHA-256: `895BECB9E3CA0C18040AAF2CB96AE4A4E2CEF2979E29DB32F2397C5DA9349EC7`.


## The cutscene anchor and a first-person Hologuise

Cutscene and pilot views map the head's room position at the start of the view to the actor's eye, instead of the recentre point: walking around the room since the recentre no longer shifts the cutscene view.

The Hologuise disguise is first person: the rig no longer treats it as Giant Clank, it sees from Ratchet's eye height, turns with the head and walks with the room. Its body and head glow are hidden and its own forearms and hands are drawn on the controllers, closing with the grip. A fresh trigger press or a right-hand salute at the brow makes it salute; B or A returns to Ratchet. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261024-213838.log`. SHA-256: `B8CA98C2286315AF516E37751F1D3548A4E5A2E7EC42773BE9F5DF4ED3DB113C`.


## Turrets and flown ships fire on the trigger

Mounted in a turret (Gaspar's cannons, Hoven's and Batalia's turrets), the trigger holds ○, their fire key; flying a ship (Pokitaru's jet, Gemlik's ship, the fleet's ship), it holds □, the lasers, while X and the left trigger keep ○ / R1 for the missiles. Mounted, Y leaves the turret or the ship natively instead of opening the hand's quick select. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261024-231458.log`. SHA-256: `6CAA687134FDFC75E5FFDD36B10DAED47AEAC761DD8442F7AB1D270B933DC78C`.


## A cockpit view for the flown ships

Flying Pokitaru's jet, Gemlik's ship or the fleet's ship, the VR view rides the ship: the eye sits in the fighter model's own canopy (measured on its glass texels), the view follows the ship's heading and pitch with smoothing while its roll is left to the head, and the tracking space takes the ship's quarter flight scale so the head moves inside the cockpit at Ratchet's size. Ratchet's hands and wrench, hidden natively in the ship, are not drawn. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261024-232308.log`. SHA-256: `73AA5DEF64BCC0F25EE12D800C874F835C994CB6E18CF60B555F89743F939DAF`.


## The HUD along the aim, the ship's eye above its canopy

Mounted in a turret or flying a ship, the HUD panel hangs along the aim (the turret's script camera behind its barrels, the ship's nose) instead of following the gaze, so the reticles the game draws at its screen centre mark where the guns point. The flown fighter's canopy is opaque and its hull two-sided, so the eye rides just above the canopy rather than inside it. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261024-234217.log`. SHA-256: `2B8038A502A1A22B166F47A6E75455D420AC848890330B0FED2B971719CA06F0`.


## Culling for the headset, VR turrets and spectator scenes

Level content is culled for the headset's eye in every mode: in-level movies, ship sequences and the Visibomb view used the native camera (an authored shot elsewhere), which left holes in view; the native image shown on the Visibomb's monitor still gets what it sees, and gameplay's "drawn last frame" keeps the native culls. Cutscenes start facing where Ratchet's head looks, and scenes with no one to see through (the infobot's messages) are watched from their camera as a spectator, instead of reusing the previous scene's eye, which showed Hoven's arrival from far away.

The manned turrets put the eye at their seat with a fixed view and turn toward where the right controller points, at their own speeds, the trigger firing. The ship no longer vanishes after the inter-planet loading: the new level's setup hid it along with its own instance 0. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261025-010105.log`. SHA-256: `D9AE7A087FFA881A05B3AEC5867768A0B12C6221F76A6674F041D85D0CC9D083`.


## VR Options, the helmet on the player's head, boots and Oltanis' hand items

A VR Options page sits right below Port Options (only with a VR headset), with Helmet and Boots, both On by default and saved with the port settings. With Helmet on, the worn O2 Mask or Pilot's Helmet is drawn on the player's head, in play, in Ratchet's scenes and in the ship sequences, the eye behind its visor; the scene actor's own helmet, which followed Ratchet's animated head across the view, is hidden. With Boots on, the worn boots show at the feet as the game poses them.

On Oltanis, and on any arrival without a landing after a flight streamed in VR, the hand items were hidden by the travel's hide flag that the preloaded switch never cleared; it is cleared now. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261025-103944.log`. SHA-256: `C6FB71C233EC0142073C1D6343DB69CBA49E0562A6C057ACB219A732B8B0088B`.


## Rails, reach, Hyper Strike aim, a wider visor and the camera flip

On rails the eye now predicts the body instead of trailing it, which had left the eye, the hands and the wrench's hits a metre behind at grind speed; the take-off keeps the eye in the ship's seat once Ratchet sits down; Clank's antenna dot is no longer drawn. The worn O2 Mask sits with the eye closer behind a taller visor whose glass is lighter.

The wrench stretches up to 35 % on fast swings and its hits reach with it; the Hyper Strike carries the native aim assist (a drift toward a near target, a near miss pulled onto it), needs a real downward stroke, and the wrench hand no longer grabs crate tops. A Camera flip option (off by default) turns the view with Ratchet's flips, and two-handed weapons held in one hand are heavier. The loading hitch lost its main-thread table re-read and its departure free; the swap now logs its phases. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261025-112430.log`. SHA-256: `B6615E93D3FE03A6707AF5AEA424C6591F392B757A4BC1A4365182E2263AEFDA`.


## Body direction, the wrench's whoosh, Giant Clank, the Walloper and walking in the quick select

A Direction option (Body by default, or Gaze) makes the heading and the stick follow an estimate of the body from the head and both controllers, so the player can walk one way while looking another. The wrench whooshes on each swing with the native combo sounds, glow billboards face the headset (Clank's antenna light in scenes), Giant Clank's heading follows the player in every state with his missiles leaving from his drawn hands, the Walloper gives the wrench back on a still grip and chains consecutive punches, and the quick select no longer stops Ratchet's movement. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261025-125434.log`. SHA-256: `B1B8944B2F1C449E3C03C379E64B67264C6661CB1E15BEAA33BF32B347C142A2`.


## Walking backward and sideways, Walloper punches and the wrench's whoosh

The original game always moves Ratchet the way he faces. In VR he faces the player's heading, so the native movement physics now run facing the way he actually moves (the stick, or the carried motion once it is released), and the heading comes back right after the move for the transitions, flips, weapons, aim and ledges. Walking backward or sideways no longer lurches forward on release, jumps and steps follow the motion, and the stick no longer aims melee attacks.

A Walloper punch is the right hand thrown forward relative to the head, within about 55° of the facing and away from the body: folding the arm back, swinging sideways or stepping back never punches, and the lunge goes where the player faces. The wrench's whoosh plays Ratchet's own swing sound, since the wrench's animations carry none. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261025-135137.log`. SHA-256: `DDEE3A69170E96ACD4815FFA47C7B8B1E64F6438DE4714B290E27D538565810C`.


## Quick-select pages on the trigger

While the right-stick click holds the quick select open, the right trigger turns to the next page instead of the right stick; the press fires nothing until it is released, and a trigger already held when the ring opens does not turn the page. This change requires headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261025-140220.log`. SHA-256: `34B438109C51819897F950DC061696A0A6B6D830F55B3AB4299EDD3A6B5DF68C`.


## Headset buttons in menus and text

Menus, help boxes, banners, subtitles and the spatial panels now show the headset's buttons instead of the PS2 icons: the fonts' pad icons are repainted when the level HUD loads in VR, ✕ as A, ○ as the right trigger (RT), △ as a right-stick click, □ as the right grip (RG), L1 as the left grip (LG), R1 as the left trigger (LT), L2 as X, R2 as B and the analog sticks as L / R thumbsticks; their advances follow the new glyphs, so layout and pointer targets stay aligned.

Every icon names a control that works: in the pause and freeze menus the left grip sends L1 and the left trigger R1 (previous / next map, the planet lists, the Weapons page's quick-select slot, now also while pointing at a weapon), and in play X and B, unused until now, hold L2 and R2 (hover and Clank strafes, hoverboard tricks, the Hydro-Pack boost, a ledge drop); on turrets, ships and in the Hologuise they keep ○ and □. The first glyph set was confirmed in the headset; the new shoulder bindings require headset confirmation; no tests or gameplay scripts were run.

Built in the separate checkout `rerac-vr-pad-glyphs` (release profile) while `rerac-vr-build` was in use. Build: `work/vr/build-20261025-143505.log`. SHA-256: `D31C8D55B4FD4019DD9593A8947C6BF0B8B115FBA8CA93A2206A98275BB4BECE`.


## Streamed inter-planet loading (branch `vr-streamed-loading`)

The destination planet is now built and handed to the GPU behind the flight instead of in one frame. The loader thread, which already read and lit the level, also builds the tfrag, tie, shrub and moby meshes and images and the tie and shrub entity lists. At the swap the old level's resources are freed on a cleanup thread and the old ties are hidden, then despawned a few hundred per frame; the new meshes and images are added at most 4 MiB, 12 images and 48 meshes per frame, and the tie and shrub entities are spawned within 2.5 ms per frame. The voyage ends only once all of it is in and uploaded. Moby entities still spawn at the swap (the level's spawn pass and the gameplay set-up hide them by tag); their meshes and images are streamed too. Non-flight level changes and the desktop build add everything at once, as before, from the prepared meshes.

The set-ups a level runs on its first frame (the pause menus' 3D models 62 ms, the gameplay set-up 31-37 ms, Ratchet's attachments 22-26 ms, the vendor 5 ms) wait for the cut to the landing, under black. Until then the voyage, the VR camera and the flight's audio run on the departure's play state, as before the change, and the pipeline warm-up view is placed at the level's Ratchet instance.

Measured in the headset (Kerwan to level 10): the change's frame takes 34-41 ms (was 170-225 ms), the frames after it at most 27 ms (one of 140-160 ms with the set-ups still in the flight), and the upload 35 MiB over 52 frames; the worst flight frame went from about 245 ms to 40 ms. The set-ups and the first tick of the destination (160-200 ms) run under black.

New log lines: `level switch: level NN prepared beside the load in ...`, `VR_HITCH swap ... streaming=true old_ties_deferred=N`, `VR_HITCH stream ...` (what was streamed and over how many frames), `VR_HITCH frame phase=... main=... split first=... pre_update=... fixed=... update=... post_update=... last=...` / `render=... prep=...` for every frame of 20 ms or more during a transition, and `level setup: <name> <ms> ms` for those set-ups (the gameplay set-up step by step). No tests or gameplay scripts were run.

Build: `work/vr/build-20261025-170117.log`. SHA-256: `6815E721EC019A760B364EAB14B4EA5064B9EAF7C9960EF3633B1CE66BFFB37D`. The branch builds in its own checkout, `rerac-vr-streaming` (this package applied to the pinned upstream commit); `rerac-vr-build` is unchanged.


## Weapon recall after a wrench throw, a rarer whoosh

A weapon swap refused while the swap is locked (the wrench in flight) no longer leaves a stale target that made the hand ignore that weapon afterward; while the wrench flies the trigger does nothing, and a fresh press once it is back brings the last weapon. The wrench's whoosh now needs a big fast swing. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261025-173319.log`. SHA-256: `B2BB491C812F7573110B4241BC7E0694AAE4C4300994BC112C4686AA89EEFAA0`.


## The hoverboard in VR

The board steers about its own heading (the left stick's x plus the player's sideways lean, VR Options → Hoverboard lean), the view turns with the board (no snap turn, no room-scale step on it), the right trigger boosts, the left grip is L1 and the right stick holds the four tricks; a landing tilted or turned away straightens along the board in VR. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261025-175634.log`. SHA-256: `B1347DC8EF556E7FB368DB9E306BCFD49F5F3DBC66BFE1B4796CE25F46689432`.


## Hoverboard: lean, race start, Kalebo III's board weapon

In VR the board's turn is a rate set by the stick's x plus the lean (from 1.5 cm, full at 12 cm, gentle at first), eased over 0.06 s, so it straightens as soon as both return to centre; the race start and respawns take the body's heading as the board's front; Kalebo III's board weapon is held in the right hand and fired by the right trigger, the left trigger boosts. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261025-182923.log`. SHA-256: `9BB5FC2DF4F2A7A0A48771763E6AA30C32289242DDFB8DB9D7C9F7E4B36C477B`.


## Hoverboard: the stick back, the lean like the stick

The stick steers the board through the native turn spring again, as in the first VR board change, and the lean adds to the stick's x through the same spring (a small lean also steers, the board stops turning back within 1.5 cm). These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/build-20261025-184248.log`. SHA-256: `EF81BA7003B89AADB8DE2C6FBBFD53A9667C5DE3E8B9E58BD64F08C82CBDC473`.


## Player release: RatchetVR.exe

`scripts/vr/Package.ps1` builds the player release folder and zip (overlay/docs/vr/README.md). `RatchetVR.exe` (`crates/rc-engine/src/bin/ratchet-vr.rs`) extracts the player's disc on first launch and starts the game in VR; `RatchetVR Sandbox.exe` starts the debug mode. The app-local Visual C++ runtime (14.29) ships with the binaries (linker 14.29). Release `0.1.0` was built from a fresh upstream worktree with the package applied (`rerac-vr-release`), portable x86-64 (no `target-cpu=native`): `dist\RatchetVR-0.1.0.zip`, 34.9 MB. From the unzipped zip, desktop preview only: `RatchetVR.exe <iso>` extracted the disc (SCUS_971.99, 2724 files) and booted the normal front end (preview smoke check not applicable on the title screen); `RatchetVR Sandbox.exe` reused the data and reported `ORIGINAL_GAME_SMOKE_PASS` on Kerwan. The file dialog, message boxes and a headset launch require manual confirmation. No tests were run.

## Menu pointer on the memory card slot lists (release 0.1.1)

The New Game, Save and Load slot lists had no pointer targets, and pointing at another widget of a slot page moved the focus off the slot list, whose native update only runs focused, so neither the trigger nor A nor the sticks picked a slot. Each slot is now a pointer target (hover moves the native cursor and the shared last slot, the trigger confirms through the native ✕; Load only picks a used slot), and on a slot page the pointer never moves the focus off the slot list. Release `0.1.1` was built in `rerac-vr-release` with `scripts/vr/Package.ps1 -Version 0.1.1`: `dist\RatchetVR-0.1.1.zip`, 34.9 MB. These changes require headset confirmation; no tests or gameplay scripts were run.

Build: `work/vr/package-20261025-224720.log`. SHA-256 (zip): `9733945E5D32B4E2D3C7E02A41E2735755CD2689A7C84401992F95E859AE6230`.
