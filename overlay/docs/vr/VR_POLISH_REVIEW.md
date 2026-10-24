# VR interaction and presentation review â€” 2026-10-24

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
