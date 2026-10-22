# ReRAC VR PoC — implementation candidate

**Status: source authored; Rust compilation, Windows execution and headset validation have NOT been performed. No GitHub repository was created or pushed by this delivery.**

This package is a source overlay plus bootstrap/publishing automation for **npiriou**. It is not a prebuilt game, a complete upstream checkout, or a proven playable mod. The bootstrap obtains the public upstream checkout locally and preserves its history and license.

## What this first PoC should demonstrate

Stand inside a terrain snapshot of Novalis in first-person stereo; look around and lean naturally; see tracked controllers; teleport and snap turn; point independently of your head and hit diagnostic targets without shooting through terrain. A generated calibration room tests the VR stack before any game assets are needed.

This is a **VR integration / exploration PoC**, not a campaign or original-combat PoC. The implementation deliberately does not drive Ratchet's simulation, enemies, weapons, jumping, progression or saves. See `overlay/docs/vr/ACCEPTANCE.md` for concrete completion gates.

## What is in the package

Six Rust modules under `overlay/crates/rc-engine/src/vr/` implement the candidate scene, OpenXR actions, tracking rig, teleport/snap controls, geometric queries, diagnostic shooting and logs. There are 15 authored, unexecuted Rust unit tests. PowerShell scripts perform environment checks, guarded patch application, building, launching and private publication. A manual Windows CI workflow is provided.

Only two existing Rust-project files are patched: `rc-engine/Cargo.toml` and `rc-engine/src/main.rs`. The VR branch is off by default. The overlay also adds ignore rules and archives inherited workflows so an experiment cannot accidentally run upstream release automation.

## Exact starting points

- ReRAC: `89c87e98f9d8296187df2a6310c67b32144090d3`.
- bevy_oxr: `4e2998531bd4f7d9cdce156148f9a5b7dd5cd260`.
- Expected engine: Bevy 0.19; explicit OpenXR crate: 0.21.1.

The XR revision is pinned to the internally consistent 0.5.0 workspace immediately before a version-only bump. It targets Bevy 0.19 in the inspected manifest. This is source-level compatibility evidence, not a successful dependency-resolution or compilation result. Do not silently upgrade either project to repair a build.

## Local prerequisites

Use native Windows with Git, Rust's MSVC toolchain, Microsoft C++ build tools plus Windows SDK, CMake and PowerShell. Publishing also needs GitHub CLI authenticated locally as `npiriou`. Headset tests need a working PICO Connect / SteamVR connection and an active OpenXR runtime. The local agent must audit these first; these scripts do not install software or request credentials.

Do not give an AI/chat a GitHub token. Use `gh auth login` locally when authentication is required. Review third-party source/build scripts before running them with development-machine privileges.

## Preferred use: hand this folder to your local coding agent

Open `overlay/docs/vr/AGENT_HANDOFF.md` in your local agent and instruct it to follow the file. The agent should resolve build errors and collect real evidence before calling the PoC complete. You should not need to edit source.

### Default bootstrap: build before publication

From the extracted package folder, after local GitHub authentication:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\Bootstrap.ps1
```

The command clones upstream into a **new** `$HOME\source\rerac-vr-poc` folder, checks out the exact revision, verifies original Git blob hashes, applies the overlay, runs the build/test script, commits the intended files, creates **npiriou/rerac-vr-poc as PRIVATE**, verifies visibility, then pushes only `main` and verifies the remote commit.

It refuses an existing destination folder, existing repository name, mismatched GitHub account, unexpected upstream or an existing `origin` remote. It does not force-push, mirror every branch, or upload game data. `ExecutionPolicy Bypass` affects only this PowerShell process, not the machine's policy.

A build failure stops before repository creation and leaves the prepared checkout for your agent to fix. **Do not rerun bootstrap into that existing folder.** Work there, run `scripts/vr/Build.ps1`, commit narrowly scoped fixes, then run `scripts/vr/Publish-Private.ps1` from that checkout.

### Prepare locally without creating a repository

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\Bootstrap.ps1 -PrepareOnly
```

This still runs builds. For a source-only preparation with no compilation, add `-PublishSourceOnly`; combined with `-PrepareOnly`, it neither builds nor publishes.

### Explicit source-only publication

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\Bootstrap.ps1 -PublishSourceOnly
```

This deliberately skips build validation and publishes an **unverified source candidate**, not a working build. It exists to let a local agent continue from a private repository; it does not establish that the code compiles.

If creation succeeds but the subsequent push fails, the new private repository may exist without code. The publishing script refuses to overwrite or reuse destinations automatically. Your agent must inspect the exact repository and its privacy, then resume the normal non-force push safely. Do not create a public fork as a workaround.

## Why an independent private repository, not GitHub's Fork button?

GitHub requires a fork of a public repository to remain public. This bootstrap makes an independent private repository while retaining the upstream commit history, `upstream` remote and ISC license. Source changes can be kept private; do not make a public fork first.

Reference: https://docs.github.com/en/pull-requests/reference/forks

## Validation honesty

`validation.json` distinguishes package checks actually executed here from tests not executed. Python package checks validate packaging, selected patch guards, manifests and source invariants. They do **not** execute Rust, PowerShell, OpenXR or gameplay. No headset, GPU-timing or PICO controller mapping result is implied.

The package contains no ISO, extracted game assets, compiled binary, credentials or full Git history. Upstream source is fetched by the bootstrap on your machine. Maintain the license notices in `LICENSE` and the cloned upstream repository.
