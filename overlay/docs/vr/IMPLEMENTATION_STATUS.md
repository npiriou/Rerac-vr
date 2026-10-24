# VR arsenal and traversal expansion

Baseline snapshot: commit `3ee729e`. No new commit was requested for the changes below. No tests were run.

The VR and non-VR compile paths passed and the final executable launched normally into original Novalis, initialized both stereo views and applied the starting native inventory/ammo. This is integration evidence, not physical gesture validation.

| Requirement | Implementation / remaining physical validation |
| --- | --- |
| Snapshot current state | Done: `3ee729e` before this expansion |
| Kerwan start / all destinations | Default boot on level 3 (Metropolis), native level IDs 0..18 unlocked; ship list repaired to contiguous destinations 1..18 (0 is its terminator); all extracted level cores present, full per-planet VR playthroughs unverified |
| Correct both hands by 90 degrees | Thumb up / fingers forward on original brown meshes; visual feedback pending |
| Bomb fits glove | 0.104 m native radius; centre offset off the palm at (-0.135,+0.01,-0.125) controller metres, advanced 8 cm from the inner wrist toward the fingers; fit feedback pending |
| Native hands with weapons | Full brown cuffs; same native holding/firing pose and item attachment frame, including optional authored support hand; matched weapon glove replaces brown hand for throws |
| All weapons | Original native inventory granted; controller fire/aim wrapper, exact special aim for Blaster/Pyro/Visibomb/Suck Cannon; all native thrown glove types; physical behavior across weapons pending |
| Two-handed barrel grip | Left grip at original native support position on Suck Cannon/Devastator/Visibomb/Pyrocitor/Hydrodisplacer/RYNO; both hands guide aim; physical alignment pending |
| All gadgets | Owned startup inventory; individual grip mounts for Swingshot/Trespasser/Hydrodisplacer/Metal Detector, upright PDA/Hologuise remotes; native utility updates/trigger, wearable selection, gadget camera panel, Hologuise exit; physical fit and original level-specific interactions need play feedback |
| Vendor in world | Native screens/canvases on model's animated monitor quads; live locomotion/headset view; separate right-stick/trigger/Y shopping input; no fade/exit teleport; physical readability/purchase flow pending |
| Left trigger / R1 pack jumps and flips | Left index trigger sends native R1, fresh ground A requests pack jump with native side/back flip priority for a timed directional flick; right trigger independent, ordinary A immediate; feel feedback pending |
| Platform fall-through | Downward foot sweep plus retained rollback history address two concrete mechanisms; full traversal feedback pending |
| Hand ledge grab / pull-up | Native ledge/clearance check plus tracked grip reach; Fixed hand anchors and opposite hand displacement drive native ledge probes/hang physics; simultaneous two-arm downward shove invokes A-equivalent climb; 22 cm / 2.2 m/s two-hand shove required within 0.10 s, no slow/single-hand pull; joystick translation disabled, release-both drop remains; thresholds/moving edges need feedback |
| Arm-stroke swimming | Submerged palm drag feeds bounded native movement; native buoyancy/dive/oxygen/collision retained, blended swimmer eye height; stroke feedback pending |

Controls: Held right-stick click + right-hand displacement chooses an item. While the click is held, right-stick left/right changes wheel page. Original pause inventory equips wearable/back gadgets. Trigger uses the held gun/gadget or prepares its glove object; gesture + release throws. Left grip near the foregrip supports a long weapon. Grip by a native edge catches it; pull down to climb. Turn palms edge-on for swimming recovery. Visibomb: left stick steers, A boosts, fresh trigger detonates after native delay. B exits Hologuise.

The Suck Cannon retains its original captured-creature ammunition rather than receiving fabricated targets. Conventional reserves start at capacity once per application launch and then decrease normally.

## Physical weapon feedback correction

The user reported back-of-hand ammunition for all four gloves, floating/misoriented Devastator and Suck Cannon, barrels at the palm rather than handles on Blaster/Morph/Pyro/RYNO, a body-driven Morph ray, and requested physical Walloper punches. The four-glove point now uses controller -X. Eight weapon mounts use measured original grip centres independently of muzzle lists. Foregrips include side-handle offsets. Morph aim/lock and beam centreline follow the controller. The selected Walloper responds to an extending right-hand punch with native damage/effects and directional push, as well as its existing trigger lunge. New VR/non-VR builds and a normal original-Novalis startup passed; no automated tests. Physical confirmation of these corrections is pending.


## Latest reported regressions

Ship destination truncation came from level 0 inside the zero-terminated map order. Startup repairs that order. Black vendor/PDA monitors had their content 0.002 game units ahead of anchor joints, behind opaque front faces measured at 0.0224..0.0397; content now clears those faces at 0.05. Extra full-frame panels are suppressed in engine cutscenes and ship sequences. Empty trigger recalls the last held weapon, excluding the wrench (tracked grip preserves the saved Circle weapon), with a fresh press required to fire. Walloper now uses 7 cm / 1.45 m/s initiation, 0.06 s quiet rearm and 0.35 s cooldown, native horizontal lunge and original broad hit volumes. Build/startup evidence is recorded separately; no tests or headset interaction sequence were run for these changes.


## Combat and physical traversal refinements

Walloper physical punches and trigger attacks now reuse the native horizontal lunge and three-sphere damage/effects; punch-derived vertical launch is disabled. Wrench recall is excluded and native Circle saved weapon is preserved. The wrench is 20% larger with up to 15% swing elongation, native reach extension and head radius. Crates require an intentional faster swing. A downward airborne stroke invokes native Hyper Strike with native gates, accelerated fall, damage 2 and shockwave. Scripted gold-bolt pickup views omit the duplicate screen. Magnetic-track camera/hands and room movement use native body up. Ledge grips anchor hands and pull the body laterally through native probes; both arms shoved down invoke native A-equivalent climb. Right controller haptics cover wrench target hits and actual Blaster spawns. Physical thresholds and comfort require user feedback; compilation/startup is not physical validation.


## Arsenal/gadget haptics, damage and skip

Distinct firing/use feedback now covers the original handheld arsenal and active gadgets, with native-state triggers and cadence, reduced recoil for the supporting left hand, and no recoil for empty/cancelled shots. Native accepted damage has a cumulative counter so same-tick health pickups do not hide the hit; both hands receive a strong priority pulse plus a tail. Y queues the native scene/movie skip combination through its opening window, then suppresses the held click on return to play. Native cleanup/progression remains; an explicit VR click also permits the current normally-unskippable movie to finish through that same cleanup path. Headset feedback on intensity, cadence and actual skip interaction is pending; no tests were run.


## Planar Walloper, deliberate strikes, stronger ledge shove and visible body

Physical Walloper punches now enter the original lunge state instead of delivering a standalone instant hit; the native windup, horizontal speed/dash, hit window and effects run. Tracked movement has no punch-derived vertical launch. Wrench enemy hits require 4 m/s tip + 1.8 m/s hand + 10 cm travel; crates require 5 m/s tip + 2.2 m/s hand + 12 cm travel, with no wrist-only bypass. Ledge translation rejects joystick/d-pad input in XR and the native hero driver; climbing by gesture requires both gripped arms pushed 22 cm at 2.2 m/s within 0.10 s, with slow/single-hand climbing removed. Swingshot housing receives a 90-degree roll from wrist right to wrist top about its anchored grip/forward axis. Gameplay body rendering is disabled again; simulation and original animation remain native. Physical feel and fit need user feedback, not inferred from compilation/startup.


## Spatial equipment presentation

Gameplay body hidden again. Complete native glove/cuff triangles use the original layered pose, with held hands placed through the original item attachment; support holds are selected by native b18=2 (including Hydrodisplacer). Per-item deployment haptics follow mechanical animation changes and native sound markers, preserving use feedback. Pause/freeze UI uses native clipped primitive meshes, animated frame geometry and stereo menu models at a world anchor; no composed-game screen is shown. Visibomb samples its native missile camera on the original articulated joint-9 screen, without headset overlay panels or missile scanlines/fog replacing the HMD world. Native missile controls remain. Left trigger retains full native R1, visible crouch height and A/flip routing independent of pack availability; wrench throw input is unchanged. Compilation and ordinary startup do not validate physical fit or interactions; those need user feedback. No tests were run.


## Pause controller pointer

Right-controller ray and contact dot target native menu option rows, settings and equipment grids. Hover sets native focus/cursors; fresh right-trigger presses confirm through native actions, and trigger-held pointing controls sound sliders. Port Options arrows scroll its original rows. Disabled entries and transitions reject activation, loss of tracking/focus clears input, and held trigger is consumed through menu close until release. Native controls remain a fallback. Physical pointing/readability feedback pending; no tests or gameplay interaction sequences were run.


## Wrench wall contacts and Hyper Strike haptics

Hyper Strike has a light right-hand initiation cue and a strong grounded impact in the right hand, a softer left-hand shock and a short right-hand tail. A native landing counter increments once per accepted grounded slam; airborne damage/shockwave samples cannot repeat the landing feedback. Accepted damage retains haptic priority.

The held wrench head sweeps solid level geometry between tracked poses, with radius overlap and grip/eye obstruction recovery. Its native model and gripping hand stop at the surface; original sparks and the wrench wall sound accompany a deliberate impact. Haptic strength/duration depend on incoming normal speed, with a 1.3 m/s head approach and 0.65 m/s hand movement gate, a 0.22 s cooldown and withdrawal before rearming. Resting pressure does not repeat feedback. Damageable enemies/crates retain their existing stronger swing requirements. Safe head history is stored relative to the VR rig, preventing body locomotion from becoming physical strike velocity. Thrown-wrench input/flight remain native.

Pause capture now preserves valid tracked controller poses while resetting combat gestures, so the spatial menu pointer can receive hand poses during pause. Build/startup records do not validate physical contact feel or haptic intensity; those need headset feedback. No tests were run.


## Wrench rebound, stable pause content and anchored ledges

Deliberate wall impacts now give the held wrench and its native gripping hand a short visual recoil opposite the measured incoming stroke: 3–5.5 cm over 150 ms, then back to the tracked grip. The displacement is presentation-only, collision constrained and excluded from physical hit/velocity history. Existing contact rearming, sparks and haptics drive the recoil once per impact.

Spatial pause primitives now keep native painter order using explicit transparent-batch sort bias in both eyes. Previously meshes sorted by bounds, so panel clears/glass could cover later text despite per-vertex depths. Repeating monitor effects now use repeat samplers rather than clamping distant UVs to opaque border pixels. Noise keeps additive blending and FIX-style opacity independently of sampled alpha. Content sits 0.035 units ahead of the nominal frame plane; the controller pointer intersects that same plane. Native 3D frame and weapon preview geometry remains spatial.

Tracked ledge movement now closes the current world-space hand-to-anchor error instead of integrating controller deltas. A held ledge no longer refreshes its target from last tick's predicted velocity, removing the positive-feedback slide. A native validated lateral step is limited to 0.045 units per tick, with a 0.008-unit dead zone; failed, disconnected or excessive height/yaw changes restore the complete previous ledge and found flag. Catch entry preserves tangential body position; probes add no tangential displacement. Grips stay fixed until release, reject distant tracking discontinuities and travel with moving platforms. Native climbing still uses A or the existing deliberate two-arm downward shove; joystick translation remains disabled.

Compilation and ordinary startup do not confirm headset readability, contact feel or hand-over-hand traversal. Those remain pending user feedback. No tests or gameplay interaction scripts were run.


## Ledge contact feedback and stronger wrench/crate contact

Each accepted native hand anchor increments a per-hand contact counter and gives that controller a light 0.23 / 25 ms / 175 Hz latch pulse. An accepted ledge-climb jump entry increments a separate counter, whether requested by A or the deliberate two-arm shove, and drives both hands at 0.65 / 75 ms / 85 Hz plus a 0.24 / 30 ms / 145 Hz tail after 95 ms. Held grips do not repeat cues; rejected climbs do not emit them. Native damage feedback retains priority, and focus/menu/tracking lifecycle filtering remains active.

Wrench enemy hits now require 5.5 m/s physical tip speed, 2.4 m/s translating hand speed and 14 cm stroke. Crates require 6.5 m/s tip, 2.8 m/s hand and 16 cm stroke. For the same effective mass, the tip-speed minima represent approximately 89% / 69% more kinetic energy than the previous enemy/crate values. Motion still comes from tracking space, independent of locomotion, snap turns and visual rebound.

Intact crates now participate in held-wrench constraints using their original mesh and primitive colliders. A crate-only scene query prevents damageable enemies from masking that geometry, while the independent world query preserves wall blocking. The head sweep/overlap stops the native wrench and its gripping hand at a live crate; deliberate contact reuses the existing short recoil and contact haptic. Crate damage requires a real contact with that crate as well as the stronger stroke thresholds. Break/damage behavior remains native, and disabled collision volumes cease blocking the model after destruction. The first impact remains the reported contact if a second corner correction is needed.

No tests or gameplay interaction scripts were run. Native compilation and ordinary startup are recorded separately from pending headset feedback on grip/climb rhythm, harder strike thresholds and crate blocking/breakage.


## Normal, debug and debug-gold launch presets

`RC_VR_MODE` / `Run.ps1 -Mode` selects `normal` (default), `debug` or `debug-gold`. Normal forces the native front end, clears inherited level/scene-skip/grant/unlock/beat-game/skip-logo overrides, and never enters the VR inventory grant/refill path. Native New Game initializes the original save template and travels to level 0; Load Game retains normal progression. The previous direct-Kerwan launch, all inventory and planets, first-launch full reserves and skipped story scenes are now explicitly debug-only. Save roots are isolated by launch mode, without deleting existing saves.

Gold mode uses the original `item_offer::GOLD_ITEMS` table `[19,10,11,16,17,15,21,20,25,9]`. Flags are installed during native boot state creation and enforced by the debug grant. The existing gold look/effects paths remain native. Gold and ordinary weapons keep identical item IDs, classes, animation skeletons and attachment frames: they use the same VR transforms, grip/muzzle settings, native palm/foregrip poses, glove projectile placement, two-hand aiming and per-item deployment/use haptic profiles. No duplicate gold calibration table can drift away from the ordinary weapon's settings.

The native title scene supplies the frontend VR origin. Boot/title/main-menu primitives remain fixed in tracking space while the authored title camera moves, with original full-frame frontend fades retained. Native title camera layer changes complete before eye-layer mirroring. Gameplay pause keeps its existing world anchor and original 3D previews. Start is now Y; native A/menu pointer confirmation works on the main-menu page. Logos/movie presentation retains the native video surface.

Validation records cover compilation and ordinary debug-gold startup, with ten gold flags and full debug inventory/reserves. No tests or interaction scripts are run. Native normal-mode New Game/Load Game flow and physical frontend/gold-weapon use still need headset feedback.

## Walloper glove, ship pointer destination and physical gadget insertion


The Walloper hides the native right-hand glove mesh while held, preventing fingers from clipping through its housing; the secondary hand stays visible. In the ship, controller pointing updates both the native list cursor and the actual travel destination before confirmation. Known planet dots on the galaxy map can also be pointed at and confirmed.

The Hydrodisplacer and Trespasser accept physical insertion into their native pad/lock collision geometry. Approach with the tool tip aligned into the target, then push the right controller at least 3.5 cm with the tip passing approximately 1.2 cm into the contact surface. Only tracked controller translation counts; body locomotion, wrist rotation and tool animation do not supply that stroke. An accepted insertion enters the original water-transfer or lock-puzzle logic, retains its existing event haptics and suppresses the original automatic pull of Ratchet toward the pad. Withdraw before another activation. Native trigger activation remains available. Physical alignment, reach and insertion feel still require headset feedback.

The interaction feeds a separate tracked utility target and one-shot use request; the actual ground moby is never spoofed. The request and target are retained for up to 250 ms until the native action acknowledges them, covering the moby-before-hero update order. The active target then survives native water cutaways and lock puzzle input capture. A bound lock excludes other ground locks from the same activation. Water pad glow/animation and lock readiness accept the tracked target. Tracking-space translation, approach normal, penetration, short stroke, native readiness, nearby target and obstruction checks gate activation. A latched contact requires withdrawal before reuse; weapon/level/recenter changes invalidate it. Compilation and ordinary startup do not validate physical interaction. No tests or interaction scripts were run.

## Headset heading, spatial sound and grind-rail camera


Headset orientation now supplies a separate native locomotion frame and audio listener. Ratchet's ordinary free-movement heading follows physical turns and snap turns, including while stationary. Ground movement can strafe/back up without turning the avatar away from the headset. Magnetic-track planar velocity follows the requested direction projected onto the local surface instead of the stale body heading. Native authored camera state remains independent for scripts and menus; rail, ledge, attack and scripted interaction poses keep their native body orientation.

Spatial sound allocation (hero/mobys/items, contextual interactions and UI) and per-tick mixing use the physical headset's position and full forward/left/up basis, with post-move body translation applied to the sampled eye. Desktop listeners keep the original camera fallback. Normal VR stick input is consumed directly in the levelled headset frame, avoiding an additional rotation through the desktop camera; rail lean/switch axes remain native.

Grind rails anchor VR yaw to the native rail tangent and follow its changing direction, without copying Ratchet's sideways riding stance. The rail grade is blended into camera pitch at up to 1.8 rad/s, then blended back to level on exit; head look-around remains independent. Entry records the current tracking-space head position, room-scale lean does not move the capsule off the rail, and exit consumes the remaining physical offset through ordinary collision-constrained room-scale movement. Pause retains the rail reference. Each rendered frame advances the pitch blend only once.

Compilation/startup evidence is recorded separately. Physical turning, stereo left/right, magnetic-track direction and rail comfort still require headset feedback. No tests or gameplay interaction scripts were run.


## PC performance configuration

The visibility rewrite was rejected and archived outside the active source trees. Original engine entrypoints remain unchanged. The current PC configuration uses a native CPU Release build, a dedicated NVIDIA maximum-performance application profile, default task pools and production Vulkan debug/validation flags. Earlier configurations have free-play headset timing records; the active Release build has either its own capture or an explicit pending status in validation.json. No controlled FPS gain or full visual certification is claimed. No tests, scripted gameplay or commit. See [PERFORMANCE.md](PERFORMANCE.md) and the pc_performance_configuration record.


## Rail comfort, ledge reach, gold Devastator support and physical Hyper Strike

Rail camera direction is sampled across 1.5 units behind and 2.5–5 units ahead of the native spline cursor, rather than copying a single segment tangent. Yaw uses a 350 ms exponential response, a 0.006-radian dead zone and a 1.05 rad/s turn limit. Pitch uses a 500 ms response, a 0.008-radian dead zone and a 0.35 rad/s limit. Translation has an 80 ms filter while riding, recovering on exit; large teleports rebase. Rail entry preserves the current gaze instead of forcing a turn. Physical headset rotation stays direct.

Ledge hanging places the neutral eye about 38 cm below the ledge/palms, with the existing posture blend. Valid native ledges accept a hand within 80 cm horizontally and 50 cm vertically (48 cm into/across the wall for initial reach). Retention and native top/clearance checks remain. Pull-up needs a 12 cm downward stroke at 1.15 m/s from both gripped hands within 180 ms. Lateral anchor correction and its drift/validation limits are unchanged.

Gold Devastator support is anchored at the centre of the actual class-157 side handle, model coordinates (0.125, -0.415, -0.270). The support-grip capture/aim target and rendered left palm use the same point. The original layered finger/wrist pose is retained; the ordinary Devastator is unchanged.

Hyper Strike's airborne animation gates and native height/cable rules are shared by button and VR paths. In particular, the native flip gates are strictly after frame 18, or 20 for flip kind 3, rather than the former VR-only 25/26 gates. A 5 cm downward wrench-hand stroke at 1.2 m/s uses a separate interpolated 28 ms velocity window; normal melee/throw smoothing and hit thresholds remain unchanged. The gesture can stay prepared within the same native input-buffer windows while a flip becomes eligible. An accepted VR gesture enters native state 0x14 at its falling animation phase, cancels upward velocity, and skips the additional upward/windup delay. Native descent acceleration, terminal velocity, shockwave/contact damage and landing haptics remain. Desktop/button windup is retained.

No tests or gameplay interaction scripts were run. The game is not launched or restarted at the user's request. Compilation evidence is separate from pending headset feedback on comfort, grabbing/pull-up, the gold handle and immediate Hyper Strike.


## Native ledge hands, grip priority, Start/Triangle swap and High defaults

Each hand bound to a ledge uses Ratchet's original reach/hang animation palette without weapon-holding arm layers. The glove follows the native body/wall orientation, wrist depth and height at the lip, translated along the ledge to that hand's accepted anchor. Controller rotation no longer turns an attached glove into a free-hand pose. One held hand is sufficient to enter/maintain the hang and move laterally; releasing the other does not drop Ratchet. The existing two-hand downward stroke remains the pull-up gesture.

A fresh right-grip squeeze does not request the wrench while the left grip is held, a ledge candidate is available, or the right hand is attached/hanging. A new wrench request waits one native collision/probe tick so a right-hand-only catch has priority. A suppressed squeeze requires release and a fresh press before it can equip a wrench later; releasing from a ledge cannot become a wrench throw. The frame in which a right-hand anchor is accepted also hides held weapon/glove geometry. Ordinary wrench throwing remains available outside ledge use.

Touch/PICO Y (Index left B / Microsoft left menu) now sends native Start: pause, frontend start and queued cinematic skip. Right thumbstick click sends native Triangle: contextual interaction, vendor cancel, and held quick select with right-hand sector selection. Hold the right-stick click and move the right stick horizontally for quick-select pages. Left thumbstick click remains Select/map. Existing Start skip suppression through release and Triangle contextual priority are retained.

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

[Detailed VR polish review](VR_POLISH_REVIEW.md) records every requested change, compilation evidence and pending headset checks for this batch.
