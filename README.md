# ReRAC VR

An OpenXR integration overlay for the original [ReRAC](https://github.com/re-rac/rerac) game, pinned to upstream `89c87e98f9d8296187df2a6310c67b32144090d3`.

`RC_VR=1` starts the original application: its level loader, Ratchet controller and animations, moby simulation, scenery, collisions, audio and level transitions. Two OpenXR world cameras and matching sky cameras render the same entities and original materials. Locomotion uses ReRAC's existing PS2 pad input. Tracked-hand wrench impacts use the original collision scene and hit records; wrench and bomb throws use the original projectiles and damage.

Gameplay is **first person**, on the existing complete level. The view follows Ratchet with physical head tracking and independent 30-degree snap turns. Eye height is 1.6 game units on foot and blends to 0.45 while swimming. Jump/flip animation and the follow camera cannot rotate the headset; magnetic tracks rotate the tracking frame with the native body up axis. The original brown Ratchet hand meshes and textures follow both controllers; the original Bomb Glove replaces the right hand when equipped or preparing a bomb. The original wrench model follows the right grip. The original animated gameplay body is hidden. Existing upstream gameplay limitations remain.

Physical headset movement parallel to the current floor moves the original Ratchet capsule through its world/moby collision solver. The view remains directly above that resolved body position; snap turns do not accumulate a camera offset. The wrench's authored attachment-list-0 grip frame is aligned to the tracked palm, with the shaft pointing forward along the controller. The same attachment is used for presentation, melee, launch and return.

## Local use

Apply to a fresh checkout of the pinned upstream commit with `Apply-Overlay.ps1 -Repository <path>`, then build for play with `scripts/vr/Build.ps1 -Release`.

```powershell
.\scripts\vr\Run.ps1 -Release -DataDir 'X:\dev\rerac\extracted'
```

The standard launch is `normal`: the native boot flow, logos, Press Start and main menu (New Game, Load Game, Options), with original progression and starting equipment/ammunition. New Game uses the native initial save template. VR grants and planet unlocks are disabled in this mode.

| Launch mode | Start and progression |
| --- | --- |
| `normal` (default) | Native boot/title/main menu and normal progression |
| `debug` | Kerwan / Metropolis (level 3), all planets and native inventory unlocked, ammunition initially full |
| `debug-gold` | Same as debug, with all ten native gold upgrades enabled |

```powershell
.\scripts\vr\Run.ps1 -Release -Mode debug -DataDir 'X:\dev\rerac\extracted'
.\scripts\vr\Run.ps1 -Release -Mode debug-gold -DataDir 'X:\dev\rerac\extracted'
```

Debug modes accept `-Scene novalis` for the previous level-1 start. Each mode uses a separate native memory card under `work/vr/saves/<mode>`. Debug saves cannot unlock normal-mode progression. All native level transitions use the same VR integration; full VR playthroughs of every destination remain unverified. Use your supported RAC1 extraction; no disc data or binaries are shipped here. Audio stays enabled unless you explicitly set `RC_AUDIO=0`.

For an automated desktop check of the original game and both stereo rendering paths:

```powershell
.\scripts\vr\Run.ps1 -Release -Mode debug -Scene novalis -DataDir 'X:\dev\rerac\extracted' -DesktopPreview -SmokeTestSeconds 12
```

The window remains playable with the original keyboard/gamepad controls. Preview renders two offscreen eye views; it does not simulate a headset. The old calibration room remains available only with explicit `-Scene calibration`.

## Controls

| VR control (Touch / PICO through SteamVR) | Original game action |
| --- | --- |
| Left stick | Move Ratchet relative to headset heading / navigate menus |
| Right stick | 30-degree snap turn; return to centre before turning again |
| Vendor open | Left stick walks, A jumps, right stick browses / adjusts quantity, fresh right-trigger presses buy / confirm, Y cancels / closes. Displays stay on the vendor's monitors |
| Right A | Immediate jump / double jump. Hold the left index trigger + press A for pack jumps or a native side/back flip with a timed directional flick. Confirm in menus |
| Left index trigger | Native R1 modifier: packs and flips; right trigger remains weapon use |
| Right grip held | Hold the wrench; swing the hand to hit |
| Downward wrench strike in the air | Native Hyper Strike: accelerated descent and double wrench damage |
| Release right grip during a throwing motion | Throw the wrench in the direction of the hand's motion; automatic return |
| Release right grip slowly | Put the wrench away; squeeze again to hold it |
| Right trigger held, a gun or hand gadget selected | Native fire/use action aimed by the right controller |
| Right trigger held, a throwing glove selected | Prepare its native bomb, mine, Doom canister or decoy; maintain the hold, then throw by moving and releasing |
| Right trigger with an empty hand or wrench | Re-equip the last gun/glove/gadget, ignoring the wrench; release and press again to use it |
| Release right trigger during a throwing motion | Throw the bomb with the hand's direction and speed; consume one original ammunition unit |
| Release right trigger slowly | Cancel the prepared bomb without using ammunition |
| Left Y | Triangle — contextual interactions; hold for quick select, move the left hand toward an icon, release to confirm |
| Walloper selected: punch with the right hand | Original lunge and horizontal dash, with native windup, speed and broad damage window; no vertical punch propulsion |
| Left grip near a heavy weapon's barrel | Hold the foregrip and guide the weapon with both hands; release to return to one hand |
| Grip near a ledge while falling | Catch with a grip; move gripping hands along the edge to pull the body; shove both arms down at least 22 cm at 2.2 m/s to climb/jump as A (left-stick movement disabled while hanging); release both grips to drop |
| Submerged palm strokes | Push water opposite the motion of the hands; turn palms edge-on for recovery. Push up to dive |
| Y held + right stick left/right | Previous / next quick-select page (return the stick to centre between changes) |
| B / X | Original menu controls; B also exits the Hologuise |
| Left stick click | Select — map |
| Right stick click | Pause during play; skip the current scene/movie during a cinematic (one click queues through its opening fade) |
| Either joystick in menus | Directional navigation; return to centre between steps. A confirms, Y goes back |
| Keyboard F8 | Recenter headset |

A jumps immediately on press, with no long-press delay. The left index trigger sends native R1: use it with A for pack jumps, or a side/back directional flick + A for the native flip chooser. Flip priority precedes the special pack jump; the right trigger remains independent for weapons. R1 retains the original movement-state rules, with a blended crouch eye height; headset rotation never follows a flip animation. Teleportation has no XR gameplay binding. A regular gamepad and the original keyboard bindings remain usable. Initial recenter waits until the headset is focused. F8 recalibrates the physical headset pose without moving Ratchet. Camera pitch/roll track physical head motion relative to the current floor, including magnetic tracks. The original pause menus are placed directly in the world; the ordinary HUD retains its headset layer.

In `debug` and `debug-gold`, all original weapons and gadgets (native inventory IDs 1..35) are available at startup. Normal mode grants nothing beyond native progression. Hold Y and move the left hand toward an icon, then release Y. Change pages with the right stick while holding Y. Wearable/back gadgets are equipped through the original pause inventory. Native models, cadence, ammunition, collisions, damage, effects and interactions are retained. The Visibomb shows its native remote camera on the articulated screen built into the weapon: steer with the left stick, A boosts and a new trigger press detonates after its native delay. The Drone Device activates when selected, as in the original game.

In the debug modes, every owned weapon with an ammunition reserve starts at its native capacity once per application launch; use then decreases it normally. The Suck Cannon still captures creatures as its ammunition. Complete original brown gloves include their forearm cuffs. Held weapons retain Ratchet's native layered gripping-hand pose; throwing weapons use their matching original glove. The Swingshot is rolled 90 degrees onto the top of the wrist while preserving the palm anchor and forward direction. Calibrated weapons put their rear grip at the right controller; their animated muzzle is derived separately. Throwing ammunition rests on the palm side of the glove. All native two-hand holds (Suck Cannon, Devastator, Visibomb, Pyrocitor, Hydrodisplacer and R.Y.N.O.) accept the optional left grip at their authored support-hand position.

Grip release requires a valid tracked throw gesture; slow releases and tracking/focus loss do not launch the wrench. The wrench is 20% larger, stretches by up to another 15% during a swing, and sweeps its head with the native 0.35-unit radius plus 0.17-unit reach extension. Enemy hits require 5.5 m/s tip speed, 2.4 m/s translating-hand speed and a 14 cm stroke. Crates require 6.5 m/s tip speed, 2.8 m/s hand speed and a 16 cm stroke, plus actual contact with their native collision geometry. Intact crates stop the held wrench and gripping hand, with the existing contact recoil and haptics; a weak stroke cannot break them. Wrist-only motion cannot bypass these requirements. A per-target cooldown limits repeated damage. Weapons and active gadgets have distinct feedback profiles tied to native firing/use, capture, attachment, puzzle and scan events. Weapon deployment adds individual profiles following the original unfolding motion changes and animation sound markers. A supporting left hand receives reduced heavy-weapon recoil. Accepted damage gives a strong priority pulse on both hands. Snap turning and locomotion do not count as swings. These thresholds, glove/weapon alignment and physical comfort still require headset feedback.

The Bomb Glove requires ownership and available ammunition from the existing inventory. With its glove selected, the bomb follows the hand while the right trigger is held. After a valid release gesture, the original bomb simulation handles gravity, collisions, fuse, explosion, damage and effects. A slow release, lost tracking/focus, pause, death or a blocked hand cancels preparation without consuming ammunition. A fresh trigger press is required for each new bomb.

## Implementation and validation

The visibility rewrite was rolled back and archived outside the active source trees. For play, use the Release build; the measured PC also uses a native CPU build, a dedicated NVIDIA maximum-performance application profile and production Vulkan GPU diagnostics settings. No engine source or graphics quality settings were changed for this configuration. Live timing and rollback details: [PERFORMANCE.md](overlay/docs/vr/PERFORMANCE.md).

Guarded source edits are in `patch-manifest.json`; new modules and scripts are in `overlay/`. `resolved-Cargo.lock` pins the verified dependency graph. The default desktop path remains available without `RC_VR=1` and without the optional `vr` feature.

The build script compiles by default without running tests. Only pass `-RunTests` when the user explicitly requests tests, as required by AGENTS.md. Tests from earlier runs are recorded separately. See [local build evidence](LOCAL_BUILD.md), [runtime records](STARTUP_READINESS.json), and [VR implementation details](overlay/docs/vr/README.md). Physical comfort, GPU/compositor budget and full campaign completion require separate validation.

The gameplay body is hidden again. Pause/freeze menus use original world-placed primitives, native animated frames and stereo menu models around their opening pose, without a composed-frame headset screen. Left trigger holds native R1 (crouch with blended eye-height change, timed directional flips and pack jumps with A); wrench throwing retains its grip/release gesture.

In the spatial pause menu, aim the right controller at an option or equipment icon and press its trigger to confirm. Cyan marks an actionable target. Hold the trigger on an audio slider to set it by pointing; the native joystick/buttons remain available. Release the trigger before firing after leaving the menu.

Hyper Strike now gives a strong landing pulse, with a softer echo in the left hand. Held wrench swings stop against solid walls with native sparks/sound and force-scaled impact haptics; resting against a wall does not vibrate repeatedly.

Wall impacts give the wrench a brief recoil opposite the stroke. Spatial pause content has stable transparent draw order and repeating/additive monitor effects; 3D weapon previews remain. Ledge traversal now corrects actual hand-anchor error and freezes held targets against velocity-driven drift.

Accepted ledge grips give a light pulse in the corresponding hand. An accepted native climb launch, by A or the two-arm shove, gives both hands a stronger push pulse and a short tail.

Gold variants reuse the same item IDs and model classes as their ordinary counterparts. They automatically share all VR grip/orientation/muzzle settings, native primary/support-hand poses, two-hand aiming, glove/palm placement and deployment/use haptic profiles. Native gold flags supply the gold metal map and original upgraded gameplay behavior.


The Walloper hides the native right-hand glove mesh while held, preventing fingers from clipping through its housing; the secondary hand stays visible. In the ship, controller pointing updates both the native list cursor and the actual travel destination before confirmation. Known planet dots on the galaxy map can also be pointed at and confirmed.

The Hydrodisplacer and Trespasser accept physical insertion into their native pad/lock collision geometry. Approach with the tool tip aligned into the target, then push the right controller at least 3.5 cm with the tip passing approximately 1.2 cm into the contact surface. Only tracked controller translation counts; body locomotion, wrist rotation and tool animation do not supply that stroke. An accepted insertion enters the original water-transfer or lock-puzzle logic, retains its existing event haptics and suppresses the original automatic pull of Ratchet toward the pad. Withdraw before another activation. Native trigger activation remains available. Physical alignment, reach and insertion feel still require headset feedback.

## Headset heading, spatial sound and grind-rail camera


Headset orientation now supplies a separate native locomotion frame and audio listener. Ratchet's ordinary free-movement heading follows physical turns and snap turns, including while stationary. Ground movement can strafe/back up without turning the avatar away from the headset. Magnetic-track planar velocity follows the requested direction projected onto the local surface instead of the stale body heading. Native authored camera state remains independent for scripts and menus; rail, ledge, attack and scripted interaction poses keep their native body orientation.

Spatial sound allocation (hero/mobys/items, contextual interactions and UI) and per-tick mixing use the physical headset's position and full forward/left/up basis, with post-move body translation applied to the sampled eye. Desktop listeners keep the original camera fallback. Normal VR stick input is consumed directly in the levelled headset frame, avoiding an additional rotation through the desktop camera; rail lean/switch axes remain native.

Grind rails anchor VR yaw to the native rail tangent and follow its changing direction, without copying Ratchet's sideways riding stance. The rail grade is blended into camera pitch at up to 1.8 rad/s, then blended back to level on exit; head look-around remains independent. Entry records the current tracking-space head position, room-scale lean does not move the capsule off the rail, and exit consumes the remaining physical offset through ordinary collision-constrained room-scale movement. Pause retains the rail reference. Each rendered frame advances the pitch blend only once.

Compilation/startup evidence is recorded separately. Physical turning, stereo left/right, magnetic-track direction and rail comfort still require headset feedback. No tests or gameplay interaction scripts were run.
