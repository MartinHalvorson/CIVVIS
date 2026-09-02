#!/usr/bin/env python3
"""Regression tests for the durable 5,000-completed-game rotation state."""
from __future__ import annotations

import datetime as dt
import json
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

TOOLS = Path(__file__).resolve().parent
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

import continuous_batch_scheduler as scheduler


def complete_rows(path: Path, *, seed_first: int, target_games: int, complete_games: int) -> None:
    """Write a tiny all-seats ledger; six physical rows represent one game."""
    lines = [
        {
            "kind": "header",
            "all_seats": True,
            "design": "independent",
            "players": 6,
            "batch": {
                "seed_first": seed_first,
                "seed_last": seed_first + target_games - 1,
                "target_games": target_games,
                "target_seats": target_games * 6,
            },
        }
    ]
    for game in range(complete_games):
        seed = seed_first + game
        for seat in range(6):
            lines.append({
                "kind": "game",
                "seed": seed,
                "arm": 0,
                "game": game,
                "seat": seat,
                "win": seat == 0,
                "winner": 0,
            })
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row) + "\n" for row in lines), encoding="utf-8")


class Reservations(unittest.TestCase):
    def test_worker_cap_is_eighty_five_percent_floor(self):
        self.assertEqual(scheduler.workers_for_cores(18), 15)
        self.assertEqual(scheduler.workers_for_cores(1), 1)
        self.assertEqual(scheduler.workers_for_cores(10), 8)

    def test_interrupted_segment_never_reuses_its_unplayed_tail(self):
        state = scheduler.new_state(171_011_669, 5_000)
        first = scheduler.reserve_segment(state, scheduler.empty_status())
        self.assertEqual((first["seed_first"], first["seed_last"]), (171_011_669, 171_016_668))
        # Imagine the process stopped after 746 games. The next target is the
        # remaining *completed-game* difference, but must start after all 5k
        # seeds already reserved for the interrupted process.
        partial = {"complete_games": 746}
        second = scheduler.reserve_segment(state, partial)
        self.assertEqual(second["seed_first"], 171_016_669)
        self.assertEqual(second["target_games"], 4_254)
        self.assertEqual(state["next_seed"], second["seed_last"] + 1)

    def test_state_rejects_a_next_seed_inside_a_reserved_window(self):
        state = scheduler.new_state(10, 5)
        scheduler.reserve_segment(state, scheduler.empty_status())
        state["next_seed"] = 11
        with self.assertRaisesRegex(scheduler.SchedulerError, "reuses a reserved seed"):
            scheduler.validate_state(state)


class SourcePreparation(unittest.TestCase):
    def test_pinned_gene_screen_build_enables_developer_tools(self):
        """The scheduler must build the feature-gated tournament binary."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            commit = "a" * 40
            source_dir = root / "sources" / commit
            (source_dir / ".git").mkdir(parents=True)
            binary = source_dir / "target" / "release" / "gene_screen"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"test binary")
            commands: list[list[str]] = []

            def git_output(repo: Path, *args: str) -> str:
                if args == ("rev-parse", "origin/main"):
                    return commit
                if args == ("rev-parse", "HEAD"):
                    self.assertEqual(repo, source_dir)
                    return commit
                if args == ("status", "--porcelain"):
                    self.assertEqual(repo, source_dir)
                    return ""
                self.fail(f"unexpected git command: {args}")

            def run_checked(command, **_kwargs):
                commands.append(list(command))
                return SimpleNamespace(stdout="")

            with mock.patch.object(scheduler, "git_output", side_effect=git_output), \
                    mock.patch.object(scheduler, "run_checked", side_effect=run_checked), \
                    mock.patch.object(scheduler, "sha256", return_value="b" * 64):
                source = scheduler.ensure_source(root, scheduler.new_state(100, 1), root / "repo")

            self.assertEqual(source["commit"], commit)
            self.assertIn(
                [
                    "cargo", "build", "--release", "--locked", "--features",
                    "developer-tools", "--bin", "gene_screen",
                ],
                commands,
            )


class ValidatedRows(unittest.TestCase):
    def test_scheduler_counts_one_game_from_six_seat_rows(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(500, 1)
            batch = state["current"]
            reservation = scheduler.reserve_segment(state, scheduler.empty_status())
            complete_rows(root / batch["rows"], seed_first=reservation["seed_first"],
                          target_games=1, complete_games=1)
            status = scheduler.refresh_status(root, state)
            self.assertEqual(status["complete_games"], 1)
            self.assertEqual(status["complete_seats"], 6)
            self.assertEqual(status["records"], 7)
            self.assertEqual(batch["wins"], 1)

    def test_rows_from_an_unreserved_header_fail_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(900, 1)
            batch = state["current"]
            complete_rows(root / batch["rows"], seed_first=901, target_games=1, complete_games=1)
            with self.assertRaisesRegex(scheduler.SchedulerError, "outside this scheduler state"):
                scheduler.refresh_status(root, state)

    def test_non_six_player_header_cannot_enter_a_standard_rotation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(1_000, 1)
            batch = state["current"]
            reservation = scheduler.reserve_segment(state, scheduler.empty_status())
            row_path = root / batch["rows"]
            complete_rows(row_path, seed_first=reservation["seed_first"], target_games=1, complete_games=1)
            rows = [json.loads(line) for line in row_path.read_text().splitlines()]
            rows[0]["players"] = 5
            row_path.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")
            with self.assertRaisesRegex(scheduler.SchedulerError, "not safe to count"):
                scheduler.refresh_status(root, state)

    def test_publication_rechecks_the_frozen_analysis_before_any_pr_side_effect(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(2_000, 1)
            batch = state["current"]
            reservation = scheduler.reserve_segment(state, scheduler.empty_status())
            complete_rows(root / batch["rows"], seed_first=reservation["seed_first"],
                          target_games=1, complete_games=1)
            scheduler.refresh_status(root, state)
            batch["phase"] = "frozen"
            (root / batch["analysis"]).write_text(json.dumps({
                "kind": "gene_screen_analysis",
                "games": 2,
                "seats": 6,
                "batch": {
                    "complete_games": 1,
                    "complete_seats": 6,
                    "target_games": 1,
                    "target_seats": 6,
                },
            }), encoding="utf-8")
            with self.assertRaisesRegex(scheduler.SchedulerError, "no longer matches"):
                scheduler.publish_batch(
                    root, root / "scheduler-state.json", state, repo=root,
                    machine="test-machine", agent="continuous-batch")


class PublicationRecovery(unittest.TestCase):
    def pushed_publication(self, root: Path) -> tuple[dict, dict, Path]:
        state = scheduler.new_state(2_000, 1)
        batch = state["current"]
        worktree = root / "publication-worktree"
        worktree.mkdir()
        batch.update({
            "phase": "publishing",
            "complete_games": 1,
            "complete_seats": 6,
            "wins": 1,
            "publication": {
                "stage": "pushed",
                "worktree": str(worktree),
                "pr_number": 123,
                "report": "docs/gene_screens/example.json",
            },
        })
        return state, batch, worktree

    def test_already_merged_publication_is_persisted_without_shipping_again(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, batch, _worktree = self.pushed_publication(root)

            def run(command, **_kwargs):
                self.assertEqual(command[:3], ["gh", "pr", "view"])
                return SimpleNamespace(stdout=json.dumps({
                    "state": "MERGED",
                    "mergedAt": "2026-08-25T13:35:00Z",
                    "mergeCommit": {"oid": "a" * 40},
                }))

            with mock.patch.object(scheduler, "refresh_status", return_value={"complete_games": 1}), \
                    mock.patch.object(scheduler, "validate_analysis"), \
                    mock.patch.object(scheduler, "run_checked", side_effect=run):
                scheduler.publish_batch(
                    root, root / "scheduler-state.json", state, repo=root,
                    machine="test-machine", agent="continuous-batch")

            self.assertEqual(batch["phase"], "published")
            self.assertEqual(batch["publication"]["stage"], "merged")
            self.assertEqual(batch["publication"]["merged_at"], "2026-08-25T13:35:00Z")
            self.assertEqual(batch["publication"]["merge_commit"], "a" * 40)
            persisted = json.loads((root / "scheduler-state.json").read_text(encoding="utf-8"))
            self.assertEqual(persisted["current"]["publication"]["stage"], "merged")

    def test_claimed_publication_merged_by_an_operator_skips_regeneration(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, batch, _worktree = self.pushed_publication(root)
            batch["publication"]["stage"] = "claimed"

            def run(command, **_kwargs):
                self.assertEqual(command[:3], ["gh", "pr", "view"])
                return SimpleNamespace(stdout=json.dumps({
                    "state": "MERGED",
                    "mergedAt": "2026-08-25T13:37:00Z",
                    "mergeCommit": {"oid": "c" * 40},
                }))

            with mock.patch.object(scheduler, "refresh_status", return_value={"complete_games": 1}), \
                    mock.patch.object(scheduler, "validate_analysis"), \
                    mock.patch.object(scheduler, "run_checked", side_effect=run):
                scheduler.publish_batch(
                    root, root / "scheduler-state.json", state, repo=root,
                    machine="test-machine", agent="continuous-batch")

            self.assertEqual(batch["phase"], "published")
            self.assertEqual(batch["publication"]["stage"], "merged")
            self.assertEqual(batch["publication"]["merge_commit"], "c" * 40)

    def test_merged_publication_recovers_after_ship_removed_its_worktree(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, batch, worktree = self.pushed_publication(root)
            batch["publication"]["stage"] = "prepared"
            worktree.rmdir()
            queried_from = []

            def run(command, **kwargs):
                self.assertEqual(command[:3], ["gh", "pr", "view"])
                queried_from.append(kwargs["cwd"])
                return SimpleNamespace(stdout=json.dumps({
                    "state": "MERGED",
                    "mergedAt": "2026-08-25T13:38:00Z",
                    "mergeCommit": {"oid": "d" * 40},
                }))

            with mock.patch.object(scheduler, "refresh_status", return_value={"complete_games": 1}), \
                    mock.patch.object(scheduler, "validate_analysis"), \
                    mock.patch.object(scheduler, "run_checked", side_effect=run):
                scheduler.publish_batch(
                    root, root / "scheduler-state.json", state, repo=root,
                    machine="test-machine", agent="continuous-batch")

            self.assertEqual(queried_from, [scheduler.ROOT])
            self.assertEqual(batch["phase"], "published")
            self.assertEqual(batch["publication"]["stage"], "merged")
            self.assertEqual(batch["publication"]["merge_commit"], "d" * 40)

    def test_merge_race_after_lookup_is_recovered_before_propagating_ship_error(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, batch, _worktree = self.pushed_publication(root)
            views = iter((
                {"state": "OPEN", "mergedAt": None, "mergeCommit": None},
                {
                    "state": "MERGED",
                    "mergedAt": "2026-08-25T13:36:00Z",
                    "mergeCommit": {"oid": "b" * 40},
                },
            ))

            def run(command, **_kwargs):
                if command[:3] == ["gh", "pr", "view"]:
                    return SimpleNamespace(stdout=json.dumps(next(views)))
                self.assertEqual(command, [sys.executable, "tools/civvis_collab.py", "ship"])
                raise scheduler.SchedulerError("ship publication PR failed: PR is already merged")

            with mock.patch.object(scheduler, "refresh_status", return_value={"complete_games": 1}), \
                    mock.patch.object(scheduler, "validate_analysis"), \
                    mock.patch.object(scheduler, "run_checked", side_effect=run):
                scheduler.publish_batch(
                    root, root / "scheduler-state.json", state, repo=root,
                    machine="test-machine", agent="continuous-batch")

            self.assertEqual(batch["phase"], "published")
            self.assertEqual(batch["publication"]["stage"], "merged")
            self.assertEqual(batch["publication"]["merge_commit"], "b" * 40)


class PublicationMetadata(unittest.TestCase):
    def test_artifact_name_is_unique_to_the_batch_and_uses_seats(self):
        batch = scheduler.new_batch(1, 5_000, ident="rotation-a")
        batch["complete_seats"] = 30_000
        name = scheduler.reporting_filename(batch)
        self.assertIn("30000-total-seats", name)
        self.assertIn("rotation-a", name)

    def test_body_names_games_seats_and_the_pinned_binary(self):
        batch = scheduler.new_batch(1, 5_000, ident="rotation-b")
        batch.update({
            "complete_games": 5_000,
            "complete_seats": 30_000,
            "wins": 5_000,
            "source": {
                "commit": "a" * 40,
                "binary_sha256": "b" * 64,
            },
        })
        body = scheduler.publication_body(
            batch, "docs/gene_screens/example.json", machine="test-machine",
            agent="continuous-batch", coordinated="#1234", computer="Test Mac")
        self.assertIn("5,000 validated completed games / 30,000 seats / 5,000 wins", body)
        self.assertIn("changes no game mechanics", body)
        self.assertIn("reselects the default genome", body)
        self.assertIn("Computer: `Test Mac`", body)
        self.assertIn("Coordinated with: #1234", body)
        self.assertIn("publish validated 5,000-completed-game", body)
        self.assertIn("overwrite-guard: allow this report deliberately regenerates", body)

    def test_every_publication_reselects_defaults_from_its_latest_three_reports(self):
        self.assertEqual(
            scheduler.reporting_write_command("docs/gene_screens/example.json"),
            [
                sys.executable, "tools/genes.py", "write",
                "--reselect-deployment-defaults",
                "--reporting-batch",
                "docs/gene_screens/example.json",
            ],
        )

    def test_the_guard_knows_every_path_genes_py_write_records(self):
        """⭐ The allowed set is DERIVED from the writers, not restated here.

        `PUBLICATION_GENERATED_FILES` is deliberately explicit — a new
        generated artifact should be reviewed into it rather than swept into a
        publication. But the list lives in this file and the writing lives in
        `tools/genes.py`, so when #2584 taught `genes.py write` to record
        `tools/genome_cost_floor.json` the two drifted, and the fail-closed
        guard refused every continuous publication on every machine until
        someone noticed. Restating the tuple in a test cannot catch that: it is
        the same hand-maintenance a second time.

        This asks the writers what they write. `genes.py write` ends by writing
        these four paths and calling `genome_cost.record`, which writes its own;
        a sixth output added as a module path constant fails here instead of in
        a stalled tournament.
        """
        import genes
        import genome_cost

        recorded = {
            genes.LEDGER_JSON,
            genes.RANKING_MD,
            genes.EVIDENCE_MD,
            genes.REGISTRY_PATH,
            genome_cost.RECORD_JSON,
        }
        relative = {
            str(path.resolve().relative_to(genes.ROOT.resolve()))
            for path in recorded
        }
        self.assertEqual(
            relative,
            set(scheduler.PUBLICATION_GENERATED_FILES),
            "every path `genes.py write` records must be allowed through the "
            "publication guard, and nothing else",
        )

    def test_publication_claim_and_guard_include_every_generated_ranking_artifact(self):
        self.assertEqual(
            scheduler.PUBLICATION_GENERATED_FILES,
            (
                "docs/gene_ledger.json",
                "GENE_HEURISTIC_RANKING.md",
                "docs/GENE_RANKING_EVIDENCE.md",
                "src/ai/advanced/genes.rs",
                "tools/genome_cost_floor.json",
            ),
        )
        batch = scheduler.new_batch(1, 1, ident="publication-artifacts")
        batch.update({
            "complete_games": 1,
            "complete_seats": 6,
            "wins": 1,
            "source": {"commit": "a" * 40, "binary_sha256": "b" * 64},
        })
        body = scheduler.publication_body(
            batch, "docs/gene_screens/example.json", machine="test-machine",
            agent="continuous-batch", coordinated="none", computer="Test Mac")
        self.assertIn("`docs/GENE_RANKING_EVIDENCE.md`", body)
        self.assertIn("`src/ai/advanced/genes.rs`", body)


class PublicationTiming(unittest.TestCase):
    def test_freeze_persists_the_completion_timestamp_before_analyzer_work(self):
        """A failed analyzer retry must not inflate the batch's reported rate."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(3_000, 1)
            batch = state["current"]
            batch["created_at"] = "2026-08-25T10:00:00Z"
            reservation = scheduler.reserve_segment(state, scheduler.empty_status())
            complete_rows(root / batch["rows"], seed_first=reservation["seed_first"],
                          target_games=1, complete_games=1)
            batch["source"] = {"binary": "/not-run", "worktree": str(root)}
            state_path = root / "scheduler-state.json"

            with mock.patch.object(scheduler, "utc_now", return_value="2026-08-25T10:01:00Z"), \
                    mock.patch.object(scheduler, "run_checked",
                                      side_effect=scheduler.SchedulerError("analyzer failed")):
                with self.assertRaisesRegex(scheduler.SchedulerError, "analyzer failed"):
                    scheduler.freeze_analysis(root, state, state_pathname=state_path)

            self.assertEqual(batch["completed_at"], "2026-08-25T10:01:00Z")
            persisted = json.loads(state_path.read_text(encoding="utf-8"))
            self.assertEqual(persisted["current"]["completed_at"], "2026-08-25T10:01:00Z")

    def test_reporting_artifact_carries_exact_whole_batch_rate_inputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "analysis.json"
            target = root / "report.json"
            source.write_text(json.dumps({
                "kind": "gene_screen_analysis", "games": 3_000, "seats": 18_000,
            }), encoding="utf-8")
            batch = scheduler.new_batch(1, 3_000, ident="timed")
            batch.update({
                "complete_games": 3_000,
                "created_at": "2026-08-25T10:00:00Z",
                "completed_at": "2026-08-25T10:25:00Z",
            })

            scheduler.write_reporting_artifact(source, target, batch)

            self.assertNotIn("continuous_batch_timing",
                             json.loads(source.read_text(encoding="utf-8")))
            self.assertEqual(json.loads(target.read_text(encoding="utf-8"))[
                "continuous_batch_timing"], {
                    "schema": "continuous_batch_timing/v1",
                    "started_at": "2026-08-25T10:00:00Z",
                    "completed_at": "2026-08-25T10:25:00Z",
                    "elapsed_seconds": 1_500,
                    "completed_games": 3_000,
                })


class DeadlineRotation(unittest.TestCase):
    DEADLINE = scheduler.parse_utc_timestamp("2026-08-26T12:00:00Z", name="test deadline")

    def test_new_deadline_state_persists_the_deadline_and_successor_goal(self):
        state = scheduler.new_state(10, 1_000_000, deadline_at=self.DEADLINE, next_goal_games=3_000)
        deadline = state["current"]["deadline"]
        self.assertEqual(deadline, {
            "schema": scheduler.CONTINUOUS_BATCH_DEADLINE_SCHEMA,
            "deadline_at": "2026-08-26T12:00:00Z",
            "next_goal_completed_games": 3_000,
        })
        scheduler.validate_state(state)

    def test_deadline_snapshot_discards_only_a_terminal_partial_game_group(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            raw = root / "rows-continuous.jsonl"
            target = root / "rows-deadline-cutoff.jsonl"
            complete_rows(raw, seed_first=700, target_games=100, complete_games=2)
            raw_bytes = raw.read_bytes()
            # The process can be interrupted between two rows of its final
            # six-seat write.  The raw ledger stays intact for audit.
            raw.write_bytes(raw_bytes + b'{"kind":"game","seed":702,"arm":0,"seat":0')

            status, dropped = scheduler.snapshot_complete_prefix(raw, target)

            self.assertEqual(status["complete_games"], 2)
            self.assertEqual(status["complete_seats"], 12)
            self.assertEqual(dropped, 1)
            self.assertEqual(raw.read_bytes(), raw_bytes + b'{"kind":"game","seed":702,"arm":0,"seat":0')
            self.assertEqual(scheduler.summarize(target)["complete_games"], 2)

    def test_sealed_deadline_uses_actual_games_but_keeps_raw_target_auditable(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(900, 100, deadline_at=self.DEADLINE, next_goal_games=3_000)
            batch = state["current"]
            reservation = scheduler.reserve_segment(state, scheduler.empty_status())
            raw = root / batch["rows"]
            complete_rows(raw, seed_first=reservation["seed_first"], target_games=100, complete_games=2)
            raw_bytes = raw.read_bytes()
            raw.write_bytes(raw_bytes + b'{"kind":"game","seed":902,"arm":0,"seat":0')
            state_path = root / "scheduler-state.json"

            with mock.patch.object(scheduler, "freeze_analysis") as freeze:
                scheduler.seal_deadline_cutoff(
                    root, state_path, state, stopped_at="2026-08-26T12:00:00Z")

            self.assertEqual(state["goal_completed_games"], 2)
            self.assertEqual(batch["goal_completed_games"], 2)
            self.assertEqual(batch["raw_rows"], batch["directory"] + "/rows-continuous.jsonl")
            self.assertEqual(batch["deadline"]["original_goal_completed_games"], 100)
            self.assertEqual(batch["deadline"]["actual_completed_games"], 2)
            self.assertEqual(batch["deadline"]["dropped_trailing_records"], 1)
            self.assertEqual(scheduler.refresh_status(root, state)["complete_games"], 2)
            self.assertTrue(freeze.called)

    def test_rotation_switches_to_the_deadline_successor_goal_once_published(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(1_100, 100, deadline_at=self.DEADLINE, next_goal_games=3_000)
            batch = state["current"]
            state["goal_completed_games"] = 2
            batch.update({"goal_completed_games": 2, "phase": "published", "complete_games": 2,
                          "complete_seats": 12, "wins": 2, "source": {"commit": "a"},
                          "publication": {"stage": "merged"}})
            batch["deadline"].update({
                "cutoff_at": "2026-08-26T12:00:00Z",
                "original_goal_completed_games": 100,
                "actual_completed_games": 2,
                "raw_rows": batch["rows"],
                "raw_rows_sha256": "a" * 64,
                "frozen_rows": batch["rows"],
                "dropped_trailing_records": 0,
            })

            scheduler.rotate(root / "scheduler-state.json", state)

            self.assertEqual(state["goal_completed_games"], 3_000)
            self.assertEqual(state["current"]["goal_completed_games"], 3_000)
            self.assertNotIn("deadline", state["current"])
            self.assertEqual(state["history"][0]["deadline"]["actual_completed_games"], 2)

    def test_tick_seals_an_already_due_deadline_before_reading_live_rows(self):
        state = scheduler.new_state(1_200, 100, deadline_at=self.DEADLINE, next_goal_games=3_000)
        with mock.patch.object(scheduler, "seal_deadline_cutoff") as seal, \
                mock.patch.object(scheduler.dt, "datetime", wraps=scheduler.dt.datetime) as clock:
            clock.now.return_value = scheduler.parse_utc_timestamp(
                "2026-08-26T12:00:01Z", name="after deadline")
            self.assertEqual(scheduler.tick(
                Path("/state"), Path("/state/scheduler-state.json"), state,
                repo=Path("/repo"), jobs=1, machine="machine", agent="agent", publish=True),
                "frozen_deadline")
        self.assertTrue(seal.called)

    def test_restarted_scheduler_waits_for_its_persisted_live_child(self):
        state = scheduler.new_state(1_300, 10)
        reservation = scheduler.reserve_segment(state, scheduler.empty_status())
        reservation.update({"launch_state": "running", "pid": 4242, "process_group": 4242})
        with mock.patch.object(scheduler, "process_is_alive", return_value=True), \
                mock.patch.object(scheduler, "refresh_status") as status:
            outcome = scheduler.tick(
                Path("/state"), Path("/state/scheduler-state.json"), state,
                repo=Path("/repo"), jobs=1, machine="machine", agent="agent", publish=True)
        self.assertEqual(outcome, "active_segment")
        status.assert_not_called()


class OperatorCommands(unittest.TestCase):
    """cut / status / publish never need the daemon's lock, a restart, or a hand-edited state."""

    DEADLINE = scheduler.parse_utc_timestamp("2026-08-26T12:00:00Z", name="test deadline")

    @staticmethod
    def running_state(root: Path, *, seed: int = 2_000, goal: int = 100):
        state = scheduler.new_state(seed, goal)
        scheduler.reserve_segment(state, scheduler.empty_status())
        path = root / "scheduler-state.json"
        scheduler.atomic_json(path, state)
        return state, path

    @staticmethod
    def consumed_cut_deadline(batch: dict) -> None:
        batch["deadline"] = {
            "schema": scheduler.CONTINUOUS_BATCH_DEADLINE_SCHEMA,
            "deadline_at": "2026-08-26T12:00:00Z",
            "next_goal_completed_games": 100,
            "requested_via": scheduler.DEADLINE_REQUESTED_VIA_CUT,
            "cutoff_at": "2026-08-26T12:00:03Z",
            "original_goal_completed_games": 100,
            "actual_completed_games": 2,
            "raw_rows": batch["rows"],
            "raw_rows_sha256": "a" * 64,
            "frozen_rows": batch["rows"],
            "dropped_trailing_records": 0,
        }
        batch["raw_rows"] = batch["rows"]

    def test_cut_request_becomes_the_running_batch_deadline(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, path = self.running_state(root)
            request = scheduler.write_cut_request(root, deadline_at=self.DEADLINE, note="table refresh")
            with self.assertRaises(scheduler.SchedulerError):
                scheduler.write_cut_request(root, deadline_at=self.DEADLINE)

            self.assertEqual(scheduler.adopt_cut_request(root, path, state), self.DEADLINE)

            deadline = state["current"]["deadline"]
            self.assertEqual(deadline["schema"], scheduler.CONTINUOUS_BATCH_DEADLINE_SCHEMA)
            self.assertEqual(deadline["deadline_at"], "2026-08-26T12:00:00Z")
            self.assertEqual(deadline["next_goal_completed_games"], 100)
            self.assertEqual(deadline["requested_via"], scheduler.DEADLINE_REQUESTED_VIA_CUT)
            self.assertEqual(deadline["note"], "table refresh")
            scheduler.validate_state(state)
            self.assertEqual(scheduler.batch_deadline(state["current"]), self.DEADLINE)
            # Durable before the request file moves into the batch's audit directory.
            persisted = json.loads(path.read_text(encoding="utf-8"))
            self.assertEqual(persisted["current"]["deadline"]["deadline_at"], "2026-08-26T12:00:00Z")
            self.assertFalse(request.exists())
            self.assertTrue((root / state["current"]["directory"] / scheduler.CUT_REQUEST_NAME).is_file())
            self.assertIsNone(scheduler.adopt_cut_request(root, path, state))

    def test_cut_request_that_cannot_apply_is_set_aside_with_its_reason(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, path = self.running_state(root)
            state["current"]["phase"] = "frozen"
            scheduler.write_cut_request(root, deadline_at=self.DEADLINE)

            self.assertIsNone(scheduler.adopt_cut_request(root, path, state))

            self.assertNotIn("deadline", state["current"])
            self.assertFalse(scheduler.cut_request_path(root).exists())
            rejected = sorted(root.glob("cut-request.rejected-*.json"))
            self.assertEqual(len(rejected), 1)
            record = json.loads(rejected[0].read_text(encoding="utf-8"))
            self.assertIn("not running", record["rejected"])
            self.assertIn(scheduler.CUT_REQUEST_SCHEMA, record["request"])

            state["current"]["phase"] = "running"
            state["current"]["deadline"] = {
                "schema": scheduler.CONTINUOUS_BATCH_DEADLINE_SCHEMA,
                "deadline_at": "2026-08-26T13:00:00Z",
                "next_goal_completed_games": 100,
            }
            scheduler.write_cut_request(root, deadline_at=self.DEADLINE)
            self.assertIsNone(scheduler.adopt_cut_request(root, path, state))
            self.assertEqual(state["current"]["deadline"]["deadline_at"], "2026-08-26T13:00:00Z")
            self.assertEqual(len(list(root.glob("cut-request.rejected-*.json"))), 2)

    def test_restart_without_deadline_flags_keeps_a_cut_deadline(self):
        state = scheduler.new_state(2_100, 100)
        state["current"]["deadline"] = {
            "schema": scheduler.CONTINUOUS_BATCH_DEADLINE_SCHEMA,
            "deadline_at": "2026-08-26T12:00:00Z",
            "next_goal_completed_games": 100,
            "requested_via": scheduler.DEADLINE_REQUESTED_VIA_CUT,
        }
        scheduler.verify_deadline_invocation(state, deadline_at=None, next_goal_games=None)
        del state["current"]["deadline"]["requested_via"]
        with self.assertRaises(scheduler.SchedulerError):
            scheduler.verify_deadline_invocation(state, deadline_at=None, next_goal_games=None)

    def test_unchanged_service_arguments_load_a_state_frozen_by_a_cut(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, path = self.running_state(root, seed=2_200)
            batch = state["current"]
            self.consumed_cut_deadline(batch)
            batch.update({"phase": "frozen", "goal_completed_games": 2, "complete_games": 2,
                          "complete_seats": 12, "wins": 2, "frozen_at": "2026-08-26T12:00:03Z"})
            state["goal_completed_games"] = 2
            scheduler.validate_state(state)
            scheduler.atomic_json(path, state)

            loaded = scheduler.load_state(path, seed_floor=None, goal_games=100)

            self.assertEqual(loaded["goal_completed_games"], 2)
            with self.assertRaises(scheduler.SchedulerError):
                scheduler.load_state(path, seed_floor=None, goal_games=3_000)

    def test_status_reads_while_another_process_holds_the_lock(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.running_state(root)
            with (root / "scheduler.lock").open("a+") as lock:
                scheduler.fcntl.flock(lock.fileno(), scheduler.fcntl.LOCK_EX | scheduler.fcntl.LOCK_NB)
                import io
                from contextlib import redirect_stdout
                buffer = io.StringIO()
                with redirect_stdout(buffer):
                    code = scheduler.main(["status", "--state-dir", str(root), "--json"])
            self.assertEqual(code, 0)
            report = json.loads(buffer.getvalue())
            self.assertEqual(report["schema"], "continuous_batch_status/v1")
            self.assertEqual(report["phase"], "running")
            self.assertTrue(report["scheduler_running"])
            self.assertEqual(report["remaining_games"], 100)
            self.assertIsNone(report["games_per_hour"])
            self.assertFalse(report["cut_request_pending"])

    def test_cut_command_only_targets_a_running_batch(self):
        import io
        from contextlib import redirect_stderr, redirect_stdout
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, path = self.running_state(root)
            with redirect_stdout(io.StringIO()):
                self.assertEqual(scheduler.main(["cut", "--state-dir", str(root), "--in", "0"]), 0)
            request = json.loads(scheduler.cut_request_path(root).read_text(encoding="utf-8"))
            self.assertEqual(request["schema"], scheduler.CUT_REQUEST_SCHEMA)
            scheduler.parse_utc_timestamp(request["deadline_at"], name="request deadline")
            scheduler.cut_request_path(root).unlink()

            state["current"]["phase"] = "frozen"
            scheduler.atomic_json(path, state)
            errors = io.StringIO()
            with redirect_stderr(errors):
                self.assertEqual(scheduler.main(["cut", "--state-dir", str(root)]), 2)
            self.assertIn("only a running batch can be cut", errors.getvalue())
            self.assertFalse(scheduler.cut_request_path(root).exists())

    def test_publish_command_refuses_a_running_batch(self):
        import io
        from contextlib import redirect_stderr
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.running_state(root)
            errors = io.StringIO()
            with redirect_stderr(errors):
                code = scheduler.main([
                    "publish", "--state-dir", str(root), "--repo", str(scheduler.ROOT),
                    "--goal-games", "100"])
            self.assertEqual(code, 2)
            self.assertIn("publish applies to a frozen batch", errors.getvalue())

    def test_run_segment_adopts_a_cut_request_and_stops_its_child(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, path = self.running_state(root, seed=2_300)
            batch = state["current"]
            binary = root / "fake-gene-screen"
            binary.write_text("#!/bin/sh\nexec sleep 30\n", encoding="utf-8")
            binary.chmod(0o755)
            batch["source"] = {"binary": str(binary), "worktree": str(root), "commit": "a"}
            reservation = batch["reservations"][0]
            scheduler.write_cut_request(root, deadline_at=dt.datetime.now(dt.timezone.utc))

            result = scheduler.run_segment(root, path, state, batch, reservation, jobs=1)

            self.assertTrue(result.stopped_at_deadline)
            self.assertIsNotNone(reservation["deadline_stopped_at"])
            self.assertEqual(reservation["launch_state"], "finished")
            self.assertEqual(reservation["complete_games_at_start"], 0)
            self.assertEqual(batch["deadline"]["requested_via"], scheduler.DEADLINE_REQUESTED_VIA_CUT)
            self.assertIsNotNone(scheduler.batch_deadline(batch))
            self.assertTrue((root / batch["directory"] / scheduler.CUT_REQUEST_NAME).is_file())


    def test_repeated_tick_outcomes_are_logged_once_with_their_count(self):
        log = scheduler.OutcomeLog()
        self.assertTrue(log.note("active_segment").endswith(" active_segment"))
        self.assertIsNone(log.note("active_segment"))
        self.assertIsNone(log.note("active_segment"))
        self.assertTrue(log.note("frozen_deadline").endswith(
            " frozen_deadline (previous outcome repeated 2 more times)"))
        self.assertTrue(log.note("awaiting_publication").endswith(" awaiting_publication"))


if __name__ == "__main__":
    unittest.main()
