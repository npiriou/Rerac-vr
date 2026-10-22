# Local AI agent handoff

## User contract

The user will not edit or inspect source. You own source changes, environment diagnosis, native Windows build/test execution, logs and safe Git operation. The user performs headset tests and reports observations. Do not claim tests were performed when you cannot access the required runtime or hardware.

This delivery is an uncompiled implementation candidate. Expect integration issues until compilation and hardware testing establish otherwise. The first task is not additional gameplay features: it is making this candidate build and verifying the calibration room.

## Work sequence

1. Read `README.md`, `docs/vr/README.md`, `docs/vr/ACCEPTANCE.md` and `PROVENANCE.json`. In the overlay package, the last three files live under `overlay/` until application.
2. Audit the native Windows MSVC Rust toolchain, C++ linker/Windows SDK, CMake, Git, PowerShell and local GitHub CLI authentication. Ask for approval for missing software installations. Never request tokens in chat or access unrelated personal files.
3. Use the root package bootstrap with `-PrepareOnly -PublishSourceOnly` to prepare a clean local checkout without building/publishing, or use the default build-gated workflow. Keep the exact upstream/XR revisions. Preserve upstream license/history. Keep all game data outside Git.
4. Before patching the clone manually, record the unmodified baseline build result on the chosen machine. The package's own build script checks the non-VR path *after* patching; it is not proof that the pristine upstream build was tested.
5. Run `scripts/vr/Build.ps1`. Fix actual compiler errors in small commits. Do not replace this project with another engine, upgrade the entire dependency graph or delete tests. Record each executed command and exit status. The standalone Rust test module can be compiled without Bevy; the integration cannot.
6. Run the calibration scene with PICO Connect/SteamVR already working. Verify STAGE space, two eye views and real controller mappings. Ask the user for the hardware checklist results. Rendering on a flat desktop window is not a VR test; this candidate has no spectator mirror.
7. Only after calibration passes, run Novalis against the user's local extracted data. Choose a validated spawn and consistent scale. Document omitted non-terrain scenery. Test teleport, snap, occlusion and diagnostic targets.
8. Use a Release build and the runtime's frame-timing tools for performance. This code's CPU loop intervals are not GPU/compositor timing. Collect actual dimensions and driver versions instead of assuming a hardware budget.
9. Commit only reviewed code/docs and the resolved lockfile. Audit staged paths. Run `scripts/vr/Publish-Private.ps1` for the new independent private repository. It checks the authenticated login, creates with `private=true`, verifies privacy before push and verifies the remote SHA. No public fork, force push or mirror push. If creation partially succeeded, inspect and resume the exact destination rather than replacing it.
10. Report BUILD / AUTOMATED TESTS / HEADSET TESTS / PRIVATE PUSH separately, with evidence. Continue toward gameplay milestone B only after milestone A's scope and failures are understood.

## Likely integration checkpoints

Check the pinned bevy_oxr startup scheduling: action creation must precede suggestions/attachment; action spaces must be valid and attached to the tracking root. Check Bevy query disjointness and deferred command ordering. Check PICO-over-SteamVR's actual interaction profile instead of assuming it is native PICO or Touch. Invalid/inactive inputs must cancel gates, not trigger a release.

Check rendered and submitted eye transforms both include the same tracking root. The backend owns eye projections and eye separation; never impose the flat-game projection or scale the tracking root. Do not parent the headset to an animated bone.

The geometric spawn finder is a diagnostic convenience, not a gameplay spawn selector. Terrain-only collision is neither full level collision nor a capsule controller. A sampled landing column cannot establish that the entire room-scale play area is clear. The obstruction shell is experimental; test it rather than claiming safety.

Bevy custom material fidelity, the original GameFrame path, scripted camera behavior, 60 Hz simulation interpolation, original weapons/enemies and save progression have NOT been integrated. There is no original-combat proof in this package. Preserve the distinction in documentation and commit messages.

## Execution constraints of the originating session

The originating session could read the public source and authenticated GitHub login, but its GitHub tools could not create repositories or push code. Its local environment lacked Rust, PowerShell, outbound downloads, game data and a headset. Thus the source and scripts were authored, not compiled/executed against Windows. `validation.json` in the package records only package/static checks actually run there.

Do not reinterpret those checks as build validation or a headset certification. Never report the requested repository as created until GitHub confirms it exists and is private.
