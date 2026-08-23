#!/usr/bin/env python3
"""The paired harness refuses the conclusions the prose kept having to warn about."""

from __future__ import annotations

import datetime as dt
import json
import re
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import speed_ab  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
WORKFLOWS = REPO / ".github" / "workflows"


def workflow_text() -> str:
    """Every workflow, with comments stripped.

    Stripped for the same reason `test_ci_wiring.py` strips them: this file's
    own first draft matched `- 'tools/speed_ab.py'` inside a `paths:` filter
    and called that "wired".
    """
    return "\n".join(
        "\n".join(line.partition("#")[0]
                  for line in path.read_text(encoding="utf-8").splitlines())
        for path in sorted(WORKFLOWS.glob("*.yml")))


def gate_invocation() -> list[str]:
    """The argv `speed.yml` actually runs the harness with."""
    found = re.search(r"python3 tools/speed_ab\.py(?P<args>(?:[^\n]*\\\n)*[^\n]*)",
                      workflow_text())
    if not found:
        return []
    return found.group("args").replace("\\\n", " ").split()


def gate_flags() -> dict[str, str]:
    """`--flag value` pairs from that argv, so a shape can be compared as data."""
    argv = gate_invocation()
    return {argv[i].lstrip("-"): argv[i + 1]
            for i in range(len(argv) - 1)
            if argv[i].startswith("--") and not argv[i + 1].startswith("--")}


def a_run(cpu: float, turns: int, digest: str = "same"):
    return (cpu, digest, turns)


def a_pair(seed: int, base_cpu: float, cand_cpu: float, base_turns: int,
           cand_turns: int | None = None, agree: bool = True) -> dict:
    cand_turns = base_turns if cand_turns is None else cand_turns
    return {
        "seed": seed, "interleave": 0,
        "baseline": {"cpu": base_cpu, "turns": base_turns},
        "candidate": {"cpu": cand_cpu, "turns": cand_turns},
        "agree": agree,
        "delta_pct": speed_ab.pct(base_cpu / base_turns, cand_cpu / cand_turns),
    }


def a_result(pairs: list[dict]) -> dict:
    return speed_ab.summarise(pairs, [p["seed"] for p in pairs], 1,
                              {"start": 1.0, "peak": 1.0, "end": 1.0})


class TheReportIdentityCheck(unittest.TestCase):
    def test_the_timing_line_is_stripped_not_compared(self):
        """Two runs of the same game differ in elapsed time and nothing else."""
        a = "[12.5s] done\nRome score=17\nChina score=17"
        b = "[13.9s] done\nRome score=17\nChina score=17"
        self.assertEqual(speed_ab.report_digest(a), speed_ab.report_digest(b))

    def test_any_other_difference_is_a_different_game(self):
        a = "[12.5s] done\nRome score=17"
        b = "[12.5s] done\nRome score=18"
        self.assertNotEqual(speed_ab.report_digest(a), speed_ab.report_digest(b))


class TheTurnCountComesFromTheReport(unittest.TestCase):
    """The divisor of the primary metric, read from the game's own words.

    `standings()` in `src/main.rs` prints exactly one of three lines. All three
    are here because the metric has no meaning without one of them, and a
    fourth wording would make this harness silently wrong if it defaulted.
    """

    def test_a_victory_names_its_turn(self):
        self.assertEqual(speed_ab.turns_played(
            "[9.1s]\nWinner: Greece (player 2) by religious on turn 165\n"
            "  Rome       score=497  cities=5\n"), 165)

    def test_a_draw_names_its_turn(self):
        self.assertEqual(speed_ab.turns_played(
            "[9.1s]\nDraw: turn limit reached on turn 250\n"), 250)

    def test_an_undecided_game_names_its_turn(self):
        self.assertEqual(speed_ab.turns_played(
            "[9.1s]\nNo winner: turn 120 of 120, and no enabled victory was "
            "achieved\n"), 120)

    def test_a_report_it_cannot_read_is_a_hard_error(self):
        """⚠ Not a zero, and not a fallback to whole-game time.

        An engine wording change has to turn this red. Defaulting the divisor
        would quietly restore the mixture that `docs/SIMULATOR_PERFORMANCE.md`
        retracted a -48.7% reading over.
        """
        with self.assertRaises(SystemExit) as raised:
            speed_ab.turns_played("[9.1s]\nRome score=17\n")
        self.assertIn("no turn count", str(raised.exception))

    def test_the_score_victory_wording_is_read_too(self):
        """A game that reaches the clock announces a `score` victory, and the
        250-turn screen ends that way in about half its games."""
        self.assertEqual(speed_ab.turns_played(
            "[3.2s]\nWinner: Rome (player 0) by score on turn 40\n"), 40)


class TheArmsAlternate(unittest.TestCase):
    """Never all of A and then all of B: host load has to fall on both."""

    def _order(self, seeds, interleaves=1):
        order = []

        def fake(binary, seed, opts):
            order.append((seed, Path(binary).name))
            return a_run(1.0, 100)

        with mock.patch.object(speed_ab, "run_once", fake):
            speed_ab.compare(Path("/x/base"), Path("/x/cand"), seeds,
                             speed_ab.DEFAULTS, interleaves)
        return order

    def test_both_arms_run_on_every_seed(self):
        order = self._order([10, 11, 12])
        for seed in (10, 11, 12):
            arms = {name for s, name in order if s == seed}
            self.assertEqual(arms, {"base", "cand"}, f"seed {seed}: {order}")

    def test_the_order_flips_between_seeds(self):
        """A host drifting in one direction would otherwise always favour
        whichever arm runs second."""
        order = self._order([10, 11])
        first_of = {seed: [name for s, name in order if s == seed][0]
                    for seed in (10, 11)}
        self.assertNotEqual(first_of[10], first_of[11], order)

    def test_the_order_flips_between_interleaves_too(self):
        """Repeating a seed must not repeat its ordering, or the extra pairs
        only re-measure the same bias more precisely."""
        order = self._order([10], interleaves=2)
        self.assertEqual(len(order), 4, order)
        self.assertNotEqual(order[0][1], order[2][1], order)

    def test_more_interleaves_make_more_pairs(self):
        with mock.patch.object(speed_ab, "run_once",
                               lambda b, s, o: a_run(1.0, 100)):
            result = speed_ab.compare(Path("/x/base"), Path("/x/cand"),
                                      [10, 11, 12], speed_ab.DEFAULTS, 3)
        self.assertEqual(len(result["deltas"]), 9)
        self.assertEqual(result["interleaves"], 3)


class ThePrimaryMetricIsPerCompletedTurn(unittest.TestCase):
    """#2301: whole-game time is cost times length, and only one is overhead.

    `docs/SIMULATOR_PERFORMANCE.md` (2026-08-22) retracts a -48.7% reading of
    `precise_evacuation` because withholding it made games run 745 turns → 951.
    Per completed turn the same change is -33.3% on both revisions it was
    measured on.
    """

    def test_equal_turns_make_the_two_metrics_the_same_number(self):
        """The property a byte-identical optimization relies on. If these ever
        differ for equal turn totals, one of them is computed wrong."""
        result = a_result([a_pair(1, 100.0, 90.0, 250),
                           a_pair(2, 200.0, 180.0, 250)])
        self.assertAlmostEqual(result["per_turn_change_pct"],
                               result["change_pct"], places=9)
        self.assertEqual(result["length_change_pct"], 0.0)

    def test_a_length_change_separates_them(self):
        """The retracted reading, in miniature: the same cost per turn, over
        fewer turns, reads as a large whole-game saving and no saving at all."""
        result = a_result([a_pair(1, 100.0, 80.0, 1000, 800, agree=False)])
        self.assertAlmostEqual(result["per_turn_change_pct"], 0.0, places=9)
        self.assertAlmostEqual(result["change_pct"], -20.0, places=9)
        self.assertAlmostEqual(result["length_change_pct"], -20.0, places=9)

    def test_the_disagreement_is_printed_not_hidden(self):
        said = speed_ab.secondary(a_result(
            [a_pair(1, 100.0, 80.0, 1000, 800, agree=False)]))
        self.assertIn("DISAGREE", said)
        self.assertIn("LENGTH", said)
        self.assertIn("1000 turns", said)

    def test_agreement_is_printed_too(self):
        """Saying "these agree" is worth a line: it is what makes a
        byte-identical optimization readable at a glance."""
        said = speed_ab.secondary(a_result([a_pair(1, 100.0, 90.0, 250)]))
        self.assertIn("agree", said)
        self.assertIn("250 turns", said)


class TheGateStatisticSurvivesOneBadPair(unittest.TestCase):
    """A median, because the fleet's machines are not quiet.

    Measured on `mbp-m5-max-128` 2026-08-23 with twelve sibling agents
    building: the same binary against itself read 0.1134 s/turn at load 6 and
    0.1761 at load 94 — the absolute inflated 55% — while the interleaved
    paired delta stayed inside ±0.6%. What interleaving cannot cancel is a
    burst landing on one arm of one pair, and that is what a median can.
    """

    def _four_clean_and_one_contended(self):
        pairs = [a_pair(seed, 100.0, 100.0, 100) for seed in range(4)]
        pairs.append(a_pair(9, 100.0, 250.0, 100))
        return a_result(pairs)

    def test_one_contended_pair_does_not_move_the_median(self):
        result = self._four_clean_and_one_contended()
        self.assertAlmostEqual(result["median_pct"], 0.0, places=9)

    def test_it_does_move_the_pooled_ratio(self):
        """Which is why the pooled number is reported and not gated on."""
        result = self._four_clean_and_one_contended()
        self.assertGreater(result["per_turn_change_pct"], 25.0)

    def test_the_spread_shows_the_bad_pair(self):
        result = self._four_clean_and_one_contended()
        self.assertEqual(result["range_pct"][1], 150.0)
        self.assertGreater(result["resolution_pct"], 0.0)

    def test_a_uniform_regression_is_not_hidden_by_the_median(self):
        """The median is robust, not blind. Every pair slower is a regression
        and the statistic has to say so."""
        result = a_result([a_pair(seed, 100.0, 140.0, 100) for seed in range(5)])
        self.assertAlmostEqual(result["median_pct"], 40.0, places=9)
        self.assertTrue(speed_ab.over_budget(result, 8.0))


class TheRunReportsItsOwnConditions(unittest.TestCase):
    """A number without its conditions is not a measurement.

    That is already the perf ledger's rule for the simulator; this applies it
    to the instrument. Load average and the pair-to-pair spread go in the
    output and into every recorded row.
    """

    def test_the_load_average_is_carried_through(self):
        with mock.patch.object(speed_ab, "run_once",
                               lambda b, s, o: a_run(1.0, 100)), \
             mock.patch.object(speed_ab, "load_average", lambda: 7.5):
            result = speed_ab.compare(Path("/x/a"), Path("/x/b"), [1, 2],
                                      speed_ab.DEFAULTS)
        self.assertEqual(result["load"]["peak"], 7.5)
        self.assertEqual(result["load"]["start"], 7.5)

    def test_a_run_too_noisy_for_its_budget_says_so(self):
        """⚠ The half of the honesty that matters. A green verdict out of a run
        whose own pairs disagree by more than the budget is "not seen", not
        "not there", and the log has to distinguish those."""
        noisy = a_result([a_pair(1, 100.0, 100.0, 100),
                          a_pair(2, 100.0, 130.0, 100),
                          a_pair(3, 100.0, 70.0, 100),
                          a_pair(4, 100.0, 125.0, 100)])
        said = speed_ab.dispersion(noisy, budget=1.0)
        self.assertIn("WIDER THAN", said)
        self.assertIn("not seen", said)

    def test_a_tight_run_makes_no_such_excuse(self):
        tight = a_result([a_pair(seed, 100.0, 100.1, 100) for seed in range(5)])
        said = speed_ab.dispersion(tight, budget=8.0)
        self.assertNotIn("WIDER THAN", said)
        self.assertIn("resolves", said)

    def test_one_wild_pair_moves_the_confidence_and_not_the_verdict(self):
        """⚠ The two halves pull in opposite directions and both are wanted.

        The VERDICT must ignore a contended pair — that is why it is a median.
        The CONFIDENCE must not: at five pairs, one reading at +90% genuinely
        does mean this run could not have resolved eight percent, and a
        resolution line that shrugged that off would be the over-claim the
        line exists to prevent. So the median is unmoved and the sigma is not.
        """
        clean = [0.1, 0.0, -0.1, 0.05, 0.02]
        wild = clean[:-1] + [90.0]
        self.assertAlmostEqual(speed_ab.robust_sigma(clean), 0.0, delta=0.5)
        self.assertGreater(speed_ab.robust_sigma(wild), 5.0)
        self.assertAlmostEqual(
            a_result([a_pair(i, 100.0, 100.0 * (1 + d / 100), 100)
                      for i, d in enumerate(wild)])["median_pct"],
            0.05, delta=0.06)


class TheVerdictIsNotOverstated(unittest.TestCase):
    def _result(self, base, cand, mismatched=False):
        return a_result([a_pair(1, base, cand, 100, agree=not mismatched)])

    def test_a_change_inside_the_floor_is_not_a_result(self):
        self.assertIn("NOISE", speed_ab.verdict(self._result(100.0, 100.1)))
        self.assertIn("NOISE", speed_ab.verdict(self._result(100.0, 99.9)))

    def test_a_change_outside_the_floor_is_reported_as_one(self):
        self.assertIn("-11.", speed_ab.verdict(self._result(100.0, 89.0)))

    def test_the_floor_can_be_widened_for_a_noisier_host(self):
        """A hosted runner does not resolve a tenth of a percent, and printing
        one as a result there trains the reader to ignore the line."""
        said = speed_ab.verdict(self._result(100.0, 100.5), floor=1.0)
        self.assertIn("NOISE", said)

    def test_disagreeing_arms_beat_any_overhead_claim(self):
        """A change that alters behaviour has no overhead measurement at all,
        however large or clean the timing difference looks.

        ⚠ CHANGED 2026-08-22, AND THE HALF THAT CHANGED IS NAMED HERE. This
        test used to assert `assertNotIn("-50", said)` — that the percentage
        is withheld entirely when the arms disagree. The claim above it is
        correct and is kept intact below. Withholding the NUMBER is what could
        not survive: a promoted feature changes play by construction, so the
        reports differ by construction, and #2059 — six times slower, the
        event `docs/SIMULATOR_PERFORMANCE.md` wrote the standing rule after —
        would have been answered with a refusal and no six.

        So both statements are made, and they are different statements: this
        is not overhead and no optimization claim survives it, AND this is
        what the changed behaviour costs.
        """
        said = speed_ab.verdict(self._result(100.0, 50.0, mismatched=True))
        self.assertIn("NOT a measure of overhead", said)
        self.assertIn("no optimization claim", said)
        self.assertIn("-50.00%", said)

    def test_disagreeing_arms_exit_nonzero(self):
        with mock.patch.object(speed_ab, "run_once",
                               lambda b, s, o: a_run(1.0, 100, Path(b).name)), \
             mock.patch.object(speed_ab, "civvis_processes", lambda: 0), \
             mock.patch.object(Path, "is_file", lambda self: True):
            code = speed_ab.main(["--baseline", "/x/a", "--candidate", "/x/b",
                                  "--games", "2"])
        self.assertEqual(code, 1)


class TheHostIsWatched(unittest.TestCase):
    def test_other_civvis_games_are_counted(self):
        lines = ("civvis simulate --seed 1\n"
                 "/usr/bin/python3 something_else.py\n"
                 "target/ci/ai_eval advanced advanced_v1\n")
        with mock.patch.object(speed_ab.subprocess, "run",
                               lambda *a, **k: mock.Mock(stdout=lines)):
            self.assertEqual(speed_ab.civvis_processes(), 2)

    def test_an_unremarkable_process_table_is_quiet(self):
        with mock.patch.object(speed_ab.subprocess, "run",
                               lambda *a, **k: mock.Mock(stdout="zsh\nFinder\n")):
            self.assertEqual(speed_ab.civvis_processes(), 0)


class TheBudgetIsWhatMakesItAGate(unittest.TestCase):
    """`--budget` is the whole reason this harness can run unattended.

    Without it the exit code is non-zero exactly when the reports disagree —
    which is every pull request that changes how the agent plays, i.e. most of
    them. A gate wired that way is red on the ordinary case and silent on the
    dangerous one.
    """

    def _result(self, change, mismatched=False):
        return a_result([a_pair(1, 100.0, 100.0 + change, 100,
                                agree=not mismatched)])

    def test_no_budget_never_fails_on_cost(self):
        self.assertFalse(speed_ab.over_budget(self._result(900.0, True), None))

    def test_inside_the_budget_passes(self):
        self.assertFalse(speed_ab.over_budget(self._result(7.9), 8.0))

    def test_outside_the_budget_fails(self):
        self.assertTrue(speed_ab.over_budget(self._result(8.1), 8.0))

    def test_a_regression_that_also_changes_play_is_still_a_regression(self):
        """#2059 in miniature, and the only case the gate exists for."""
        self.assertTrue(speed_ab.over_budget(self._result(515.0, True), 8.0))

    def test_getting_faster_is_never_over_budget(self):
        """The boring case, which is also almost every run. A gate that fires
        here is a gate somebody turns off within a day."""
        self.assertFalse(speed_ab.over_budget(self._result(-48.7, True), 8.0))
        self.assertFalse(speed_ab.over_budget(self._result(0.0), 8.0))

    def test_an_improvement_does_not_read_as_a_regression(self):
        """It printed "done slower" for a −0.33% reading in the first
        end-to-end run of the new wording. Half of what this tool reports is
        an improvement."""
        self.assertIn("faster", speed_ab.verdict(self._result(-5.0)))
        self.assertNotIn("slower", speed_ab.verdict(self._result(-5.0)))
        self.assertIn("slower", speed_ab.verdict(self._result(+5.0)))

    def test_a_longer_game_at_the_same_cost_per_turn_is_not_over_budget(self):
        """⚠ THE WHOLE POINT OF THE NEW METRIC. Whole-game CPU here is +25%
        and the simulator did not get one instruction slower; the extra time
        is 25% more turns. `gene_screen` prices that on the play axis, and
        charging it here is how a -48.7% reading got published."""
        longer = a_result([a_pair(1, 100.0, 125.0, 1000, 1250, agree=False)])
        self.assertAlmostEqual(longer["change_pct"], 25.0, places=9)
        self.assertFalse(speed_ab.over_budget(longer, 8.0))

    def test_the_same_length_at_a_higher_cost_per_turn_is(self):
        slower = a_result([a_pair(1, 100.0, 125.0, 1000, 1000)])
        self.assertTrue(speed_ab.over_budget(slower, 8.0))


class TheGateConfirmsBeforeItBlocksAnybody(unittest.TestCase):
    """A required check that fails at random is worse than an advisory one.

    The confirmation runs a second paired block on DISJOINT seeds and the run
    fails only when both agree. It costs nothing on an ordinary pull request,
    because an ordinary pull request never reaches it.
    """

    def _main(self, cost, extra=()):
        """`cost(seed)` gives the candidate's CPU; baseline is always 100."""
        seen = []

        def fake(binary, seed, opts):
            seen.append(seed)
            base = "base" in Path(binary).name
            return a_run(100.0 if base else cost(seed), 100)

        with mock.patch.object(speed_ab, "run_once", fake), \
             mock.patch.object(speed_ab, "civvis_processes", lambda: 0), \
             mock.patch.object(Path, "is_file", lambda self: True):
            code = speed_ab.main(["--baseline", "/x/base", "--candidate",
                                  "/x/cand", "--seeds", "1", "--games", "3",
                                  "--budget", "8", *extra])
        return code, seen

    def test_a_real_regression_is_confirmed_and_fails(self):
        code, seen = self._main(lambda seed: 200.0)
        self.assertEqual(code, 1)
        self.assertEqual(sorted(set(seen)), [1, 2, 3, 4, 5, 6],
                         "the confirmation has to use a disjoint block")

    def test_one_noisy_block_alone_does_not_block_the_fleet(self):
        code, seen = self._main(lambda seed: 200.0 if seed <= 3 else 100.0)
        self.assertEqual(code, 0)
        self.assertIn(4, seen)

    def test_an_ordinary_run_never_pays_for_the_confirmation(self):
        code, seen = self._main(lambda seed: 100.0)
        self.assertEqual(code, 0)
        self.assertEqual(sorted(set(seen)), [1, 2, 3])

    def test_no_confirm_fails_on_the_first_block(self):
        code, seen = self._main(lambda seed: 200.0, extra=("--no-confirm",))
        self.assertEqual(code, 1)
        self.assertEqual(sorted(set(seen)), [1, 2, 3])


class TheAbsoluteLedger(unittest.TestCase):
    """A relative gate cannot see drift, by construction.

    Every pull request is measured against the commit before it, so the fleet
    can lose five percent a month with every single run reading green.
    `docs/census.json` already solved this shape — record the reading, let it
    move, make the diff the signal — and this is that device for cost.
    """

    def test_a_reading_carries_the_conditions_it_was_taken_under(self):
        ledger = {"shape": {}, "readings": []}
        row = speed_ab.record_reading(
            ledger, "mbp-m5-max-128", 52.6, 600, commit="abc1234",
            note="quiet window", load={"start": 3.1, "peak": 6.4, "end": 4.0},
            spread_pct=0.31, date="2026-08-23")
        self.assertAlmostEqual(row["seconds_per_turn"], 52.6 / 600, places=6)
        self.assertEqual(row["load_peak"], 6.4)
        self.assertEqual(row["pair_spread_pct"], 0.31)
        self.assertEqual(ledger["readings"], [row])

    def test_the_newest_reading_for_a_machine_wins(self):
        ledger = {"readings": [
            {"machine": "a", "date": "2026-01-01", "seconds_per_turn": 1.0},
            {"machine": "b", "date": "2026-01-02", "seconds_per_turn": 2.0},
            {"machine": "a", "date": "2026-02-01", "seconds_per_turn": 3.0}]}
        self.assertEqual(speed_ab.newest_reading(ledger, "a")["seconds_per_turn"], 3.0)
        self.assertIsNone(speed_ab.newest_reading(ledger, "c"))

    def test_a_different_shape_is_not_drift_and_says_so(self):
        """⚠ The trap this closes: change the workflow's turn count and every
        later run silently reports a 40% "drift" against a number measured on
        a different game."""
        ledger = {"shape": {"turns": 120}, "readings": [
            {"machine": "m", "date": "2026-08-23", "seconds_per_turn": 0.088,
             "commit": "abc", "load_peak": 1.0}]}
        said = speed_ab.ledger_line(ledger, "m", 0.12, {"turns": 250},
                                    {"peak": 1.0})
        self.assertIn("NOT COMPARABLE", said)

    def test_the_same_shape_reports_drift(self):
        ledger = {"shape": {"turns": 120}, "readings": [
            {"machine": "m", "date": "2026-08-23", "seconds_per_turn": 0.100,
             "commit": "abc", "load_peak": 1.0}]}
        said = speed_ab.ledger_line(ledger, "m", 0.110, {"turns": 120},
                                    {"peak": 1.2})
        self.assertIn("+10.0%", said)

    def test_a_machine_with_no_reading_is_not_an_error(self):
        ledger = {"shape": {"turns": 120}, "readings": []}
        said = speed_ab.ledger_line(ledger, "github-ubuntu-latest", 0.1,
                                    {"turns": 120}, {"peak": 1.0})
        self.assertIn("no reading", said)

    def test_a_transcribed_reading_needs_its_provenance(self):
        """A runner measures the trunk's absolute cost on every pull request
        and cannot commit the row. Transcribing it is allowed; transcribing it
        anonymously is not."""
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "ledger.json"
            with self.assertRaises(SystemExit):
                speed_ab.main(["--baseline", "/x/a", "--candidate", "/x/b",
                               "--ledger", str(path), "--record-ledger",
                               "--ledger-machine", "github-ubuntu-latest",
                               "--ledger-cpu", "300.0", "--ledger-turns", "600"])
            code = speed_ab.main([
                "--baseline", "/x/a", "--candidate", "/x/b",
                "--ledger", str(path), "--record-ledger",
                "--ledger-machine", "github-ubuntu-latest",
                "--ledger-cpu", "300.0", "--ledger-turns", "600",
                "--ledger-commit", "5205a424", "--note", "from PR #2328 run 1"])
            self.assertEqual(code, 0)
            written = json.loads(path.read_text())
        self.assertEqual(written["readings"][0]["seconds_per_turn"], 0.5)
        self.assertEqual(written["shape"]["turns"], speed_ab.GATE_SHAPE["turns"])

    def test_recording_without_a_machine_is_refused(self):
        with mock.patch.object(Path, "is_file", lambda self: True), \
             self.assertRaises(SystemExit):
            speed_ab.main(["--baseline", "/x/a", "--candidate", "/x/b",
                           "--record-ledger"])


class TheCommittedLedgerIsWellFormed(unittest.TestCase):
    """The file itself, not the code that writes it.

    `docs/census.json`'s discipline: the number is free to move, and it cannot
    move without somebody choosing to record it. What has to hold is that every
    row says what it measured and under what conditions.
    """

    @classmethod
    def setUpClass(cls):
        cls.ledger = json.loads(speed_ab.LEDGER.read_text(encoding="utf-8"))

    def test_it_exists_and_has_readings(self):
        self.assertTrue(speed_ab.LEDGER.is_file(), speed_ab.LEDGER)
        self.assertTrue(self.ledger["readings"], "an empty ledger measures nothing")

    def test_every_reading_is_self_consistent(self):
        for row in self.ledger["readings"]:
            with self.subTest(row=row.get("date"), machine=row.get("machine")):
                self.assertGreater(row["turns"], 0)
                self.assertAlmostEqual(row["seconds_per_turn"],
                                       row["cpu_seconds"] / row["turns"],
                                       places=5)

    def test_every_reading_carries_its_conditions(self):
        """⚠ Measured 2026-08-23: the same binary at the same shape read
        0.1134 s/turn at load 6 and 0.1761 at load 94 on the same Mac. An
        absolute with no load average on it is not comparable to anything."""
        for row in self.ledger["readings"]:
            with self.subTest(row=row.get("date"), machine=row.get("machine")):
                self.assertTrue(row["machine"])
                self.assertTrue(row["commit"])
                self.assertGreater(len(row["note"]), 10,
                                   "a row nobody can source is not a baseline")
                self.assertIsNotNone(row["load_peak"])
                dt.date.fromisoformat(row["date"])

    def test_readings_are_in_the_order_they_were_taken(self):
        for machine in {row["machine"] for row in self.ledger["readings"]}:
            dates = [row["date"] for row in self.ledger["readings"]
                     if row["machine"] == machine]
            self.assertEqual(dates, sorted(dates), machine)


class TheGateIsActuallyWired(unittest.TestCase):
    """AGENTS.md: "a guard you add runs in the same change that adds it".

    `test_ci_wiring.py` checks that a tool claiming CI is *named* by some
    workflow. That is not enough here — naming it without `--budget` would run
    the harness and then ignore its verdict, the same defect one level down.
    """

    def test_a_workflow_runs_the_harness_with_a_budget(self):
        # ⚠ Anchored on `python3 …`, not on the bare path. The first draft
        # matched `- 'tools/speed_ab.py'` in the workflow's own `paths:` filter
        # and read the trailing quote as the argument list — a check that
        # passed by finding the file's NAME while the step that runs it could
        # have been deleted. Verified by deleting the step: it now fails.
        argv = gate_invocation()
        self.assertTrue(argv, "tools/speed_ab.py is named in a workflow but "
                              "never executed")
        self.assertIn("--budget", argv,
                      "the harness runs but nothing reads its verdict")

    def test_the_budget_is_tight_enough_to_be_worth_running(self):
        """#2289 shipped `--budget 50`, which cannot see the 5-10% creep that
        actually accumulates. The paired delta survived a load average of 94
        inside ±0.6%, so there is no defence left for fifty."""
        budget = float(gate_flags()["budget"])
        self.assertLessEqual(budget, 10.0)
        self.assertGreater(budget, 1.0, "below the host noise this is a "
                                        "coin-flip gate, which is worse than none")


class TheWorkflowMeasuresTheScreensShape(unittest.TestCase):
    """The gate's shape is data in two files, and they have to be one shape.

    #2301: 87% of `precise_evacuation`'s bill is on minor seats, so the
    city-state count is the leg that decides whether this gate can see the
    cost that dominates the fleet's compute. #2289 measured six of them; the
    screen plays nine.
    """

    def test_the_map_row_is_the_screens_own(self):
        flags = gate_flags()
        self.assertEqual(flags["width"], "74")
        self.assertEqual(flags["height"], "46")
        self.assertEqual(flags["city-states"], "9")
        self.assertEqual(flags["players"], "6")
        self.assertEqual(flags["speed"], "online")
        self.assertEqual(flags["map"], "continents")

    def test_the_screen_constants_say_the_same_thing(self):
        """Read from `gene_screen.rs`, so moving the screen moves this gate or
        turns it red. A hand-copied shape is right on the day it is written."""
        source = (REPO / "src" / "bin" / "gene_screen.rs").read_text(encoding="utf-8")
        constants = dict(re.findall(
            r"const SCREEN_(\w+):\s*\w+\s*=\s*([A-Za-z:]+|\d+)\s*;", source))
        flags = gate_flags()
        self.assertEqual(constants["PLAYERS"], flags["players"])
        self.assertEqual(constants["WIDTH"], flags["width"])
        self.assertEqual(constants["HEIGHT"], flags["height"])
        self.assertEqual(constants["CITY_STATES"], flags["city-states"])
        self.assertEqual(constants["MAP"], "MapScript::Continents")

    def test_the_tools_defaults_are_the_workflows_arguments(self):
        """Two places holding one shape is one place too many unless something
        compares them."""
        flags = gate_flags()
        for name, value in speed_ab.GATE_SHAPE.items():
            with self.subTest(leg=name):
                self.assertEqual(flags[name.replace("_", "-")], str(value))
        self.assertEqual(int(flags["seeds"]), speed_ab.GATE_SEEDS)
        self.assertEqual(int(flags["games"]), speed_ab.GATE_GAMES)
        self.assertEqual(int(flags["interleaves"]), speed_ab.GATE_INTERLEAVES)

    def test_the_committed_ledger_measures_that_same_shape(self):
        """⚠ THE DEFECT THIS EXISTS FOR, and `main` took the same species of
        it on 2026-08-23: `17a27004` landed a generated ranking out of step
        with its generator and failed six tests every PR inherits (#2336). An
        absolute cost recorded at one shape and a gate running another is that
        defect with no test in front of it — the drift line would compare two
        different games and call the difference regression."""
        ledger = json.loads(speed_ab.LEDGER.read_text(encoding="utf-8"))
        flags = gate_flags()
        expected = speed_ab.ledger_shape(
            int(flags["seeds"]), int(flags["games"]), int(flags["interleaves"]),
            {name: type(value)(flags[name.replace("_", "-")])
             for name, value in speed_ab.GATE_SHAPE.items()})
        self.assertEqual(ledger["shape"], expected)

    def test_enough_pairs_for_the_median_to_mean_something(self):
        """A median of two is a mean, and a spread of two pairs is a guess."""
        pairs = int(gate_flags()["games"]) * int(gate_flags()["interleaves"])
        self.assertGreaterEqual(pairs, 4)


class TheCheckAlwaysReportsAVerdict(unittest.TestCase):
    """What has to be true before `paired-cost` can ever become required.

    `required_check_state` in `tools/civvis_collab.py` reads a required check
    with no run as *pending* and a `skipped` conclusion as a failure. So a
    `paths:` filter, or a job-level `if:`, turns promotion into a fleet-wide
    hang on every documentation-only pull request. The scope decision lives in
    the first step instead, which always succeeds.
    """

    @classmethod
    def setUpClass(cls):
        cls.raw = (WORKFLOWS / "speed.yml").read_text(encoding="utf-8")
        cls.body = "\n".join(line.partition("#")[0]
                             for line in cls.raw.splitlines())

    def test_the_trigger_has_no_paths_filter(self):
        trigger = self.body.split("jobs:")[0]
        self.assertNotIn("paths:", trigger)

    def test_the_job_is_never_skipped_wholesale(self):
        job = self.body.split("paired-cost:")[1].split("steps:")[0]
        self.assertNotIn("if:", job,
                         "a skipped job reports `skipped`, which "
                         "required_check_state reads as a failure")

    def test_the_scope_step_gates_the_expensive_part(self):
        self.assertIn("steps.scope.outputs.measure == 'true'", self.body)
        self.assertIn("git diff --name-only", self.body)

    def test_it_is_required_and_this_file_knows_it(self):
        """⚠ Advisory, it could not do the job it exists for.

        `ship` waits on `REQUIRED_CHECKS` and merges without reading anything
        else, so #2059 would have merged again with a red advisory X beside
        it. The three properties that make requiring it safe are each pinned
        by a test in this class: it always reports, its verdict is a median
        confirmed on a second disjoint block, and an intended cost can be
        acknowledged in the pull request body.
        """
        sys.path.insert(0, str(REPO / "tools"))
        import civvis_collab as collab  # noqa: PLC0415

        self.assertIn("paired-cost", collab.REQUIRED_CHECKS)
        self.assertNotIn("paired-cost", collab.ADVISORY_CHECKS)


class TheRequiredGateHasAWayToSayYes(unittest.TestCase):
    """A promoted feature is a performance event by definition.

    So a blocking cost gate WILL fire on an intended cost, and one with no way
    to accept that cost is a gate somebody deletes the first Friday it is
    inconvenient. `tools/overwrite_guard.py` already established the shape of
    the answer here: a line in the pull request body. The reason is mandatory,
    because that sentence — the number, and why it is worth paying — is exactly
    the one #2059 never wrote.
    """

    def test_a_reason_is_an_acknowledgement(self):
        self.assertEqual(
            speed_ab.acknowledged(
                "## What changed\nthing\n\npaired-cost: allow +31%/turn, the "
                "envelope cache is the point\n"),
            "+31%/turn, the envelope cache is the point")

    def test_a_bare_marker_is_not(self):
        """A token anybody can paste without thinking gates nothing."""
        self.assertIsNone(speed_ab.acknowledged("paired-cost: allow\n"))
        self.assertIsNone(speed_ab.acknowledged("paired-cost: allow   \n"))

    def test_an_unmarked_body_is_not(self):
        self.assertIsNone(speed_ab.acknowledged("we made it faster, honest"))
        self.assertIsNone(speed_ab.acknowledged(None))

    def test_an_acknowledged_cost_passes_without_a_second_measurement(self):
        seen = []

        def fake(binary, seed, opts):
            seen.append(seed)
            return a_run(100.0 if "base" in Path(binary).name else 300.0, 100)

        with mock.patch.object(speed_ab, "run_once", fake), \
             mock.patch.object(speed_ab, "civvis_processes", lambda: 0), \
             mock.patch.object(Path, "is_file", lambda self: True), \
             mock.patch.dict(speed_ab.os.environ,
                             {"PR_BODY": "paired-cost: allow +200%, measured"}):
            code = speed_ab.main(["--baseline", "/x/base", "--candidate",
                                  "/x/cand", "--seeds", "1", "--games", "3",
                                  "--budget", "8", "--acknowledge-env", "PR_BODY"])
        self.assertEqual(code, 0)
        self.assertEqual(sorted(set(seen)), [1, 2, 3],
                         "an acknowledged cost should not pay for a "
                         "confirmation block it cannot change")

    def test_without_the_marker_the_same_run_fails(self):
        with mock.patch.object(
                speed_ab, "run_once",
                lambda b, s, o: a_run(100.0 if "base" in Path(b).name else 300.0, 100)), \
             mock.patch.object(speed_ab, "civvis_processes", lambda: 0), \
             mock.patch.object(Path, "is_file", lambda self: True), \
             mock.patch.dict(speed_ab.os.environ, {"PR_BODY": "no marker here"}):
            code = speed_ab.main(["--baseline", "/x/base", "--candidate",
                                  "/x/cand", "--seeds", "1", "--games", "3",
                                  "--budget", "8", "--acknowledge-env", "PR_BODY"])
        self.assertEqual(code, 1)

    def test_the_workflow_offers_the_hatch(self):
        """A required gate whose escape hatch is not wired is a required gate
        with no escape hatch."""
        argv = gate_invocation()
        self.assertIn("--acknowledge-env", argv)
        variable = argv[argv.index("--acknowledge-env") + 1]
        raw = (WORKFLOWS / "speed.yml").read_text(encoding="utf-8")
        self.assertRegex(raw, rf"{variable}: \$\{{\{{ github.event.pull_request.body")


if __name__ == "__main__":
    unittest.main()
