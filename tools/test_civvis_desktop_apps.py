import importlib.util
import os
import pathlib
import plistlib
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "tools/civvis_desktop_apps.py"
SPEC = importlib.util.spec_from_file_location("civvis_desktop_apps", SCRIPT)
desktop = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(desktop)

# The launchers are zsh, and so is the tab watcher. A Linux CI runner has
# no zsh, so the checks that parse or run one announce themselves as
# skipped there rather than erroring — the rest of the suite, including the
# default contract, is platform-independent and must still run.
ZSH = shutil.which("zsh")
NEEDS_ZSH = unittest.skipUnless(ZSH, "zsh is not installed on this runner")


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
                self.template,
                app,
                self.revision,
                "2026-08-03T17:01:00Z",
                ROOT,
            )
            self.assertNotIn("@@", text)
            self.assertIn('readonly civvis_mode="{}"'.format(app.mode), text)
            self.assertIn('readonly civvis_commit="{}"'.format(self.revision.commit), text)
            self.assertIn('readonly civvis_repo="{}"'.format(ROOT), text)
            # Only the syntax check needs zsh; everything this test asserts
            # about what the launcher says is plain text, and must keep running
            # on a runner that has no zsh.
            if ZSH:
                with tempfile.NamedTemporaryFile(
                    "w", suffix=".zsh", encoding="utf-8"
                ) as launcher:
                    launcher.write(text)
                    launcher.flush()
                    subprocess.run((ZSH, "-n", launcher.name), check=True)
            rendered[app.mode] = text

        for expected in (
            "--victories science,culture,religious,diplomatic,domination,score",
            "--fixed-setup --source-check-interval 1200",
        ):
            self.assertIn(expected, rendered["rust"])
            self.assertIn(expected, rendered["wasm"])
        # The launcher names no world. The stock opening world belongs to
        # `stock_opening_params` in src/server.rs, the supervisor's defaults
        # follow it, and a flag here would be the third copy that let the Rust
        # and WASM channels quietly stop showing the same game.
        for world_flag in ("--players", "--width", "--height", "--city-states",
                           "--turns", "--map", "--shape", "--poles", "--speed"):
            self.assertNotIn(world_flag, rendered["rust"])
            self.assertNotIn(world_flag, rendered["wasm"])
        self.assertIn('readonly civvis_port="8785"', rendered["rust"])
        self.assertIn('readonly civvis_port="8790"', rendered["wasm"])
        for launcher in rendered.values():
            self.assertIn(
                'if status_matches_build "${civvis_status}"; then\n'
                '    # A live file server can begin serving a newly installed bundle',
                launcher,
            )
            self.assertIn('    open_civvis_page true\n    start_tab_watcher', launcher)
            self.assertIn('"${refresh_tool}" refresh', launcher)
            self.assertIn("--max-build-age-minutes 20 --no-launch", launcher)
            self.assertIn("set minimized of chrome_window to false", launcher)
        self.assertEqual([app.label for app in desktop.APPS], ["CIVVIS Rust", "CIVVIS Wasm"])

    def test_refresh_rebuilds_only_stale_or_old_pairs(self):
        held = tempfile.TemporaryDirectory()
        self.addCleanup(held.cleanup)
        desktop_dir = pathlib.Path(held.name)
        for app in desktop.APPS:
            launcher = (
                desktop_dir
                / app.bundle_name
                / "Contents/Resources"
                / app.launcher_script_name
            )
            launcher.parent.mkdir(parents=True)
            launcher.touch()
        current = {
            "mode": "rust",
            "commit": self.revision.commit,
            "commit_time": self.revision.committed_at,
            "built_at": "2026-08-03T17:01:00Z",
        }
        wasm = {**current, "mode": "wasm"}
        with mock.patch.object(
            desktop, "launcher_metadata", side_effect=(current, wasm)
        ), mock.patch.object(desktop, "age_minutes", side_effect=(12, 13)):
            self.assertFalse(
                desktop.installed_pair_needs_refresh(
                    desktop_dir, self.revision, 20
                )
            )

        older = {**current, "commit": "b" * 40}
        with mock.patch.object(
            desktop, "launcher_metadata", side_effect=(older, wasm)
        ), mock.patch.object(desktop, "age_minutes", side_effect=(12, 13)):
            self.assertTrue(
                desktop.installed_pair_needs_refresh(
                    desktop_dir, self.revision, 20
                )
            )

        with mock.patch.object(
            desktop, "launcher_metadata", side_effect=(current, wasm)
        ), mock.patch.object(desktop, "age_minutes", return_value=21):
            self.assertTrue(
                desktop.installed_pair_needs_refresh(
                    desktop_dir, self.revision, 20
                )
            )

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
                self.assertEqual(info["CFBundleExecutable"], "CIVVISLauncher")
                self.assertEqual(info["CFBundleVersion"], self.revision.short)
                self.assertTrue(info["LSUIElement"])
            self.assertEqual(len(identifiers), 2)

    @NEEDS_ZSH
    def test_native_launcher_executes_the_adjacent_zsh_script(self):
        source = ROOT / "tools/desktop/CIVVIS Launcher.c"
        with tempfile.TemporaryDirectory() as held:
            contents = pathlib.Path(held) / "CIVVIS Test.app/Contents"
            macos = contents / "MacOS"
            resources = contents / "Resources"
            macos.mkdir(parents=True)
            resources.mkdir()
            executable = macos / "CIVVISLauncher"
            marker = resources / "marker"
            script = resources / desktop.APPS[0].launcher_script_name
            script.write_text(
                "#!/bin/zsh\nprint -r -- native-wrapper > \"${0:A:h}/marker\"\n",
                encoding="utf-8",
            )
            script.chmod(0o644)
            subprocess.run(
                (
                    "/usr/bin/xcrun",
                    "--sdk",
                    "macosx",
                    "clang",
                    "-Os",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-mmacosx-version-min=13.0",
                    str(source),
                    "-o",
                    str(executable),
                ),
                check=True,
            )
            subprocess.run((str(executable),), check=True)
            self.assertEqual(marker.read_text(encoding="utf-8"), "native-wrapper\n")
            self.assertFalse(script.stat().st_mode & 0o111)

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
                desktop_app = desktop_dir / app.bundle_name
                installed = desktop_app / "Contents/Resources"
                self.assertTrue(desktop_app.is_symlink())
                self.assertEqual(
                    desktop_app.resolve(),
                    (state / "installed" / app.bundle_name).resolve(),
                )
                self.assertTrue((installed / "new").is_file())
                self.assertFalse((installed / "old").exists())
            for old, _ in swap.archives:
                self.assertTrue((old / "Contents/Resources/old").is_file())

            desktop.rollback_install(swap)
            for app in desktop.APPS:
                restored = desktop_dir / app.bundle_name / "Contents/Resources"
                self.assertTrue((restored / "old").is_file())
                self.assertFalse((restored / "new").exists())

    def test_install_archives_legacy_reverse_named_apps(self):
        with tempfile.TemporaryDirectory() as held:
            root = pathlib.Path(held)
            apps = root / "apps"
            desktop_dir = root / "Desktop"
            for app in desktop.APPS:
                (apps / app.bundle_name / "Contents/Resources").mkdir(parents=True)
                legacy = (
                    desktop_dir
                    / (app.legacy_labels[0] + ".app")
                    / "Contents/Resources"
                )
                legacy.mkdir(parents=True)
                (legacy / "old").write_text(app.mode, encoding="utf-8")

            swap = desktop.install_apps(apps, desktop_dir, root / "state")

            self.assertEqual(len(swap.archives), 2)
            for app in desktop.APPS:
                self.assertFalse(
                    (desktop_dir / (app.legacy_labels[0] + ".app")).exists()
                )
                self.assertTrue((desktop_dir / app.bundle_name).is_dir())

    def test_refresh_swaps_private_bundles_without_rewriting_desktop_links(self):
        with tempfile.TemporaryDirectory() as held:
            root = pathlib.Path(held)
            desktop_dir = root / "Desktop"
            state = root / "state"
            staged_roots = (root / "first", root / "second")
            for generation, apps in enumerate(staged_roots, start=1):
                for app in desktop.APPS:
                    resources = apps / app.bundle_name / "Contents/Resources"
                    resources.mkdir(parents=True)
                    (resources / "generation").write_text(
                        str(generation), encoding="utf-8"
                    )
                    (resources / "BUILD.txt").write_text(
                        "Commit: {}\n".format(str(generation) * 40),
                        encoding="utf-8",
                    )

            desktop.install_apps(staged_roots[0], desktop_dir, state)
            links = {
                app.mode: (
                    os.readlink(desktop_dir / app.bundle_name),
                    (desktop_dir / app.bundle_name).lstat().st_ino,
                )
                for app in desktop.APPS
            }

            desktop_dir.chmod(0o555)
            try:
                with mock.patch.object(
                    pathlib.Path,
                    "symlink_to",
                    side_effect=AssertionError("refresh rewrote a Desktop link"),
                ):
                    desktop.install_apps(staged_roots[1], desktop_dir, state)
            finally:
                desktop_dir.chmod(0o755)

            for app in desktop.APPS:
                link = desktop_dir / app.bundle_name
                self.assertEqual(
                    (os.readlink(link), link.lstat().st_ino), links[app.mode]
                )
                self.assertEqual(
                    (link / "Contents/Resources/generation").read_text(
                        encoding="utf-8"
                    ),
                    "2",
                )

    def test_install_lock_exposes_only_a_live_refresh_pid(self):
        with tempfile.TemporaryDirectory() as held:
            state = pathlib.Path(held)
            lock = state / "desktop-apps.lock"
            with desktop.install_lock(state):
                self.assertEqual(int(lock.read_text(encoding="utf-8")), desktop.os.getpid())
            self.assertEqual(lock.read_text(encoding="utf-8"), "")

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
            supervisor_script=pathlib.Path("/build/spectator_supervisor.py"),
            source_snapshot="snapshot",
            version="0.6.0",
        )
        rust = desktop.build_note(desktop.APPS[0], artifacts, artifacts.native_built_at)
        wasm = desktop.build_note(desktop.APPS[1], artifacts, artifacts.wasm_built_at)
        for note in (rust, wasm):
            self.assertIn("Commit: " + self.revision.commit, note)
            self.assertIn(desktop.DEFAULT_PRESET, note)
        self.assertIn("Engine: 8,000,000 bytes", wasm)

    def test_native_build_is_stamped_only_after_cargo_finishes(self):
        with tempfile.TemporaryDirectory() as held:
            root = pathlib.Path(held)
            repo = root / "repo"
            state = root / "state"
            repo.mkdir()
            native_target = state / "cargo-cache/native-target"
            wasm_target = state / "cargo-cache/wasm-target"
            source = state / "build-aaaaaaa-20260803T170000Z/source"

            def fake_run(command, *, cwd=None, env=None, capture=False):
                if command[:3] == ("git", "worktree", "add"):
                    (source / "beta").mkdir(parents=True)
                    (source / "tools").mkdir()
                    (source / "beta/serve.py").write_text("serve\n", encoding="utf-8")
                    (source / "tools/spectator_supervisor.py").write_text(
                        "supervise\n", encoding="utf-8"
                    )
                    (source / "Cargo.toml").write_text(
                        '[package]\nversion = "0.6.0"\n', encoding="utf-8"
                    )
                elif command[0:2] == ("cargo", "build"):
                    self.assertNotIn("CIVVIS_COMMIT", env)
                    self.assertNotIn("CIVVIS_COMMIT_TIME", env)
                    self.assertNotIn("CIVVIS_BUILT_AT", env)
                    (native_target / "release").mkdir(parents=True)
                    (native_target / "release/civvis").write_bytes(b"native")
                elif command[0] == "./beta/publish.sh":
                    lane = pathlib.Path(command[-1]) / desktop.VIEWER_LANE
                    lane.mkdir(parents=True, exist_ok=True)
                    (lane / "civvis.wasm").write_bytes(b"wasm")
                    (lane / "build.json").write_text(
                        '{"commit":"%s","built_at":"2026-08-03T17:09:00Z",'
                        '"wasm_bytes":4,"bundle_bytes":8}\n' % self.revision.commit,
                        encoding="utf-8",
                    )
                elif command[:3] == ("git", "worktree", "remove"):
                    shutil.rmtree(source)
                return subprocess.CompletedProcess(command, 0, "", "")

            def completed_native_timestamp():
                self.assertTrue((native_target / "release/civvis").is_file())
                return "2026-08-03T17:08:00Z"

            with mock.patch.object(
                desktop, "reusable_cargo_targets", return_value=(native_target, wasm_target)
            ), mock.patch.object(
                desktop, "compact_stamp", return_value="20260803T170000Z"
            ), mock.patch.object(
                desktop, "verify_default_contract"
            ), mock.patch.object(
                desktop, "build_environment", return_value={"PATH": "/bin"}
            ), mock.patch.object(
                desktop, "runtime_source_snapshot", return_value="snapshot"
            ), mock.patch.object(
                desktop, "utc_now", side_effect=completed_native_timestamp
            ), mock.patch.object(
                desktop, "run", side_effect=fake_run
            ):
                artifacts = desktop.build_artifacts(
                    repo, self.revision, state, self.template_path
                )

            self.assertEqual(artifacts.native_built_at, "2026-08-03T17:08:00Z")
            self.assertEqual(artifacts.wasm_built_at, "2026-08-03T17:09:00Z")

    def test_listener_verification_waits_for_long_lived_owners(self):
        attached_rust = {"pid": 101, "ppid": 99, "command": "rust"}
        owned_rust = {
            "pid": 101,
            "ppid": 88,
            "command": "/state/rust-runtime/target/spectator/civvis play",
        }
        owned_wasm = {
            "pid": 202,
            "ppid": 1,
            "command": "/Desktop/CIVVIS Wasm.app/Contents/Resources/serve.py site",
        }
        with mock.patch.object(
            desktop,
            "listener",
            side_effect=(attached_rust, owned_wasm, owned_rust, owned_wasm),
        ), mock.patch.object(desktop.time, "sleep") as sleep:
            listeners = desktop.wait_for_detached_listeners(ROOT)

        self.assertEqual(listeners, {"rust": owned_rust, "wasm": owned_wasm})
        sleep.assert_called_once_with(0.25)

    def test_refresh_agent_checks_often_enough_to_keep_builds_under_twenty_minutes(self):
        with tempfile.TemporaryDirectory() as held:
            home = pathlib.Path(held)
            with mock.patch.object(desktop.pathlib.Path, "home", return_value=home):
                payload = desktop.refresh_agent_payload(
                    ROOT, home / "Desktop", home / "state"
                )
        self.assertEqual(payload["StartInterval"], 60)
        self.assertEqual(desktop.REFRESH_REBUILD_AGE_MINUTES, 10)
        self.assertEqual(desktop.MAX_BUILD_AGE_MINUTES, 20)
        arguments = payload["ProgramArguments"]
        self.assertIn("refresh", arguments)
        self.assertIn("--no-launch", arguments)
        self.assertEqual(
            arguments[arguments.index("--rebuild-age-minutes") + 1], "10"
        )
        self.assertEqual(
            arguments[arguments.index("--max-build-age-minutes") + 1], "20"
        )

    def test_refresh_rebuild_and_acceptance_ages_are_independent(self):
        arguments = desktop.parse_args(["refresh"])
        self.assertEqual(arguments.rebuild_age_minutes, 10)
        self.assertEqual(arguments.max_build_age_minutes, 20)

        arguments = desktop.parse_args(
            [
                "refresh",
                "--rebuild-age-minutes",
                "7",
                "--max-build-age-minutes",
                "19",
            ]
        )
        self.assertEqual(arguments.rebuild_age_minutes, 7)
        self.assertEqual(arguments.max_build_age_minutes, 19)

    def test_first_cached_build_reuses_the_newest_legacy_cargo_targets(self):
        with tempfile.TemporaryDirectory() as held:
            state = pathlib.Path(held)
            older = state / "build-aaaaaaa-20260803T220000Z"
            newer = state / "build-bbbbbbb-20260803T230000Z"
            for index, build in enumerate((older, newer), start=1):
                for name in ("native-target", "wasm-target"):
                    target = build / name
                    target.mkdir(parents=True)
                    (target / "generation").write_text(str(index), encoding="utf-8")
                desktop.os.utime(build, (index, index))

            native, wasm = desktop.reusable_cargo_targets(state)

            self.assertEqual(
                (native / "generation").read_text(encoding="utf-8"), "2"
            )
            self.assertEqual(
                (wasm / "generation").read_text(encoding="utf-8"), "2"
            )
            self.assertFalse((newer / "native-target").exists())
            self.assertFalse((newer / "wasm-target").exists())

    def test_endless_refresh_prunes_only_old_generated_artifacts(self):
        with tempfile.TemporaryDirectory() as held:
            state = pathlib.Path(held)
            previous = state / "previous"
            previous.mkdir()
            builds = []
            archives = []
            for index in range(5):
                build = state / f"build-abcdef{index}-20260803T22000{index}Z"
                archive = previous / f"CIVVIS-Rust-abcdef{index}.app"
                build.mkdir()
                archive.mkdir()
                desktop.os.utime(build, (index, index))
                desktop.os.utime(archive, (index, index))
                builds.append(build)
                archives.append(archive)
            unrelated = state / "operator-data"
            unrelated.mkdir()

            desktop.prune_generated_state(state, keep_builds=2, keep_archives=3)

            self.assertEqual([path.exists() for path in builds], [False] * 3 + [True] * 2)
            self.assertEqual(
                [path.exists() for path in archives], [False] * 2 + [True] * 3
            )
            self.assertTrue(unrelated.is_dir())

    @NEEDS_ZSH
    def test_tab_watcher_is_valid_zsh_and_owns_both_lifecycles(self):
        watcher = ROOT / "tools/desktop/CIVVIS Tab Watcher.zsh.in"
        subprocess.run((ZSH, "-n", str(watcher)), check=True)
        text = watcher.read_text(encoding="utf-8")
        self.assertIn("chrome_has_tab", text)
        self.assertIn("stop_owned_processes", text)
        self.assertIn('misses >= 5', text)

    def test_shared_page_names_each_desktop_channel(self):
        # The whole page source: the document plus its one external script —
        # the channel/title logic sits in an inline block, and the split must
        # not silently move what this test pins.
        page = (ROOT / "web/index.html").read_text(encoding="utf-8") + (
            ROOT / "web/assets/app.js"
        ).read_text(encoding="utf-8")
        self.assertIn(
            'channel === "rust") document.title = "CIVVIS · Civ VI Simulator (Rust)"', page
        )
        self.assertIn('channel === "wasm" || channel === "beta"', page)
        self.assertIn('document.title = "CIVVIS · Civ VI Simulator (Wasm)"', page)

    def test_viewer_lane_matches_the_publisher(self):
        """The packager must read the lane `beta/publish.sh` actually writes.

        This is the check that was missing. The publisher moved the viewer from
        `beta/` to `test/` when the site became two lanes, the packager kept
        reading `beta/build.json`, and every desktop build since failed on a
        FileNotFoundError raised *after* a successful cargo and wasm build —
        so the refresh agent burned a full build every cycle for a day and the
        suite never noticed, because nothing here named the path.

        Pinning the constant against the publisher rather than against the
        literal "test" means the next rename fails here instead of in a log
        nobody reads.
        """
        # Check the regression itself first, so a reintroduced `beta/` path
        # fails on the path rather than on a missing constant.
        # assertFalse rather than assertNotIn: the container is the whole
        # 1,600-line script, and dumping it buries the one line that matters.
        packager = SCRIPT.read_text(encoding="utf-8")
        self.assertFalse(
            "beta/build.json" in packager,
            "packager reads beta/build.json; publish.sh writes the manifest "
            "into the viewer lane, so this raises FileNotFoundError after a "
            "successful build",
        )
        self.assertFalse(
            "beta/civvis.wasm" in packager,
            "packager checks beta/civvis.wasm; the module is published into "
            "the viewer lane",
        )
        publish = (ROOT / "beta/publish.sh").read_text(encoding="utf-8")
        # The publisher emits one complete lane with the viewer at its ROOT
        # ("."), so the module and the page assembly land directly in $out. A
        # publisher that grows a subdirectory lane again must change the
        # constant with it — that is the drift this test exists to catch.
        lane = desktop.VIEWER_LANE
        prefix = "" if lane == "." else f"{lane}/"
        self.assertIn(
            f'"$out/{prefix}civvis.wasm"',
            publish,
            f"publish.sh does not write civvis.wasm into the {lane!r} lane",
        )
        self.assertIn(
            f'"$out/{prefix}"',
            publish,
            f"publish.sh does not assemble the page in the {lane!r} lane",
        )
        if lane == ".":
            self.assertNotIn(
                '"$out/test/',
                publish,
                "publish.sh still writes a test/ sublane; the packager and "
                "the deploy workflow both read the lane root",
            )


if __name__ == "__main__":
    unittest.main()
