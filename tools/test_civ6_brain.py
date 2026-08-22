#!/usr/bin/env python3
"""Focused persistence, strategy-selection and decider protocol checks."""

from __future__ import annotations

import json
import io
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_brain
import civ6_play  # noqa: E402


class FakeProc:
    def __init__(self, lines: list[str]) -> None:
        self.stdout = io.StringIO("".join(lines))
        self.stdin = io.StringIO()

    def poll(self):
        return None


class _Decider(civ6_brain.Decider):
    def __init__(self, lines: list[str]) -> None:
        self.proc = FakeProc(lines)
        self.binary = Path("/nonexistent")
        self.run_dir = Path("/nonexistent")
        self.victory = "domination"

    def start(self) -> None:  # pragma: no cover - must never be reached
        raise AssertionError("the canned process must not be replaced")


class DeciderProtocolTest(unittest.TestCase):
    def test_a_plain_response_is_read(self) -> None:
        decider = _Decider([
            '{"turn":1,"orders":[{"kind":"unit","subject":7,"verb":"MOVE_TO",'
            '"x":3,"y":4}],"note":"ok"}\n'
        ])
        rows, note = decider.ask(1)
        self.assertEqual(rows, [("unit", 7, "MOVE_TO", 3, 4)])
        self.assertEqual(note, "ok")

    def test_non_response_json_is_skipped(self) -> None:
        decider = _Decider([
            '{"kind":"genome","strategy":"stock"}\n',
            '{"turn":1,"orders":[],"note":"real"}\n',
        ])
        rows, note = decider.ask(1)
        self.assertEqual(rows, [])
        self.assertEqual(note, "real")


class _RuntimeCommandRunner:
    def __init__(self, revision: str, fail_build: bool = False) -> None:
        self.revision = revision
        self.fail_build = fail_build
        self.calls: list[tuple[list[str], Path]] = []

    def __call__(self, command, cwd, capture_output, text, timeout, env):
        command = list(command)
        cwd = Path(cwd)
        self.calls.append((command, cwd))
        if command[0] == "git" and command[-2:] == ["rev-parse", "origin/main"]:
            return SimpleNamespace(returncode=0, stdout=self.revision + "\n", stderr="")
        if command[0] == "git" and "worktree" in command and "add" in command:
            source = Path(command[-2])
            (source / "tools").mkdir(parents=True)
            (source / ".git").write_text("gitdir: fake\n")
            (source / "tools" / "civ6_brain.py").write_text("# fetched brain\n")
            return SimpleNamespace(returncode=0, stdout="", stderr="")
        if command[0] == "cargo":
            if self.fail_build:
                return SimpleNamespace(returncode=101, stdout="", stderr="compile error")
            built = Path(env["CARGO_TARGET_DIR"]) / "release" / "civvis_orders"
            built.parent.mkdir(parents=True, exist_ok=True)
            built.write_bytes(b"verified GitHub binary")
            built.chmod(0o755)
        return SimpleNamespace(returncode=0, stdout="", stderr="")


class GitHubRuntimeUpdaterTest(unittest.TestCase):
    NEW = "a" * 40
    OLD = "b" * 40

    def updater(self, root: Path, runner: _RuntimeCommandRunner,
                current: str | None = OLD) -> civ6_brain.GitHubRuntimeUpdater:
        repo = root / "repo"
        repo.mkdir()
        return civ6_brain.GitHubRuntimeUpdater(
            repo=repo,
            current_revision=current,
            cache_root=root / "cache",
            command_runner=runner,
        )

    def test_new_main_is_built_offline_and_published_by_exact_sha(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runner = _RuntimeCommandRunner(self.NEW)
            updater = self.updater(root, runner)

            offered = updater.refresh_once()

            self.assertIsNotNone(offered)
            self.assertEqual(offered.revision, self.NEW)
            self.assertEqual(
                offered.binary,
                (root / "cache" / "published" / self.NEW /
                 "civvis_orders").resolve(),
            )
            self.assertEqual(offered.binary.read_bytes(), b"verified GitHub binary")
            self.assertTrue(offered.brain.is_file())
            self.assertEqual(updater.take_ready(), offered)
            commands = [call[0] for call in runner.calls]
            self.assertTrue(any(command[-4:] == ["fetch", "--quiet", "origin", "main"]
                                for command in commands))
            self.assertTrue(any("worktree" in command and self.NEW in command
                                for command in commands))
            self.assertTrue(any(command[:3] == ["cargo", "build", "--release"]
                                and "--locked" in command for command in commands))

    def test_multiple_main_advances_are_built_and_offered_during_one_game(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runner = _RuntimeCommandRunner(self.NEW)
            updater = self.updater(root, runner)

            first = updater.refresh_once()
            self.assertEqual(updater.take_ready(), first)
            runner.revision = "c" * 40
            second = updater.refresh_once()

            self.assertIsNotNone(second)
            self.assertEqual(second.revision, "c" * 40)
            self.assertEqual(updater.take_ready(), second)
            commands = [call[0] for call in runner.calls]
            self.assertTrue(any(
                command[:4] == ["git", "-C", str(updater.source), "checkout"]
                and command[-1] == "c" * 40
                for command in commands
            ))
            self.assertEqual(
                sum(command[0] == "cargo" for command in commands), 2
            )

    def test_a_failed_github_build_never_replaces_the_verified_runtime(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            updater = self.updater(
                root, _RuntimeCommandRunner(self.NEW, fail_build=True)
            )

            with self.assertRaisesRegex(RuntimeError, "compile error"):
                updater.refresh_once()

            self.assertIsNone(updater.take_ready())
            self.assertFalse(
                (root / "cache" / "published" / self.NEW / "civvis_orders").exists()
            )

    def test_an_unchanged_main_does_not_rebuild_or_restart(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runner = _RuntimeCommandRunner(self.NEW)
            updater = self.updater(root, runner, current=self.NEW)

            self.assertIsNone(updater.refresh_once())
            self.assertIsNone(updater.take_ready())
            self.assertFalse(any(call[0][0] == "cargo" for call in runner.calls))

    def test_runtime_exec_preserves_the_run_and_replaces_provenance(self) -> None:
        runtime = civ6_brain.LiveRuntime(
            revision=self.NEW,
            binary=Path("/verified/civvis_orders"),
            brain=Path("/github/tools/civ6_brain.py"),
        )

        command = civ6_brain.runtime_exec_command(
            runtime,
            ["old-brain.py", "--run-dir", "/run", "--bin=/old/bin",
             "--victory", "science"],
        )

        self.assertEqual(command[0], sys.executable)
        self.assertEqual(command[1], "/github/tools/civ6_brain.py")
        self.assertIn("--bin=/verified/civvis_orders", command)
        self.assertEqual(
            command[command.index("--runtime-revision") + 1], self.NEW
        )
        self.assertEqual(command[command.index("--run-dir") + 1], "/run")

    def test_decider_drops_its_old_process_before_using_the_new_binary(self) -> None:
        decider = civ6_brain.Decider(
            Path("/old/civvis_orders"), Path("/run"), "science"
        )
        with mock.patch.object(decider, "stop") as stop:
            changed = decider.use_runtime(Path("/verified/civvis_orders"))

        self.assertTrue(changed)
        stop.assert_called_once_with()
        self.assertEqual(decider.binary, Path("/verified/civvis_orders"))

    def test_runtime_handoff_is_durable_and_names_both_revisions(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            run = Path(temporary)
            runtime = civ6_brain.LiveRuntime(
                revision=self.NEW,
                binary=Path("/verified/civvis_orders"),
                brain=Path("/github/tools/civ6_brain.py"),
            )

            civ6_brain.record_runtime_event(
                run, "handoff", 42, self.OLD, runtime
            )

            event = json.loads((run / "runtime_updates.jsonl").read_text())
            self.assertEqual(event["status"], "handoff")
            self.assertEqual(event["turn"], 42)
            self.assertEqual(event["from_revision"], self.OLD)
            self.assertEqual(event["to_revision"], self.NEW)
            self.assertEqual(event["source"], "origin/main")
            self.assertRegex(
                event["utc"], r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$",
                "every runtime row carries when, so staleness is computable",
            )

    def test_the_heartbeat_is_written_on_success_and_carries_a_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            updater = self.updater(root, _RuntimeCommandRunner(self.NEW))
            updater._write_heartbeat()
            beat = json.loads(updater.heartbeat_path().read_text())
            self.assertEqual(beat["current_revision"], self.OLD)
            self.assertEqual(beat["last_error"], "")
            self.assertRegex(beat["utc"], r"Z$")

            # A refresh failure reaches the heartbeat, so the ladder check can
            # say WHY the game may be playing old code, not only that it might.
            updater._last_error = "cargo build failed (101)"
            updater._write_heartbeat()
            beat = json.loads(updater.heartbeat_path().read_text())
            self.assertEqual(beat["last_error"], "cargo build failed (101)")


class Civ6BrainTest(unittest.TestCase):
    def test_new_government_progression_is_not_blocked(self) -> None:
        seen: set[str] = set()
        rows = [("government", None, "GOVERNMENT_CLASSICAL_REPUBLIC", None, None)]

        guarded, blocked = civ6_brain.guard_government_orders(
            {"government": "GOVERNMENT_CHIEFDOM"}, rows, seen
        )

        self.assertEqual(guarded, rows)
        self.assertEqual(blocked, [])
        self.assertEqual(seen, {"GOVERNMENT_CHIEFDOM"})

    def test_return_to_an_observed_government_is_blocked(self) -> None:
        seen = {
            "GOVERNMENT_MONARCHY",
            "GOVERNMENT_THEOCRACY",
            "GOVERNMENT_MERCHANT_REPUBLIC",
        }
        rows = [
            ("research", None, "TECH_INDUSTRIALIZATION", None, None),
            ("government", None, "GOVERNMENT_MONARCHY", None, None),
        ]

        guarded, blocked = civ6_brain.guard_government_orders(
            {"government": "GOVERNMENT_MERCHANT_REPUBLIC"}, rows, seen
        )

        self.assertEqual(
            guarded,
            [("research", None, "TECH_INDUSTRIALIZATION", None, None)],
        )
        self.assertEqual(
            blocked,
            ["GOVERNMENT_MONARCHY: return to a previously used government"],
        )

    def test_anarchy_does_not_restart_the_previous_government(self) -> None:
        seen = {"GOVERNMENT_MERCHANT_REPUBLIC"}
        rows = [
            ("government", None, "GOVERNMENT_MERCHANT_REPUBLIC", None, None),
            ("unit", 7, "MOVE_TO", 3, 4),
        ]

        guarded, blocked = civ6_brain.guard_government_orders(
            {"government": None, "policy_slots": 0}, rows, seen
        )

        self.assertEqual(guarded, [("unit", 7, "MOVE_TO", 3, 4)])
        self.assertEqual(
            blocked,
            ["GOVERNMENT_MERCHANT_REPUBLIC: government transition in progress"],
        )

    def test_opening_government_choice_remains_available(self) -> None:
        seen: set[str] = set()
        rows = [("government", None, "GOVERNMENT_CHIEFDOM", None, None)]

        guarded, blocked = civ6_brain.guard_government_orders(
            {"government": None, "policy_slots": 0}, rows, seen
        )

        self.assertEqual(guarded, rows)
        self.assertEqual(blocked, [])

    def test_resume_checkpoint_contains_only_ready_turns_for_this_run(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            conn = civ6_brain.connect(Path(temporary) / "orders.sqlite")
            civ6_brain.write_turn(conn, "live", 3, [("research", None, "TECH_MINING", None, None)])
            conn.execute("INSERT INTO ready (run, turn, count) VALUES (?,?,?)", ("other", 4, 0))
            conn.commit()

            self.assertEqual(civ6_brain.completed_turns(conn, "live"), {3})
            self.assertEqual(civ6_brain.completed_turns(conn, "other"), {4})
            self.assertEqual(civ6_brain.completed_turns(conn, "missing"), set())
            conn.close()

    def test_a_combat_frame_is_written_beside_the_opening_board_not_over_it(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            conn = civ6_brain.connect(Path(temporary) / "orders.sqlite")
            civ6_brain.write_turn(conn, "live", 9, [
                ("unit", 5, "MOVE_TO", 3, 2), ("unit", 5, "RANGE_ATTACK", 4, 2)])
            civ6_brain.write_turn(conn, "live", 9, [("unit", 6, "FORTIFY", None, None)], frame=1)

            rows = conn.execute(
                "SELECT frame, seq, verb FROM orders WHERE run = 'live' AND turn = 9 ORDER BY seq"
            ).fetchall()
            self.assertEqual(rows, [
                (0, 0, "MOVE_TO"), (0, 1, "RANGE_ATTACK"),
                (1, civ6_brain.FRAME_SEQ_STRIDE, "FORTIFY"),
            ], "the frame's rows sit beside the opening board's, in their own seq band")
            ready = conn.execute(
                "SELECT frame, count FROM ready WHERE run = 'live' AND turn = 9").fetchall()
            self.assertEqual(ready, [(1, 1)], "one ready row per turn names the newest frame")
            # The turn is still a completed turn for a resuming brain.
            self.assertEqual(civ6_brain.completed_turns(conn, "live"), {9})
            # Rewriting the opening board leaves the frame alone and vice versa.
            civ6_brain.write_turn(conn, "live", 9, [("unit", 7, "FORTIFY", None, None)])
            rows = conn.execute(
                "SELECT frame, seq FROM orders WHERE run = 'live' AND turn = 9 ORDER BY seq"
            ).fetchall()
            self.assertEqual(rows, [(0, 0), (1, civ6_brain.FRAME_SEQ_STRIDE)])
            conn.close()

    def test_a_database_from_before_combat_frames_gains_the_column(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "orders.sqlite"
            old = civ6_brain.sqlite3.connect(str(path))
            old.executescript("""
                CREATE TABLE orders (run TEXT NOT NULL, turn INTEGER NOT NULL, seq INTEGER NOT NULL,
                    kind TEXT NOT NULL, subject INTEGER, verb TEXT, x INTEGER, y INTEGER,
                    PRIMARY KEY (run, turn, seq));
                CREATE TABLE ready (run TEXT NOT NULL, turn INTEGER NOT NULL, count INTEGER NOT NULL,
                    PRIMARY KEY (run, turn));
                INSERT INTO orders VALUES ('live', 2, 0, 'unit', 5, 'FORTIFY', NULL, NULL);
                INSERT INTO ready VALUES ('live', 2, 1);
            """)
            old.commit()
            old.close()

            conn = civ6_brain.connect(path)
            columns = {row[1] for row in conn.execute("PRAGMA table_info(orders)")}
            self.assertIn("frame", columns)
            columns = {row[1] for row in conn.execute("PRAGMA table_info(ready)")}
            self.assertIn("frame", columns)
            self.assertEqual(
                conn.execute("SELECT frame FROM orders WHERE turn = 2").fetchall(), [(0,)],
                "rows from before the column read as the opening board")
            # And a frame can now be written on top of the migrated schema.
            civ6_brain.write_turn(conn, "live", 2, [("unit", 5, "RANGE_ATTACK", 4, 2)], frame=1)
            self.assertEqual(
                conn.execute("SELECT frame, seq FROM orders WHERE turn = 2 ORDER BY seq").fetchall(),
                [(0, 0), (1, civ6_brain.FRAME_SEQ_STRIDE)])
            conn.close()

    def test_completed_game_turns_recovers_only_finished_turns_for_the_run(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            events = Path(temporary) / "events.jsonl"
            events.write_text("\n".join([
                json.dumps({"kind": "state", "run": "live", "turn": 5}),
                json.dumps({"kind": "turn", "run": "other", "turn": 6}),
                json.dumps({"kind": "turn", "run": "live", "turn": 7}),
                json.dumps({"kind": "turn", "run": "live", "turn": "8"}),
                json.dumps({"kind": "turn", "run": "live", "turn": "bad"}),
                "not json",
            ]) + "\n")

            self.assertEqual(civ6_brain.completed_game_turns(events, "live"), {7, 8})
            self.assertEqual(civ6_brain.completed_game_turns(events, "other"), {6})

    def test_default_orders_database_is_scoped_to_its_run(self) -> None:
        run = Path("/tmp/civvis-run")

        self.assertEqual(civ6_brain.orders_db_path(run), run / "orders.sqlite")
        self.assertEqual(
            civ6_brain.orders_db_path(run, "/tmp/explicit-orders.sqlite"),
            Path("/tmp/explicit-orders.sqlite"),
        )

    def test_decider_passes_the_selected_strategy_and_reported_civilization(self) -> None:
        decider = civ6_brain.Decider(
            Path("/tmp/civvis-orders"), Path("/tmp/live-run"), "civvis",
            strategy="auto", with_=["stacked-escort"],
        )
        decider.set_civ("CIVILIZATION_ROME")

        command = decider.command()

        self.assertEqual(command[0], "/tmp/civvis-orders")
        self.assertIn("--fresh-board", command)
        self.assertEqual(command[command.index("--strategy") + 1], "auto")
        self.assertEqual(command[command.index("--civ") + 1], "CIVILIZATION_ROME")
        self.assertEqual(command[command.index("--with") + 1], "stacked-escort")

    def test_per_turn_decider_forwards_the_exact_force_on_arm(self) -> None:
        response = SimpleNamespace(returncode=0, stdout='{"orders":[]}', stderr="")
        with mock.patch.object(civ6_brain.subprocess, "run", return_value=response) as run:
            rows = civ6_brain.civvis_orders(
                Path("/tmp/civvis-orders"), Path("/tmp/live-run"), 17, "science",
                without=["peacetime-deterrence"], with_=["stacked-escort"],
            )
        self.assertEqual(rows, [])
        command = run.call_args.args[0]
        self.assertEqual(command[command.index("--with") + 1], "stacked-escort")
        self.assertEqual(
            command[command.index("--without") + 1], "peacetime-deterrence"
        )

    def test_default_decider_keeps_stock_weights(self) -> None:
        decider = civ6_brain.Decider(
            Path("/tmp/civvis-orders"),
            Path("/tmp/live-run"),
            civ6_brain.DEFAULT_VICTORY,
        )

        command = decider.command()

        # The brain no longer declares a default; it reads the chain's one copy.
        self.assertIs(civ6_brain.DEFAULT_VICTORY, civ6_play.DEFAULT_CIVVIS_VICTORY)
        self.assertEqual(civ6_brain.DEFAULT_STRATEGY, "")
        self.assertEqual(command[command.index("--victory") + 1],
                         civ6_brain.DEFAULT_VICTORY)
        self.assertNotIn("--strategy", command)
        self.assertNotIn("--with", command)


class SeatCivTest(unittest.TestCase):
    """The civ Civilization VI dealt must reach the decider, or `--strategy auto`
    answers only half the brief and reports `per_civ:false`."""

    def _run(self, *lines: str) -> Path:
        run = Path(tempfile.mkdtemp())
        (run / "events.jsonl").write_text("\n".join(lines))
        return run

    def test_the_dealt_civ_is_read_and_stripped_to_the_league_name(self) -> None:
        run = self._run(
            '{"kind":"tiles","turn":1}',
            '{"kind":"seat","civ":"CIVILIZATION_ROME","leader":"LEADER_JULIUS_CAESAR"}',
        )
        self.assertEqual(civ6_brain.seat_civ(run), "Rome")

    def test_a_run_with_no_seat_event_yet_is_none_not_a_guess(self) -> None:
        """⚠ None, never a default. A wrong civ would narrow the league to a table
        that does not describe this game; no civ correctly falls back to the
        overall pick."""
        self.assertIsNone(civ6_brain.seat_civ(self._run('{"kind":"tiles","turn":1}')))

    def test_a_missing_run_directory_does_not_raise(self) -> None:
        """The decider starts lazily and this runs on the way in; an exception here
        would take the whole turn down over a naming detail."""
        self.assertIsNone(civ6_brain.seat_civ(Path("/nonexistent-run-dir")))

    def test_an_unprefixed_civ_is_passed_through(self) -> None:
        run = self._run('{"kind":"seat","civ":"Rome"}')
        self.assertEqual(civ6_brain.seat_civ(run), "Rome")


if __name__ == "__main__":
    unittest.main()
