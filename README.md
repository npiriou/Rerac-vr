# ReRAC VR

An OpenXR integration overlay for the original [ReRAC](https://github.com/re-rac/rerac) game, pinned to upstream `89c87e98f9d8296187df2a6310c67b32144090d3`.

`RC_VR=1` now starts the original application: its level loader, Ratchet controller and animations, moby simulation, scenery, weapons, collisions, audio and level transitions. Two OpenXR world cameras and matching sky cameras render the same entities and original materials. VR controls feed ReRAC's existing PS2 pad input. There is no separate VR implementation of movement or combat.

The initial gameplay view is **third person**. The headset adds stereo and head tracking to the original follow camera, while both sticks control the original game. Motion aiming and a first-person body conversion are not implemented. Existing upstream gameplay limitations remain.

## Local use

Apply to a fresh checkout of the pinned upstream commit with `Apply-Overlay.ps1 -Repository <path>`, then build with `scripts/vr/Build.ps1`.

```powershell
.\scripts\vr\Run.ps1 -Scene novalis -DataDir 'X:\dev\rerac\extracted'
```

The default scene is now Novalis in the original game, with the original hero initialization and floor collision. Use your supported RAC1 extraction; no disc data or binaries are shipped here. Audio stays enabled unless you explicitly set `RC_AUDIO=0`.

For an automated desktop check of the original game and both stereo rendering paths:

```powershell
.\scripts\vr\Run.ps1 -Scene novalis -DataDir 'X:\dev\rerac\extracted' -DesktopPreview -SmokeTestSeconds 12
```

The window remains playable with the original keyboard/gamepad controls. Preview renders two offscreen eye views; it does not simulate a headset. The old calibration room remains available only with explicit `-Scene calibration`.

## Controls

| VR control (Touch / PICO through SteamVR) | Original game action |
| --- | --- |
| Left stick | Move Ratchet relative to headset heading / navigate menus |
| Right stick | Slower horizontal orbit of the original follow camera; head motion looks up/down |
| Right A | Cross — jump / confirm |
| Right B | Square — wrench |
| Left X | Circle — equipped weapon |
| Left Y | Triangle — quick select / back |
| Left stick click | Select — map |
| Right stick click | Start — pause |
| Right trigger | Equipped weapon (additional binding) |
| Left trigger | Strafe / aim |
| Right grip | Crouch |
| Keyboard F8 | Recenter headset |

A regular gamepad and the original keyboard bindings remain usable. Initial recenter waits until the headset is focused, rather than using its pose while on the desk. F8 recalibrates the physical headset pose without moving Ratchet. Camera pitch/roll come from physical head motion, keeping the virtual horizon level. The original pause menus and HUD are reused in headset panels.

## Implementation and validation

Guarded source edits are in `patch-manifest.json`; new modules and scripts are in `overlay/`. `resolved-Cargo.lock` pins the verified dependency graph. The default desktop path remains available without `RC_VR=1` and without the optional `vr` feature.

The build script compiles by default without running tests. Only pass `-RunTests` when the user explicitly requests tests, as required by AGENTS.md. Tests from earlier runs are recorded separately. See [local build evidence](LOCAL_BUILD.md), [runtime records](STARTUP_READINESS.json), and [VR implementation details](overlay/docs/vr/README.md). Physical comfort, GPU/compositor budget and full campaign completion require separate validation.
