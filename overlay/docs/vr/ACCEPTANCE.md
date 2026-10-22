# PoC acceptance gates

**Initial status: all Rust build and hardware gates below are NOT RUN.** Authored source is not a passing acceptance test. The package's Python static checks are recorded separately in its root `validation.json`.

## Milestone A: VR integration / exploration PoC

The target is a repeatable first-person Novalis terrain inspection session with independent controller aiming. It is deliberately smaller than playable Ratchet & Clank VR.

| Gate | Pass condition | Initial state |
|---|---|---|
| A1 — native build | VR and non-VR compile; renderer-independent tests and upstream quick suite pass, with skips explicitly listed | NOT RUN |
| A2 — calibration | Generated scene launches without game data; correct stereo, natural scale, one-metre cube, valid head/hand tracking | NOT RUN |
| A3 — independent aim | Player can look away from the target while pointing/firing; hit ray matches controller aim, not gaze | NOT RUN |
| A4 — terrain | Novalis terrain appears in both eyes without one-eye holes or head-motion-dependent disappearance on the checked route | NOT RUN |
| A5 — locomotion | Teleport only to valid clear floor; one snap per neutral-to-stick cycle; no head displacement caused by snap turn | NOT RUN |
| A6 — obstruction | A target behind terrain cannot be hit; placing a controller through a wall cannot fire through it; head intersection cover behaves predictably | NOT RUN |
| A7 — lifecycle | Focus loss, controller loss, recenter, headset removal/resume and a repeated launch do not fire, teleport or crash unexpectedly | NOT RUN |
| A8 — performance | Release build; actual runtime set to 90 Hz; recorded PC specs and render resolution; application CPU/GPU timings fit the 11.11 ms display period on the test route, with no sustained reprojection | NOT RUN |
| A9 — repeatability | A 10-minute inspection/test session completes without a crash, accumulated tracking offset, stuck inputs or a reproducible growing resource leak | NOT RUN |
| A10 — publication | Independent private repository verified before push and remote commit verified afterward; no assets/secrets committed | NOT RUN |

The 10-minute item is a test procedure duration, not a development estimate. A8 is a target, not a performance measurement. Runtime-period logs and application-loop FPS alone are insufficient evidence. Missing original scenery is documented scope, but missing terrain in one eye is not.

A successful A is a technical VR PoC. Do not call it a working first-person game conversion.

## Milestone B: first playable combat slice — NOT IMPLEMENTED

A subsequent gameplay PoC should complete one short original-game route with Ratchet's movement/collision, jumping, one actual weapon driven from the controller, one enemy interaction, ammo/damage feedback and restart/respawn. The original 60 Hz simulation must then be decoupled from headset rendering and tested with identical input traces across render cadences. It must keep scripted camera behavior out of the headset transform.

That milestone proves the gameplay conversion, not just stereo exploration. It is not part of the current terrain/debug-target candidate.

## Hardware report template

Record: tested commit; successful build record/executable hash; Windows version; GPU/driver/CPU/RAM; headset/runtime versions; PICO connection method; active OpenXR runtime; runtime-selected interaction profile; refresh rate and render resolution; each gate's PASS/FAIL/NOT RUN result; precise reproduction steps; log filenames and a short recording when useful.

Never mark a gate PASS because code exists or the process starts. Preserve failures; do not relax criteria merely to get a green report.
