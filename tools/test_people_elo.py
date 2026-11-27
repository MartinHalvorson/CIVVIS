#!/usr/bin/env python3
"""The people's record: a hotseat game's line, and how it reaches the ledger
branch.

    python3 tools/test_people_elo.py
"""

from __future__ import annotations

import base64
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import people_elo  # noqa: E402


def summary(**more) -> dict:
    """A duel's summary as `civ6_hotseat.table_record` writes it."""
    base = {
        "tag": "duel-dev-1", "utc": "2026-11-27T09:00:00Z", "started": "2026-11-27T06:30:00Z",
        "person": "dev", "person_seat": 0, "leader": "LEADER_GILGAMESH", "revision": "945f056d",
        "players": 2, "map": "Pangaea.lua", "map_size": "MAPSIZE_DUEL", "speed": "GAMESPEED_ONLINE",
        "difficulty": "DIFFICULTY_PRINCE", "civvis_seats": [1], "seat_agents": {"1": "holy"},
        "victories_off": [], "turn": 143,
        "result": {"kind": "victory", "team": 1, "victory": 4, "victory_type": "VICTORY_RELIGIOUS",
                   "turn": 143, "ours": True, "winner_seat": 1},
    }
    base.update(more)
    return base


class Branch:
    """GitHub's contents API for one file, as `people_elo.gh_api` is called."""

    def __init__(self, text: str | None = None, lose_races: int = 0, big: bool = False):
        self.text, self.commits, self.lose_races, self.big = text, [], lose_races, big
        self.calls: list[tuple] = []

    def sha(self) -> str:
        return f"sha-{len(self.commits)}"

    def __call__(self, repo, *args, body=None):
        self.calls.append(args)
        if "PUT" in args:
            sent = json.loads(body)
            if self.lose_races:
                # Another writer got there first: the file is no longer the one read.
                self.lose_races -= 1
                self.commits.append("someone else")
                return 1, b"", "gh: people.jsonl does not match (HTTP 409)"
            if sent.get("sha") != (self.sha() if self.text is not None else None):
                return 1, b"", "gh: sha does not match (HTTP 409)"
            self.text = base64.b64decode(sent["content"]).decode()
            self.commits.append(sent["message"])
            assert sent["branch"] == "ledger"
            return 0, b"{}", ""
        if self.text is None:
            return 1, b"", "gh: Not Found (HTTP 404)"
        if "Accept: application/vnd.github.raw+json" in args:
            return 0, self.text.encode(), ""
        content = "" if self.big else base64.b64encode(self.text.encode()).decode()
        return 0, json.dumps({"sha": self.sha(), "size": len(self.text), "content": content}).encode(), ""


class Row(unittest.TestCase):
    def test_a_game_the_agent_won_is_that_agents(self):
        row = people_elo.game_row(summary())
        self.assertEqual(row, {
            "tag": "duel-dev-1", "at": "2026-11-27T06:30:00Z", "ended": "2026-11-27T09:00:00Z",
            "person": "dev", "leader": "LEADER_GILGAMESH", "agents": ["holy"], "winner": "holy",
            "victory": "VICTORY_RELIGIOUS", "turn": 143, "players": 2, "map": "Pangaea.lua",
            "map_size": "MAPSIZE_DUEL", "speed": "GAMESPEED_ONLINE", "difficulty": "DIFFICULTY_PRINCE",
            "diplomatic": True, "revision": "945f056d"})

    def test_a_victory_that_is_not_a_civvis_seats_is_the_persons(self):
        # Every other seat at a person's table is ours.
        won = summary(result={"kind": "victory", "team": 0, "victory": 0,
                              "victory_type": "VICTORY_TECHNOLOGY", "turn": 201, "ours": False,
                              "winner_seat": None})
        row = people_elo.game_row(won)
        self.assertEqual((row["winner"], row["victory"], row["turn"]),
                         (people_elo.PERSON, "VICTORY_TECHNOLOGY", 201))
        # Every CIVVIS seat eliminated: theirs too.
        row = people_elo.game_row(summary(result={"kind": "defeat", "turn": 90, "ours": False}))
        self.assertEqual((row["winner"], row["victory"], row["turn"]),
                         (people_elo.PERSON, "VICTORY_CONQUEST", 90))

    def test_the_agent_that_won_is_named_at_a_table_of_several(self):
        table = summary(players=4, civvis_seats=[1, 2, 3], seat_agents={"1": "holy", "3": "tourist"},
                        result={"kind": "victory", "team": 3, "victory": 2, "turn": 180,
                                "ours": True, "winner_seat": 3})
        row = people_elo.game_row(table)
        # A seat `--agents` did not name plays the flagship; an old summary
        # with no name for the victory keeps the host's index.
        self.assertEqual((row["agents"], row["winner"], row["victory"]),
                         (["holy", "jack", "tourist"], "tourist", 2))
        # Given up at a table of several: the person lost, to no one agent.
        resigned = people_elo.game_row({**table, "result": None}, winner="civvis", victory="resignation")
        self.assertEqual((resigned["winner"], resigned["victory"]), ("civvis", "resignation"))
        self.assertIn("dev lost to holy, jack, tourist  (resignation", people_elo.said(resigned))

    def test_a_game_the_game_did_not_end_needs_someone_to_say_who_won(self):
        open_game = summary(result=None)
        with self.assertRaisesRegex(people_elo.NotRecorded, "--winner"):
            people_elo.game_row(open_game)
        conceded = people_elo.game_row(open_game, winner="civvis")
        self.assertEqual((conceded["winner"], conceded["victory"], conceded["turn"]),
                         ("holy", people_elo.CONCESSION, 143))
        self.assertEqual(people_elo.game_row(open_game, winner="person")["winner"], people_elo.PERSON)
        with self.assertRaisesRegex(people_elo.NotRecorded, "not at the table"):
            people_elo.game_row(open_game, winner="tourist")

    def test_a_game_with_no_person_or_no_civvis_seat_is_not_a_persons_game(self):
        with self.assertRaisesRegex(people_elo.NotRecorded, "no person"):
            people_elo.game_row(summary(person=None))
        self.assertEqual(people_elo.game_row(summary(person=None), name="dev")["person"], "dev")
        with self.assertRaisesRegex(people_elo.NotRecorded, "no CIVVIS seat"):
            people_elo.game_row(summary(civvis_seats=[]))

    def test_a_table_nobody_was_at_is_refused_even_under_a_name(self):
        # `selfplay-duel-20261127T0654Z`'s summary: CIVVIS on both seats.
        unattended = summary(person=None, civvis_seats=[0, 1], seat_agents={"0": "jack", "1": "holy"})
        del unattended["person_seat"]
        for name in (None, "dev"):
            with self.assertRaisesRegex(people_elo.NotRecorded, "unattended table"):
                people_elo.game_row(unattended, name=name)

    def test_a_game_ended_when_the_game_said_so(self):
        # The same line whenever the summary was written: at the ending, and
        # again when the person closed Civ VI an hour later.
        at = {"kind": "victory", "team": 1, "victory": 4, "victory_type": "VICTORY_RELIGIOUS",
              "turn": 143, "ours": True, "winner_seat": 1, "at": "2026-11-27T08:55:00Z"}
        first = people_elo.game_row(summary(result=at, utc="2026-11-27T08:55:03Z"))
        later = people_elo.game_row(summary(result=at, utc="2026-11-27T10:00:00Z"))
        self.assertEqual(first, later)
        self.assertEqual(first["ended"], "2026-11-27T08:55:00Z")
        self.assertEqual(people_elo.game_row(summary())["ended"], "2026-11-27T09:00:00Z")

    def test_the_diplomatic_switch_is_the_tables_own(self):
        self.assertIs(people_elo.game_row(summary(victories_off=["VICTORY_DIPLOMATIC"]))["diplomatic"], False)
        self.assertIsNone(people_elo.game_row(summary(victories_off=None))["diplomatic"])


class Ledger(unittest.TestCase):
    REPO = Path("/nowhere")

    def test_the_first_game_starts_the_record_and_the_same_game_twice_changes_nothing(self):
        branch = Branch()
        row = people_elo.game_row(summary())
        self.assertEqual(people_elo.publish(row, repo=self.REPO, api=branch), "published")
        self.assertEqual(people_elo.parse_rows(branch.text), {"duel-dev-1": row})
        self.assertEqual(branch.commits, ["ledger: people (dev, duel-dev-1)"])
        self.assertEqual(people_elo.publish(row, repo=self.REPO, api=branch), "already")
        self.assertEqual(len(branch.commits), 1)
        # Read and written at the record's own path on the ledger branch.
        self.assertEqual(branch.calls[0], ("repos/{owner}/{repo}/contents/people.jsonl?ref=ledger",))

    def test_a_second_game_is_added_oldest_first_and_a_corrected_one_replaces_its_line(self):
        branch = Branch()
        later = people_elo.game_row(summary(tag="duel-dev-2", started="2026-11-28T01:00:00Z"))
        first = people_elo.game_row(summary())
        people_elo.publish(later, repo=self.REPO, api=branch)
        people_elo.publish(first, repo=self.REPO, api=branch)
        self.assertEqual([json.loads(line)["tag"] for line in branch.text.splitlines()],
                         ["duel-dev-1", "duel-dev-2"])
        fixed = people_elo.game_row(summary(), winner="person")
        self.assertEqual(people_elo.publish(fixed, repo=self.REPO, api=branch), "published")
        rows = people_elo.parse_rows(branch.text)
        self.assertEqual((len(rows), rows["duel-dev-1"]["winner"]), (2, "person"))

    def test_a_write_that_lost_a_race_is_read_again_and_redone_and_gives_up_out_loud(self):
        row = people_elo.game_row(summary())
        branch = Branch("", lose_races=1)
        self.assertEqual(people_elo.publish(row, repo=self.REPO, api=branch), "published")
        self.assertEqual(branch.commits, ["someone else", "ledger: people (dev, duel-dev-1)"])
        with self.assertRaisesRegex(RuntimeError, "not written after 3 attempts.*409"):
            people_elo.publish(row, repo=self.REPO, api=Branch("", lose_races=9))

    def test_a_record_past_the_listings_size_is_read_raw(self):
        row = people_elo.game_row(summary())
        text = people_elo.rows_text({row["tag"]: row})
        self.assertEqual(people_elo.branch_record(self.REPO, api=Branch(text, big=True)), (text, "sha-0"))
        self.assertEqual(people_elo.branch_record(self.REPO, api=Branch()), ("", None))
        with self.assertRaisesRegex(RuntimeError, "could not be read"):
            people_elo.branch_record(self.REPO, api=lambda repo, *a, body=None: (1, b"", "HTTP 502"))


class Command(unittest.TestCase):
    def test_a_dry_run_prints_the_line_and_an_unfinished_game_says_why(self):
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            (run_dir / "summary.json").write_text(json.dumps(summary()))
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                self.assertEqual(people_elo.main(["record", str(run_dir), "--dry-run"]), 0)
            self.assertEqual(json.loads(out.getvalue())["winner"], "holy")
            (run_dir / "summary.json").write_text(json.dumps(summary(result=None)))
            err = io.StringIO()
            with contextlib.redirect_stderr(err):
                self.assertEqual(people_elo.main(["record", str(run_dir), "--dry-run"]), 2)
            self.assertIn("--winner person or --winner civvis", err.getvalue())

    def test_a_game_reads_as_a_sentence_and_the_rating_is_the_pages_own_fit(self):
        won = people_elo.game_row(summary(result={"kind": "defeat", "turn": 90, "ours": False}))
        self.assertEqual(people_elo.said(won),
                         "2026-11-27T06:30:00Z  dev beat holy  (conquest, turn 90, 2 players, "
                         "Duel Pangaea)  [duel-dev-1]")
        self.assertIn("dev lost to holy  (religious, turn 143", people_elo.said(people_elo.game_row(summary())))
        text = people_elo.rows_text({won["tag"]: won})
        # One game won against the strongest agent: at least a little above it.
        rated = people_elo.ratings_text(people_elo.REPO, text)
        self.assertRegex(rated, r"^dev: Elo at least \d{4} ±\d+ \(1 of 1 won, Diplo on\)$")
        self.assertEqual(people_elo.ratings_text(people_elo.REPO, ""), "nobody is rated yet")


if __name__ == "__main__":
    unittest.main()
