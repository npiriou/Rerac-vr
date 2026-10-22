# Experimental first-person VR terrain PoC

**Initial delivery: uncompiled source candidate, not headset-tested.** The target is Windows PC VR on a PICO 4 Ultra through PICO Connect and SteamVR/OpenXR. Exact compatibility must be established on the connected headset; no native standalone PICO/Android target is included.

## Scope and boundaries

The optional `vr` feature adds an alternative application entry point inside `rc-engine`. With `RC_VR=1`, this runs a generated calibration scene or a simplified static Novalis terrain snapshot. Without that environment flag, the normal desktop entry point remains selected. Requesting VR from a build without the feature fails explicitly rather than silently launching the flat game.

The existing ReRAC loader decodes the user's extracted Novalis data. The new app renders only high-detail terrain (or a fragment's designated base-only strip) with unlit standard materials. This is not the original renderer. It omits mobys, enemies, ties, shrubs, water, sky, effects, original audio, Ratchet's body, campaign scripts and original weapons. It does not use or alter the game's 60 Hz simulation. Tests of that simulation's independence from rendering belong to the later gameplay milestone.

Terrain textures, alpha tests and vertex colours are approximate. Geometry is double-sided; occlusion/LOD optimizations are deliberately not shared with the flat renderer. Missing non-terrain objects are expected, not proof of a stereo-culling bug. The physical-scale setting is provisional.

## Build, then start with calibration

Run from the cloned, patched repository root:

```powershell
.\scripts\vr\Doctor.ps1
.\scripts\vr\Build.ps1
.\scripts\vr\Run.ps1 -Scene calibration
```

The build script compiles and runs the renderer-independent Rust tests, checks the non-VR build, runs the upstream quick suite, checks the VR integration and builds a VR executable. The upstream quick suite uses `cargo xtask test-quick`, following the project's workflow; it can run without the user's ISO, and some asset-dependent tests may skip. Record skips explicitly. `-SkipUpstreamQuickTests` is only for a focused diagnostic run, not final acceptance.

A profile-specific build record contains an executable SHA-256. The launch script refuses a missing or mismatched record. A failed rebuild removes the previous success record for that profile. This prevents mistaking an unrelated executable for the new VR build.

For performance testing, build and run with `-Release` on both scripts. A development build is not the final performance criterion.

## Novalis terrain

Provide the supported RAC1 game data locally, using the project's extractor and your own disc image. Do not upload or commit it.

```powershell
.\scripts\vr\Run.ps1 -Scene novalis -DataDir 'D:\Games\ReRAC\extracted'
```

No assets are included here. The automatic inspection spawn is selected geometrically from a clear, nearly horizontal terrain triangle; it is **not** Ratchet's canonical spawn. Validate it in the headset before treating it as a playable starting point. A local agent can supply `-SpawnGame 'x,y,z'` to select a checked location. This coordinate is in decoded game units, not raw integers multiplied by 1,024.

`-UnitsPerMetre 1.0` controls how large the game world appears. It divides decoded world positions by the chosen number of game units per metre. It does not scale head/hand tracking or alter the runtime's eye separation. First verify the calibration cube is one metre on each edge, then choose a consistent world scale; no canonical Ratchet-to-human size is claimed.

## Controls

| Control | Intended action |
|---|---|
| Headset | Six-degree-of-freedom viewing; no animated head bone or camera recoil |
| Left trigger, hold then release | Aim at a clear floor surface, then teleport on valid release |
| Right thumbstick left/right | One 30-degree snap turn; return to neutral before another |
| Right trigger | One diagnostic ray shot on press; terrain occludes targets |
| Left X (Touch mapping), A (Index mapping), or menu (motion-controller mapping) | Return to spawn and align facing |
| Keyboard R / Escape | Recenter / exit |

Controller boxes represent grip poses. Aim rays use separate aim poses. Diagnostic targets turn green and log hits. These are not original weapon projectiles and do not damage original enemies. No haptics are implemented.

The code suggests Touch, Index, Microsoft motion-controller and simple-controller profiles. The simple profile has select-button shooting/teleport but no thumbstick. **The profile exposed by PICO Connect/SteamVR is not verified.** Logs print the actual profile and whether actions are active. If the controllers appear but inputs do not work, inspect the runtime bindings before changing gameplay code. Do not label compatibility complete until every intended action works on the PICO controllers.

## Comfort and collision limitations

Teleportation and snap turning are chosen for the first technical milestone. Smooth walking, jumping, Ratchet's collision and room-scale body reconciliation are not present. Teleport destinations use a rendered-terrain BVH, slope checks and a sampled column clearance check, not a complete capsule or navigation solver. A drawn alpha-cutout can still block a geometric ray.

When the tracked head intersects a terrain surface, an inward-facing black shell covers the scene; the code does not push or rotate the tracked camera. This is an experimental obstruction treatment, not a safety boundary. It cannot guarantee detection of being entirely inside a large solid object. Keep the headset's physical boundary enabled, use a clear space and stop a test on tracking or visual discomfort.

Tracking loss cancels pending fire/teleport actions rather than converting loss into a release. Controls must return to neutral before rearming. This behavior has authored unit tests but still needs real runtime/focus tests. OpenXR session restart and STAGE reference-space behavior also require physical testing.

## Diagnostics

Launch logs are stored under ignored `work/vr/`. They include the scene, upstream revision, runtime display period, eye resolution, eye-view count, tracking/action states, shot counts and target hits. Every five seconds the code reports CPU-side application-loop interval percentiles.

**Loop intervals are not GPU duration, compositor deadline measurements, end-to-end latency, or proof of 90 Hz.** Use SteamVR/runtime frame timing for the performance acceptance test. The desktop window may be blank: no spectator mirror is implemented. Use SteamVR's VR View when observing the headset output.

A manual `.github/workflows/vr-poc.yml` builds/tests on Windows without any headset or assets. It only uploads build logs; it cannot certify PICO compatibility. Upstream release workflows were retained as disabled files under `.github/upstream-workflows/`.

## Architecture map

`mod.rs` selects/starts the isolated VR app. `scene.rs` decodes/render-batches terrain and creates the calibration room and targets. `actions.rs` binds OpenXR actions and tracking spaces. `interaction.rs` applies the tracking root and independent aiming. `core.rs` contains renderer-independent math, input gates, BVH queries and 15 unit tests. `diagnostics.rs` reports bounded diagnostic samples.

Pinned provenance and sources are in `PROVENANCE.json`. Follow `ACCEPTANCE.md`; use `AGENT_HANDOFF.md` for agent-led completion.
