"""Package/static validation ONLY. Does not compile Rust or execute PowerShell.
Run: python -m unittest discover -s tests -v
"""
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = json.loads((ROOT / 'patch-manifest.json').read_text())

def text(path):
    return (ROOT / path).read_text(encoding='utf-8')

def apply_reference(original, replacements):
    """Python reference for the manifest's exact-once replacement rule."""
    result = original.replace('\r\n', '\n')
    for change in replacements:
        if not change['old'] or result.count(change['old']) != 1:
            raise ValueError('expected one patch anchor')
        result = result.replace(change['old'], change['new'])
    return result

class PackageTests(unittest.TestCase):
    def test_upstream_engine_fixture_matches_verified_git_blob(self):
        data = (ROOT / 'tests/engine-Cargo.toml.fixture').read_bytes()
        digest = hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()
        self.assertEqual(digest, MANIFEST['patches'][0]['expectedBlobSha'])

    def test_manifest_patches_engine_and_windows_trace_transport(self):
        self.assertEqual([p['path'] for p in MANIFEST['patches'][:3]],
                         ['crates/rc-engine/Cargo.toml', 'crates/rc-engine/src/main.rs', 'tools/trace/src/pine.rs'])
        self.assertIn('crates/rc-engine/src/input_map.rs', [p['path'] for p in MANIFEST['patches']])
        for patch in MANIFEST['patches']:
            p = PurePosixPath(patch['path'])
            self.assertFalse(p.is_absolute())
            self.assertNotIn('..', p.parts)
            self.assertRegex(patch['expectedBlobSha'], r'^[0-9a-f]{40}$')

    def test_patched_cargo_is_valid_toml_and_vr_is_optional(self):
        original = text('tests/engine-Cargo.toml.fixture')
        patched = apply_reference(original, MANIFEST['patches'][0]['replacements'])
        doc = tomllib.loads(patched)
        self.assertEqual(doc['dependencies']['bevy'], '0.19')
        self.assertEqual(doc['features']['dev'], ['bevy/dynamic_linking'])
        self.assertNotIn('default', doc['features'])
        for key in ['bevy_mod_openxr', 'bevy_mod_xr', 'openxr']:
            self.assertTrue(doc['dependencies'][key]['optional'])
            self.assertIn('dep:' + key, doc['features']['vr'])
        for key in ['bevy_mod_openxr', 'bevy_mod_xr']:
            self.assertEqual(doc['dependencies'][key]['rev'], MANIFEST['xrCommit'])

    def test_missing_patch_anchor_rejected_by_reference(self):
        with self.assertRaises(ValueError):
            apply_reference('unrelated', MANIFEST['patches'][0]['replacements'])

    def test_duplicate_patch_anchor_rejected_by_reference(self):
        original = text('tests/engine-Cargo.toml.fixture')
        with self.assertRaises(ValueError):
            apply_reference(original + original, MANIFEST['patches'][0]['replacements'])

    def test_reapplying_patch_rejected_by_reference(self):
        changes = MANIFEST['patches'][0]['replacements']
        once = apply_reference(text('tests/engine-Cargo.toml.fixture'), changes)
        # Some original anchors remain within their replacements: the production
        # applier ALSO checks the original Git blob hash, preventing double apply.
        digest = hashlib.sha1(b'blob ' + str(len(once.encode())).encode() + b'\0' + once.encode()).hexdigest()
        self.assertNotEqual(digest, MANIFEST['patches'][0]['expectedBlobSha'])
        applier = text('Apply-Overlay.ps1')
        self.assertIn('$Blob -ne $Patch.expectedBlobSha', applier)

    def test_game_entrypoint_preserves_original_application(self):
        out = apply_reference(text('tests/main.rs.fixture'), MANIFEST['patches'][1]['replacements'])
        self.assertLess(out.index('vr::prepare_game()'), out.index('disc_source::startup()'))
        self.assertIn('if vr::diagnostic_requested() { return vr::run(); }', out)
        self.assertNotIn('if vr::requested() { return vr::run(); }', out)
        for plugin in ['GameplayPlugin', 'MobyRenderPlugin', 'TieRenderPlugin', 'ShrubRenderPlugin', 'SkyRenderPlugin', 'AudioOutPlugin', 'LevelSwitchPlugin']:
            self.assertIn(plugin, out)
        self.assertIn('vr::install_game(&mut app)', out)
        self.assertIn('Original-game VR exited with an error', out)
        self.assertIn('refusing a silent desktop fallback', out)

    def test_game_input_uses_original_pad_and_default_launch_is_game(self):
        source = text('overlay/crates/rc-engine/src/vr/game_input.rs')
        self.assertIn('rc_game::pad::{button, PadInput}', source)
        self.assertIn('before(crate::input_map::sample_system)', source)
        launch = text('overlay/scripts/vr/Run.ps1')
        self.assertIn("$Scene='novalis'", launch)
        self.assertNotIn("$env:RC_AUDIO='0'", launch)
        self.assertNotIn("$SpawnGame='162.", launch)

    def test_bootstrap_does_not_stage_entire_worktree(self):
        body = '\n'.join(line for line in text('Bootstrap.ps1').splitlines() if not line.lstrip().startswith('#'))
        self.assertNotRegex(body, r"@\('add','\.'\)")
        self.assertIn('$Danger', body)
        self.assertIn('PublishSourceOnly', body)
        self.assertLess(body.index("& '.\\scripts\\vr\\Build.ps1'"), body.index("& '.\\scripts\\vr\\Publish-Private.ps1'"))

    def test_private_creation_and_verification_precede_push(self):
        body = text('overlay/scripts/vr/Publish-Private.ps1')
        self.assertIn("'private=true'", body)
        self.assertIn("$Info.visibility -ne 'PRIVATE'", body)
        self.assertIn('$Info.isFork', body)
        self.assertLess(body.index("$Info.visibility -ne 'PRIVATE'"), body.index("'push','-u','origin','main'"))
        self.assertIn('$Local -ne $Remote', body)
        self.assertNotIn("'--force'", body)
        self.assertNotIn("'--mirror'", body)

    def test_private_script_authenticates_exact_owner(self):
        body = text('overlay/scripts/vr/Publish-Private.ps1')
        self.assertIn("$Owner='npiriou'", body)
        self.assertIn('$Login -ine $Owner', body)
        self.assertIn("$Remotes -contains 'origin'", body)

    def test_local_launch_requires_successful_build_and_binary_hash(self):
        build = text('overlay/scripts/vr/Build.ps1')
        launch = text('overlay/scripts/vr/Run.ps1')
        self.assertIn('Remove-Item -LiteralPath $StatePath', build)
        self.assertIn('executable_sha256', build)
        self.assertIn('$BuildState.executable_sha256 -ne', launch)
        self.assertIn('$Code -ne 0', build)
        self.assertIn('$Code -ne 0', launch)

    def test_no_device_test_success_in_build_report(self):
        build = text('overlay/scripts/vr/Build.ps1')
        self.assertIn('headset_tested=$false', build)
        self.assertIn('gpu_budget_measured=$false', build)
        self.assertNotIn('headset_tested=$true', build)

    def test_manual_ci_has_no_asset_upload(self):
        ci = text('overlay/.github/workflows/vr-poc.yml')
        self.assertIn('workflow_dispatch:', ci)
        self.assertIn('contents: read', ci)
        self.assertIn('work/vr/*.log', ci)
        self.assertNotIn('extracted/', ci)
        self.assertNotIn('pull_request_target', ci)

    def test_rust_tests_are_present_and_validation_scope_is_explicit(self):
        source = text('overlay/crates/rc-engine/src/vr/core.rs')
        self.assertEqual(len(re.findall(r'#\[test\]', source)), 15)
        self.assertIn('15 standalone Rust unit tests', text('README.md'))

    def test_all_declared_vr_modules_have_payload_files(self):
        mod = text('overlay/crates/rc-engine/src/vr/mod.rs')
        for name in re.findall(r'^mod (\w+);', mod, re.M):
            self.assertTrue((ROOT / f'overlay/crates/rc-engine/src/vr/{name}.rs').is_file())

    def test_head_tracking_root_not_scaled(self):
        interaction = text('overlay/crates/rc-engine/src/vr/interaction.rs')
        self.assertIn('root.scale=Vec3::ONE', interaction)
        self.assertIn('s.fire.cancel();s.teleport.cancel()', interaction)
        self.assertIn('XrProjection', interaction)
        self.assertNotIn('GameProjection', interaction)

    def test_original_game_scope_and_physical_validation_are_explicit(self):
        scope = text('overlay/docs/vr/README.md')
        for phrase in ['original ReRAC', 'third-person', 'not tracked-hand aiming', 'not GPU duration']:
            self.assertIn(phrase, scope)
        acceptance = text('overlay/docs/vr/ACCEPTANCE.md')
        self.assertIn('Further physical validation pending', acceptance)
        self.assertIn('Full campaign completion', acceptance)

    def test_payload_is_text_and_contains_no_game_assets_or_credentials(self):
        for p in (ROOT / 'overlay').rglob('*'):
            if not p.is_file():
                continue
            self.assertIn(p.suffix, {'.rs', '.ps1', '.md', '.json', '.yml'})
            self.assertNotIn('extracted', p.parts)
            data = p.read_text(encoding='utf-8')
            self.assertNotRegex(data, r'\bgh[pousr]_[A-Za-z0-9]{30,}\b')
            self.assertNotIn('BEGIN PRIVATE KEY', data)

    def test_provenance_matches_manifest(self):
        provenance = json.loads(text('overlay/docs/vr/PROVENANCE.json'))
        self.assertEqual(provenance['upstream']['commit'], MANIFEST['upstreamCommit'])
        self.assertEqual(provenance['xr']['commit'], MANIFEST['xrCommit'])
        self.assertEqual(provenance['status'], 'native_windows_original_game_vr_play_confirmed')

    def test_pins_are_full_commit_ids(self):
        for key in ['upstreamCommit','xrCommit']:
            self.assertRegex(MANIFEST[key], r'^[0-9a-f]{40}$')

if __name__ == '__main__':
    unittest.main(verbosity=2)
