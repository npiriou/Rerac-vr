# VR interaction and presentation review — 2026-10-24

This checklist records source review and compilation separately from headset validation. The user requested a debug relaunch after completion. Automated tests were not run, in accordance with AGENTS.md; no commit or push was requested for this batch.

| Requested fix | Implementation reviewed | Headset validation |
|---|---|---|
| Wrench throw flashes | After-image and particle changes were insufficient according to the user. The current fix renders the cloned VR wrench with its original ready pose during outgoing flight, return and the catch blend; native flight rows supply the horizontal spin. Detached presentation does not inherit held-item spring modifiers. Native flight/damage/hit joints are unchanged. Original distant impact bursts are restored; numerical particle safeguards remain. | Confirm the complete return path and catch; source inspection is not visual confirmation. |
| Wrench enemy impacts | Accepted physical hits increment a persistent tracked event counter. Right-hand impact has priority 10, a stronger 85 ms kick and short tail, separate from wall contact and deployment. | Confirm successful hits, including rapid successive enemies. |
| Green cockpit and ship attitude | Starter-ship eye moved forward near its actual seat; final ship offset retained. Animated ship-root matrix supplies full pitch/roll/yaw. Boarding still samples the native head position; seated motion adopts ship attitude. Travel class-0 actors and duplicate dynamic riders are hidden. | Check green/purple ships, boarding and exit continuity, steep takeoff and landing. |
| Bolt collection | Small bilateral 16 ms, priority-0 taps on committed bolt increases, rate-limited to 90 ms. Purchases do not trigger them. | Confirm collection is subtle and weapons/damage take priority. |
| Suck Cannon | Captured-load increase and committed creature ejection have distinct priority-10 heavy kicks/tails. Two-hand support receives a second, weaker recoil. The vacuum motor retains its softer loop. | Absorb/fire enemies and loose pickups separately. |
| Destination readiness | A GPU copy retains the last unfaded space image. It covers the new world while destination meshes/images prepare and pipelines remain queued. Native movement/travel advancement waits; destination world renders behind the cover. The cover waits for twelve consecutive GPU-ready render frames, then allows the first native destination frame behind it and waits for a second readiness pass before revealing the landing. | Check several planet changes, first visits/revisits and headset focus loss. No GPU frame-cost measurement claimed. |
| Menus and frontend | Native page/widget targets remain directly clickable. Native dialog label positions map to their actual Cross/Circle/Triangle actions. Title accepts trigger on Press Start. Visible Back, Confirm and directional controls cover remaining native menu actions, including card dialogs. | Traverse new/load/save/options, map, confirmation and return flows using only pointing/trigger. |
| Vendor/PDA | Ray intersection uses the actual animated monitor quads. Buy/Close and popup Yes/No are separate actions; ammo arrows change quantity. Clicking a carousel icon selects its entry and refreshes the native ticker/hologram. Trigger outside an actionable monitor no longer buys. | Verify each monitor, PDA, prices, quantities, locked entries and cancellation. |
| Ledge retention | Established anchors no longer release from hand displacement or optical pose loss while grip remains held. Grip hysteresis prevents small analog variation from releasing. Native hang/shimmy probe failures no longer override an accepted VR grip. Damage/fall transitions can retain the held ledge. | Catch during a fall, one hand, two hands, tracking occlusion and deliberate pull-up/release. |
| Higher rungs/climbing | Physical hand-height probes supplement the capsule's narrow ledge-height window. An unbound hand can catch a higher native-validated ledge/rung while the other remains latched, allowing successive regrips. Native wall/top/clearance checks remain; arbitrary walls do not become ladders. | Check the actual ladder/rung geometry in the game; this is not validated for every level. |
| Grind rails | Continuous heading/pitch filtering replaces deadband steps. Native fixed-step positions interpolate to render time before positional filtering. A soft, speed-dependent bilateral rail texture runs at low priority. | Verify tight curves, segmented rails, slopes, entry/exit and comfort. |
| Trespasser | Physical insertion or optional trigger starts the native reach/hold animation, with both hands following the tool. Socket binding persists through the puzzle. Only the native rings, emitters and beams are shown, without added text, buttons, background or pointer. Stick directions operate the rings; Y, right stick click or a fresh trigger press cancel and withdraw. | Check physical alignment, reach animation, solve/cancel and withdrawal before reinsertion in the headset. |
| Hydrodisplacer | Physical insertion or optional trigger retains the socket through native in/use/out states. The generic VR cutaway filter no longer discards gadget animation/state calls, allowing the water command to run. Both hands follow the authored tool animation. | Verify filling and emptying, with and without trigger, and complete water-transfer/withdrawal timing in the headset. |
| Crank bolts | Socket height is the animated class-280 mesh's top. Engaged wrench is level and extends from grip to socket. Contact proximity captures without a swing. Forward stick resolves to an orbital tangent with radius correction; progress still comes from actual collision-resolved capsule movement. | Inspect top grip, level orientation, forward orbit in both directions, completion and release. |
| Swimming | Native underwater swimming/Hydro velocity and body heading follow full headset gaze. Physical stroke propulsion follows gaze; surface propulsion stays horizontal. Native speed curves, boost, oxygen and collision remain. | Look up/down/sideways with hand strokes, stick and Hydro boost. |
| Heavy weapons | Native two-arm items gain bounded rotational inertia with one grip. Physical two-hand support stabilizes the aim immediately. Render pose and muzzle use the same weighted orientation; raw attack/throw gesture velocity is untouched. | Check each heavy weapon and gold counterpart, firing aim and support acquisition/release. |

Compilation evidence and the final executable hash are recorded in `PROVENANCE.json`, package `STARTUP_READINESS.json`, `validation.json` and the local build log. Physical fit, comfort and timing above remain user checks, not inferred from compilation.

### Follow-up: flash near maximum throw range

The first after-image change did not resolve the user's observed flash. The user localized it near the maximum throw range. Source review found no deliberate visual event on the OUT-to-BACK transition, and read-only inspection of the original wrench's spin animation found stable mesh bounds. This does not identify the observed frame conclusively.

The follow-up removes large legacy wrench impact ribbons in VR, uses small unsplit sparks and actual headset-eye clearance, and prevents per-eye particle projection from normalizing a zero/view-aligned ribbon or dividing by a near/behind-eye endpoint. At the turnaround, effectively stationary/non-finite collision segments cannot produce a bounce effect. Turnaround and bounce positions are now logged separately so a persistent flash can be associated with the actual event. Confirmation still requires the user's physical throw, especially into empty space at maximum range. No automated tests run.

### Follow-up: the wrench model during return

The user confirmed no improvement from the particle-focused attempt and identified the wrench model during its return. The captured logs show ordinary OUT-to-BACK transitions without a bounce for multiple throws; their positions and return targets are finite. Read-only inspection of the original ready/spin/return-blend assets found finite, nonsingular matrices but a substantial change in the posed mesh bounds: about 1.02 units wide in ready pose versus 2.01 in spin pose at the current VR scale. This does not prove the exact faulty pixels, and is not presented as a reproduced headset result.

The VR presentation now uses the original ready mesh rigidly throughout both flight phases. The native model matrix supplies translation and horizontal spin, while the native simulation retains its authored animation, hit joints, acceleration, return, collection and sounds. The catch is also rendered in the ready pose while the native spin-to-ready blend finishes, so the attachment is computed from the same mesh pose. Held-item spring modifiers are cleared from the detached visual. Phase-change logs record the native and rendered animation IDs separately. Original distant impact effects were restored because reducing them did not solve the reported bug. No automated tests run.

### Follow-up: return path, native gadget animations and actor eye positions

The user reports that white flashes are gone but the return remains unreadable. The outgoing flight is preserved. VR return now uses bounded homing speed and slows within reach, omits the unrelated ground-height correction, and eases the rigid model into the receiving hand orientation. Catch switches to the ready pose and keeps the wrench visible in hand until the next grip release. Damage, collection and native sounds remain on the existing item path.

The generic VR camera-cutaway filter had been discarding SetAnim/state calls needed by the gadgets. Accepted socket interactions now retain those calls. Pending requests remain bound until acknowledged, and completed/cancelled physical insertions must withdraw before rearming. The lock no longer forces a final reach key on every render, and the hands are attached to both gadgets while docked. Cancellation edges are queued to the simulation tick and consumed until release, preventing a cancel input from opening pause or immediately starting another action.

Scene eyes are evaluated from the complete head attachment chain, not an inverse-bind skinning palette. Ship boarding uses the rendered actor's scale and valid streamed keys, ignoring hidden actors. Captures and death sequences retain the last living camera position with physical head tracking, so body relocation during swallowing does not move the headset into the sky. Respawn and level changes resume the live position.

These are source-level corrections. No tests or gameplay runs were requested or performed for this follow-up. Compilation results and the executable hash are recorded separately; Batalia's commando scene, ship exits, swallowing attacks, both water directions and the full wrench return still need headset confirmation.


## Planet preparation during space flight and cinema presentation

The disk worker was already asynchronous, but the engine did not publish the destination ECS world or prepare GPU resources until the flight ended. Destination publication now starts during flight. Flight-only entities, the departure simulation (whose actor IDs must not address the destination moby table), its level data and audio remain available until handoff. The destination simulation stays paused. A hidden eye-format view warms destination materials while the visible flight remains intact; shared LOD and occlusion use the destination view for the new world. Native flight completion waits for GPU readiness and can loop when needed, including first arrivals. The short blackout is now the cut to an already prepared planet, not the primary loading screen. Landing reveal waits for an actual native actor pose, not a gameplay tick that may stay stopped during the cinematic.

Flat movies and slideshows use a black surround with the level sky/world excluded. Their screen is level, larger, preserves the composed frame aspect and stays at the viewer's entry pose rather than following every head movement. Recenter repositions it. No interface text was added.

ECS publication and LevelStartup remain synchronous and may still produce a brief hitch during flight; this change does not claim fully asynchronous scene construction or measured hitch-free travel. The transition behavior and movie presentation require user headset confirmation. No tests or scripted gameplay were run.


## Wrench return, utility gadgets and crank jaws

The thrown wrench now returns along one planned path from where it turned back to the tracked hand as it moves. It starts at rest, accelerates, then eases into the catch in roughly 0.35-0.75 s depending on distance. The native spin continues, slows near the hand and is phased so that it ends on the held heading; only the last fifth of the path blends in the hand's tilt, so the model no longer flips while a fast spin is blended toward the hand. The headset draws the same path between the 60 Hz game ticks. Hits, bolt collection, the catch and the outbound flight are unchanged.

The Trespasser lock puzzle again draws the original translucent grey background and frame with its rings, nodes and beams; HUD prompts stay hidden. The panel is placed just above the plugged prong tip, facing the viewer, rather than 0.55 units above the hand.

The Hydrodisplacer's own update is the native water-transfer sequence. While plugged it has no controller aim, and the tracked-weapon suspension reset it to the reach pose every tick, so the transfer never started. A docked Hydrodisplacer now keeps its native update: reach, the fill/empty command, the cutaway and the withdraw. During the native water cutaway, the headset follows the authored camera glide, kept level, between its two black fades.

On bolt cranks, the open jaws of the original crank pose (sequence 0xb) now straddle the hex head, centred on the bolt axis at the head's mid-height. This is the placement in Ratchet's native crank poses on his 1.05 ring, measured from the original assets. Previously the hub of the head was planted at the top centre of the bolt. The shaft points toward the hand with at most a 20 degree tilt and lengthens or shortens about the jaws.

These changes require headset confirmation. No tests or gameplay scripts were run.


## Trespasser panel, Hydrodisplacer pads and menu buttons

The Trespasser panel had moved behind the lock's upright plate and disappeared. Measured on the class-615 lock, it now sits over the plugged tool, at 55 % of the native hand-to-prong line and 0.42 units above the tips, facing the eyes about a metre away, with nothing of the lock in between.

VR skips the Hydrodisplacer's native water cutaway: no fades and no camera change; the transfer runs as on a pad without cutaway cameras. When the pad's tap is retracted into the ground (suck), a downward push of the nozzle onto it now plugs the tool, and the tool stays in the hand during the suck instead of jumping to the native pose at the pad centre. Utility tools keep their aim while their nozzle touches a surface, so plugging no longer resets the gadget out of its ready state.

The French VR button bar (Retour, arrows, Valider) drawn at the bottom of the pause and front-end menus is removed with its hit areas. Native pointing and the controller buttons remain.

No second wrench model is drawn by the engine at the turnaround. Per-frame diagnostics (`VR_GAME wrench_frame`) now log what each rendered frame draws around it. These changes require headset confirmation; no tests or gameplay scripts were run.


## Ledges, loading, ship opening, caught wrench and the Clank bodies

From a hang, A and the two-arm shove now act exactly as native ✕. A VR-only rule refused the climb whenever geometry stood above the lip, which blocked every rung and every higher ledge; natively the climb jump then catches the higher ledge, lands on an open top or regrabs the same one (reproduced with the native hero code at the logged Aridia hangs). The shove is a buffered ✕, measured from each hand's recent top rather than its grab point, keeps its reference through brief tracking loss and needs 0.9 m/s.

The freeze during level changes came from per-level tables derived from the level overlay, built lazily on the main thread after the swap (about 7 s on a first visit, 1 s on a repeat). The loader now builds the destination's tables during the flight, and its spawn, emitter and GS-map reads use the destination level instead of the departing one.

During the canopy-opening landing shot the authored actor sinks Ratchet 2.5 units under the pad; the eye keeps the native cockpit seat until the cut that ends that shot. A caught wrench is held only while the grip is squeezed.

Clank and Giant Clank are first person: native eye heights 0.9 and 9.5, Giant Clank's tracking space scaled ×5.9, the body, rotor and antenna glow not drawn, their own hands on the controllers, physical punches pressing the native □ attack, the trigger holding Giant Clank's missiles, free facing and room-scale walking.

The reported wrench duplicate at maximum range is not explained: per-frame diagnostics show one smooth model without pose change or teleport. These changes require headset confirmation; no tests or gameplay scripts were run.


## Clank fixes and the wrench's spin centre

Clank's repeated launches came from room-scale walking: the capsule resolver ignored Ratchet's hidden moby instead of the body, so Clank collided with his own moby and was pushed up. It now ignores the body, as the native tick does. Clank's and Giant Clank's hands are placed after the item pass that hid them.

The thrown wrench now spins about its middle, like the original's centred spin pose, instead of the ready mesh's grip origin, whose head swept a metre-wide circle through the eyes when thrown upward. The flight carries the middle from its place in the hand, and the return ends at the held middle; the first tick back no longer doubles the spin step. These changes require headset confirmation; no tests or gameplay scripts were run.


## Clank's physical fists, his VR ledges and closing hands

Clank's and Giant Clank's punches are physical, like Ratchet's wrench. Each tracked hand is a fist: a deliberate stroke (2 m/s, 12 cm, extending) strikes where that hand is, for a few ticks, with the body's original sphere: Clank's (radius 0.25, damage 1) or Giant Clank's fist (radius 2.0, damage 4). The native punch animation no longer plays and B no longer punches. The striking hand gets a light pulse, and a strong one only when the native sphere reports a hit.

Clank's ledges use Ratchet's VR ledge system on his own states and dimensions: a gripping hand near the edge is needed to grab, the anchored hands move him along the edge through the native probes (the stick is ignored), A or the two-arm shove climbs as a buffered native ✕, releasing both grips drops. His hands are drawn hooked on the edge in his native hang pose, and the eye drops below the lip while hanging.

Ratchet's and Clank's free hands close into a fist with the analog grip: the finger joints curl about their measured local axes toward the palm and the thumb folds across. A hand holding a weapon or a ledge keeps its pose. These changes require headset confirmation; no tests or gameplay scripts were run.


## Cutscene eye, Clank's scenes and climb, the wrench's spin, Umbris mines and Rilgar

Cutscenes are seen at the gameplay eye height: the eye sits 0.68 above Ratchet's head attach, where it used to be about 0.53 m lower than in play. A scene without Ratchet is seen through Clank's eyes (his scene body and antenna glow hidden), instead of a viewpoint frozen where gameplay stopped. Clank's ledge climb keeps its native arc, but the eye rises at most 0.3 above the lip.

The thrown wrench's tracked spin is wrapped to ±π: on long throws it grew past the range of the game's VU0 sin/cos, which turned the wrench into a huge white horizontal plane and moved its hit sphere away. The Umbris floating and chain mines read the attacker's class through `attacker_class`, so wrench hits detonate them (an upstream port bug, also without VR).

On Rilgar, a lurker holding the hidden Ratchet 20 above itself parks the eye at the last living viewpoint. The hoverboard turns the current gaze to the board's heading at its start and at each respawn, so the race no longer starts backwards. Room-scale steps stop while a carrier (hover car, Kerwan transports, ferries, path platforms) holds Ratchet with its edge brake, so physical steps no longer push him off a taxi. These changes require headset confirmation; no tests or gameplay scripts were run.


## Ratchet's eye at 1.1 m

Ratchet's VR eye sits 1.1 above his feet, his own eye height, instead of the 1.6 of the native first-person camera, in gameplay and in cutscenes (0.18 above his head attach). The hanging and cable eyes are capped at it, the crouched eye is 0.6, and the line-of-sight checks from the eye to the hands use the same height. These changes require headset confirmation; no tests or gameplay scripts were run.


## Clank's eye height, the two-hand pull-up, a firmer wrench throw and the taxis

Clank's VR eye sits 0.63 above his feet, where his model's eyes are in the idle pose (measured the same way, Ratchet's are at 1.09-1.10), in gameplay and in his scenes. A ledge pull-up, for Ratchet and for Clank, needs both hands on the ledge and a settled hang (30 ticks, as the native grab animation must end before ✕ climbs); shoves made earlier are dropped. The wrench is thrown only by a release at 2.5 m/s or more.

Carriers no longer eject Ratchet: the VR thin-floor sweep is skipped on the ticks a carrier moves him. It swept from the carrier's old floor and left him a few centimetres above the carried one on descents, an airborne tick that detached the carry. These changes require headset confirmation; no tests or gameplay scripts were run.


## Review fixes and cleanup

A review of the VR layer fixed what was wrong without a headset session. The eye-height blend ran twice per frame and now uses the frame's dt; a sloped rail pitches about its own side axis instead of rolling the horizon for a player recentred facing elsewhere; leaving a rail turns the lean into a room-scale step instead of a jump of the view. One body-eye helper now serves the root, the death and cinematic fallbacks and every eye-to-hand line-of-sight test. A snap turn no longer fires when a stick still pushed leaves quick select, the shop or a menu.

Clank's grip no longer arms Ratchet's wrench, death releases the bolt crank, a thrown wrench keeps flying while the right controller is untracked, the Visibomb monitor hides when its pose is missing, and the gold Decoy Glove throws gold decoys. Native-path exactness was restored for the Walloper's glow point and the flight sky. 137 comment lines corrupted by a cp1252 re-save were restored, and duplicated constants and dead code were removed. These changes require headset confirmation; no tests or gameplay scripts were run.


## The cutscene anchor and a first-person Hologuise

Cutscene and pilot views map the head's room position at the start of the view to the actor's eye, instead of the recentre point: walking around the room since the recentre no longer shifts the cutscene view.

The Hologuise disguise is first person: the rig no longer treats it as Giant Clank, it sees from Ratchet's eye height, turns with the head and walks with the room. Its body and head glow are hidden and its own forearms and hands are drawn on the controllers, closing with the grip. A fresh trigger press or a right-hand salute at the brow makes it salute; B or A returns to Ratchet. These changes require headset confirmation; no tests or gameplay scripts were run.


## Turrets and flown ships fire on the trigger

Mounted in a turret (Gaspar's cannons, Hoven's and Batalia's turrets), the trigger holds ○, their fire key; flying a ship (Pokitaru's jet, Gemlik's ship, the fleet's ship), it holds □, the lasers, while X and the left trigger keep ○ / R1 for the missiles. Mounted, Y leaves the turret or the ship natively instead of opening the hand's quick select. These changes require headset confirmation; no tests or gameplay scripts were run.


## A cockpit view for the flown ships

Flying Pokitaru's jet, Gemlik's ship or the fleet's ship, the VR view rides the ship: the eye sits in the fighter model's own canopy (measured on its glass texels), the view follows the ship's heading and pitch with smoothing while its roll is left to the head, and the tracking space takes the ship's quarter flight scale so the head moves inside the cockpit at Ratchet's size. Ratchet's hands and wrench, hidden natively in the ship, are not drawn. These changes require headset confirmation; no tests or gameplay scripts were run.


## The HUD along the aim, the ship's eye above its canopy

Mounted in a turret or flying a ship, the HUD panel hangs along the aim (the turret's script camera behind its barrels, the ship's nose) instead of following the gaze, so the reticles the game draws at its screen centre mark where the guns point. The flown fighter's canopy is opaque and its hull two-sided, so the eye rides just above the canopy rather than inside it. These changes require headset confirmation; no tests or gameplay scripts were run.


## Culling for the headset, VR turrets and spectator scenes

Level content is culled for the headset's eye in every mode: in-level movies, ship sequences and the Visibomb view used the native camera (an authored shot elsewhere), which left holes in view; the native image shown on the Visibomb's monitor still gets what it sees, and gameplay's "drawn last frame" keeps the native culls. Cutscenes start facing where Ratchet's head looks, and scenes with no one to see through (the infobot's messages) are watched from their camera as a spectator, instead of reusing the previous scene's eye, which showed Hoven's arrival from far away.

The manned turrets put the eye at their seat with a fixed view and turn toward where the right controller points, at their own speeds, the trigger firing. The ship no longer vanishes after the inter-planet loading: the new level's setup hid it along with its own instance 0. These changes require headset confirmation; no tests or gameplay scripts were run.


## VR Options, the helmet on the player's head, boots and Oltanis' hand items

A VR Options page sits right below Port Options (only with a VR headset), with Helmet and Boots, both On by default and saved with the port settings. With Helmet on, the worn O2 Mask or Pilot's Helmet is drawn on the player's head, in play, in Ratchet's scenes and in the ship sequences, the eye behind its visor; the scene actor's own helmet, which followed Ratchet's animated head across the view, is hidden. With Boots on, the worn boots show at the feet as the game poses them.

On Oltanis, and on any arrival without a landing after a flight streamed in VR, the hand items were hidden by the travel's hide flag that the preloaded switch never cleared; it is cleared now. These changes require headset confirmation; no tests or gameplay scripts were run.


## Rails, reach, Hyper Strike aim, a wider visor and the camera flip

On rails the eye now predicts the body instead of trailing it, which had left the eye, the hands and the wrench's hits a metre behind at grind speed; the take-off keeps the eye in the ship's seat once Ratchet sits down; Clank's antenna dot is no longer drawn. The worn O2 Mask sits with the eye closer behind a taller visor whose glass is lighter.

The wrench stretches up to 35 % on fast swings and its hits reach with it; the Hyper Strike carries the native aim assist (a drift toward a near target, a near miss pulled onto it), needs a real downward stroke, and the wrench hand no longer grabs crate tops. A Camera flip option (off by default) turns the view with Ratchet's flips, and two-handed weapons held in one hand are heavier. The loading hitch lost its main-thread table re-read and its departure free; the swap now logs its phases. These changes require headset confirmation; no tests or gameplay scripts were run.


## Body direction, the wrench's whoosh, Giant Clank, the Walloper and walking in the quick select

A Direction option (Body by default, or Gaze) makes the heading and the stick follow an estimate of the body from the head and both controllers, so the player can walk one way while looking another. The wrench whooshes on each swing with the native combo sounds, glow billboards face the headset (Clank's antenna light in scenes), Giant Clank's heading follows the player in every state with his missiles leaving from his drawn hands, the Walloper gives the wrench back on a still grip and chains consecutive punches, and the quick select no longer stops Ratchet's movement. These changes require headset confirmation; no tests or gameplay scripts were run.


## Walking backward and sideways, Walloper punches and the wrench's whoosh

The original game always moves Ratchet the way he faces. In VR he faces the player's heading, so the native movement physics now run facing the way he actually moves (the stick, or the carried motion once it is released), and the heading comes back right after the move for the transitions, flips, weapons, aim and ledges. Walking backward or sideways no longer lurches forward on release, jumps and steps follow the motion, and the stick no longer aims melee attacks.

A Walloper punch is the right hand thrown forward relative to the head, within about 55° of the facing and away from the body: folding the arm back, swinging sideways or stepping back never punches, and the lunge goes where the player faces. The wrench's whoosh plays Ratchet's own swing sound, since the wrench's animations carry none. These changes require headset confirmation; no tests or gameplay scripts were run.


## Quick-select pages on the trigger

While the right-stick click holds the quick select open, the right trigger turns to the next page instead of the right stick; the press fires nothing until it is released, and a trigger already held when the ring opens does not turn the page. This change requires headset confirmation; no tests or gameplay scripts were run.


## Headset buttons in menus and text

Menus, help boxes, banners, subtitles and the spatial panels now show the headset's buttons instead of the PS2 icons: the fonts' pad icons are repainted when the level HUD loads in VR, ✕ as A, ○ as the right trigger (RT), △ as a right-stick click, □ as the right grip (RG), L1 as the left grip (LG), R1 as the left trigger (LT), L2 as X, R2 as B and the analog sticks as L / R thumbsticks; their advances follow the new glyphs, so layout and pointer targets stay aligned.

Every icon names a control that works: in the pause and freeze menus the left grip sends L1 and the left trigger R1 (previous / next map, the planet lists, the Weapons page's quick-select slot, now also while pointing at a weapon), and in play X and B, unused until now, hold L2 and R2 (hover and Clank strafes, hoverboard tricks, the Hydro-Pack boost, a ledge drop); on turrets, ships and in the Hologuise they keep ○ and □. The first glyph set was confirmed in the headset; the new shoulder bindings require headset confirmation; no tests or gameplay scripts were run.


## Streamed inter-planet loading (branch `vr-streamed-loading`)

The destination planet is now built and handed to the GPU behind the flight instead of in one frame. The loader thread, which already read and lit the level, also builds the tfrag, tie, shrub and moby meshes and images and the tie and shrub entity lists. At the swap the old level's resources are freed on a cleanup thread and the old ties are hidden, then despawned a few hundred per frame; the new meshes and images are added at most 4 MiB, 12 images and 48 meshes per frame, and the tie and shrub entities are spawned within 2.5 ms per frame. The voyage ends only once all of it is in and uploaded. Moby entities still spawn at the swap (the level's spawn pass and the gameplay set-up hide them by tag); their meshes and images are streamed too. Non-flight level changes and the desktop build add everything at once, as before, from the prepared meshes.

The set-ups a level runs on its first frame (the pause menus' 3D models 62 ms, the gameplay set-up 31-37 ms, Ratchet's attachments 22-26 ms, the vendor 5 ms) wait for the cut to the landing, under black. Until then the voyage, the VR camera and the flight's audio run on the departure's play state, as before the change, and the pipeline warm-up view is placed at the level's Ratchet instance.

Measured in the headset (Kerwan to level 10): the change's frame takes 34-41 ms (was 170-225 ms), the frames after it at most 27 ms (one of 140-160 ms with the set-ups still in the flight), and the upload 35 MiB over 52 frames; the worst flight frame went from about 245 ms to 40 ms. The set-ups and the first tick of the destination (160-200 ms) run under black.

New log lines: `level switch: level NN prepared beside the load in ...`, `VR_HITCH swap ... streaming=true old_ties_deferred=N`, `VR_HITCH stream ...` (what was streamed and over how many frames), `VR_HITCH frame phase=... main=... split first=... pre_update=... fixed=... update=... post_update=... last=...` / `render=... prep=...` for every frame of 20 ms or more during a transition, and `level setup: <name> <ms> ms` for those set-ups (the gameplay set-up step by step). No tests or gameplay scripts were run.


## Weapon recall after a wrench throw, a rarer whoosh

A weapon swap refused while the swap is locked (the wrench in flight) no longer leaves a stale target that made the hand ignore that weapon afterward; while the wrench flies the trigger does nothing, and a fresh press once it is back brings the last weapon. The wrench's whoosh now needs a big fast swing. These changes require headset confirmation; no tests or gameplay scripts were run.


## The hoverboard in VR

The board steered toward the stick's direction about the player's heading, and the view did not turn with it: pushing forward meant "go where I face", turning the torso or the hands turned the board, a lap meant turning around in the room, and a snap turn swerved the board by 30°. In play the input mask also dropped □ and L1, so the manual boost and the L1 trick (and with it the four-trick skill point) were unreachable.

The board now steers about its own heading: the left stick's x, plus the player's sideways lean (the neck's offset from where it was when the board was placed: straight within 4 cm, full turn at 20 cm; VR Options → Hoverboard lean, `vr_board_lean`, on by default), turns it at the native rate times their amount. The view follows the board's heading (time constant 0.12 s, at most 4 rad/s), stays anchored where the head was at the start or respawn so a lean moves the eye, not the rider, and takes no room-scale step; there is no snap turn on the board. The right trigger boosts (□), the left grip is L1, and in a jump the right stick up / right / down / left holds the L1 / R1 / L2 / R2 tricks (the left trigger, X and B still do). The body's spin cannot be seen from its eyes, so a landing tilted beyond 60° or turned more than 80° from the board straightens the body along the board instead of throwing Ratchet off; an unfinished trick still does. These changes require headset confirmation; no tests or gameplay scripts were run.
