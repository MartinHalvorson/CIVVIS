import importlib.util
import pathlib
import plistlib
import subprocess
import tempfile
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "tools/civvis_desktop_apps.py"
SPEC = importlib.util.spec_from_file_location("civvis_desktop_apps", SCRIPT)
desktop = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(desktop)


class DesktopAppsTests(unittest.TestCase):
    def setUp(self):
        self.revision = desktop.Revision(
            commit="a" * 40,
            short="a" * 7,
            committed_at="2026-08-03T17:00:00Z",
            title="Keep the launchers together",
        )
        self.template_path = ROOT / "tools/desktop/CIVVIS Launcher.zsh.in"
        self.template = self.template_path.read_text(encoding="utf-8")

    def test_one_template_renders_both_channel_launchers(self):
        rendered = {}
        for app in desktop.APPS:
            text = desktop.render_launcher(
                self.template, app, self.revision, "2026-08-03T17:01:00Z"
            )
            self.assertNotIn("@@", text)
            self.assertIn('readonly civvis_mode="{}"'.format(app.mode), text)
            self.assertIn('readonly civvis_commit="{}"'.format(self.revision.commit), text)
            with tempfile.NamedTemporaryFile("w", suffix=".zsh", encoding="utf-8") as launcher:
                launcher.write(text)
                launcher.flush()
                subprocess.run(("/bin/zsh", "-n", launcher.name), check=True)
            rendered[app.mode] = text

        for expected in (
            "--players 6 --width 74 --height 46 --city-states 9",
            "--speed online --map continents --shape flat --poles poles",
            "--start-era ancient --spectate",
            "--victories science,culture,religious,diplomatic,domination,score",
        ):
            self.assertIn(expected, rendered["rust"])
            self.assertIn(expected, rendered["wasm"])
        self.assertIn('readonly civvis_port="8785"', rendered["rust"])
        self.assertIn('readonly civvis_port="8790"', rendered["wasm"])

    def test_repository_defaults_match_the_launcher_contract(self):
        desktop.verify_default_contract(ROOT, self.template_path)

    def test_info_plists_keep_the_apps_distinct(self):
        with tempfile.TemporaryDirectory() as held:
            root = pathlib.Path(held)
            identifiers = set()
            for app in desktop.APPS:
                target = root / (app.mode + ".plist")
                desktop.write_info_plist(target, app, self.revision.short)
                with target.open("rb") as source:
                    info = plistlib.load(source)
                identifiers.add(info["CFBundleIdentifier"])
                self.assertEqual(info["CFBundleDisplayName"], app.label)
                self.assertEqual(info["CFBundleVersion"], self.revision.short)
                self.assertTrue(info["LSUIElement"])
            self.assertEqual(len(identifiers), 2)

    def test_install_archives_both_apps_before_replacing_them(self):
        with tempfile.TemporaryDirectory() as held:
            root = pathlib.Path(held)
            apps = root / "apps"
            desktop_dir = root / "Desktop"
            state = root / "state"
            for app in desktop.APPS:
                staged = apps / app.bundle_name / "Contents/Resources"
                current = desktop_dir / app.bundle_name / "Contents/Resources"
                staged.mkdir(parents=True)
                current.mkdir(parents=True)
                (staged / "new").write_text(app.mode, encoding="utf-8")
                (current / "old").write_text(app.mode, encoding="utf-8")
                (current / "BUILD.txt").write_text(
                    "Commit: {}\n".format("b" * 40), encoding="utf-8"
                )

            swap = desktop.install_apps(apps, desktop_dir, state)

            self.assertEqual(len(swap.archives), 2)
            for app in desktop.APPS:
                installed = desktop_dir / app.bundle_name / "Contents/Resources"
                self.assertTrue((installed / "new").is_file())
                self.assertFalse((installed / "old").exists())
            for old, _ in swap.archives:
                self.assertTrue((old / "Contents/Resources/old").is_file())

            desktop.rollback_install(swap)
            for app in desktop.APPS:
                restored = desktop_dir / app.bundle_name / "Contents/Resources"
                self.assertTrue((restored / "old").is_file())
                self.assertFalse((restored / "new").exists())

    def test_build_note_carries_exact_revision_and_shared_preset(self):
        artifacts = desktop.BuildArtifacts(
            root=pathlib.Path("/build"),
            revision=self.revision,
            native_binary=pathlib.Path("/build/civvis"),
            native_built_at="2026-08-03T17:01:00Z",
            wasm_site=pathlib.Path("/build/site"),
            wasm_built_at="2026-08-03T17:02:00Z",
            wasm_bytes=8_000_000,
            bundle_bytes=12_000_000,
            serve_script=pathlib.Path("/build/serve.py"),
            version="0.6.0",
        )
        rust = desktop.build_note(desktop.APPS[0], artifacts, artifacts.native_built_at)
        wasm = desktop.build_note(desktop.APPS[1], artifacts, artifacts.wasm_built_at)
        for note in (rust, wasm):
            self.assertIn("Commit: " + self.revision.commit, note)
            self.assertIn(desktop.DEFAULT_PRESET, note)
        self.assertIn("Engine: 8,000,000 bytes", wasm)

    def test_listener_verification_waits_for_launcher_to_exit(self):
        attached_rust = {"pid": 101, "ppid": 99, "command": "rust"}
        detached_rust = {"pid": 101, "ppid": 1, "command": "rust"}
        detached_wasm = {"pid": 202, "ppid": 1, "command": "wasm"}
        with mock.patch.object(
            desktop,
            "listener",
            side_effect=(attached_rust, detached_wasm, detached_rust, detached_wasm),
        ), mock.patch.object(desktop.time, "sleep") as sleep:
            listeners = desktop.wait_for_detached_listeners(ROOT)

        self.assertEqual(listeners, {"rust": detached_rust, "wasm": detached_wasm})
        sleep.assert_called_once_with(0.25)


if __name__ == "__main__":
    unittest.main()
