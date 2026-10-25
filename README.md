# ReRAC VR

An OpenXR integration overlay for the original [ReRAC](https://github.com/re-rac/rerac) game, pinned to upstream `89c87e98f9d8296187df2a6310c67b32144090d3`.

`RC_VR=1` starts the original application: its level loader, Ratchet controller and animations, moby simulation, scenery, collisions, audio and level transitions. Two OpenXR world cameras and matching sky cameras render the same entities and original materials. Locomotion uses ReRAC's existing PS2 pad input. Tracked-hand wrench impacts use the original collision scene and hit records; wrench and bomb throws use the original projectiles and damage.

Gameplay is **first person**, on the existing complete level. The view follows Ratchet with physical head tracking and independent 30-degree snap turns. Eye height is 1.1 game units on foot (Ratchet's eyes, also in cutscenes) and blends to 0.45 while swimming. Jump/flip animation and the follow camera cannot rotate the headset; magnetic tracks rotate the tracking frame with the native body up axis. The original brown Ratchet hand meshes and textures follow both controllers; the original Bomb Glove replaces the right hand when equipped or preparing a bomb. The original wrench model follows the right grip. The original animated gameplay body is hidden. Existing upstream gameplay limitations remain.

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
| Vendor open | Left stick walks, A jumps, right stick browses / adjusts quantity, fresh right-trigger presses buy / confirm, right-stick click cancels / closes. Displays stay on the vendor's monitors |
| Right A | Immediate jump / double jump. Hold the left index trigger + press A for pack jumps or a native side/back flip with a timed directional flick. Confirm in menus |
| Left index trigger | Native R1 modifier: packs and flips; right trigger remains weapon use. R1 in menus: next map / planet / quick-select slot |
| Left grip in menus | L1: previous map / planet / quick-select slot |
| Right grip held | Hold the wrench; swing the hand to hit |
| Downward wrench strike in the air | Native Hyper Strike: accelerated descent and double wrench damage |
| Release right grip during a throwing motion | Throw the wrench in the direction of the hand's motion; automatic return |
| Release right grip slowly | Put the wrench away; squeeze again to hold it |
| Right trigger held, a gun or hand gadget selected | Native fire/use action aimed by the right controller |
| Right trigger held, a throwing glove selected | Prepare its native bomb, mine, Doom canister or decoy; maintain the hold, then throw by moving and releasing |
| Right trigger with an empty hand or wrench | Re-equip the last gun/glove/gadget, ignoring the wrench; release and press again to use it |
| Release right trigger during a throwing motion | Throw the bomb with the hand's direction and speed; consume one original ammunition unit |
| Release right trigger slowly | Cancel the prepared bomb without using ammunition |
| Right stick click | Triangle — contextual interactions; hold for quick select, move the right hand toward an icon, release to confirm |
| Walloper selected: punch with the right hand | Original lunge and horizontal dash, with native windup, speed and broad damage window; no vertical punch propulsion |
| Left grip near a heavy weapon's barrel | Hold the foregrip and guide the weapon with both hands; release to return to one hand |
| Grip near a ledge while falling | Catch with a grip; move gripping hands along the edge to pull the body; shove the gripping hands down at least 12 cm at 0.9 m/s (from their recent top) to climb/jump exactly as A (left-stick movement disabled while hanging); release both grips to drop |
| Submerged palm strokes | Push water opposite the motion of the hands; turn palms edge-on for recovery. Push up to dive |
| Right stick click held + right trigger | Next quick-select page (a trigger already held when the ring opens does not turn it; the press fires nothing until released) |
| B / X | □ / ○ in menus. In play X holds L2 and B R2 (hover and Clank strafes, hoverboard tricks, Hydro-Pack boost, ledge drop); B exits the Hologuise and both keep □ / ○ on turrets and ships |
| Left stick click | Select — map |
| Left Y | Pause during play; skip the current scene/movie during a cinematic (one click queues through its opening fade) |
| Either joystick in menus | Directional navigation; return to centre between steps. A confirms, Y goes back |
| Keyboard F8 | Recenter headset |

A jumps immediately on press, with no long-press delay. The left index trigger sends native R1: use it with A for pack jumps, or a side/back directional flick + A for the native flip chooser. Flip priority precedes the special pack jump; the right trigger remains independent for weapons. R1 retains the original movement-state rules, with a blended crouch eye height; headset rotation never follows a flip animation. Teleportation has no XR gameplay binding. A regular gamepad and the original keyboard bindings remain usable. Initial recenter waits until the headset is focused. F8 recalibrates the physical headset pose without moving Ratchet. Camera pitch/roll track physical head motion relative to the current floor, including magnetic tracks. The original pause menus are placed directly in the world; the ordinary HUD retains its headset layer.

In `debug` and `debug-gold`, all original weapons and gadgets (native inventory IDs 1..35) are available at startup. Normal mode grants nothing beyond native progression. Hold the right-stick click and move the right hand toward an icon, then release the click. Change pages with the right stick while holding its click. Wearable/back gadgets are equipped through the original pause inventory. Native models, cadence, ammunition, collisions, damage, effects and interactions are retained. The Visibomb shows its native remote camera on the articulated screen built into the weapon: steer with the left stick, A boosts and a new trigger press detonates after its native delay. The Drone Device activates when selected, as in the original game.

In the debug modes, every owned weapon with an ammunition reserve starts at its native capacity once per application launch; use then decreases it normally. The Suck Cannon still captures creatures as its ammunition. Complete original brown gloves include their forearm cuffs. Held weapons retain Ratchet's native layered gripping-hand pose; throwing weapons use their matching original glove. The Swingshot is rolled 90 degrees onto the top of the wrist while preserving the palm anchor and forward direction. Calibrated weapons put their rear grip at the right controller; their animated muzzle is derived separately. Throwing ammunition rests on the palm side of the glove. All native two-hand holds (Suck Cannon, Devastator, Visibomb, Pyrocitor, Hydrodisplacer and R.Y.N.O.) accept the optional left grip at their authored support-hand position.

Grip release requires a valid tracked throw gesture; slow releases and tracking/focus loss do not launch the wrench. The wrench is 20% larger, stretches by up to another 15% during a swing, and sweeps its head with the native 0.35-unit radius plus 0.17-unit reach extension. Enemy hits require 5.5 m/s tip speed, 2.4 m/s translating-hand speed and a 14 cm stroke. Crates require 6.5 m/s tip speed, 2.8 m/s hand speed and a 16 cm stroke, plus actual contact with their native collision geometry. Intact crates stop the held wrench and gripping hand, with the existing contact recoil and haptics; a weak stroke cannot break them. Wrist-only motion cannot bypass these requirements. A per-target cooldown limits repeated damage. Weapons and active gadgets have distinct feedback profiles tied to native firing/use, capture, attachment, puzzle and scan events. Weapon deployment adds individual profiles following the original unfolding motion changes and animation sound markers. A supporting left hand receives reduced heavy-weapon recoil. Accepted damage gives a strong priority pulse on both hands. Snap turning and locomotion do not count as swings. These thresholds, glove/weapon alignment and physical comfort still require headset feedback.

The Bomb Glove requires ownership and available ammunition from the existing inventory. With its glove selected, the bomb follows the hand while the right trigger is held. After a valid release gesture, the original bomb simulation handles gravity, collisions, fuse, explosion, damage and effects. A slow release, lost tracking/focus, pause, death or a blocked hand cancels preparation without consuming ammunition. A fresh trigger press is required for each new bomb.

## Implementation and validation

The visibility rewrite was rolled back and archived outside the active source trees. For play, use the Release build; the measured PC also uses a native CPU build, a dedicated NVIDIA maximum-performance application profile and production Vulkan GPU diagnostics settings. No engine source or graphics quality settings were changed for this configuration. Live timing and rollback details: [PERFORMANCE.md](overlay/docs/vr/PERFORMANCE.md).

Guarded source edits are in `patch-manifest.json`; new modules and scripts are in `overlay/`. `resolved-Cargo.lock` pins the verified dependency graph. The default desktop path remains available without `RC_VR=1` and without the optional `vr` feature.

The build script compiles by default without running tests. Only pass `-RunTests` when the user explicitly requests tests, as required by AGENTS.md. Tests from earlier runs are recorded separately. See [local build evidence](LOCAL_BUILD.md), [runtime records](STARTUP_READINESS.json), and [VR implementation details](overlay/docs/vr/README.md). Physical comfort, GPU/compositor budget and full campaign completion require separate validation.

The gameplay body is hidden again. Pause/freeze menus use original world-placed primitives, native animated frames and stereo menu models around their opening pose, without a composed-frame headset screen. Left trigger holds native R1 (crouch with blended eye-height change, timed directional flips and pack jumps with A); wrench throwing retains its grip/release gesture.

In the spatial pause menu, aim the right controller at an option or equipment icon and press its trigger to confirm. Cyan marks an actionable target. Hold the trigger on an audio slider to set it by pointing; the native joystick/buttons remain available. Release the trigger before firing after leaving the menu.

Menus, help boxes, banners and subtitles show the headset's buttons instead of the PS2 icons: ✕ becomes A, ○ the right trigger (RT), △ a right-stick click, □ the right grip (RG), L1 the left grip (LG), R1 the left trigger (LT), L2 X, R2 B, and the analog sticks a left (L) or right (R) thumbstick. Each icon names a control that sends that button.

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


## Rail comfort, ledge reach, gold Devastator support and physical Hyper Strike

Rail camera direction is sampled across 1.5 units behind and 2.5–5 units ahead of the native spline cursor, rather than copying a single segment tangent. Yaw uses a 350 ms exponential response, a 0.006-radian dead zone and a 1.05 rad/s turn limit. Pitch uses a 500 ms response, a 0.008-radian dead zone and a 0.35 rad/s limit. Translation has an 80 ms filter while riding, recovering on exit; large teleports rebase. Rail entry preserves the current gaze instead of forcing a turn. Physical headset rotation stays direct.

Ledge hanging places the neutral eye about 38 cm below the ledge/palms, with the existing posture blend. Valid native ledges accept a hand within 80 cm horizontally and 50 cm vertically (48 cm into/across the wall for initial reach). Retention and native top/clearance checks remain. Pull-up needs a 12 cm downward stroke at 0.9 m/s, measured from each hand's recent top, from every gripping hand within 180 ms; it acts as a buffered native ✕. Lateral anchor correction and its drift/validation limits are unchanged.

Gold Devastator support is anchored at the centre of the actual class-157 side handle, model coordinates (0.125, -0.415, -0.270). The support-grip capture/aim target and rendered left palm use the same point. The original layered finger/wrist pose is retained; the ordinary Devastator is unchanged.

Hyper Strike's airborne animation gates and native height/cable rules are shared by button and VR paths. In particular, the native flip gates are strictly after frame 18, or 20 for flip kind 3, rather than the former VR-only 25/26 gates. A 5 cm downward wrench-hand stroke at 1.2 m/s uses a separate interpolated 28 ms velocity window; normal melee/throw smoothing and hit thresholds remain unchanged. The gesture can stay prepared within the same native input-buffer windows while a flip becomes eligible. An accepted VR gesture enters native state 0x14 at its falling animation phase, cancels upward velocity, and skips the additional upward/windup delay. Native descent acceleration, terminal velocity, shockwave/contact damage and landing haptics remain. Desktop/button windup is retained.

No tests or gameplay interaction scripts were run. The game is not launched or restarted at the user's request. Compilation evidence is separate from pending headset feedback on comfort, grabbing/pull-up, the gold handle and immediate Hyper Strike.


## Native ledge hands, grip priority, Start/Triangle swap and High defaults

Each hand bound to a ledge uses Ratchet's original reach/hang animation palette without weapon-holding arm layers. The glove follows the native body/wall orientation, wrist depth and height at the lip, translated along the ledge to that hand's accepted anchor. Controller rotation no longer turns an attached glove into a free-hand pose. One held hand is sufficient to enter/maintain the hang and move laterally; releasing the other does not drop Ratchet. The existing two-hand downward stroke remains the pull-up gesture.

A fresh right-grip squeeze does not request the wrench while the left grip is held, a ledge candidate is available, or the right hand is attached/hanging. A new wrench request waits one native collision/probe tick so a right-hand-only catch has priority. A suppressed squeeze requires release and a fresh press before it can equip a wrench later; releasing from a ledge cannot become a wrench throw. The frame in which a right-hand anchor is accepted also hides held weapon/glove geometry. Ordinary wrench throwing remains available outside ledge use.

Touch/PICO Y (Index left B / Microsoft left menu) now sends native Start: pause, frontend start and queued cinematic skip. Right thumbstick click sends native Triangle: contextual interaction, vendor cancel, and held quick select with right-hand sector selection. Hold the right-stick click and press the right trigger for the next quick-select page. Left thumbstick click remains Select/map. Existing Start skip suppression through release and Triangle contextual priority are retained.

Port Options now offers a High preset: requested 8x MSAA (clamped to GPU support), anisotropic Sharp textures, Maximum model detail and Sharp-pixel HUD. VR adopts those settings and enabled shadows once, including existing saved settings, then preserves subsequent menu choices. Fresh VR settings default to High; explicit environment overrides remain available. The former forced RC_MSAA=0 is removed, and the headset's world/sky eye cameras follow Port Options MSAA as well. Desktop original defaults and display/window geometry remain independent.

Compilation evidence is separate from physical headset feedback. No tests or gameplay interaction scripts are run. The game is not launched or restarted.


## Physical cranks and first-person cinematics

The right hand now chooses quick-select sectors while holding the right-stick click (Triangle). Y remains Start/pause/skip.

Bring the held wrench head within 50 cm of a bolt crank socket and keep the right grip held. Walk Ratchet around the bolt with the left stick or physical room-scale movement, as in the original game. Progress uses the actual capsule orbit after collision; rotating the wrist or moving only the hand contributes nothing. The wrench uses its original crank sequence 0xb at frame 2 and stretches from the tracked grip to the socket while the glove stays rigid. The head remains seated during snap turns and brief reach excursions. Distance release requires more than 2.40 units of hand reach or 3.40 units of body distance for 250 ms; releasing the grip still disengages immediately. Body orbit progress remains valid out to 3.0 units. A full orbit counts as one native turn; original driven doors/platforms, completion, mission and save code remain. Releasing or withdrawing the hand disengages without throwing the wrench. Native desktop cranking remains available outside VR.

Short gameplay cutaways retain their event timelines but no longer draw letterbox bars in VR or impose their cinematic holds, forced poses/teleports or blocking entry fades. Ratchet keeps native movement, jumping and weapon input. Dialogue/engine scenes retain their authored actor progression: the VR eye follows the animated Ratchet head with free headset rotation and the duplicate scene body hidden. Planet-side takeoff/landing follows the animated Ratchet actor's actual helmet/head attachment through boarding, sitting and climbing out, retaining free head rotation. When no Ratchet actor is available, the cockpit uses the animated ship root and native Ratchet seat with a raised 1.45-unit seated eye offset (previously 0.85). Space loading between planets retains the original external camera; no cockpit override is applied there. Full-motion video remains video.

Kerwan cable-slide eye height leaves 55 cm below the cable (target 0.55–1.15 game units above the capsule origin), blended through the existing posture transition. Thrown wrenches spin in a horizontal plane through outbound and returning flight, independent of release wrist pitch/roll; the native travel and collision paths remain.

These changes require physical headset confirmation, especially crank socket alignment, scene head position and cockpit placement on all three ships. No automated tests were run, per AGENTS.md.


The scene eye now resolves the native head attachment chain (list 4), rather than treating manipulator list 0x18 as a bone index. Ship boarding/landing samples that same actual head chain with current streamed animation and joint modifiers; it adds 0.15 game units at the helmet attachment for the eye position. Wrench socket stretching uses the authored crank pose, rigid grip position and a bounded axial scale, so its head remains on the bolt while the handle follows the controller. Shaft stretch does not stretch the visible holding glove. These placements still require headset confirmation; no automated tests or game relaunch were performed for this revision.


## Clank pack haptics

All three packs now have their own bilateral motor feedback, independent of held weapon feedback:

| Pack | Feedback and native trigger |
|---|---|
| Heli-Pack | Fine, alternating rotor beats at 165–180 Hz; stronger during native long-jump lift/high-jump acceleration and double-jump lift, softer while gliding. Starts at actual takeoff or the native high-jump curve window, with a short spin-up and gentle release. |
| Thruster-Pack | Deeper ignition kicks and jet rumble at 50–90 Hz. Uses the original `thruster_flame::thrusting` predicate, including boosts, powered flips/double jumps and ledge-climb flame windows. Distinct glide/hover strength, rising/falling stomp phases and a strong two-hand impact on accepted grounded stomp. |
| Hydro-Pack | Smooth 50–65 Hz propulsion in native Hydro swimming; strength follows speed, with a stronger acceleration profile under native R1/R2 boost. No motor vibration while merely equipped on land or motionless without propulsion. |

Pack motor pulses have their own cadence and owners, so firing or changing a hand weapon cannot reset them and weapon continuous-use feedback remains available simultaneously. Pack background rumble has lower priority than weapons; accepted damage retains highest priority. Pause, focus loss, death, pack swaps and stopped simulation clear pack pulses. Body-worn pack feedback can reach connected controllers through optical hand occlusion. No tests or gameplay scripts were run, and physical intensity/rhythm still require headset feedback.

## Solid VR hands and held weapons

Free hands, original glove cuffs, held weapons and gadgets now stop against native solid world/moby geometry and live crate colliders. Conservative contact spheres are derived from slices of each actual animated mesh, including deployed parts, rather than a single controller point. Fast movement sweeps from the previous safe pose; initial eye recovery handles starting beyond a wall. Contact slides along surfaces and broken/disabled crates stop blocking the models.

A held weapon and its original gripping hands receive one shared translation, preserving both native handles. Free hands resolve separately. Accepted ledge grips and a wrench seated in a crank retain their intentional native anchoring; thrown wrench flight retains native collisions. Physical controller poses and velocities remain the gesture source. Fixed weapon aim/muzzle and glove preparation/release positions use safe contacts, and the live Visibomb monitor stays attached to the rendered weapon. Primitive-only crate line queries are now included in wrench contact detection.

Compilation is separate from headset validation. Conservative clearance, all weapon sizes, corners, two-hand contact, held ammunition and frame cost still require headset confirmation. No tests, game launch or commit were requested for this change.

## VR interaction and presentation polish

[Detailed VR polish review](overlay/docs/vr/VR_POLISH_REVIEW.md) records every requested change, compilation evidence and pending headset checks for this batch.


## Clank and Giant Clank in first person

Clank and Giant Clank sections are first person. The eye sits at the original first-person heights, 0.9 above Clank's feet and 9.5 above Giant Clank's, and Giant Clank's tracking space is scaled to his size (×5.9: head, hands and room-scale steps). In VR the body, Clank's rotor and his antenna glow are not drawn; his own hands follow the controllers, cut from his model in a rest pose. Each hand is a physical fist, like Ratchet's wrench: a deliberate punch (2 m/s, 12 cm) strikes where that hand is with the body's original sphere and damage (Clank's, or Giant Clank's fist); the native punch animation is not used. Free hands close into a fist with the grip. Clank's ledges work as Ratchet's: grip to catch, hands anchored on the edge move him along it, A or a two-arm shove climbs, releasing both grips drops. The right trigger holds Giant Clank's missiles (○), the right-stick click is △ (Clank's commands, Giant Clank's beam). Room-scale walking moves the bodies too. Headset validation is pending.
