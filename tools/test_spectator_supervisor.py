import importlib.util
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch


MODULE_PATH = Path(__file__).with_name("spectator_supervisor.py")
SPEC = importlib.util.spec_from_file_location("spectator_supervisor", MODULE_PATH)
assert SPEC and SPEC.loader
supervisor = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(supervisor)


#: Held for the whole module run; removed when the interpreter exits.
_SANDBOX = tempfile.TemporaryDirectory(prefix="civvis-test-gamelock-")


def setUpModule() -> None:
    """⚠ Never ask THIS machine whether a spectator may start.

    `gamelock` reads the operator-halt marker out of the operator's home
    directory, so the halt recorded here on 2026-08-19 made eleven of these
    tests fail on that Mac and pass on the runner — a halt no test had asked
    for. `test_ladder_watchdog.py`'s `setUpModule` tells the whole story; this
    is the same sandbox for the same reason.
    """
    sandbox = Path(_SANDBOX.name)
    halt, lock = sandbox / "operator-halt.json", sandbox / "civ6-game.lock"
    intent = sandbox / "operator-intent"
    intent.write_text("running\n")
    os.environ["CIVVIS_OPERATOR_HALT_FILE"] = str(halt)
    os.environ["CIVVIS_GAME_LOCK_DIR"] = str(lock)
    os.environ["CIVVIS_OPERATOR_INTENT_FILE"] = str(intent)
    supervisor.gamelock.OPERATOR_HALT = halt
    supervisor.gamelock.LOCK = lock
    supervisor.gamelock.OPERATOR_INTENT = intent


class OperatorHaltTests(unittest.TestCase):
    def test_wait_for_operator_resume_logs_once_and_rechecks_the_marker(self):
        with (
            patch.object(
                supervisor.gamelock,
                "operator_halt_description",
                side_effect=["operator pause", "operator pause", None],
            ),
            patch.object(supervisor, "log") as logged,
            patch.object(supervisor.time, "sleep") as slept,
        ):
            supervisor.wait_for_operator_resume(0.25)

        logged.assert_called_once_with(
            "verification not authorized; no spectator will start: operator pause"
        )
        self.assertEqual(slept.call_args_list, [((0.25,), {}), ((0.25,), {})])

    def test_main_waits_for_a_halt_before_taking_a_port_or_starting_work(self):
        args = SimpleNamespace(
            cooldown=supervisor.FINAL_COUNTDOWN_SECONDS,
            prepare_once=False,
            port=8766,
        )
        calls = []
        with (
            patch.object(supervisor, "parse_args", return_value=args),
            patch.object(
                supervisor,
                "wait_for_operator_resume",
                side_effect=lambda: calls.append("halt-check"),
            ),
            patch.object(
                supervisor,
                "acquire_single_instance",
                side_effect=lambda _port: calls.append("port-lock") or False,
            ),
            patch.object(supervisor, "log"),
        ):
            self.assertEqual(supervisor.main(), 0)

        self.assertEqual(calls, ["halt-check", "port-lock"])

    def test_server_launch_boundary_refuses_a_halt_without_spawning(self):
        with (
            patch.object(
                supervisor.gamelock,
                "operator_halt_description",
                return_value="operator pause",
            ),
            patch.object(supervisor.subprocess, "Popen") as spawned,
        ):
            with self.assertRaisesRegex(
                supervisor.OperatorHaltRequested, "operator pause"
            ):
                supervisor.start_server(8766, {"players": 4}, False)

        spawned.assert_not_called()

    def test_server_launch_boundary_refuses_without_verification_intent(self):
        with (
            patch.object(supervisor.gamelock, "operator_halt_description",
                         return_value=None),
            patch.object(supervisor.gamelock, "verification_intent_description",
                         return_value="verification intent is stopped"),
            patch.object(supervisor.subprocess, "Popen") as spawned,
        ):
            with self.assertRaisesRegex(
                supervisor.VerificationIntentDisabled, "intent is stopped"
            ):
                supervisor.start_server(8766, {"players": 4}, False)

        spawned.assert_not_called()


class CanonicalSyncTests(unittest.TestCase):
    def test_supervisor_update_reexecs_canonical_code_and_adopts_the_live_game(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "canonical"
            script = source / "tools" / "spectator_supervisor.py"
            script.parent.mkdir(parents=True)
            script.write_text("print('new supervisor')\n", encoding="utf-8")
            with (
                patch.object(supervisor, "SOURCE_ROOT", source),
                patch.object(supervisor, "RUNNING_SUPERVISOR_SHA256", "old"),
            ):
                command = supervisor.updated_supervisor_command(
                    4321,
                    ["--port", "8766", "--adopt-pid", "99", "--no-open"],
                )

        self.assertEqual(
            command,
            [
                supervisor.sys.executable,
                str(script),
                "--port",
                "8766",
                "--no-open",
                "--adopt-pid",
                "4321",
            ],
        )

    def test_supervisor_update_rejects_invalid_python(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "canonical"
            script = source / "tools" / "spectator_supervisor.py"
            script.parent.mkdir(parents=True)
            script.write_text("def broken(:\n", encoding="utf-8")
            with (
                patch.object(supervisor, "SOURCE_ROOT", source),
                patch.object(supervisor, "RUNNING_SUPERVISOR_SHA256", "old"),
            ):
                self.assertIsNone(supervisor.updated_supervisor_command(4321, []))

    def test_cargo_resolution_survives_a_minimal_service_environment(self):
        configured = "/opt/rust/bin/cargo"
        with patch.dict(supervisor.os.environ, {"CARGO": configured}):
            self.assertEqual(supervisor.cargo_executable(), configured)

        cargo_name = "cargo.exe" if supervisor.os.name == "nt" else "cargo"
        with (
            patch.dict(supervisor.os.environ, {}, clear=True),
            patch.object(supervisor.shutil, "which", return_value=None),
            patch.object(supervisor.Path, "home", return_value=Path("/service-user")),
        ):
            self.assertEqual(
                supervisor.cargo_executable(),
                str(Path("/service-user/.cargo/bin") / cargo_name),
            )

    def test_missing_executable_is_a_failed_command_not_a_supervisor_crash(self):
        with patch.object(
            supervisor.subprocess, "run", side_effect=FileNotFoundError("missing")
        ):
            result = supervisor.command("missing-tool")
        self.assertEqual(result.returncode, 127)
        self.assertIn("missing-tool unavailable", result.stdout)

    def test_deployment_sync_resets_a_private_worktree_without_touching_checkout(self):
        calls = []

        def fake_command(*args, **kwargs):
            calls.append((args, kwargs))
            if args == ("git", "rev-parse", "--verify", "origin/main"):
                return SimpleNamespace(returncode=0, stdout="new\n")
            if args == ("git", "rev-parse", "--show-toplevel"):
                return SimpleNamespace(returncode=0, stdout=f"{source}\n")
            if args == ("git", "rev-parse", "--short", "HEAD"):
                return SimpleNamespace(returncode=0, stdout="new\n")
            return SimpleNamespace(returncode=0, stdout="")

        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "canonical"
            source.mkdir()
            (source / ".git").touch()
            with (
                patch.object(supervisor, "SOURCE_ROOT", source),
                patch.object(supervisor, "SYNC_REMOTE", "origin"),
                patch.object(supervisor, "SYNC_BRANCH", "main"),
                patch.object(supervisor, "command", side_effect=fake_command),
            ):
                self.assertTrue(supervisor.sync_canonical_source())

        called_args = [args for args, _ in calls]
        self.assertIn(("git", "fetch", "--prune", "origin", "main"), called_args)
        self.assertIn(("git", "reset", "--hard", "origin/main"), called_args)
        self.assertIn(
            ("git", "clean", "-fdx", "--", *supervisor.RUNTIME_INPUTS),
            called_args,
        )
        self.assertFalse(any(args[:2] == ("git", "merge") for args in called_args))
        for args, kwargs in calls:
            if args[:2] in (("git", "reset"), ("git", "clean")):
                self.assertEqual(kwargs.get("cwd"), source)

    def test_deployment_creates_the_private_worktree_at_origin_main(self):
        calls = []

        def fake_command(*args, **kwargs):
            calls.append((args, kwargs))
            if args == ("git", "rev-parse", "--show-toplevel"):
                return SimpleNamespace(returncode=0, stdout=f"{source}\n")
            if args == ("git", "rev-parse", "--short", "HEAD"):
                return SimpleNamespace(returncode=0, stdout="new\n")
            return SimpleNamespace(returncode=0, stdout="new\n")

        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "canonical"
            with (
                patch.object(supervisor, "SOURCE_ROOT", source),
                patch.object(supervisor, "command", side_effect=fake_command),
            ):
                self.assertTrue(supervisor.sync_canonical_source())

        self.assertIn(
            (
                ("git", "worktree", "add", "--detach", str(source), "origin/main"),
                {},
            ),
            calls,
        )

    def test_deployment_refuses_to_build_the_shared_checkout(self):
        successful = SimpleNamespace(returncode=0, stdout="main\n")
        with (
            patch.object(supervisor, "SOURCE_ROOT", supervisor.ROOT),
            patch.object(supervisor, "command", return_value=successful),
        ):
            self.assertFalse(supervisor.sync_canonical_source())


class SessionSettingsTests(unittest.TestCase):
    def test_every_map_type_can_launch_on_either_world_shape(self):
        for map_type in supervisor.MAP_TYPES:
            for shape in supervisor.MAP_SHAPES:
                with self.subTest(map=map_type, shape=shape):
                    with patch.object(
                        supervisor.sys,
                        "argv",
                        [
                            "spectator_supervisor.py",
                            "--map",
                            map_type,
                            "--shape",
                            shape,
                            "--poles",
                            "randomized",
                        ],
                    ):
                        parsed = supervisor.parse_args()
                    settings = {
                        "players": parsed.players,
                        "width": parsed.width,
                        "height": parsed.height,
                        "city_states": parsed.city_states,
                        "turns": parsed.turns,
                        "map": parsed.map,
                        "shape": parsed.shape,
                        "poles": parsed.poles,
                        "speed": parsed.speed,
                    }
                    command = supervisor.server_command(8766, settings, False)
                    self.assertEqual(command[command.index("--map") + 1], map_type)
                    self.assertEqual(command[command.index("--shape") + 1], shape)
                    self.assertEqual(command[command.index("--poles") + 1], "randomized")

    def test_launch_victories_are_validated_and_keep_score_disabled(self):
        self.assertEqual(
            supervisor.parse_victories("science,culture,domination"),
            ["science", "culture", "domination"],
        )
        with self.assertRaises(supervisor.argparse.ArgumentTypeError):
            supervisor.parse_victories("science,spaceship")

    def test_preserves_live_map_and_player_settings(self):
        state = {
            "players": [
                {"is_minor": False},
                {"is_minor": False},
                {"is_minor": True},
                {"is_minor": True, "is_barbarian": True},
            ],
            "map": {
                "width": 55,
                "height": 24,
                "script": "continents",
                "shape": "planet",
                "poles": "randomized",
            },
            "game_speed": "online",
            "leader_pool": "expanded",
            "max_turns": 250,
            "victory_conditions": {
                "science": True,
                "culture": True,
                "religious": False,
                "diplomatic": False,
                "domination": True,
                "score": True,
            },
        }
        defaults = {
            "players": 4,
            "width": 60,
            "height": 38,
            "city_states": 6,
            "turns": 500,
            "map": "pangaea",
            "shape": "flat",
            "poles": "poles",
            "speed": "standard",
            "leader_pool": "civ6",
        }
        # Climate, pacing and victory selection follow the live game. Seat
        # count, map script and world shape are redrawn instead of carried, and
        # the board size is dropped with the seat count so `civvis play` can
        # size the map for whatever was drawn. `/state` is fog-of-war trimmed,
        # so the majors it lists are only the ones the viewing player can see --
        # deriving `--players` from them ratcheted the exhibition down game
        # after game and never recovered.
        carried = supervisor.session_settings(state, defaults)
        self.assertEqual(
            {key: value for key, value in carried.items()
             if key not in ("players", "map", "shape")},
            {"turns": 250, "poles": "randomized",
             "speed": "online", "leader_pool": "expanded",
             "victories": ["science", "culture", "domination", "score"]},
        )
        self.assertIn(carried["players"], supervisor.SIMULATION_PLAYER_COUNTS)
        self.assertIn(carried["map"], supervisor.MAP_TYPES)
        self.assertIn(carried["shape"], supervisor.MAP_SHAPES)

    def test_world_shape_is_drawn_for_every_world(self):
        """A restart must not be able to strand the exhibition on one shape.

        Shape used to follow the finished world, seeded from `--shape`, whose
        default is Flat. That made Flat an absorbing state: the keeper starts a
        fresh supervisor whenever it finds none running, that supervisor seats
        its first world from the flags, and every world after it inherited Flat
        from the world before. Measured off the archived saves: the 240 worlds
        from 2026-07-27T23:26Z to 2026-07-29T02:07:40Z were every one of them a
        globe, the keeper restarted the supervisor at 02:05:54Z, and the ~160
        worlds since have every one of them been flat -- which took the sky,
        the Moon, Mars and the expedition off the exhibition entirely, because
        a sky is something only a Planet world has.
        """
        flat_world = {
            "map": {"script": "pangaea", "shape": "flat", "poles": "poles"},
            "game_speed": "online",
            "max_turns": 250,
        }
        # Seeded from the flag the keeper actually passes: none, so Flat.
        flat_defaults = {"players": 6, "turns": 250, "map": "pangaea",
                         "shape": "flat", "poles": "poles", "speed": "online"}
        drawn = {
            supervisor.session_settings(flat_world, flat_defaults)["shape"]
            for _ in range(200)
        }
        self.assertEqual(drawn, set(supervisor.MAP_SHAPES))
        self.assertEqual(
            set(supervisor.rolled_simulation_settings(flat_defaults)["shape"]
                for _ in range(200)),
            set(supervisor.MAP_SHAPES),
        )

    def test_a_staged_lobby_choice_still_beats_the_draw(self):
        """An explicit setup-panel handoff is a choice, not an axis to roll."""
        staged = {
            "next_game_settings": {
                "players": 8,
                "width": 90,
                "height": 38,
                "city_states": 12,
                "turns": 250,
                "map": "continents",
                "speed": "online",
                "leader_pool": "civ6",
                "victories": ["science"],
                "shape": "planet",
                "poles": "poles",
            },
            "map": {"script": "pangaea", "shape": "flat", "poles": "poles"},
        }
        for _ in range(20):
            chosen = supervisor.session_settings(staged, {"turns": 250})
            self.assertEqual(chosen["shape"], "planet")
            self.assertEqual(chosen["map"], "continents")
            self.assertEqual(chosen["players"], 8)

    def test_fixed_desktop_setup_reuses_the_launch_contract(self):
        defaults = {
            "players": 6,
            "width": 74,
            "height": 46,
            "city_states": 9,
            "turns": 250,
            "map": "continents",
            "shape": "flat",
            "poles": "poles",
            "speed": "online",
            "victories": [
                "science", "culture", "religious", "diplomatic", "domination", "score"
            ],
        }
        finished = {
            "players": [{"is_minor": False}] * 10,
            "map": {"script": "water_world", "shape": "planet"},
            "max_turns": 500,
        }
        self.assertEqual(
            supervisor.session_settings(finished, defaults, fixed_setup=True),
            defaults,
        )

    def test_fogged_state_does_not_shrink_the_next_game(self):
        """A trimmed observation must not become the next game's seat count."""
        fogged = {
            "players": [{"is_minor": False}, {"is_minor": True}],
            "map": {"width": 74, "height": 46, "script": "pangaea"},
            "game_speed": "online",
            "max_turns": 250,
        }
        defaults = {
            "players": 6,
            "width": 74,
            "height": 46,
            "city_states": 9,
            "turns": 250,
            "map": "pangaea",
            "speed": "online",
        }
        # The seat count is redrawn from the policy, never read off the
        # observation -- this trimmed one lists a single major, and a game
        # started from it would be a world with nobody in it.
        for _ in range(50):
            carried = supervisor.session_settings(fogged, defaults)
            self.assertIn(carried["players"], supervisor.SIMULATION_PLAYER_COUNTS)
        # City states go with the seat count: dropping them is what lets
        # `civvis play` seat the right number for the size it just picked.
        self.assertNotIn("city_states", carried)

    def test_borrowed_turns_do_not_ratchet_the_next_game_longer(self):
        """"One more turn" raises `max_turns`; the next game must not inherit it."""
        played_on = {
            "players": [{"is_minor": False}, {"is_minor": False}],
            "map": {"width": 74, "height": 46, "script": "pangaea"},
            "game_speed": "online",
            # 250 turns of game plus two presses of "one more turn".
            "max_turns": 300,
            "decided": {"winner": 0, "civ": "Rome", "victory_type": "science", "turn": 244},
        }
        defaults = {
            "players": 6,
            "width": 74,
            "height": 46,
            "city_states": 9,
            "turns": 250,
            "map": "pangaea",
            "speed": "online",
        }
        self.assertEqual(supervisor.session_settings(played_on, defaults)["turns"], 250)
        # A world that was never extended still carries its own limit forward.
        untouched = {**played_on, "max_turns": 180}
        del untouched["decided"]
        self.assertEqual(supervisor.session_settings(untouched, defaults)["turns"], 180)

    def test_playing_on_is_told_apart_from_the_next_world(self):
        """Only the same seed, live and already decided, is an extension."""
        self.assertTrue(
            supervisor.playing_on(
                {"winner": None, "seed": 77, "decided": {"winner": 1, "turn": 210}}, 77
            )
        )
        # The server's own cooldown elapsed first and rolled into another
        # world: live and unrecorded, and still owed a freshly built process.
        self.assertFalse(
            supervisor.playing_on({"winner": None, "seed": 78, "decided": None}, 77)
        )
        # A decided world whose seed moved on is that same handoff, one poll
        # later, with the previous game's record still in view.
        self.assertFalse(
            supervisor.playing_on(
                {"winner": None, "seed": 78, "decided": {"winner": 1, "turn": 210}}, 77
            )
        )
        self.assertFalse(supervisor.playing_on({"winner": 0, "seed": 77}, 77))
        self.assertFalse(supervisor.playing_on(None, 77))

    def test_terminal_result_predicate_includes_draws_and_old_wins(self):
        self.assertFalse(supervisor.game_finished(None))
        self.assertFalse(supervisor.game_finished({"winner": None}))
        self.assertTrue(supervisor.game_finished({"winner": 0}))
        self.assertTrue(supervisor.game_finished({"winner": None, "finished": True}))
        self.assertTrue(
            supervisor.game_finished({"winner": None, "victory_type": "draw"}),
            "raw saves have no computed finished field"
        )
        self.assertEqual(
            supervisor.victory_verdict({"winner": None, "victory_type": "draw"}),
            "draw",
        )

    def test_selected_settings_override_the_live_game_at_the_next_boundary(self):
        selected = {
            "players": 6,
            "width": 74,
            "height": 46,
            "city_states": 9,
            "turns": 330,
            "map": "continents",
            "shape": "planet",
            "poles": "randomized",
            "speed": "quick",
            "leader_pool": "expanded",
            "victories": ["science", "domination"],
        }
        state = {
            "players": [{"is_minor": False}] * 4,
            "map": {"width": 60, "height": 38, "script": "pangaea"},
            "game_speed": "online",
            "max_turns": 250,
            "next_game_settings": selected,
        }
        defaults = {
            "players": 4,
            "width": 60,
            "height": 38,
            "city_states": 6,
            "turns": 250,
            "map": "pangaea",
            "speed": "online",
        }

        # A live world does not trigger a new random draw on every supervisor
        # poll. Its current launch setup remains the fallback until a result
        # actually asks for the next world.
        active = {**state, "winner": None, "next_game_settings": None}
        self.assertEqual(supervisor.settings_at_boundary(active, defaults), defaults)
        finished = {**state, "winner": 0, "next_game_settings": None}
        self.assertEqual(
            supervisor.settings_at_boundary(
                finished, defaults, current_is_authoritative=True
            ),
            defaults,
        )
        self.assertEqual(supervisor.session_settings(state, defaults), selected)
        self.assertEqual(supervisor.settings_at_boundary(state, defaults), selected)

    def test_empty_state_uses_defaults(self):
        defaults = {
            "players": 6,
            "width": 74,
            "height": 46,
            "city_states": 9,
            "turns": 500,
            "map": "pangaea",
            "speed": "standard",
            "leader_pool": "civ6",
        }
        # Everything the policy does not govern still comes from the flags.
        rolled = supervisor.session_settings({}, defaults)
        self.assertEqual(
            {key: value for key, value in rolled.items()
             if key not in ("players", "map", "shape", "speed")},
            {"turns": 500, "leader_pool": "civ6"},
        )
        self.assertIn(rolled["players"], supervisor.SIMULATION_PLAYER_COUNTS)
        self.assertIn(rolled["map"], supervisor.MAP_TYPES)
        # A world with nothing to inherit from is still a world with a shape,
        # and it is drawn like the other two axes rather than left to the flag.
        self.assertIn(rolled["shape"], supervisor.MAP_SHAPES)
        # Standard was asked for and Online is what the exhibition simulates.
        self.assertEqual(rolled["speed"], "online")

    def test_missing_live_victory_settings_keep_previous_selection(self):
        defaults = {
            "players": 4,
            "width": 60,
            "height": 38,
            "city_states": 6,
            "turns": 250,
            "map": "pangaea",
            "speed": "online",
            "victories": ["science", "culture", "domination", "score"],
        }
        self.assertEqual(
            supervisor.session_settings({}, defaults)["victories"],
            ["science", "culture", "domination", "score"],
        )

    def test_manual_new_game_request_keeps_normalized_settings_and_rejects_stale_instances(self):
        request = {
            "mode": "fresh_code",
            "server_instance": 4321,
            "paused": False,
            "settings": {
                "players": 4,
                "width": 60,
                "height": 38,
                "city_states": 6,
                "turns": 330,
                "map": "continents",
                "shape": "planet",
                "poles": "randomized",
                "speed": "quick",
                "leader_pool": "expanded",
                "victories": ["science", "culture", "domination"],
            },
        }
        self.assertEqual(
            supervisor.manual_new_game_request(
                {"server_instance": 4321, "supervisor_request": request}
            ),
            (
                "fresh_code",
                {
                    "players": 4,
                    "width": 60,
                    "height": 38,
                    "city_states": 6,
                    "turns": 330,
                    "map": "continents",
                    "shape": "planet",
                    "poles": "randomized",
                    "speed": "quick",
                    "leader_pool": "expanded",
                    "victories": ["science", "culture", "domination"],
                },
                False,
            ),
        )
        self.assertIsNone(
            supervisor.manual_new_game_request(
                {"server_instance": 9999, "supervisor_request": request}
            )
        )

    def test_invalid_shape_or_poles_rejects_a_supervisor_handoff(self):
        settings = {
            "players": 4,
            "width": 60,
            "height": 38,
            "city_states": 6,
            "turns": 250,
            "map": "pangaea",
            "shape": "cube",
            "poles": "poles",
            "speed": "online",
            "victories": ["science"],
        }
        self.assertIsNone(supervisor.normalized_simulation_settings(settings))
        settings["shape"] = "planet"
        settings["poles"] = "sometimes"
        self.assertIsNone(supervisor.normalized_simulation_settings(settings))

    def test_result_standings_preserves_winner_and_excludes_non_major_players(self):
        state = {
            "winner": 2,
            "players": [
                {
                    "id": 0,
                    "civ": "Rome",
                    "score": 300,
                    "cities": 5,
                    "faith": 90,
                    "military": 240,
                },
                {
                    "id": 2,
                    "civ": "Egypt",
                    "score": 250,
                    "cities": 4,
                    "faith": 800,
                    "military": 120,
                },
                {"id": 4, "civ": "Geneva", "score": 999, "is_minor": True},
                {"id": 5, "civ": "Barbarians", "score": 999, "is_barbarian": True},
            ],
        }

        standings = supervisor.result_standings(state)

        self.assertEqual(
            standings,
            "Rome (score 300, cities 5, faith 90, military 240); "
            "winner Egypt (score 250, cities 4, faith 800, military 120)",
        )


class SourceSnapshotTests(unittest.TestCase):
    def test_snapshot_tracks_runtime_inputs_only(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "src").mkdir()
            source = root / "src" / "lib.rs"
            source.write_text("pub fn value() -> u8 { 1 }\n", encoding="utf-8")
            readme = root / "README.md"
            readme.write_text("first\n", encoding="utf-8")
            with patch.object(supervisor, "SOURCE_ROOT", root):
                original = supervisor.source_snapshot()
                readme.write_text("second\n", encoding="utf-8")
                self.assertEqual(supervisor.source_snapshot(), original)
                source.write_text("pub fn value() -> u8 { 2 }\n", encoding="utf-8")
                self.assertNotEqual(supervisor.source_snapshot(), original)

    def test_runtime_dirtiness_is_scoped_to_compiled_inputs(self):
        clean = SimpleNamespace(returncode=0, stdout="")
        with patch.object(supervisor, "command", return_value=clean) as command:
            self.assertFalse(supervisor.runtime_inputs_dirty())
        command.assert_called_once_with(
            "git",
            "status",
            "--porcelain",
            "--",
            *supervisor.RUNTIME_INPUTS,
            cwd=supervisor.SOURCE_ROOT,
        )

        changed = SimpleNamespace(returncode=0, stdout=" M src/game.rs\n")
        with patch.object(supervisor, "command", return_value=changed):
            self.assertTrue(supervisor.runtime_inputs_dirty())

    def test_changed_source_discards_obsolete_build_before_promoting(self):
        builds = []

        def fake_command(*args, **_kwargs):
            # PATHEXT can hand back "cargo.EXE", so compare case-insensitively.
            if Path(args[0]).name.lower() in ("cargo", "cargo.exe") and args[1:3] == (
                "build",
                "--release",
            ):
                builds.append(args)
            return SimpleNamespace(returncode=0, stdout="")

        with (
            patch.object(supervisor, "source_snapshot", side_effect=["old", "new", "new", "new"]),
            patch.object(supervisor, "command", side_effect=fake_command),
            patch.object(supervisor, "promote_binary") as promote,
            patch.object(supervisor, "write_runtime_metadata") as metadata,
        ):
            self.assertTrue(supervisor.build_latest())
        self.assertEqual(len(builds), 2)
        self.assertEqual(builds[0][1:], ("build", "--release", "--bin", "civvis"))
        promote.assert_called_once_with()
        metadata.assert_called_once_with("new")

    def test_release_build_embeds_the_canonical_revision(self):
        calls = []

        def fake_command(*args, **kwargs):
            calls.append((args, kwargs))
            if args == ("git", "rev-parse", "--short", "HEAD"):
                return SimpleNamespace(returncode=0, stdout="abc1234\n")
            return SimpleNamespace(returncode=0, stdout="")

        with (
            patch.object(supervisor, "source_snapshot", side_effect=["new", "new"]),
            patch.object(supervisor, "runtime_matches", return_value=False),
            patch.object(supervisor, "command", side_effect=fake_command),
            patch.object(supervisor, "promote_binary"),
            patch.object(supervisor, "write_runtime_metadata"),
        ):
            self.assertTrue(supervisor.build_latest())

        cargo_call = next(
            kwargs
            for args, kwargs in calls
            if Path(args[0]).name.lower() in ("cargo", "cargo.exe")
        )
        self.assertEqual(cargo_call["environment"]["CIVVIS_COMMIT"], "abc1234")

    def test_failed_latest_build_never_promotes_stale_binary(self):
        failed = SimpleNamespace(returncode=1, stdout="compile error")
        with (
            patch.object(supervisor, "source_snapshot", return_value="current"),
            patch.object(supervisor, "command", return_value=failed),
            patch.object(supervisor, "promote_binary") as promote,
        ):
            self.assertFalse(supervisor.build_latest())
        promote.assert_not_called()

    def test_promotion_replaces_a_runtime_a_game_is_still_executing(self):
        real_replace = os.replace
        targets = []

        def fake_replace(source, destination):
            targets.append(Path(destination).name)
            if len(targets) == 1:
                # Windows denies overwriting the image of a running process.
                raise PermissionError(5, "Access is denied")
            return real_replace(source, destination)

        with tempfile.TemporaryDirectory() as directory:
            runtime = Path(directory) / "spectator" / supervisor.BINARY_NAME
            runtime.parent.mkdir()
            runtime.write_bytes(b"the build a game is playing on")
            source = Path(directory) / "source"
            (source / "target" / "release").mkdir(parents=True)
            (source / "target" / "release" / supervisor.BINARY_NAME).write_bytes(b"newer")
            with (
                patch.object(supervisor, "RUNTIME_BINARY", runtime),
                patch.object(supervisor, "SOURCE_ROOT", source),
                patch.object(supervisor.os, "replace", side_effect=fake_replace),
            ):
                supervisor.promote_binary()

            self.assertEqual(runtime.read_bytes(), b"newer")
            self.assertTrue(any(name.endswith(".retired1") for name in targets))
            self.assertEqual(list(runtime.parent.glob(runtime.name + ".retired*")), [])

    def test_matching_runtime_skips_redundant_cargo_build(self):
        with (
            patch.object(supervisor, "source_snapshot", return_value="current"),
            patch.object(supervisor, "runtime_matches", return_value=True),
            patch.object(supervisor, "refresh_runtime_metadata") as refresh,
            patch.object(supervisor, "command") as command,
            patch.object(supervisor, "promote_binary") as promote,
        ):
            self.assertTrue(supervisor.build_latest())
        refresh.assert_called_once_with("current")
        command.assert_not_called()
        promote.assert_not_called()

    def test_exact_runtime_refreshes_stale_git_identity_without_rebuilding(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "civvis"
            runtime.write_bytes(b"verified binary")
            metadata_path = root / "build.json"
            metadata_path.write_text(
                json.dumps(
                    {
                        "revision": "local",
                        "dirty": True,
                        "source_snapshot": "same-source",
                        "binary_sha256": "stale",
                        "built_at": "original-build-time",
                    }
                ),
                encoding="utf-8",
            )
            revision = SimpleNamespace(returncode=0, stdout="published\n")
            with (
                patch.object(supervisor, "RUNTIME_BINARY", runtime),
                patch.object(supervisor, "RUNTIME_METADATA", metadata_path),
                patch.object(supervisor, "command", return_value=revision),
                patch.object(supervisor, "runtime_inputs_dirty", return_value=False),
            ):
                supervisor.refresh_runtime_metadata("same-source")

            refreshed = json.loads(metadata_path.read_text(encoding="utf-8"))
            self.assertEqual(refreshed["revision"], "published")
            self.assertEqual(refreshed["commit_time"], "published")
            self.assertFalse(refreshed["dirty"])
            self.assertEqual(refreshed["source_snapshot"], "same-source")
            self.assertEqual(refreshed["built_at"], "original-build-time")
            self.assertEqual(
                refreshed["binary_sha256"],
                supervisor.hashlib.sha256(runtime.read_bytes()).hexdigest(),
            )

    def test_matching_source_rejects_a_tampered_promoted_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "civvis"
            runtime.write_bytes(b"unexpected bytes")
            metadata_path = root / "build.json"
            metadata_path.write_text(
                json.dumps(
                    {
                        "source_snapshot": "same-source",
                        "binary_sha256": supervisor.hashlib.sha256(
                            b"expected bytes"
                        ).hexdigest(),
                    }
                ),
                encoding="utf-8",
            )
            with (
                patch.object(supervisor, "RUNTIME_BINARY", runtime),
                patch.object(supervisor, "RUNTIME_METADATA", metadata_path),
            ):
                self.assertFalse(supervisor.runtime_matches("same-source"))

    def test_matching_source_rebuilds_an_unstamped_or_stale_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "civvis"
            runtime.write_bytes(b"verified binary")
            metadata_path = root / "build.json"
            metadata = {
                "source_snapshot": "same-source",
                "binary_sha256": supervisor.hashlib.sha256(
                    runtime.read_bytes()
                ).hexdigest(),
            }
            metadata_path.write_text(json.dumps(metadata), encoding="utf-8")
            with (
                patch.object(supervisor, "RUNTIME_BINARY", runtime),
                patch.object(supervisor, "RUNTIME_METADATA", metadata_path),
                patch.object(supervisor, "source_revision", return_value="current"),
            ):
                self.assertFalse(supervisor.runtime_matches("same-source"))
                metadata["embedded_revision"] = "previous"
                metadata_path.write_text(json.dumps(metadata), encoding="utf-8")
                self.assertFalse(supervisor.runtime_matches("same-source"))
                metadata["embedded_revision"] = "current"
                metadata_path.write_text(json.dumps(metadata), encoding="utf-8")
                self.assertTrue(supervisor.runtime_matches("same-source"))

    def test_single_update_attempt_returns_control_after_a_failed_build(self):
        with (
            patch.object(supervisor, "sync_canonical_source", return_value=True) as sync,
            patch.object(supervisor, "build_latest", return_value=False) as build,
        ):
            self.assertFalse(supervisor.prepare_latest_once())
        sync.assert_called_once_with()
        build.assert_called_once_with(max_attempts=1)

    def test_runtime_replacement_distinguishes_in_process_restart_from_deployment(self):
        self.assertFalse(supervisor.runtime_replacement_pending("current", "current"))
        self.assertTrue(supervisor.runtime_replacement_pending("previous", "current"))
        self.assertTrue(supervisor.runtime_replacement_pending(None, "current"))
        self.assertFalse(supervisor.runtime_replacement_pending("previous", None))

    def test_promoted_identity_changes_for_a_new_binary_with_the_same_source_snapshot(self):
        with tempfile.TemporaryDirectory() as directory:
            metadata_path = Path(directory) / "build.json"
            metadata_path.write_text(
                json.dumps(
                    {
                        "source_snapshot": "same-runtime-inputs",
                        "binary_sha256": "first-binary",
                    }
                ),
                encoding="utf-8",
            )
            with patch.object(supervisor, "RUNTIME_METADATA", metadata_path):
                self.assertEqual(supervisor.promoted_runtime_id(), "first-binary")
                metadata_path.write_text(
                    json.dumps(
                        {
                            "source_snapshot": "same-runtime-inputs",
                            "binary_sha256": "second-binary",
                        }
                    ),
                    encoding="utf-8",
                )
                self.assertEqual(supervisor.promoted_runtime_id(), "second-binary")
                self.assertTrue(
                    supervisor.runtime_replacement_pending(
                        "first-binary", supervisor.promoted_runtime_id()
                    )
                )

    def test_boundary_uses_verified_runtime_instead_of_retrying_broken_source(self):
        with tempfile.TemporaryDirectory() as directory:
            runtime = Path(directory) / "civvis"
            runtime.touch()
            with (
                patch.object(supervisor, "RUNTIME_BINARY", runtime),
                patch.object(supervisor, "prepare_latest_once", return_value=False) as once,
                patch.object(supervisor, "prepare_latest") as retry,
            ):
                self.assertFalse(supervisor.prepare_boundary_runtime(15.0))
        once.assert_called_once_with()
        retry.assert_not_called()

    def test_boundary_waits_for_a_build_when_no_verified_runtime_exists(self):
        with tempfile.TemporaryDirectory() as directory:
            runtime = Path(directory) / "missing-civvis"
            with (
                patch.object(supervisor, "RUNTIME_BINARY", runtime),
                patch.object(supervisor, "prepare_latest_once", return_value=False),
                patch.object(supervisor, "prepare_latest") as retry,
            ):
                self.assertTrue(supervisor.prepare_boundary_runtime(7.5))
        retry.assert_called_once_with(7.5)

    def test_boundary_rechecks_canonical_head_after_build(self):
        with (
            patch.object(supervisor, "prepare_latest_once", return_value=True) as prepare,
            patch.object(supervisor, "sync_canonical_source", return_value=True) as sync,
            patch.object(supervisor, "source_snapshot", return_value="fresh") as snapshot,
            patch.object(supervisor, "runtime_matches", return_value=True) as matches,
        ):
            self.assertTrue(supervisor.prepare_boundary_runtime(15.0))

        prepare.assert_called_once_with()
        sync.assert_called_once_with()
        snapshot.assert_called_once_with()
        matches.assert_called_once_with("fresh")

    def test_boundary_rebuilds_when_head_moves_during_cargo(self):
        with (
            patch.object(
                supervisor, "prepare_latest_once", side_effect=[True, True]
            ) as prepare,
            patch.object(
                supervisor, "sync_canonical_source", side_effect=[True, True]
            ) as sync,
            patch.object(supervisor, "source_snapshot", return_value="fresh"),
            patch.object(
                supervisor, "runtime_matches", side_effect=[False, True]
            ),
        ):
            self.assertTrue(supervisor.prepare_boundary_runtime(15.0))

        self.assertEqual(prepare.call_count, 2)
        self.assertEqual(sync.call_count, 2)

    def test_live_refresh_requires_both_fresh_code_and_a_safe_checkpoint(self):
        checkpoint = Path("/tmp/civvis-live-refresh.json")
        with (
            patch.object(supervisor, "prepare_latest_once", return_value=False),
            patch.object(supervisor, "capture_checkpoint") as capture,
        ):
            self.assertFalse(supervisor.prepare_live_refresh(8766, checkpoint))
        capture.assert_not_called()

        with (
            patch.object(supervisor, "prepare_latest_once", return_value=True),
            patch.object(supervisor, "capture_checkpoint", return_value=False),
        ):
            self.assertFalse(supervisor.prepare_live_refresh(8766, checkpoint))

        with (
            patch.object(supervisor, "prepare_latest_once", return_value=True),
            patch.object(supervisor, "capture_checkpoint", return_value=True) as capture,
        ):
            self.assertTrue(supervisor.prepare_live_refresh(8766, checkpoint))
        capture.assert_called_once_with(8766, checkpoint)

    def test_active_prebuild_always_fetches_canonical_source(self):
        with patch.object(
            supervisor, "prepare_latest_once", return_value=True
        ) as prepare:
            self.assertTrue(supervisor.prebuild_latest_once())
        prepare.assert_called_once_with()


class RecoveryTests(unittest.TestCase):
    def setUp(self):
        # Recovery tests exercise `main()` while the real exhibition may be
        # running on the same machine. The per-port production lock is an
        # external deployment boundary, not part of those state-machine
        # scenarios; letting it consult the live 8766 lock makes the suite
        # return before any mocked recovery behavior runs.
        # The durable operator halt is also host state, not a recovery-state
        # input.  Individual halt-contract tests cover it above; keeping it
        # out of these simulated game loops makes their result independent of
        # whether an operator has intentionally paused this machine today.
        halt_wait = patch.object(supervisor, "wait_for_operator_resume")
        halt_wait.start()
        self.addCleanup(halt_wait.stop)
        instance_guard = patch.object(
            supervisor, "acquire_single_instance", return_value=True
        )
        instance_guard.start()
        self.addCleanup(instance_guard.stop)
        # `main()` tests use the real production port number but exercise a
        # mocked state machine. Keep the new port-ownership preflight hermetic
        # unless an individual case is explicitly modelling a live incumbent.
        owner_probe = patch.object(supervisor, "port_owner_state", return_value=None)
        owner_probe.start()
        self.addCleanup(owner_probe.stop)
        # The countdown used to be settable, so these cases pinned it back to
        # its floor. It is a constant now and `--cooldown` cannot move it, so
        # there is nothing left to pin.

    @staticmethod
    def supervisor_args(**overrides):
        values = {
            "port": 8766,
            "players": 4,
            "width": 60,
            "height": 38,
            "city_states": 6,
            "turns": 500,
            "map": "pangaea",
            "speed": "standard",
            "cooldown": 0.0,
            "poll": 0.01,
            "build_retry": 0.01,
            "source_check_interval": 30.0,
            "unresponsive_timeout": 20.0,
            "busy_timeout": 0.0,
            "stall_timeout": 30.0,
            "checkpoint_interval": 5.0,
            "max_resume_attempts": 2,
            "live_refresh_grace": 1800.0,
            "no_open": True,
            "adopt_pid": 321,
        }
        values.update(overrides)
        return SimpleNamespace(**values)

    def test_live_owner_exits_before_touching_the_server(self):
        with (
            patch.object(supervisor, "parse_args", return_value=self.supervisor_args()),
            patch.object(supervisor, "acquire_single_instance", return_value=False),
            patch.object(supervisor, "start_server") as start,
        ):
            self.assertEqual(supervisor.main(), 0)
        start.assert_not_called()

    def test_explicit_halt_stops_an_adopted_spectator_before_the_next_poll(self):
        """A marker written during a live world must stop it, not only block a restart."""
        with (
            patch.object(supervisor, "parse_args", return_value=self.supervisor_args()),
            patch.object(supervisor, "process_alive", return_value=True),
            patch.object(supervisor, "read_status", return_value={"turn": 12}),
            patch.object(supervisor, "source_snapshot", return_value="snapshot"),
            patch.object(supervisor, "runtime_matches", return_value=False),
            patch.object(
                supervisor.gamelock,
                "operator_halt_description",
                return_value="operator pause",
            ),
            patch.object(supervisor, "stop_background_prebuild") as stop_build,
            patch.object(supervisor, "stop_server") as stop_server,
            patch.object(supervisor, "log"),
        ):
            self.assertEqual(supervisor.main(), 0)

        stop_build.assert_called_once_with(None)
        stop_server.assert_called_once_with(None, 321)

    def test_successor_detection_closes_the_cooldown_restart_race(self):
        finished = {"server_instance": 7, "seed": 11, "winner": 2}
        self.assertFalse(supervisor.successor_started(None, 7, 11))
        self.assertFalse(supervisor.successor_started(finished, 7, 11))
        self.assertTrue(
            supervisor.successor_started({**finished, "winner": None}, 7, 11)
        )
        self.assertTrue(
            supervisor.successor_started({**finished, "seed": 12}, 7, 11)
        )
        self.assertFalse(
            supervisor.successor_started(
                {**finished, "winner": None, "finished": True, "victory_type": "draw"},
                7,
                11,
            ),
            "a draw on the same world is still the result being held"
        )

    def test_successor_grace_observes_the_server_owned_restart(self):
        finished = {"server_instance": 7, "seed": 11, "winner": 2}
        successor = {"server_instance": 7, "seed": 12, "winner": None}
        with patch.object(
            supervisor, "read_status", side_effect=[finished, successor]
        ):
            self.assertEqual(
                supervisor.wait_for_successor(8766, 7, 11, timeout=0.2),
                successor,
            )

    def test_manual_restart_uses_existing_runtime_without_building(self):
        requested = {
            "players": 2,
            "width": 44,
            "height": 26,
            "city_states": 3,
            "turns": 250,
            "map": "pangaea",
            "speed": "online",
            "leader_pool": "civ6",
            "victories": ["science", "score"],
        }
        active = {
            "seed": 9,
            "turn": 42,
            "current": 2,
            "winner": None,
            "server_instance": 321,
            "supervisor_request": {
                "mode": "restart",
                "server_instance": 321,
                "paused": True,
                "settings": requested,
            },
        }
        replacement = {"seed": 10, "turn": 1, "current": 0, "winner": None}
        process = SimpleNamespace(pid=654)
        with tempfile.TemporaryDirectory() as directory:
            checkpoint = Path(directory) / "save.json"
            checkpoint.write_text("old checkpoint", encoding="utf-8")
            with (
                patch.object(supervisor, "parse_args", return_value=self.supervisor_args()),
                patch.object(supervisor, "checkpoint_path", return_value=checkpoint),
                patch.object(supervisor, "process_alive", return_value=True),
                patch.object(supervisor, "source_snapshot", return_value="current"),
                patch.object(supervisor, "runtime_matches", return_value=True),
                patch.object(
                    supervisor,
                    "read_status",
                    side_effect=[active, active, KeyboardInterrupt],
                ),
                patch.object(supervisor, "start_server", return_value=process) as start,
                patch.object(supervisor, "wait_for_server", return_value=replacement),
                patch.object(supervisor, "start_background_prebuild") as build,
                patch.object(supervisor, "stop_server"),
            ):
                self.assertEqual(supervisor.main(), 0)

        start.assert_called_once_with(
            8766, requested, False, initially_paused=True
        )
        build.assert_not_called()
        self.assertFalse(checkpoint.exists())

    def test_fresh_code_request_starts_fallback_while_build_runs(self):
        requested = {
            "players": 4,
            "width": 60,
            "height": 38,
            "city_states": 6,
            "turns": 330,
            "map": "continents",
            "speed": "quick",
            "leader_pool": "expanded",
            "victories": ["science", "culture", "domination"],
        }
        active = {
            "seed": 9,
            "turn": 42,
            "current": 2,
            "winner": None,
            "server_instance": 321,
            "supervisor_request": {
                "mode": "fresh_code",
                "server_instance": 321,
                "paused": False,
                "settings": requested,
            },
        }
        replacement = {"seed": 10, "turn": 1, "current": 0, "winner": None}
        process = SimpleNamespace(pid=654)
        worker = SimpleNamespace(pid=777, poll=lambda: None)
        with tempfile.TemporaryDirectory() as directory:
            checkpoint = Path(directory) / "save.json"
            with (
                patch.object(supervisor, "parse_args", return_value=self.supervisor_args()),
                patch.object(supervisor, "checkpoint_path", return_value=checkpoint),
                patch.object(supervisor, "process_alive", return_value=True),
                patch.object(supervisor, "source_snapshot", return_value="current"),
                patch.object(
                    supervisor,
                    "runtime_matches",
                    side_effect=[True, False, False],
                ),
                patch.object(
                    supervisor,
                    "read_status",
                    side_effect=[active, active, replacement, KeyboardInterrupt],
                ),
                patch.object(
                    supervisor, "start_background_prebuild", return_value=worker
                ) as build,
                patch.object(supervisor, "stop_background_prebuild"),
                patch.object(supervisor, "capture_checkpoint", return_value=False),
                patch.object(supervisor, "start_server", return_value=process) as start,
                patch.object(supervisor, "wait_for_server", return_value=replacement),
                patch.object(supervisor, "stop_server"),
                patch.object(supervisor.time, "sleep"),
            ):
                self.assertEqual(supervisor.main(), 0)

        build.assert_called_once_with()
        start.assert_called_once_with(
            8766, requested, False, initially_paused=False
        )

    def test_busy_server_detection_distinguishes_compute_from_idle(self):
        process = SimpleNamespace(pid=321)
        with patch.object(supervisor, "process_cpu_percent", return_value=99.7):
            self.assertTrue(supervisor.process_busy(process, None))
        with patch.object(supervisor, "process_cpu_percent", return_value=0.0):
            self.assertFalse(supervisor.process_busy(process, None))
        with patch.object(supervisor, "process_cpu_percent", return_value=None):
            self.assertFalse(supervisor.process_busy(process, None))

    def test_cpu_measurement_reads_real_compute_on_this_platform(self):
        idle = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])
        spinning = subprocess.Popen([sys.executable, "-c", "while True: pass"])
        try:
            # Let interpreter startup finish first. The supervisor measures a
            # server that has been up and serving, never a process 250ms old,
            # and startup burn landing inside the sample window is not the
            # compute this is looking for. Reading it as compute is what made
            # this test fail on CI with "1.0 not less than 1.0".
            time.sleep(0.5)
            self.assertGreater(supervisor.process_cpu_percent(spinning.pid), 50.0)
            self.assertLess(supervisor.process_cpu_percent(idle.pid), 1.0)
        finally:
            for process in (idle, spinning):
                process.kill()
                process.wait()

    def test_a_process_that_stopped_computing_stops_reading_as_busy(self):
        """The reading `process_busy` gates hang recovery on must decay.

        `unavailable_recovery_due` never replaces a busy process unless an
        operator set `--busy-timeout`, so a measurement that stays high after
        the work stops leaves a hung game running forever. This is the case
        the old `ps -o %cpu=` branch got wrong on Linux, where that column is
        CPU time over the process's whole life.
        """
        worked = subprocess.Popen(
            [sys.executable, "-c",
             "import time\nend = time.time() + 1.5\n"
             "while time.time() < end: pass\ntime.sleep(60)"]
        )
        try:
            self.assertGreater(supervisor.process_cpu_percent(worked.pid), 50.0)
            time.sleep(1.5)
            self.assertLess(supervisor.process_cpu_percent(worked.pid), 1.0)
            self.assertFalse(supervisor.process_busy(worked, None))
        finally:
            worked.kill()
            worked.wait()

    def test_cumulative_cpu_column_is_parsed_in_every_shape_ps_prints(self):
        self.assertAlmostEqual(supervisor.parse_cpu_time("0:00.01"), 0.01)
        self.assertAlmostEqual(supervisor.parse_cpu_time("  1:30  "), 90.0)
        self.assertAlmostEqual(supervisor.parse_cpu_time("1:02:03"), 3_723.0)
        self.assertAlmostEqual(supervisor.parse_cpu_time("2-03:04:05"), 183_845.0)
        for junk in ("", "   ", "not a time", "1:2:3:4", "-"):
            self.assertIsNone(supervisor.parse_cpu_time(junk), junk)

    def test_a_zero_window_cannot_yield_a_rate(self):
        self.assertIsNone(supervisor.process_cpu_percent(os.getpid(), window=0.0))

    def test_liveness_probe_reports_truth_without_killing_the_process(self):
        process = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])
        try:
            # A probe, not a kill: os.kill(pid, 0) terminates on Windows.
            self.assertTrue(supervisor.pid_alive(process.pid))
            self.assertTrue(supervisor.pid_alive(process.pid))
            self.assertIsNone(process.poll())
            self.assertTrue(supervisor.process_alive(None, process.pid))
        finally:
            process.kill()
            process.wait()
        self.assertFalse(supervisor.pid_alive(process.pid))
        self.assertFalse(supervisor.process_alive(None, process.pid))

    def test_port_owner_lookup_identifies_the_live_listener(self):
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen(1)
            port = listener.getsockname()[1]
            self.assertEqual(supervisor.pid_listening_on(port), os.getpid())
        self.assertIsNone(supervisor.pid_listening_on(port))

    def test_readiness_rejects_another_servers_status_response(self):
        """A stale listener must never make a failed child look ready.

        This is the failure mode after a supervisor temporarily loses its
        deployment path: the old game keeps answering HTTP while the newly
        spawned binary fails its bind.  Accepting that response disconnects
        the supervisor from the real server and strands every later handoff.
        """
        child = SimpleNamespace(pid=123, poll=lambda: None, returncode=None)
        with (
            patch.object(supervisor, "pid_listening_on", return_value=456),
            patch.object(supervisor, "read_status") as status,
        ):
            with self.assertRaises(supervisor.ServerPortOwnershipError) as raised:
                supervisor.wait_for_server(8766, child)

        self.assertEqual(raised.exception.expected_pid, 123)
        self.assertEqual(raised.exception.owner_pid, 456)
        status.assert_not_called()

    def test_cold_start_rechecks_port_after_preparation_before_launching(self):
        """A server that survived a repaired deployment is adopted, not raced."""
        args = self.supervisor_args(adopt_pid=None)
        active = {"seed": 3, "turn": 88, "current": 1, "winner": None}
        with tempfile.TemporaryDirectory() as directory:
            missing_runtime = Path(directory) / "civvis"
            checkpoint = Path(directory) / "save.json"
            with (
                patch.object(supervisor, "parse_args", return_value=args),
                patch.object(supervisor, "RUNTIME_BINARY", missing_runtime),
                patch.object(supervisor, "checkpoint_path", return_value=checkpoint),
                patch.object(
                    supervisor,
                    "port_owner_state",
                    side_effect=[None, (4321, active)],
                ) as owner,
                patch.object(supervisor, "prepare_latest") as prepare,
                patch.object(supervisor, "reexec_updated_supervisor"),
                patch.object(supervisor, "process_alive", return_value=True),
                patch.object(supervisor, "source_snapshot", return_value="current"),
                patch.object(supervisor, "runtime_matches", return_value=True),
                patch.object(supervisor, "promoted_runtime_id", return_value=None),
                patch.object(supervisor, "read_json", return_value={}),
                patch.object(supervisor, "read_status", side_effect=KeyboardInterrupt),
                patch.object(supervisor, "start_server") as start,
                patch.object(supervisor, "stop_server"),
            ):
                self.assertEqual(supervisor.main(), 0)

        prepare.assert_called_once_with(args.build_retry)
        self.assertEqual(owner.call_count, 2)
        start.assert_not_called()

    def test_cold_start_adopts_the_game_already_serving_the_port(self):
        active = {"seed": 3, "turn": 88, "current": 1, "winner": None}
        with (
            patch.object(supervisor, "parse_args", return_value=self.supervisor_args()),
            patch.object(supervisor, "pid_listening_on", return_value=4242),
            patch.object(supervisor, "process_alive", return_value=True),
            patch.object(supervisor, "source_snapshot", return_value="current"),
            patch.object(supervisor, "runtime_matches", return_value=True),
            patch.object(
                supervisor, "read_status", side_effect=[active, KeyboardInterrupt]
            ),
            patch.object(supervisor, "start_server") as start,
            patch.object(supervisor, "start_background_prebuild"),
            patch.object(supervisor, "stop_server"),
        ):
            self.assertEqual(supervisor.main(), 0)
        start.assert_not_called()

    def test_stop_server_retires_a_server_it_did_not_spawn(self):
        process = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])
        try:
            supervisor.stop_server(None, process.pid)
            self.assertFalse(supervisor.pid_alive(process.pid))
        finally:
            process.kill()
            process.wait()
    def test_active_compute_has_no_default_wall_clock_kill(self):
        self.assertFalse(
            supervisor.unavailable_recovery_due(
                True, 3_600.0, True, 60.0, 0.0
            )
        )
        self.assertTrue(
            supervisor.unavailable_recovery_due(
                True, 61.0, False, 60.0, 0.0
            )
        )
        self.assertTrue(
            supervisor.unavailable_recovery_due(
                True, 601.0, True, 60.0, 600.0
            )
        )
        self.assertTrue(
            supervisor.unavailable_recovery_due(
                False, 0.0, False, 60.0, 0.0
            )
        )

    def test_late_game_checkpoints_allow_slow_serialization(self):
        self.assertEqual(supervisor.capture_checkpoint.__defaults__, (30.0,))

    def test_pause_restoration_posts_the_explicit_state(self):
        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def read(self):
                return b'{"paused":true,"spectator_paused":true}'

        with patch.object(supervisor, "urlopen", return_value=Response()) as request:
            state = supervisor.set_spectator_pause(8766, True)

        self.assertTrue(state["spectator_paused"])
        posted = request.call_args.args[0]
        self.assertEqual(posted.full_url, "http://127.0.0.1:8766/pace")
        self.assertEqual(json.loads(posted.data), {"paused": True})

    def test_progress_marker_tracks_player_steps_within_a_turn(self):
        first = {"seed": 7, "turn": 12, "current": 1, "winner": None}
        stepped = {**first, "current": 2}
        self.assertNotEqual(
            supervisor.progress_marker(first), supervisor.progress_marker(stepped)
        )

    def test_full_state_read_allows_late_game_serialization(self):
        with patch.object(supervisor, "read_json", return_value={}) as read:
            self.assertEqual(supervisor.read_state(8766), {})
        read.assert_called_once_with(8766, "/state", 5.0)

    def test_resume_detection_allows_progress_after_checkpoint_readiness(self):
        marker = (9, 22, 3, None)
        self.assertTrue(
            supervisor.resumed_checkpoint(
                {"seed": 9, "turn": 24, "current": 1, "winner": None}, marker
            )
        )
        self.assertFalse(
            supervisor.resumed_checkpoint(
                {"seed": 10, "turn": 1, "current": 0, "winner": None}, marker
            )
        )

    def test_stall_recovery_respects_an_intentional_browser_pause(self):
        self.assertTrue(supervisor.should_nudge({}, stalled_for=31, timeout=30))
        self.assertFalse(
            supervisor.should_nudge(
                {"spectator_paused": True}, stalled_for=300, timeout=30
            )
        )

    def test_a_game_played_by_hand_is_never_nudged_or_recovered(self):
        """A single-player game stands still between turns by design.

        Its stall clock says nothing about its health, and the recovery step
        it cannot take would report the server as unavailable and restart it
        out from under the player.
        """
        playing = {"spectate": False, "turn": 12, "current": 0, "winner": None}
        self.assertTrue(supervisor.played_by_hand(playing))
        self.assertFalse(supervisor.should_nudge(playing, stalled_for=3600, timeout=30))
        # Only an explicit false counts: the exhibition always says so, and a
        # state that could not be read keeps the supervision it has today.
        for watched in ({}, {"spectate": True}, {"turn": 4}):
            self.assertFalse(supervisor.played_by_hand(watched))
            self.assertTrue(
                supervisor.should_nudge(watched, stalled_for=31, timeout=30)
            )

    def test_a_finished_game_does_not_take_its_own_seat_forever(self):
        """The exhibition must resume after the game that took it ends.

        Observed live on 2026-07-25: a single-player game reached a diplomatic
        victory on turn 243 and the supervisor parked on it, logging "a
        single-player game took this process" and re-archiving the same 8.5 MB
        save every six seconds — 355 identical copies, about 90 MB a minute —
        while five merged commits waited for a promotion that never came. The
        handoff was asking `played_by_hand`, which a finished game keeps
        answering `false` for as long as it is reachable, so the process was
        handed back to the game that had just released it.
        """
        finished_key = ("instance-40048", 171790132)
        ended = {
            "spectate": False,
            "turn": 243,
            "winner": 0,
            "victory_type": "diplomatic",
            "server_instance": "instance-40048",
            "seed": 171790132,
        }
        self.assertTrue(supervisor.played_by_hand(ended))
        self.assertFalse(supervisor.takes_over_the_seat(ended, finished_key))
        # An AI-only world that ended is likewise nobody's seat.
        self.assertFalse(
            supervisor.takes_over_the_seat({**ended, "spectate": True}, finished_key)
        )
        # Somebody really taking the seat from the result screen keeps it: the
        # process is the same, the game is not.
        took_over = {
            "spectate": False,
            "turn": 1,
            "winner": None,
            "server_instance": "instance-40048",
            "seed": 902_113_447,
        }
        self.assertTrue(supervisor.takes_over_the_seat(took_over, finished_key))
        # A new process holding a live human game counts too.
        self.assertTrue(
            supervisor.takes_over_the_seat(
                {**took_over, "server_instance": "instance-40052"}, finished_key
            )
        )
        # And a game still in play but watched, or a state that could not be
        # read at all, leaves the exhibition free to cycle.
        self.assertFalse(
            supervisor.takes_over_the_seat({**took_over, "spectate": True}, finished_key)
        )
        self.assertFalse(supervisor.takes_over_the_seat(None, finished_key))

    def test_server_command_can_resume_an_atomic_checkpoint(self):
        settings = {
            "players": 4,
            "width": 60,
            "height": 38,
            "city_states": 6,
            "turns": 500,
            "map": "pangaea",
            "speed": "standard",
        }
        checkpoint = Path("/tmp/civvis-checkpoint.json")
        command = supervisor.server_command(8766, settings, False, checkpoint)
        self.assertEqual(command[command.index("--resume") + 1], str(checkpoint))
        self.assertIn("--supervised", command)
        self.assertIn("--no-open", command)

    def test_the_countdown_is_ten_seconds_and_no_launcher_can_change_it(self):
        """The result screen's number has no input, here or on the server.

        It used to come from `--cooldown` via `--restart-ms`, and the number a
        viewer read was whichever value had won that chain — twice in one
        evening the exhibition counted down from something nobody had chosen.
        """
        self.assertEqual(supervisor.FINAL_COUNTDOWN_SECONDS, 10.0)
        for asked in (0.0, 5.0, 9.999, 10.0, 12.5, 60.0, 110.0, float("inf")):
            self.assertEqual(supervisor.final_countdown_seconds(asked), 10.0)
        # And the server is never handed a duration at all.
        command = supervisor.server_command(
            8766,
            {
                "players": 4,
                "width": 60,
                "height": 38,
                "city_states": 6,
                "turns": 500,
                "map": "pangaea",
                "speed": "standard",
            },
            False,
        )
        self.assertNotIn("--restart-ms", command)

    def test_every_world_is_online_speed_and_an_ancient_start(self):
        """The two pins, enforced where all three launch paths meet.

        A rolled world, a staged lobby handoff and a resumed checkpoint all
        reach `civvis play` through `server_command`, so that is where the
        exhibition's one kind of game is decided. A handoff naming Epic is a
        real operator action and gets said out loud rather than swallowed.
        """
        asked_for_epic = {
            "players": 4,
            "width": 60,
            "height": 38,
            "city_states": 6,
            "turns": 250,
            "map": "continents",
            "speed": "epic",
        }
        command = supervisor.server_command(8766, asked_for_epic, False)
        self.assertEqual(command[command.index("--speed") + 1], "online")
        self.assertEqual(command[command.index("--start-era") + 1], "ancient")
        self.assertEqual(command.count("--speed"), 1)

    def test_no_world_is_started_with_teams(self):
        """`civvis play` reads an absent `--teams` as a free-for-all."""
        for settings in (
            {"players": 4, "turns": 250, "map": "pangaea", "speed": "online"},
            {"players": 8, "width": 84, "height": 54, "city_states": 12,
             "turns": 250, "map": "islands", "speed": "online",
             "teams": [0, 0, 1, 1, 2, 2, 3, 3]},
        ):
            command = supervisor.server_command(8766, settings, False)
            self.assertNotIn("--teams", command)

    def test_a_rolled_world_lets_the_binary_size_its_own_board(self):
        """Seat count varies, so the board that holds it cannot be pinned."""
        rolled = supervisor.rolled_simulation_settings(
            {"players": 6, "width": 74, "height": 46, "city_states": 9,
             "turns": 250, "map": "pangaea", "speed": "online"}
        )
        for dropped in ("width", "height", "city_states"):
            self.assertNotIn(dropped, rolled)
        command = supervisor.server_command(8766, rolled, False)
        for flag in ("--width", "--height", "--city-states"):
            self.assertNotIn(flag, command)
        # An explicit size still travels -- that is how a resume keeps the
        # board its checkpoint was written on.
        sized = supervisor.server_command(
            8766, {**rolled, "width": 74, "height": 46, "city_states": 9}, False
        )
        self.assertEqual(sized[sized.index("--width") + 1], "74")
        self.assertEqual(sized[sized.index("--city-states") + 1], "9")

    def test_rolled_worlds_vary_across_both_axes(self):
        """Variety is the point; a roll that always answers 6/pangaea is not."""
        seen_players, seen_maps = set(), set()
        for _ in range(400):
            rolled = supervisor.rolled_simulation_settings(
                {"players": 6, "turns": 250, "map": "pangaea", "speed": "online"}
            )
            seen_players.add(rolled["players"])
            seen_maps.add(rolled["map"])
        self.assertEqual(seen_players, set(supervisor.SIMULATION_PLAYER_COUNTS))
        self.assertEqual(seen_maps, set(supervisor.MAP_TYPES))

    def test_server_command_carries_manual_victory_settings(self):
        settings = {
            "players": 4,
            "width": 60,
            "height": 38,
            "city_states": 6,
            "turns": 330,
            "map": "continents",
            "speed": "quick",
            "leader_pool": "expanded",
            "victories": ["science", "culture", "domination"],
        }
        command = supervisor.server_command(8766, settings, False)
        self.assertEqual(
            command[command.index("--victories") + 1],
            "science,culture,domination",
        )
        self.assertEqual(command[command.index("--leader-pool") + 1], "expanded")

    def test_server_command_can_pause_before_the_stepper_starts(self):
        settings = {
            "players": 4,
            "width": 60,
            "height": 38,
            "city_states": 6,
            "turns": 250,
            "map": "pangaea",
            "speed": "online",
        }
        paused = supervisor.server_command(
            8766, settings, False, initially_paused=True
        )
        running = supervisor.server_command(8766, settings, False)
        self.assertIn("--paused", paused)
        self.assertNotIn("--paused", running)

    def test_server_starts_beside_the_promoted_binary_not_the_shared_web_tree(self):
        settings = {
            "players": 4,
            "width": 60,
            "height": 38,
            "city_states": 6,
            "turns": 500,
            "map": "pangaea",
            "speed": "standard",
        }
        with tempfile.TemporaryDirectory() as directory:
            runtime = Path(directory) / "promoted" / "civvis"
            runtime_metadata = runtime.parent / "build.json"
            process = SimpleNamespace(pid=123)
            with (
                patch.object(supervisor, "RUNTIME_BINARY", runtime),
                patch.object(supervisor, "RUNTIME_METADATA", runtime_metadata),
                patch.object(
                    supervisor, "server_command", return_value=[str(runtime), "play"]
                ),
                patch.object(
                    supervisor.subprocess, "Popen", return_value=process
                ) as popen,
            ):
                self.assertIs(supervisor.start_server(8766, settings, False), process)

        popen.assert_called_once_with(
            [str(runtime), "play"],
            cwd=runtime.parent,
            text=True,
            env=supervisor.os.environ.copy(),
            **supervisor._NO_WINDOW,
        )

    def test_server_launch_exports_commit_and_build_times_from_promoted_metadata(self):
        settings = {
            "players": 6,
            "width": 74,
            "height": 46,
            "city_states": 9,
            "turns": 250,
            "map": "continents",
            "speed": "online",
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "civvis"
            metadata = root / "build.json"
            metadata.write_text(
                json.dumps(
                    {
                        "revision": "a" * 40,
                        "commit_time": "2026-08-03T20:00:00Z",
                        "built_at": "2026-08-03T20:05:00Z",
                    }
                ),
                encoding="utf-8",
            )
            process = SimpleNamespace(pid=123)
            with (
                patch.object(supervisor, "RUNTIME_BINARY", runtime),
                patch.object(supervisor, "RUNTIME_METADATA", metadata),
                patch.object(
                    supervisor, "server_command", return_value=[str(runtime), "play"]
                ),
                patch.object(supervisor.subprocess, "Popen", return_value=process) as popen,
            ):
                supervisor.start_server(8785, settings, False)

        environment = popen.call_args.kwargs["env"]
        self.assertEqual(environment["CIVVIS_COMMIT"], "a" * 40)
        self.assertEqual(environment["CIVVIS_COMMIT_TIME"], "2026-08-03T20:00:00Z")
        self.assertEqual(environment["CIVVIS_BUILT_AT"], "2026-08-03T20:05:00Z")

    def test_checkpoint_write_is_atomic_and_finished_saves_are_not_resumed(self):
        class Response:
            def __init__(self, payload):
                self.payload = payload

            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def read(self):
                return self.payload

        active = {"seed": 9, "turn": 22, "current": 3, "winner": None}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "save.json"
            with patch.object(
                supervisor,
                "urlopen",
                return_value=Response(json.dumps(active).encode()),
            ):
                self.assertTrue(supervisor.capture_checkpoint(8766, path))
            self.assertEqual(json.loads(path.read_text()), active)
            self.assertFalse(path.with_suffix(".json.new").exists())
            self.assertEqual(supervisor.checkpoint_marker(path), (9, 22, 3, None))

            path.write_text(json.dumps({**active, "winner": 1}), encoding="utf-8")
            self.assertIsNone(supervisor.checkpoint_marker(path))

    def test_versioned_save_envelope_is_unwrapped_for_progress_checks(self):
        active = {"seed": 10, "turn": 23, "current": 1, "winner": None}
        envelope = {
            "format": "civvis.save",
            "protocol": "civvis-json",
            "protocol_version": 1,
            "save_format_version": 1,
            "game": active,
        }

        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def read(self):
                return json.dumps(envelope).encode()

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "save.json"
            with patch.object(supervisor, "urlopen", return_value=Response()):
                self.assertTrue(supervisor.capture_checkpoint(8766, path))
            self.assertEqual(json.loads(path.read_text()), envelope)
            self.assertEqual(supervisor.checkpoint_marker(path), (10, 23, 1, None))

    def test_finished_result_archives_exact_save_and_runtime_metadata(self):
        class Response:
            def __init__(self, payload):
                self.payload = payload

            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def read(self):
                return self.payload

        save = {
            "seed": 91,
            "turn": 188,
            "winner": 2,
            "victory_type": "culture",
            "game_speed": "standard",
            "max_turns": 500,
            "map_script": "pangaea",
        }
        payload = json.dumps(save, separators=(",", ":")).encode()
        state = {
            **save,
            "server_instance": 4321,
            "players": [
                {
                    "id": 2,
                    "civ": "Egypt",
                    "score": 400,
                    "cities": 6,
                    "faith": 120,
                    "military": 90,
                },
            ],
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime_metadata = root / "build.json"
            runtime_metadata.write_text(
                json.dumps({"revision": "abc123", "dirty": False}),
                encoding="utf-8",
            )
            with (
                patch.object(supervisor, "urlopen", return_value=Response(payload)),
                patch.object(supervisor, "RUNTIME_METADATA", runtime_metadata),
            ):
                archived = supervisor.archive_result(8766, state, root / "results")

            self.assertIsNotNone(archived)
            assert archived is not None
            self.assertEqual(archived.read_bytes(), payload)
            result_paths = list((root / "results").glob("*.result.json"))
            self.assertEqual(len(result_paths), 1)
            result = json.loads(result_paths[0].read_text(encoding="utf-8"))
            self.assertEqual(result["runtime"]["revision"], "abc123")
            self.assertEqual(result["save"], archived.name)
            self.assertIn("winner Egypt", result["standings"])

    def test_active_result_is_never_archived(self):
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory)
            self.assertIsNone(
                supervisor.archive_result(
                    8766, {"seed": 1, "winner": None}, destination
                )
            )
            self.assertEqual(list(destination.iterdir()), [])

    def test_cold_supervisor_start_resumes_an_active_checkpoint(self):
        args = SimpleNamespace(
            port=8766,
            players=4,
            width=60,
            height=38,
            city_states=6,
            turns=500,
            map="pangaea",
            speed="standard",
            cooldown=10.0,
            poll=0.5,
            build_retry=15.0,
            source_check_interval=30.0,
            unresponsive_timeout=20.0,
            stall_timeout=30.0,
            checkpoint_interval=5.0,
            max_resume_attempts=2,
            live_refresh_grace=1800.0,
            no_open=True,
            adopt_pid=None,
        )
        state = {"seed": 9, "turn": 22, "current": 3, "winner": None}
        process = SimpleNamespace(pid=321)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "civvis"
            runtime.touch()
            checkpoint = root / "save.json"
            checkpoint.write_text(json.dumps(state), encoding="utf-8")
            with (
                patch.object(supervisor, "parse_args", return_value=args),
                patch.object(supervisor, "RUNTIME_BINARY", runtime),
                patch.object(supervisor, "checkpoint_path", return_value=checkpoint),
                patch.object(supervisor, "pid_listening_on", return_value=None),
                patch.object(supervisor, "start_server", return_value=process) as start,
                patch.object(supervisor, "wait_for_server", return_value=state),
                patch.object(supervisor, "read_status", side_effect=KeyboardInterrupt),
                patch.object(supervisor, "stop_server") as stop,
            ):
                self.assertEqual(supervisor.main(), 0)
        start.assert_called_once_with(
            8766,
            {
                "players": 4,
                "width": 60,
                "height": 38,
                "city_states": 6,
                "turns": 500,
                "map": "pangaea",
                "speed": "standard",
                "leader_pool": "civ6",
            },
            False,
            checkpoint,
        )
        self.assertGreaterEqual(stop.call_count, 2)

    def test_adopted_stale_runtime_refreshes_and_resumes_without_waiting_for_a_win(self):
        args = SimpleNamespace(
            port=8766,
            players=4,
            width=60,
            height=38,
            city_states=6,
            turns=500,
            map="pangaea",
            speed="standard",
            cooldown=0.0,
            poll=0.01,
            build_retry=0.01,
            source_check_interval=30.0,
            unresponsive_timeout=20.0,
            busy_timeout=0.0,
            stall_timeout=30.0,
            checkpoint_interval=5.0,
            max_resume_attempts=2,
            live_refresh_grace=1800.0,
            no_open=True,
            adopt_pid=321,
        )
        active = {"seed": 9, "turn": 42, "current": 2, "winner": None}
        replacement = SimpleNamespace(pid=654)
        events = []

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "civvis"
            runtime.touch()
            checkpoint = root / "save.json"

            def prepare(_port, path):
                path.write_text(json.dumps(active), encoding="utf-8")
                events.append(("prepare", None))
                return True

            def stop(process, adopted_pid):
                events.append(("stop", getattr(process, "pid", adopted_pid)))

            def start(*args, **_kwargs):
                events.append(("start", args[3]))
                return replacement

            with (
                patch.object(supervisor, "parse_args", return_value=args),
                patch.object(supervisor, "RUNTIME_BINARY", runtime),
                patch.object(supervisor, "checkpoint_path", return_value=checkpoint),
                patch.object(supervisor, "process_alive", return_value=True),
                patch.object(supervisor, "runtime_matches", side_effect=[False, True]),
                patch.object(supervisor, "source_snapshot", return_value="fresh"),
                patch.object(supervisor, "capture_checkpoint", side_effect=prepare),
                patch.object(supervisor, "read_status", side_effect=[active, active, KeyboardInterrupt]),
                patch.object(supervisor, "start_server", side_effect=start),
                patch.object(supervisor, "wait_for_server", return_value=active),
                patch.object(supervisor, "stop_server", side_effect=stop),
            ):
                self.assertEqual(supervisor.main(), 0)

        self.assertLess(events.index(("prepare", None)), events.index(("stop", 321)))
        self.assertIn(("start", checkpoint), events)

    def test_active_prebuild_runs_out_of_process_while_monitoring_continues(self):
        args = SimpleNamespace(
            port=8766,
            players=4,
            width=60,
            height=38,
            city_states=6,
            turns=500,
            map="pangaea",
            speed="standard",
            cooldown=0.0,
            poll=0.01,
            build_retry=0.01,
            source_check_interval=30.0,
            unresponsive_timeout=20.0,
            busy_timeout=0.0,
            stall_timeout=30.0,
            checkpoint_interval=5.0,
            max_resume_attempts=2,
            live_refresh_grace=1800.0,
            no_open=True,
            adopt_pid=321,
        )
        active = {"seed": 9, "turn": 42, "current": 2, "winner": None}
        worker = SimpleNamespace(pid=777, poll=lambda: None)

        with (
            patch.object(supervisor, "parse_args", return_value=args),
            patch.object(supervisor, "process_alive", return_value=True),
            patch.object(supervisor, "source_snapshot", return_value="changed"),
            patch.object(supervisor, "runtime_matches", side_effect=[True, False]),
            patch.object(
                supervisor,
                "read_status",
                side_effect=[active, active, KeyboardInterrupt],
            ) as read,
            # Watching a live world must never build the whole observation.
            patch.object(supervisor, "read_state") as full,
            patch.object(supervisor, "capture_checkpoint", return_value=False),
            patch.object(
                supervisor, "start_background_prebuild", return_value=worker
            ) as start_build,
            patch.object(supervisor, "stop_background_prebuild") as stop_build,
            patch.object(supervisor, "prebuild_latest_once") as blocking_build,
            patch.object(supervisor, "stop_server"),
            patch.object(supervisor.time, "sleep"),
        ):
            self.assertEqual(supervisor.main(), 0)

        start_build.assert_called_once_with()
        blocking_build.assert_not_called()
        self.assertEqual(read.call_count, 3)
        full.assert_not_called()
        stop_build.assert_called_once_with(worker)

    def test_finished_boundary_stops_a_stale_prebuild_and_rechecks_head(self):
        args = SimpleNamespace(
            port=8766,
            players=4,
            width=60,
            height=38,
            city_states=6,
            turns=500,
            map="pangaea",
            speed="standard",
            cooldown=0.0,
            poll=0.01,
            build_retry=0.01,
            source_check_interval=30.0,
            unresponsive_timeout=20.0,
            busy_timeout=0.0,
            stall_timeout=30.0,
            checkpoint_interval=5.0,
            max_resume_attempts=2,
            live_refresh_grace=1800.0,
            no_open=True,
            adopt_pid=321,
        )
        active = {"seed": 9, "turn": 42, "current": 2, "winner": None}
        finished = {
            **active,
            "turn": 70,
            "winner": 1,
            "victory_type": "science",
            "players": [],
        }
        successor = {"seed": 10, "turn": 1, "current": 0, "winner": None}
        worker = SimpleNamespace(pid=777, poll=lambda: None)
        replacement = SimpleNamespace(pid=654)
        events = []

        def prepare_boundary(_retry):
            events.append("fresh-build")
            return True

        def stop(*_args):
            events.append("stop")

        with tempfile.TemporaryDirectory() as directory:
            checkpoint = Path(directory) / "save.json"
            with (
                patch.object(supervisor, "parse_args", return_value=args),
                patch.object(supervisor, "checkpoint_path", return_value=checkpoint),
                patch.object(supervisor, "process_alive", return_value=True),
                patch.object(supervisor, "source_snapshot", return_value="changed"),
                patch.object(
                    supervisor, "runtime_matches", side_effect=[True, False, False]
                ),
                patch.object(
                    supervisor,
                    "read_status",
                    # The fourth read is the boundary's own re-check for a
                    # player who took the seat during the result cooldown.
                    side_effect=[active, active, finished, finished, KeyboardInterrupt],
                ) as read,
                # The single full observation the finished boundary takes.
                patch.object(
                    supervisor, "read_state", return_value=finished
                ) as full,
                patch.object(supervisor, "capture_checkpoint", return_value=False),
                patch.object(supervisor, "archive_result"),
                patch.object(
                    supervisor, "start_background_prebuild", return_value=worker
                ),
                patch.object(supervisor, "stop_background_prebuild") as stop_build,
                patch.object(
                    supervisor, "prepare_boundary_runtime", side_effect=prepare_boundary
                ) as prepare_boundary,
                patch.object(supervisor, "start_server", return_value=replacement) as start,
                patch.object(supervisor, "wait_for_server", return_value=successor),
                patch.object(supervisor, "stop_server", side_effect=stop),
                patch.object(supervisor.time, "sleep"),
            ):
                self.assertEqual(supervisor.main(), 0)

        start.assert_called_once()
        self.assertEqual(read.call_count, 5)
        # Five status polls over one whole game, and one full observation:
        # the finished world the archive and the successor's setup read.
        self.assertEqual(full.call_count, 1)
        stop_build.assert_any_call(worker)
        prepare_boundary.assert_called_once_with(args.build_retry)
        self.assertLess(events.index("fresh-build"), events.index("stop"))

    def test_finished_boundary_preserves_verified_runtime_build_time(self):
        args = SimpleNamespace(
            port=8766,
            players=4,
            width=60,
            height=38,
            city_states=6,
            turns=500,
            map="pangaea",
            speed="standard",
            cooldown=0.0,
            poll=0.01,
            build_retry=0.01,
            source_check_interval=30.0,
            unresponsive_timeout=20.0,
            busy_timeout=0.0,
            stall_timeout=30.0,
            checkpoint_interval=5.0,
            max_resume_attempts=2,
            live_refresh_grace=1800.0,
            no_open=True,
            adopt_pid=321,
        )
        active = {"seed": 9, "turn": 42, "current": 2, "winner": None}
        finished = {
            **active,
            "turn": 70,
            "winner": 1,
            "victory_type": "science",
            "players": [],
        }
        successor = {"seed": 10, "turn": 1, "current": 0, "winner": None}
        replacement = SimpleNamespace(pid=654)

        with tempfile.TemporaryDirectory() as directory:
            checkpoint = Path(directory) / "save.json"
            with (
                patch.object(supervisor, "parse_args", return_value=args),
                patch.object(supervisor, "checkpoint_path", return_value=checkpoint),
                patch.object(supervisor, "process_alive", return_value=True),
                patch.object(supervisor, "source_snapshot", return_value="verified"),
                patch.object(supervisor, "runtime_matches", return_value=True),
                patch.object(supervisor, "promoted_runtime_id", return_value=None),
                patch.object(
                    supervisor,
                    "read_status",
                    side_effect=[active, finished, finished, KeyboardInterrupt],
                ),
                patch.object(supervisor, "read_state", return_value=finished),
                patch.object(supervisor, "archive_result"),
                patch.object(supervisor, "prepare_boundary_runtime", return_value=True),
                patch.object(supervisor, "refresh_runtime_metadata") as refresh,
                patch.object(supervisor, "write_runtime_metadata") as rewrite,
                patch.object(supervisor, "start_server", return_value=replacement),
                patch.object(supervisor, "wait_for_server", return_value=successor),
                patch.object(supervisor, "stop_server"),
                patch.object(supervisor.time, "sleep"),
            ):
                self.assertEqual(supervisor.main(), 0)

        refresh.assert_called_once_with("verified")
        rewrite.assert_not_called()

    def test_finished_server_checks_a_fresh_build_before_starting_successor(self):
        args = SimpleNamespace(
            port=8766,
            players=4,
            width=60,
            height=38,
            city_states=6,
            turns=500,
            map="pangaea",
            speed="standard",
            cooldown=0.0,
            poll=0.01,
            build_retry=0.01,
            source_check_interval=30.0,
            unresponsive_timeout=20.0,
            busy_timeout=600.0,
            stall_timeout=30.0,
            checkpoint_interval=5.0,
            max_resume_attempts=2,
            live_refresh_grace=1800.0,
            no_open=True,
            adopt_pid=None,
        )
        active = {"seed": 9, "turn": 22, "current": 3, "winner": None}
        finished = {
            **active,
            "turn": 70,
            "winner": 1,
            "victory_type": "science",
            "players": [],
        }
        successor = {"seed": 10, "turn": 1, "current": 0, "winner": None}
        first_process = SimpleNamespace(pid=321)
        second_process = SimpleNamespace(pid=654)
        events = []
        starts = []

        def start(*_args, **_kwargs):
            process = first_process if not starts else second_process
            starts.append(process)
            events.append(("start", process.pid))
            return process

        def wait(_port, process):
            events.append(("wait", process.pid))
            return active if process is first_process else successor

        def stop(process, _adopted_pid):
            events.append(("stop", getattr(process, "pid", None)))

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "civvis"
            runtime.touch()
            checkpoint = root / "save.json"
            with (
                patch.object(supervisor, "parse_args", return_value=args),
                patch.object(supervisor, "RUNTIME_BINARY", runtime),
                patch.object(supervisor, "checkpoint_path", return_value=checkpoint),
                patch.object(supervisor, "pid_listening_on", return_value=None),
                patch.object(supervisor, "runtime_matches", return_value=False),
                patch.object(supervisor, "start_server", side_effect=start),
                patch.object(supervisor, "wait_for_server", side_effect=wait),
                patch.object(
                    supervisor,
                    "read_status",
                    # The second read is the boundary re-checking whether the
                    # result screen was turned into a single-player game.
                    side_effect=[finished, finished, KeyboardInterrupt],
                ),
                patch.object(supervisor, "read_state", return_value=finished),
                patch.object(supervisor, "archive_result") as archive,
                patch.object(
                    supervisor, "prepare_boundary_runtime", return_value=False
                ) as prepare_boundary,
                patch.object(supervisor, "stop_server", side_effect=stop),
                patch.object(supervisor.time, "sleep"),
            ):
                self.assertEqual(supervisor.main(), 0)

        retired = events.index(("stop", 321))
        launched = events.index(("start", 654))
        self.assertLess(retired, launched)
        prepare_boundary.assert_called_once_with(args.build_retry)
        archive.assert_called_once_with(8766, finished)

    def test_a_player_who_takes_the_seat_during_the_cooldown_keeps_it(self):
        """The result screen offers a single-player game, and that game is
        started in the process the supervisor was about to retire. Retiring it
        anyway would take the new board away a few seconds after it appeared.
        """
        args = SimpleNamespace(
            port=8766,
            players=4,
            width=60,
            height=38,
            city_states=6,
            turns=500,
            map="pangaea",
            speed="standard",
            cooldown=0.0,
            poll=0.01,
            build_retry=0.01,
            source_check_interval=30.0,
            unresponsive_timeout=20.0,
            busy_timeout=600.0,
            stall_timeout=30.0,
            checkpoint_interval=5.0,
            max_resume_attempts=2,
            live_refresh_grace=1800.0,
            no_open=True,
            adopt_pid=None,
        )
        finished = {
            "seed": 9,
            "turn": 70,
            "current": 3,
            "winner": 1,
            "victory_type": "science",
            "players": [],
        }
        playing = {"spectate": False, "seed": 11, "turn": 1, "current": 0, "winner": None}
        server = SimpleNamespace(pid=321)
        events = []

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "civvis"
            runtime.touch()
            checkpoint = root / "save.json"
            with (
                patch.object(supervisor, "parse_args", return_value=args),
                patch.object(supervisor, "RUNTIME_BINARY", runtime),
                patch.object(supervisor, "checkpoint_path", return_value=checkpoint),
                patch.object(supervisor, "pid_listening_on", return_value=None),
                patch.object(supervisor, "runtime_matches", return_value=True),
                patch.object(
                    supervisor,
                    "start_server",
                    side_effect=lambda *a, **k: (events.append("start"), server)[1],
                ),
                patch.object(supervisor, "wait_for_server", return_value=finished),
                patch.object(
                    supervisor,
                    "read_status",
                    side_effect=[finished, playing, KeyboardInterrupt],
                ),
                patch.object(supervisor, "read_state", return_value=finished),
                patch.object(supervisor, "archive_result", return_value=None),
                patch.object(
                    supervisor,
                    "stop_server",
                    side_effect=lambda *a, **k: events.append("stop"),
                ),
                patch.object(supervisor.time, "sleep"),
            ):
                self.assertEqual(supervisor.main(), 0)

        # The cold start, and nothing after it. Retiring the finished result
        # would have added a second start for the successor; the stops around
        # it are the cold-start orphan sweep and the shutdown.
        self.assertEqual(events.count("start"), 1)
        self.assertEqual(events, ["stop", "start", "stop"])

    def test_a_world_asked_for_one_more_turn_is_not_retired(self):
        """The result screen's other offer keeps the same world instead of
        replacing it. The winner clears, the seed does not, and the supervisor
        has to recognise that as a game still being played rather than as the
        handoff it was a moment away from performing.
        """
        args = SimpleNamespace(
            port=8766,
            players=4,
            width=60,
            height=38,
            city_states=6,
            turns=500,
            map="pangaea",
            speed="standard",
            cooldown=0.0,
            poll=0.01,
            build_retry=0.01,
            source_check_interval=30.0,
            unresponsive_timeout=20.0,
            busy_timeout=600.0,
            stall_timeout=30.0,
            checkpoint_interval=5.0,
            max_resume_attempts=2,
            live_refresh_grace=1800.0,
            no_open=True,
            adopt_pid=None,
        )
        finished = {
            "seed": 9,
            "turn": 70,
            "current": 3,
            "winner": 1,
            "victory_type": "science",
            "players": [],
        }
        played_on = {
            **finished,
            "winner": None,
            "victory_type": None,
            "turn": 71,
            "max_turns": 95,
            "decided": {"winner": 1, "civ": "Greece", "victory_type": "science", "turn": 70},
        }
        server = SimpleNamespace(pid=321)
        events = []

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "civvis"
            runtime.touch()
            checkpoint = root / "save.json"
            with (
                patch.object(supervisor, "parse_args", return_value=args),
                patch.object(supervisor, "RUNTIME_BINARY", runtime),
                patch.object(supervisor, "checkpoint_path", return_value=checkpoint),
                patch.object(supervisor, "pid_listening_on", return_value=None),
                patch.object(supervisor, "runtime_matches", return_value=True),
                patch.object(
                    supervisor,
                    "start_server",
                    side_effect=lambda *a, **k: (events.append("start"), server)[1],
                ),
                patch.object(supervisor, "wait_for_server", return_value=finished),
                patch.object(
                    supervisor,
                    "read_status",
                    side_effect=[finished, played_on, KeyboardInterrupt],
                ),
                patch.object(supervisor, "read_state", return_value=finished),
                patch.object(supervisor, "archive_result", return_value=None),
                patch.object(
                    supervisor,
                    "stop_server",
                    side_effect=lambda *a, **k: events.append("stop"),
                ),
                patch.object(supervisor.time, "sleep"),
            ):
                self.assertEqual(supervisor.main(), 0)

        # As above: the cold start and nothing after it. A retirement would
        # have shown up as a second start for the successor world.
        self.assertEqual(events, ["stop", "start", "stop"])

    def _run_until_interrupt(self, grace, active, checkpoint, binary):
        """Drive `main()` over a world this supervisor started itself.

        Every other `main()` test here adopts a PID, and an adopted world is
        exactly the case that is *never* waited on — so without this the hold
        has no coverage of the loop that decides it, and a wiring regression
        (`world_started_at` never set) would pass the whole suite.
        """
        started = []
        messages = []
        args = self.supervisor_args(adopt_pid=None, live_refresh_grace=grace)
        with (
            patch.object(supervisor, "parse_args", return_value=args),
            patch.object(supervisor, "RUNTIME_BINARY", binary),
            patch.object(supervisor, "checkpoint_path", return_value=checkpoint),
            patch.object(supervisor, "checkpoint_marker", return_value=None),
            patch.object(supervisor, "resumed_checkpoint", return_value=False),
            patch.object(supervisor, "process_alive", return_value=True),
            # Without this the test is not hermetic and does not test what it
            # says: `adopt_pid=None` makes `main()` look for a game already on
            # the port, and on this machine that is the LIVE exhibition — it
            # adopted PID 80610 and took the adopted-world path, which is
            # exactly the branch that never holds.
            patch.object(supervisor, "pid_listening_on", return_value=None),
            patch.object(supervisor, "promoted_runtime_id", return_value="same"),
            patch.object(supervisor, "source_snapshot", return_value="fresh"),
            # Stale when the loop opens, so a live refresh is scheduled; ready
            # from then on, so only the hold can keep it from being taken.
            patch.object(
                supervisor, "runtime_matches", side_effect=[False] + [True] * 40
            ),
            patch.object(supervisor, "capture_checkpoint", return_value=True),
            patch.object(
                supervisor,
                "read_status",
                side_effect=[active] * 6 + [KeyboardInterrupt],
            ),
            patch.object(
                supervisor,
                "start_server",
                side_effect=lambda *a, **k: started.append((a, k))
                or SimpleNamespace(pid=654),
            ),
            patch.object(supervisor, "wait_for_server", return_value=active),
            patch.object(supervisor, "stop_server"),
            patch.object(supervisor, "start_background_prebuild", return_value=None),
            patch.object(supervisor, "stop_background_prebuild"),
            patch.object(supervisor, "log", side_effect=messages.append),
            patch.object(supervisor.time, "sleep"),
        ):
            self.assertEqual(supervisor.main(), 0)
        # The relaunch itself is the tell, not its `resume` argument: whether a
        # checkpoint path is passed depends on `checkpoint_marker`, which is
        # mocked here, so asserting on it would only be testing the mock.
        return started, messages

    def test_a_promoted_build_waits_for_the_boundary_of_a_world_it_started(self):
        active = {"seed": 9, "turn": 42, "current": 2, "winner": None,
                  "server_instance": 654}
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "civvis"
            binary.touch()
            started, messages = self._run_until_interrupt(
                600.0, active, Path(directory) / "save.json", binary
            )
        self.assertEqual(
            len(started), 1, f"a live match must not be replaced; started {started}"
        )
        self.assertFalse(
            any("resuming the active game from checkpoint" in m for m in messages),
            f"no mid-match swap may happen; got {messages}",
        )
        self.assertTrue(
            any("holding it for the next" in m for m in messages),
            f"the hold must be reported; got {messages}",
        )

    def test_a_zero_grace_still_replaces_the_runtime_mid_match(self):
        # The operator escape hatch, and the behaviour every earlier release
        # had: with no grace at all a ready build cuts in immediately.
        active = {"seed": 9, "turn": 42, "current": 2, "winner": None,
                  "server_instance": 654}
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "civvis"
            binary.touch()
            started, messages = self._run_until_interrupt(
                0.0, active, Path(directory) / "save.json", binary
            )
        self.assertTrue(
            any("resuming the active game from checkpoint" in m for m in messages),
            f"a zero grace must swap mid-match; got {messages}",
        )
        self.assertEqual(
            len(started), 2, f"the swap relaunches the server; started {started}"
        )
        self.assertFalse(any("holding it for the next" in m for m in messages))


class LiveRefreshTests(unittest.TestCase):
    """A promoted build is worth a whole game of waiting, but not an unbounded
    one. Replacing the runtime under a live match is the only thing this
    supervisor does that a viewer sees as the game stopping."""

    def test_a_ready_build_waits_for_the_boundary_instead_of_cutting_in(self):
        self.assertTrue(supervisor.refresh_waits_for_boundary(True, 0.0, 600.0))
        self.assertTrue(supervisor.refresh_waits_for_boundary(True, 90.0, 600.0))

    def test_a_world_that_never_ends_still_gets_the_new_runtime(self):
        self.assertFalse(supervisor.refresh_waits_for_boundary(True, 600.0, 600.0))
        self.assertFalse(supervisor.refresh_waits_for_boundary(True, 3600.0, 600.0))

    def test_an_inherited_world_is_never_waited_on(self):
        # An adopted PID's game may have been running for an hour or may be one
        # turn old; with no boundary in sight the stale runtime goes now.
        self.assertFalse(supervisor.refresh_waits_for_boundary(True, None, 600.0))

    def test_nothing_is_held_back_when_there_is_no_build_to_hold(self):
        self.assertFalse(supervisor.refresh_waits_for_boundary(False, 0.0, 600.0))

    def test_a_zero_or_negative_grace_restores_the_immediate_swap(self):
        self.assertFalse(supervisor.refresh_waits_for_boundary(True, 0.0, 0.0))
        self.assertFalse(supervisor.refresh_waits_for_boundary(True, 0.0, -5.0))

    def _args(self, argv: list[str]) -> object:
        with patch.object(sys, "argv", ["spectator_supervisor.py", *argv]):
            return supervisor.parse_args()

    def test_the_grace_window_outlasts_a_real_game_not_a_healthy_one(self):
        # Measured on this box: a 250-turn Online game is ~1 minute unloaded and
        # has taken over 10 minutes with the agent fleet building. A window
        # sized for the fast case fires on ordinary games — which is exactly
        # what a 600s default did in production on 2026-07-29, swapping the
        # runtime under a live match at a world age of ~600s.
        self.assertGreaterEqual(
            self._args(["--no-open"]).live_refresh_grace,
            1800.0,
            "the grace has to outlast a game played on a loaded box",
        )
        self.assertEqual(
            self._args(["--no-open", "--live-refresh-grace", "0"]).live_refresh_grace,
            0.0,
        )


class StatusDocumentContractTests(unittest.TestCase):
    """The poll loop reads /status, not /state. Every helper it feeds must
    resolve from the compact document the server actually emits — this is the
    cross-language field contract, pinned the same way the ranking tool pins
    its Wilson bounds against the Rust ones."""

    STATUS = {
        "turn": 118,
        "winner": None,
        "finished": False,
        "draw": False,
        "victory_type": None,
        "victory_label": None,
        "spectate": True,
        "seed": 774422,
        "current": 3,
        "spectator_paused": False,
        "server_instance": 91_234,
        "decided": None,
        "frames_missed": 0,
        "commit": "abc1234",
    }

    def test_progress_marker_resolves_from_the_status_document(self):
        self.assertEqual(
            supervisor.progress_marker(self.STATUS), (774422, 118, 3, None)
        )

    def test_nudge_check_reads_the_pause_flag(self):
        self.assertTrue(supervisor.should_nudge(self.STATUS, 999.0, 1.0))
        paused = dict(self.STATUS, spectator_paused=True)
        self.assertFalse(supervisor.should_nudge(paused, 999.0, 1.0))

    def test_play_on_detection_reads_decided_seed_and_winner(self):
        playing_on = dict(self.STATUS, decided={"winner": 2, "mode": "Indefinite"})
        self.assertTrue(supervisor.playing_on(playing_on, 774422))
        self.assertFalse(supervisor.playing_on(self.STATUS, 774422))

    def test_successor_identity_reads_instance_seed_and_winner(self):
        finished = dict(self.STATUS, winner=2, finished=True)
        self.assertFalse(supervisor.successor_started(finished, 91_234, 774422))
        fresh_world = dict(self.STATUS, seed=774423)
        self.assertTrue(supervisor.successor_started(fresh_world, 91_234, 774422))
        new_process = dict(self.STATUS, server_instance=91_235)
        self.assertTrue(supervisor.successor_started(new_process, 91_234, 774422))

    def test_the_compact_probe_asks_the_cheap_endpoint(self):
        with patch.object(supervisor, "read_json", return_value=self.STATUS) as read:
            self.assertEqual(supervisor.read_status(8766), self.STATUS)
        read.assert_called_once_with(8766, "/status", 5.0)

    def test_an_older_runtimes_status_falls_back_to_the_full_observation(self):
        """A runtime built before these fields is still live after this
        supervisor re-execs itself from newer source. Its `/status` has no
        progress marker in it, so driving the loop from that document would
        silently stop recognising "one more turn" and would nudge a game a
        viewer had deliberately paused. Read the world the old way instead,
        until the next boundary swaps the binary."""
        old_status = {"turn": 118, "winner": None, "spectate": True}
        world = dict(self.STATUS, players=[])
        with patch.object(
            supervisor, "read_json", side_effect=[old_status, world]
        ) as read:
            self.assertEqual(supervisor.read_status(8766), world)
        self.assertEqual(
            [call.args[1] for call in read.call_args_list], ["/status", "/state"]
        )

    def test_an_unreachable_server_is_not_mistaken_for_an_old_one(self):
        with patch.object(supervisor, "read_json", return_value=None) as read:
            self.assertIsNone(supervisor.read_status(8766))
        read.assert_called_once_with(8766, "/status", 5.0)



class TheSuiteReadsASandboxNotThisMachine(unittest.TestCase):
    """⚠ The guard on `setUpModule` — see `test_ladder_watchdog.py` for why."""

    def test_the_halt_marker_and_the_game_lock_live_in_the_sandbox(self):
        sandbox = Path(_SANDBOX.name)
        for name, path in (("OPERATOR_HALT", supervisor.gamelock.OPERATOR_HALT),
                           ("LOCK", supervisor.gamelock.LOCK)):
            self.assertTrue(
                path.is_relative_to(sandbox),
                f"gamelock.{name} is {path}, outside the test sandbox: this suite "
                f"is reading the machine it runs on. See setUpModule.")
        self.assertEqual(os.environ.get("CIVVIS_OPERATOR_HALT_FILE"),
                         str(supervisor.gamelock.OPERATOR_HALT),
                         "a subprocess a test spawns must inherit the same sandbox")
        self.assertTrue(
            supervisor.gamelock.OPERATOR_INTENT.is_relative_to(sandbox),
            "gamelock.OPERATOR_INTENT is outside the test sandbox")
        self.assertEqual(os.environ.get("CIVVIS_OPERATOR_INTENT_FILE"),
                         str(supervisor.gamelock.OPERATOR_INTENT),
                         "a subprocess a test spawns must inherit the intent sandbox")


if __name__ == "__main__":
    unittest.main()
