# ReRAC VR

An OpenXR integration overlay for the original [ReRAC](https://github.com/re-rac/rerac) game, pinned to upstream `89c87e98f9d8296187df2a6310c67b32144090d3`.

`RC_VR=1` starts the original application: its level loader, Ratchet controller and animations, moby simulation, scenery, collisions, audio and level transitions. Two OpenXR world cameras and matching sky cameras render the same entities and original materials. Locomotion uses ReRAC's existing PS2 pad input. Tracked-hand wrench impacts use the original collision scene and hit records; wrench and bomb throws use the original projectiles and damage.

Gameplay is **first person**, on the existing complete level. The view stays 1.6 game units above Ratchet's feet, with physical head tracking and independent 30-degree snap turns. Character animation and the follow camera cannot rotate the headset. The original brown Ratchet hand meshes and textures follow both controllers; the original Bomb Glove replaces the right hand when equipped or preparing a bomb. The original wrench model follows the right grip. Existing upstream gameplay limitations remain.

Physical horizontal headset movement moves the original Ratchet capsule through its world/moby collision solver. The view remains directly above that resolved body position; snap turns do not accumulate a camera offset. The wrench's authored attachment-list-0 grip frame is aligned to the tracked palm, with the shaft pointing forward along the controller. The same attachment is used for presentation, melee, launch and return.

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
| Right stick | 30-degree snap turn; return to centre before turning again |
| Right A | Cross — jump / confirm |
| Right grip held | Hold the wrench; swing the hand to hit |
| Release right grip during a throwing motion | Throw the wrench in the direction of the hand's motion; automatic return |
| Release right grip slowly | Put the wrench away; squeeze again to hold it |
| Right trigger held, Blaster or Pyrocitor selected | Fire the selected weapon along the right controller |
| Right trigger held, neither gun selected | Prepare one Bomb Glove bomb in the right hand; keep it held without starting its fuse |
| Release right trigger during a throwing motion | Throw the bomb with the hand's direction and speed; consume one original ammunition unit |
| Release right trigger slowly | Cancel the prepared bomb without using ammunition |
| Left Y | Triangle — contextual interactions; hold for quick select, move the left hand toward an icon, release to confirm |
| B / X | Original menu controls |
| Left stick click | Select — map |
| Right stick click | Start — pause |
| Keyboard F8 | Recenter headset |

A remains the original jump button, including double jump. Crouch and teleportation have no XR gameplay binding. A regular gamepad and the original keyboard bindings remain usable. Initial recenter waits until the headset is focused. F8 recalibrates the physical headset pose without moving Ratchet. Camera pitch/roll come exclusively from physical head motion. The original pause menus and HUD are reused in headset panels.

The Blaster and Pyrocitor are part of the starting VR inventory and quick select. Hold Y and move the left hand toward the weapon icon, then release Y. Aim with the right controller and hold its trigger to fire bullets or a continuous flame. Original models, ammunition consumption, cadence, collisions, damage, effects and sounds are retained. A fresh right-grip press recalls the wrench. For playtesting, all owned weapons start with their native maximum ammo on each application launch; ammunition then decreases normally.

Grip release requires a valid tracked throw gesture; slow releases and tracking/focus loss do not launch the wrench. Physical melee uses the wrench head's swept path and a per-target cooldown. Snap turning and locomotion do not count as swings. These thresholds, glove/weapon alignment and physical comfort still require headset feedback.

The Bomb Glove requires ownership and available ammunition from the existing inventory. The bomb follows the hand while the right trigger is held, taking priority over the held wrench. After a valid release gesture, the original bomb simulation handles gravity, collisions, fuse, explosion, damage and effects. A slow release, lost tracking/focus, pause, death or a blocked hand cancels preparation without consuming ammunition. A fresh trigger press is required for each new bomb.

## Implementation and validation

Guarded source edits are in `patch-manifest.json`; new modules and scripts are in `overlay/`. `resolved-Cargo.lock` pins the verified dependency graph. The default desktop path remains available without `RC_VR=1` and without the optional `vr` feature.

The build script compiles by default without running tests. Only pass `-RunTests` when the user explicitly requests tests, as required by AGENTS.md. Tests from earlier runs are recorded separately. See [local build evidence](LOCAL_BUILD.md), [runtime records](STARTUP_READINESS.json), and [VR implementation details](overlay/docs/vr/README.md). Physical comfort, GPU/compositor budget and full campaign completion require separate validation.
