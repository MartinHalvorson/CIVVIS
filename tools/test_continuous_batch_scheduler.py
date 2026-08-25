#!/usr/bin/env python3
"""Regression tests for the durable 5,000-completed-game rotation state."""
from __future__ import annotations

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
        self.assertIn("changes no game rules or default genes", body)
        self.assertIn("Computer: `Test Mac`", body)
        self.assertIn("Coordinated with: #1234", body)

    def test_publication_uses_the_renamed_generated_ranking_everywhere(self):
        """A rename must move the generator, task claim, and `git add` target.

        The old path made a finished continuous batch open a claim for a file
        `genes.py write` no longer touched, then attempt to stage that missing
        file.  Keeping the filename in one constant lets this small contract
        catch the next path move before a five-thousand-game rotation finishes.
        """
        self.assertEqual(scheduler.GENE_RANKING, "GENE_HEURISTIC_RANKING.md")
        source = (TOOLS / "continuous_batch_scheduler.py").read_text(encoding="utf-8")
        self.assertNotIn('"HEURISTIC_GENE_RANKING.md"', source)
        self.assertIn('"--path", GENE_RANKING', source)
        self.assertIn('["docs/gene_ledger.json", GENE_RANKING]', source)
        self.assertIn('"docs/gene_ledger.json", GENE_RANKING]', source)


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


if __name__ == "__main__":
    unittest.main()
