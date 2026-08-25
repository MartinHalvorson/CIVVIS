#!/usr/bin/env python3
"""Every MECHANICS.md row names what it is modelled on and what checks it.

A row in `docs/MECHANICS.md` used to be a status glyph and a paragraph. The
paragraph read as a fact and nothing tied it to the shipped game or to a
test, so a row could drift from both and stay ✅: this week alone the deal
model, the gift, the barbarian bands and three religion rules were found
divergent by accident, in code that had a green row in this table.

The table now carries `Source` and `Check`. This suite is what makes those
columns claims with teeth rather than two more paragraphs:

- an empty Source fails;
- a Check naming a test that does not exist in the tree fails — a Rust test
  is `fn <name>(` under `src/`, a Python test `def <name>(` under `tools/`,
  a tool a file under `tools/`;
- a Source citing `GameplayDB.<Table>` or a `Base/...` / `DLC/...` script
  path is checked against the install on this Mac when it is present, and
  a `:line` must be inside the file. Without the game (a hosted runner)
  that half is skipped by name, not silently passed.
"""

from __future__ import annotations

import re
import sqlite3
import sys
import unittest
from functools import lru_cache
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DOC = REPO / "docs" / "MECHANICS.md"
sys.path.insert(0, str(Path(__file__).resolve().parent))
import civ6_env  # noqa: E402
import civ6_fidelity  # noqa: E402

TOKEN = re.compile(r"`([^`]+)`")
TABLE = re.compile(r"^GameplayDB\.([A-Za-z0-9_]+)$")
SCRIPT = re.compile(r"^((?:Base|DLC)/[A-Za-z0-9_./-]+\.(?:lua|xml|sql))(?::(\d+))?$")
TEST_NAME = re.compile(r"^[a-z][a-z0-9_]{8,}$")
TOOL = re.compile(r"^[a-z0-9_]+\.py$")
DLL = "DLL — behavioural only"
NO_CHECK = "—"


def rows() -> list[dict]:
    """The coverage table: header-driven, so a reordered column still reads."""
    lines = DOC.read_text(encoding="utf-8").splitlines()
    header = None
    out = []
    for line in lines:
        if not line.startswith("|"):
            header = None
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if header is None:
            header = cells
            continue
        if all(set(cell) <= {"-", ":"} for cell in cells):
            continue
        if len(cells) != len(header):
            # A Notes cell with a literal pipe would land here; the table has
            # none, and a row that gains one should be noticed.
            raise AssertionError(f"{len(cells)} cells under a {len(header)}-column header: {line[:80]}")
        out.append(dict(zip(header, cells)))
    return out


@lru_cache(maxsize=None)
def rust_test_names() -> set[str]:
    names = set()
    for path in (REPO / "src").rglob("*.rs"):
        for found in re.finditer(r"\bfn\s+([a-z0-9_]+)\s*\(", path.read_text(encoding="utf-8", errors="replace")):
            names.add(found.group(1))
    return names


@lru_cache(maxsize=None)
def python_test_names() -> set[str]:
    names = set()
    for path in (REPO / "tools").rglob("*.py"):
        for found in re.finditer(r"^\s*def\s+([a-z0-9_]+)\s*\(", path.read_text(encoding="utf-8", errors="replace"), re.M):
            names.add(found.group(1))
    return names


def install_root() -> Path | None:
    try:
        return civ6_fidelity.find_install(None)
    except SystemExit:
        return None
    except Exception:  # noqa: BLE001 - an unreadable install is an absent one here
        return None


@lru_cache(maxsize=None)
def database_tables() -> frozenset[str] | None:
    cache = civ6_fidelity.find_cache_database()
    if cache is None:
        return None
    connection = sqlite3.connect(f"file:{cache}?mode=ro&immutable=1", uri=True)
    try:
        return frozenset(name for (name,) in connection.execute(
            "select name from sqlite_master where type = 'table'"))
    finally:
        connection.close()


class TheTableHasTheTwoColumns(unittest.TestCase):
    def test_every_row_has_a_source_and_a_check_cell(self):
        table = rows()
        self.assertGreater(len(table), 40, "the coverage table has shrunk or moved")
        for row in table:
            with self.subTest(system=row["System"]):
                self.assertIn("Source", row)
                self.assertIn("Check", row)

    def test_no_row_has_an_empty_source(self):
        empty = [row["System"] for row in rows() if not row["Source"].strip()]
        self.assertEqual(empty, [], "rows with no Source: name the table, the script line, or DLL — behavioural only")

    def test_a_missing_check_carries_its_reason_in_the_source(self):
        for row in rows():
            if row["Check"].strip() in ("", NO_CHECK):
                with self.subTest(system=row["System"]):
                    self.assertTrue(
                        DLL in row["Source"] or "CIVVIS-native" in row["Source"],
                        f"{row['System']!r} has no Check and its Source does not say why")


class EveryCheckNamesSomethingInTheTree(unittest.TestCase):
    def test_every_named_test_or_tool_exists(self):
        rust, python = rust_test_names(), python_test_names()
        missing = []
        for row in rows():
            for token in TOKEN.findall(row["Check"]):
                if TOOL.match(token):
                    if not (REPO / "tools" / token).is_file():
                        missing.append((row["System"], token))
                elif TEST_NAME.match(token):
                    if token not in rust and token not in python:
                        missing.append((row["System"], token))
        self.assertEqual(missing, [], "Check cells naming tests or tools that are not in the tree")

    def test_every_row_with_a_check_names_at_least_one_thing(self):
        for row in rows():
            if row["Check"].strip() == NO_CHECK:
                continue
            with self.subTest(system=row["System"]):
                named = [t for t in TOKEN.findall(row["Check"]) if TOOL.match(t) or TEST_NAME.match(t)]
                self.assertTrue(named, f"{row['System']!r}: a Check that names nothing is a paragraph")

    def test_the_rows_touched_this_week_have_a_check(self):
        """Deals, gifts, barbarians, religion, the difficulty ladder and
        loyalty were all found divergent in code with a green row."""
        wanted = {
            "Diplomacy (deals, alliances, grievances)": "a_gift_is_legal_buys_nothing_and_a_demand_is_refused",
            "Barbarians (camps, raiders, rewards)": "difficulty_scales_one_reported_barbarian_raid_party",
            "Religion": "military_units_step_onto_and_condemn_enemy_missionaries",
            "Difficulty levels & game speeds": "the_difficulty_ladder_is_ordered_and_neutral_at_prince",
            "Loyalty + governors (R&F)": "a_minor_city_projects_no_pressure_onto_its_neighbours",
        }
        by_name = {row["System"]: row for row in rows()}
        for system, test in wanted.items():
            with self.subTest(system=system):
                self.assertIn(f"`{test}`", by_name[system]["Check"])


class EverySourceCitesSomethingTheGameShips(unittest.TestCase):
    def test_the_citation_forms_are_recognised(self):
        self.assertTrue(TABLE.match("GameplayDB.BarbarianAttackForces"))
        self.assertTrue(SCRIPT.match("Base/Assets/UI/DiplomacyActionView.lua:2545"))
        self.assertTrue(SCRIPT.match("DLC/Expansion2/Data/Expansion2_RandomEvents.xml"))
        self.assertIsNone(SCRIPT.match("src/game.rs"))

    def test_every_cited_table_exists_in_the_gameplay_database(self):
        tables = database_tables()
        if tables is None:
            self.skipTest("no compiled Civilization VI gameplay database on this machine; table citations unchecked")
        missing = []
        for row in rows():
            for token in TOKEN.findall(row["Source"]):
                found = TABLE.match(token)
                if found and found.group(1) not in tables:
                    missing.append((row["System"], token))
        self.assertEqual(missing, [], "Source cells citing tables the shipped database does not have")

    def test_every_cited_script_exists_at_that_line(self):
        root = install_root()
        if root is None:
            self.skipTest("no Civilization VI install on this machine; script citations unchecked")
        missing = []
        for row in rows():
            for token in TOKEN.findall(row["Source"]):
                found = SCRIPT.match(token)
                if not found:
                    continue
                path = root / found.group(1)
                if not path.is_file():
                    missing.append((row["System"], token, "no such file"))
                    continue
                if found.group(2):
                    count = sum(1 for _ in path.open(encoding="utf-8", errors="replace"))
                    if int(found.group(2)) > count:
                        missing.append((row["System"], token, f"file has {count} lines"))
        self.assertEqual(missing, [], "Source cells citing scripts the install does not have")

    def test_a_source_names_something_checkable_or_says_it_cannot(self):
        for row in rows():
            source = row["Source"]
            with self.subTest(system=row["System"]):
                cited = any(TABLE.match(t) or SCRIPT.match(t) for t in TOKEN.findall(source))
                self.assertTrue(
                    cited or DLL in source or "CIVVIS-native" in source,
                    f"{row['System']!r}: cite a GameplayDB table, a shipped script, or say it is DLL-only / CIVVIS-native")


if __name__ == "__main__":
    unittest.main()
