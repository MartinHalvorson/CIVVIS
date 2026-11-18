"""The Create Game leader: its label, its read-back, and the launch that cannot verify it.

Live launch civvis-20261118T055028Z asked for LEADER_ELEANOR_FRANCE. The
control mod's one-row whitelist had already put her in the Choose Civilization
field, the harness answered "leader value was not readable" for it, and the
launch went round "setup could not be verified; backing out" thirteen times in
5.5 minutes without starting a game.

`fixtures/leader_field_ocr.json` is what the harness's own Vision passes return
for that launch's saved frame. It shows the cause was not the picture: the
label is one observation, whole, in band. The reader only knew the names in the
simulator's `data/civs.json`, where France has no Eleanor. The next launch but
one, civvis-20261118T062541Z, did the same for "Theodora", eight letters long.

Three things are held here: every shipped leader has its picker label (pinned
to `data/civ6_leader_picker_labels.json`), the field reads back for any of
them (also when Vision returns the label in pieces), and a leader that still
cannot be verified ends the launch after `LEADER_UNVERIFIED_ROUNDS` rounds with
its own exit status and summary reason.
"""

from __future__ import annotations

import contextlib
import inspect
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_leader_labels  # noqa: E402
import civ6_play  # noqa: E402

FIXTURE = json.loads((Path(__file__).resolve().parent / "fixtures"
                      / "leader_field_ocr.json").read_text(encoding="utf-8"))
DESKTOP = tuple(FIXTURE["desktop"])
WINDOW = tuple(FIXTURE["window"])
READS = FIXTURE["captures"]["eleanor_france"]
THEODORA_READS = FIXTURE["captures"]["theodora"]
ELEANOR = "Eleanor of Aquitaine (France)"

# Every leader type a game on the verification host has seated, 2026-11-18:
#   grep -o '"leader": "LEADER_[A-Z_0-9]*"' ~/civvis-civ6-runs/civvis_ladder.jsonl | sort -u
# The ledger is 168 MB and lives on that host, so the list is carried here.
SEATED_ON_THE_LEDGER = """
LEADER_ABRAHAM_LINCOLN LEADER_ALEXANDER LEADER_AMANITORE LEADER_AMBIORIX LEADER_BARBAROSSA
LEADER_BASIL LEADER_CATHERINE_DE_MEDICI LEADER_CATHERINE_DE_MEDICI_ALT LEADER_CHANDRAGUPTA
LEADER_CLEOPATRA LEADER_CLEOPATRA_ALT LEADER_CYRUS LEADER_ELEANOR_ENGLAND LEADER_ELEANOR_FRANCE
LEADER_ELIZABETH LEADER_GANDHI LEADER_GENGHIS_KHAN LEADER_GILGAMESH LEADER_GITARJA LEADER_GORGO
LEADER_HAMMURABI LEADER_HARALD_ALT LEADER_HARDRADA LEADER_HOJO LEADER_JADWIGA LEADER_JAYAVARMAN
LEADER_JOAO_III LEADER_JOHN_CURTIN LEADER_KRISTINA LEADER_KUBLAI_KHAN_MONGOLIA LEADER_KUPE
LEADER_LADY_SIX_SKY LEADER_LADY_TRIEU LEADER_LAURIER LEADER_LAUTARO LEADER_LUDWIG
LEADER_MANSA_MUSA LEADER_MATTHIAS_CORVINUS LEADER_MENELIK LEADER_MONTEZUMA LEADER_MVEMBA
LEADER_NADER_SHAH LEADER_NZINGA_MBANDE LEADER_PACHACUTI LEADER_PEDRO LEADER_PERICLES
LEADER_PETER_GREAT LEADER_PHILIP_II LEADER_POUNDMAKER LEADER_QIN_ALT LEADER_ROBERT_THE_BRUCE
LEADER_SALADIN LEADER_SALADIN_ALT LEADER_SEJONG LEADER_SEONDEOK LEADER_SHAKA
LEADER_SIMON_BOLIVAR LEADER_SULEIMAN LEADER_SULEIMAN_ALT LEADER_SUNDIATA_KEITA LEADER_T_ROOSEVELT
LEADER_T_ROOSEVELT_ROUGHRIDER LEADER_TAMAR LEADER_THEODORA LEADER_TOMYRIS LEADER_TRAJAN
LEADER_VICTORIA LEADER_WILHELMINA LEADER_WU_ZETIAN LEADER_YONGLE
""".split()


def relative(observation: dict) -> tuple[float, float, float, float]:
    """An observation's left, right, centre-x and centre-y as window fractions."""
    x, y, w, h = WINDOW
    left = observation["x"] * DESKTOP[0]
    right = left + observation["width"] * DESKTOP[0]
    middle = (observation["y"] + observation["height"] / 2) * DESKTOP[1]
    return ((left - x) / w, (right - x) / w, ((left + right) / 2 - x) / w, (middle - y) / h)


def the_label(observations: list[dict]) -> dict:
    found = [observation for observation in observations if observation["text"] == ELEANOR]
    assert len(found) == 1, found
    return found[0]


def split(observation: dict, first: str, second: str, *, gap: float = 0.3,
          lines_down: float = 0.0, shift_right: float = 0.0) -> list[dict]:
    """The observation as Vision would return it in two pieces.

    ``gap`` is in line heights (converted through the desktop's aspect, since
    boxes are normalized per axis); the second piece can be moved down by
    ``lines_down`` line heights or right by ``shift_right`` of the desktop.
    """
    share = len(first) / (len(first) + len(second))
    gap_x = gap * observation["height"] * DESKTOP[1] / DESKTOP[0]
    width = observation["width"] - gap_x
    left = dict(observation, text=first, width=width * share)
    right = dict(observation, text=second,
                 x=observation["x"] + width * share + gap_x + shift_right,
                 y=observation["y"] + lines_down * observation["height"],
                 width=width * (1 - share))
    return [left, right]


def read_field(window: list[dict], crop: list[dict], expected: str | None = None):
    with mock.patch.object(civ6_play, "desktop_size", return_value=DESKTOP), \
         mock.patch.object(civ6_play, "recognize_once", return_value=list(window)), \
         mock.patch.object(civ6_play, "_menu_crop_ocr", return_value=list(crop)):
        return civ6_play._setup_current_leader(Path("leader-picker-closed.png"), WINDOW,
                                               expected)


def observed(observations: list[dict], label: str, *, selected: bool = False):
    with mock.patch.object(civ6_play, "desktop_size", return_value=DESKTOP):
        return civ6_play._leader_observation(observations, label, WINDOW, selected=selected)


class SavedEleanorFrameTests(unittest.TestCase):
    """The frame that looped a launch thirteen times, read by the harness's own passes."""

    def test_the_label_was_one_whole_observation_inside_every_band(self) -> None:
        """The measurement: neither a split label nor an off-centre one."""
        for name in ("window", "crop", "leader_crop"):
            with self.subTest(read=name):
                label = the_label(READS[name])
                left, right, centre, row = relative(label)
                self.assertEqual(label["confidence"], 1.0)
                self.assertAlmostEqual(left, 0.466, delta=0.002)
                self.assertAlmostEqual(right, 0.563, delta=0.002)
                self.assertAlmostEqual(centre, 0.515, delta=0.002)
                self.assertAlmostEqual(row, 0.311, delta=0.002)
        # The selected-row read-back accepted it then and accepts it now.
        self.assertIsNotNone(observed(READS["leader_crop"], ELEANOR, selected=True))

    def test_the_field_reader_did_not_know_the_name(self) -> None:
        """What failed: the reader's own list of names, not the picture."""
        roster = json.loads((civ6_play.REPO_ROOT / "data" / "civs.json").read_text())
        simulator_names = {civ6_play._normalized_label(civilization["leader"])
                           for civilization in roster.values()}
        self.assertNotIn(civ6_play._normalized_label(ELEANOR), simulator_names)
        # The reader as it was: Random Leader and the simulator's names only.
        with mock.patch.dict(civ6_play.LEADER_PICKER_LABELS, clear=True):
            self.assertIsNone(read_field(READS["window"], READS["crop"]))

    def test_the_field_now_reads_back_as_the_requested_leader(self) -> None:
        self.assertEqual(read_field(READS["window"], READS["crop"], ELEANOR),
                         (ELEANOR, (444, 200)))
        # The table alone names her, and either pass alone carries the field.
        self.assertEqual(read_field(READS["window"], READS["crop"]), (ELEANOR, (444, 200)))
        self.assertEqual(read_field(READS["window"], [])[0], ELEANOR)
        self.assertEqual(read_field([], READS["crop"])[0], ELEANOR)

    def test_a_short_name_failed_the_same_way(self) -> None:
        """Length was never the cause: launch civvis-20261118T062541Z asked for
        LEADER_THEODORA and looped on the same answer. Byzantium's leader in
        the simulator's roster is Basil II."""
        self.assertEqual(THEODORA_READS["leader"], "LEADER_THEODORA")
        for name in ("window", "crop"):
            labels = [observation for observation in THEODORA_READS[name]
                      if observation["text"] == "Theodora"]
            self.assertEqual(len(labels), 1)
            self.assertAlmostEqual(relative(labels[0])[2], 0.483, delta=0.002)
        with mock.patch.dict(civ6_play.LEADER_PICKER_LABELS, clear=True):
            self.assertIsNone(read_field(THEODORA_READS["window"], THEODORA_READS["crop"]))
        self.assertEqual(read_field(THEODORA_READS["window"], THEODORA_READS["crop"],
                                    civ6_play.leader_display_name("LEADER_THEODORA")),
                         ("Theodora", (417, 201)))

    def test_a_requested_label_the_table_lacks_is_still_readable(self) -> None:
        """A leader from a pack this table has never seen resolves through the
        label the launch asked for."""
        with mock.patch.dict(civ6_play.LEADER_PICKER_LABELS, clear=True):
            self.assertEqual(read_field(READS["window"], READS["crop"], ELEANOR)[0],
                             ELEANOR)

    def test_setup_verifies_the_saved_frame_without_a_click(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             mock.patch.object(civ6_play, "desktop_size", return_value=DESKTOP), \
             mock.patch.object(civ6_play, "recognize_once",
                               return_value=list(READS["window"])), \
             mock.patch.object(civ6_play, "_menu_crop_ocr",
                               return_value=list(READS["crop"])), \
             mock.patch.object(civ6_play, "screenshot") as screenshot, \
             mock.patch.object(civ6_play, "click_at") as click, \
             contextlib.redirect_stdout(io.StringIO()) as said:
            panel = Path(temporary) / "dropdown-speed-closed.png"
            panel.write_bytes(b"frame")
            proved: dict = {}
            verified = civ6_play.select_requested_leader(
                WINDOW, "LEADER_ELEANOR_FRANCE", Path(temporary), panel=panel,
                panel_out=proved)
        self.assertTrue(verified)
        self.assertIn(f"leader: already verified {ELEANOR} (LEADER_ELEANOR_FRANCE)",
                      said.getvalue())
        self.assertEqual(proved, {"shot": panel})
        screenshot.assert_not_called()
        click.assert_not_called()

    def test_her_other_persona_is_not_verified_by_this_field(self) -> None:
        """England's Eleanor differs from France's by one word in brackets."""
        england = civ6_play.leader_display_name("LEADER_ELEANOR_ENGLAND")
        self.assertEqual(england, "Eleanor of Aquitaine (England)")
        self.assertIsNone(observed(READS["leader_crop"], england, selected=True))
        current = read_field(READS["window"], READS["crop"], england)
        self.assertNotEqual(civ6_play._normalized_label(current[0]),
                            civ6_play._normalized_label(england))

    def test_the_longest_shipped_label_needs_no_wider_band(self) -> None:
        """Left-aligned text moves its centre half a character per character:
        from the measured frame, the longest label still centres in the band
        and ends inside the 0.40-0.62 column `_leader_ocr` crops."""
        left, right, _, _ = relative(the_label(READS["window"]))
        per_character = (right - left) / len(ELEANOR)
        longest = max(civ6_play.LEADER_PICKER_LABELS.values(), key=len)
        self.assertEqual(longest, "Catherine de Medici (Magnificence)")
        # Ten percent of slack for a proportional face's wider letters.
        end = left + per_character * len(longest) * 1.10
        self.assertLess(end, 0.62)
        self.assertTrue(0.40 <= (left + end) / 2 <= 0.62)


class SplitLabelTests(unittest.TestCase):
    """One rendered line returned as neighbouring observations is still one label."""

    PIECES = ("Eleanor of Aquitaine", "(France)")

    def test_the_saved_frame_holds_a_line_vision_really_split(self) -> None:
        """Not a hypothetical: the title "SID MEIER'S" came back as two
        observations of one line, and the join reads them as one."""
        window = READS["window"]
        texts = [observation["text"] for observation in window]
        self.assertIn("SID", texts)
        self.assertIn("MEIER'", texts)
        joined = civ6_play._joined_label_observations(window, {"sidmeier"}, DESKTOP)
        self.assertEqual([(found["text"], found["pieces"]) for found in joined],
                         [("SID MEIER'", 2)])
        sid = window[texts.index("SID")]
        meier = window[texts.index("MEIER'")]
        self.assertAlmostEqual(joined[0]["x"], sid["x"], places=6)
        self.assertAlmostEqual(joined[0]["x"] + joined[0]["width"],
                               meier["x"] + meier["width"], places=6)
        # Her label, whole in every read, is never rebuilt from pieces.
        for name in ("window", "crop", "leader_crop"):
            self.assertEqual(civ6_play._joined_label_observations(
                READS[name], {civ6_play._normalized_label(ELEANOR)}, DESKTOP), [])

    def test_two_pieces_of_the_selected_row_read_back(self) -> None:
        whole = the_label(READS["leader_crop"])
        others = [observation for observation in READS["leader_crop"]
                  if observation is not whole]
        pieces = split(whole, *self.PIECES)
        self.assertIsNone(observed(others, ELEANOR, selected=True))
        joined = observed(others + pieces, ELEANOR, selected=True)
        self.assertIsNotNone(joined)
        self.assertEqual(joined["pieces"], 2)
        # The joined box is the whole label's box, so the band sees its centre.
        self.assertAlmostEqual(joined["x"], whole["x"], places=6)
        self.assertAlmostEqual(joined["x"] + joined["width"], whole["x"] + whole["width"],
                               places=6)
        # Order in the list does not matter; position on the line does.
        self.assertIsNotNone(observed(pieces[::-1] + others, ELEANOR, selected=True))

    def test_three_pieces_join_and_a_fourth_is_too_many(self) -> None:
        whole = the_label(READS["leader_crop"])
        head, tail = split(whole, "Eleanor of Aquitaine", "(France)")
        first, second = split(head, "Eleanor of", "Aquitaine")
        self.assertEqual(observed([first, second, tail], ELEANOR, selected=True)["pieces"], 3)
        one, two = split(first, "Eleanor", "of")
        self.assertIsNone(observed([one, two, second, tail], ELEANOR, selected=True))

    def test_the_closed_field_reads_a_split_label(self) -> None:
        def with_split(observations: list[dict]) -> list[dict]:
            whole = the_label(observations)
            return ([observation for observation in observations if observation is not whole]
                    + split(whole, *self.PIECES))

        label, point = read_field(with_split(READS["window"]), with_split(READS["crop"]),
                                  ELEANOR)
        self.assertEqual(label, ELEANOR)
        self.assertLessEqual(abs(point[0] - 444) + abs(point[1] - 200), 2)

    def test_a_split_row_in_the_open_picker_is_found_on_its_own_line(self) -> None:
        whole = the_label(READS["leader_crop"])
        lower = dict(whole, y=whole["y"] + 0.10)       # a list row, mid-window
        row = observed(split(lower, *self.PIECES), ELEANOR)
        self.assertIsNotNone(row)
        self.assertAlmostEqual(row["y"], lower["y"], places=6)
        # Joining does not move a band: a list row is not the selected field,
        # the field is not a list row, and a label in another column is neither.
        self.assertIsNone(observed(split(lower, *self.PIECES), ELEANOR, selected=True))
        self.assertIsNone(observed(split(dict(whole, y=whole["y"] - 0.02), *self.PIECES),
                                   ELEANOR))
        aside = dict(whole, x=whole["x"] + 0.12)        # the description panel's column
        for selected in (True, False):
            self.assertIsNone(observed(split(dict(aside, y=lower["y"] if not selected
                                                  else whole["y"]), *self.PIECES),
                                       ELEANOR, selected=selected))

    def test_pieces_that_are_not_one_line_are_not_one_label(self) -> None:
        whole = the_label(READS["leader_crop"])
        for why, pieces in {
            "the second piece is the next row": split(whole, *self.PIECES, lines_down=2.0),
            "the second piece is across a column gap": split(whole, *self.PIECES, gap=8.5),
            "the pieces are in the wrong order": split(whole, *self.PIECES[::-1]),
            "the pieces spell her other persona": split(whole, self.PIECES[0], "(England)"),
            "a piece is missing": split(whole, *self.PIECES)[:1],
        }.items():
            with self.subTest(why=why):
                self.assertIsNone(observed(pieces, ELEANOR, selected=True))
                self.assertIsNone(observed(pieces, ELEANOR))

    def test_the_description_panel_is_further_than_a_join_reaches(self) -> None:
        """The margin the gap bound was chosen against, on the saved frame."""
        label = the_label(READS["window"])
        panel = min((observation for observation in READS["window"]
                     if observation["x"] > label["x"] + label["width"]
                     and abs(observation["y"] - label["y"]) < label["height"]),
                    key=lambda observation: observation["x"])
        gap = (panel["x"] - label["x"] - label["width"]) * DESKTOP[0]
        line = label["height"] * DESKTOP[1]
        self.assertGreater(gap / line, 2 * civ6_play.LEADER_LABEL_MAX_GAP_LINES)

    def test_two_rows_of_the_list_never_join(self) -> None:
        whole = the_label(READS["leader_crop"])
        england = dict(whole, text="Eleanor of Aquitaine (England)", y=whole["y"] + 0.10)
        stray = dict(split(dict(whole, y=whole["y"] + 0.12), *self.PIECES)[1])
        self.assertIsNone(observed([england, stray], ELEANOR))


class PickerLabelTableTests(unittest.TestCase):
    """`LEADER_PICKER_LABELS` is the harvested snapshot, for every shipped leader."""

    SNAPSHOT = json.loads(civ6_leader_labels.SNAPSHOT.read_text(encoding="utf-8"))

    def test_the_table_is_the_snapshot(self) -> None:
        self.assertEqual(civ6_play.LEADER_PICKER_LABELS,
                         civ6_leader_labels.labels(self.SNAPSHOT))
        self.assertEqual(list(civ6_play.LEADER_PICKER_LABELS),
                         sorted(civ6_play.LEADER_PICKER_LABELS))

    def test_every_leader_is_offered_by_the_lanes_ruleset(self) -> None:
        """The live lane plays Gathering Storm; no label here belongs to a
        leader that ruleset does not list."""
        self.assertEqual(civ6_leader_labels.labels(self.SNAPSHOT, "RULESET_EXPANSION_2"),
                         civ6_play.LEADER_PICKER_LABELS)
        self.assertEqual(len(civ6_play.LEADER_PICKER_LABELS), 77)
        self.assertEqual(len(civ6_leader_labels.labels(self.SNAPSHOT, "RULESET_STANDARD")), 54)

    def test_every_label_says_where_it_came_from(self) -> None:
        self.assertEqual(self.SNAPSHOT["schema"], civ6_leader_labels.SCHEMA)
        self.assertIn("LeaderName AS Name", self.SNAPSHOT["query"])
        for leader, entry in self.SNAPSHOT["leaders"].items():
            with self.subTest(leader=leader):
                self.assertTrue(entry["label"].strip())
                self.assertRegex(entry["name_key"], r"^LOC_LEADER_[A-Z0-9_]+_NAME$")
                self.assertRegex(entry["civilization"], r"^CIVILIZATION_[A-Z_]+$")
                self.assertTrue(entry["civilization_name"])
                self.assertRegex(entry["text"], r"^(Base|DLC)/.+/en_US/.+\.xml$")
                self.assertTrue(entry["rows"])
                for path, rulesets in entry["rows"].items():
                    self.assertRegex(path, r"^(Base|DLC)/.+\.xml$")
                    self.assertTrue(set(rulesets) <= set(self.SNAPSHOT["rulesets"]))

    def test_a_persona_pack_renames_the_base_leader(self) -> None:
        """The rows this table used to get wrong: the pack replaces the base
        leader's row, so the plain name is on no row at all."""
        expected = {
            "LEADER_T_ROOSEVELT": "Teddy Roosevelt (Bull Moose)",
            "LEADER_T_ROOSEVELT_ROUGHRIDER": "Teddy Roosevelt (Rough Rider)",
            "LEADER_CATHERINE_DE_MEDICI": "Catherine de Medici (Black Queen)",
            "LEADER_CATHERINE_DE_MEDICI_ALT": "Catherine de Medici (Magnificence)",
            "LEADER_CLEOPATRA": "Cleopatra (Egyptian)",
            "LEADER_CLEOPATRA_ALT": "Cleopatra (Ptolemaic)",
            "LEADER_QIN": "Qin (Mandate of Heaven)",
            "LEADER_QIN_ALT": "Qin (Unifier)",
            "LEADER_SALADIN": "Saladin (Vizier)",
            "LEADER_SALADIN_ALT": "Saladin (Sultan)",
            "LEADER_SULEIMAN": "Suleiman (Kanuni)",
            "LEADER_SULEIMAN_ALT": "Suleiman (Muhteşem)",
            "LEADER_VICTORIA": "Victoria (Age of Empire)",
            "LEADER_VICTORIA_ALT": "Victoria (Age of Steam)",
            "LEADER_HARDRADA": "Harald Hardrada (Konge)",
            "LEADER_HARALD_ALT": "Harald Hardrada (Varangian)",
            "LEADER_KUBLAI_KHAN_CHINA": "Kublai Khan (China)",
            "LEADER_KUBLAI_KHAN_MONGOLIA": "Kublai Khan (Mongolia)",
            "LEADER_ELEANOR_ENGLAND": "Eleanor of Aquitaine (England)",
            "LEADER_ELEANOR_FRANCE": "Eleanor of Aquitaine (France)",
        }
        for leader, label in expected.items():
            with self.subTest(leader=leader):
                self.assertEqual(civ6_play.leader_display_name(leader), label)
        plain = {"Teddy Roosevelt", "Catherine de Medici", "Cleopatra", "Qin Shi Huang",
                 "Saladin", "Suleiman", "Victoria", "Harald Hardrada", "Kublai Khan"}
        self.assertFalse(plain & set(civ6_play.LEADER_PICKER_LABELS.values()))

    def test_a_label_the_type_does_not_spell(self) -> None:
        for leader, label in {
            "LEADER_BASIL": "Basil II", "LEADER_MENELIK": "Menelik II",
            "LEADER_LUDWIG": "Ludwig II", "LEADER_ELIZABETH": "Elizabeth I",
            "LEADER_RAMSES": "Ramses II", "LEADER_LADY_TRIEU": "Bà Triệu",
            "LEADER_JOAO_III": "João III", "LEADER_PETER_GREAT": "Peter",
            "LEADER_BARBAROSSA": "Frederick Barbarossa", "LEADER_HOJO": "Hojo Tokimune",
            "LEADER_LAURIER": "Wilfrid Laurier", "LEADER_MVEMBA": "Mvemba a Nzinga",
            "LEADER_PEDRO": "Pedro II", "LEADER_JAYAVARMAN": "Jayavarman VII",
            "LEADER_SIMON_BOLIVAR": "Simón Bolívar",
        }.items():
            with self.subTest(leader=leader):
                self.assertEqual(civ6_play.leader_display_name(leader), label)

    def test_every_leader_the_ledger_has_seated_resolves(self) -> None:
        self.assertEqual(len(SEATED_ON_THE_LEDGER), 70)
        missing = [leader for leader in SEATED_ON_THE_LEDGER
                   if leader not in civ6_play.LEADER_PICKER_LABELS]
        self.assertEqual(missing, [])

    def test_no_two_leaders_read_alike(self) -> None:
        """The read-back compares normalized text, and a join extends a prefix:
        no label may equal, or begin, another."""
        normalized = sorted(civ6_play._normalized_label(label)
                            for label in civ6_play.LEADER_PICKER_LABELS.values())
        self.assertEqual(len(set(normalized)), len(civ6_play.LEADER_PICKER_LABELS))
        self.assertEqual([(a, b) for a in normalized for b in normalized
                          if a != b and b.startswith(a)], [])

    def test_an_unshipped_type_still_gets_a_derived_name(self) -> None:
        self.assertEqual(civ6_play.leader_display_name("LEADER_NOT_A_SHIPPED_ONE"),
                         "Not A Shipped One")


def bootstrap(configure, *, leader: str = "LEADER_ELEANOR_FRANCE"):
    """Run `bootstrap_game` against a menu that always reads, with ``configure``
    standing in for the Create Game screen. Returns (started, args, mocks, stderr)."""
    args = SimpleNamespace(tag="civvis-test", leader=leader, leader_intro_defer=0.0)
    said = io.StringIO()
    with tempfile.TemporaryDirectory() as temporary, \
         mock.patch.object(civ6_play, "focus_game"), \
         mock.patch.object(civ6_play, "game_window", return_value=WINDOW), \
         mock.patch.object(civ6_play, "click_at"), \
         mock.patch.object(civ6_play, "screenshot", return_value=True), \
         mock.patch.object(civ6_play, "_poll_screen", return_value=((400, 285), [])), \
         mock.patch.object(civ6_play, "_observed_label_point", return_value=None), \
         mock.patch.object(civ6_play, "configure_and_start", side_effect=configure) as setup, \
         mock.patch.object(civ6_play, "return_to_main_menu", return_value=True) as back, \
         mock.patch.object(civ6_play, "advance_leader_intro") as intro, \
         mock.patch.object(civ6_play.time, "sleep"), \
         contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(said):
        started = civ6_play.bootstrap_game(mock.Mock(), mock.Mock(), Path(temporary), args,
                                           verify_s=0.01)
    return started, args, SimpleNamespace(setup=setup, back=back, intro=intro), said.getvalue()


class UnverifiedLeaderEndsTheLaunchTests(unittest.TestCase):
    """Thirteen rounds of the same failed read became two and a stated ending."""

    @staticmethod
    def leader_fails(bounds, args, run_dir, fault=None) -> bool:
        if fault is not None:
            fault["step"] = "leader"
        return False

    def test_setup_names_the_leader_as_the_row_that_failed(self) -> None:
        def configure(leader_ok: bool, dropdown_ok: bool = True) -> tuple[bool, dict]:
            fault: dict = {}
            with tempfile.TemporaryDirectory() as temporary, \
                 mock.patch.object(civ6_play, "set_dropdown", return_value=dropdown_ok), \
                 mock.patch.object(civ6_play, "select_requested_map", return_value=True), \
                 mock.patch.object(civ6_play, "select_requested_leader",
                                   return_value=leader_ok), \
                 mock.patch.object(civ6_play, "clear_game_modes", return_value=False), \
                 mock.patch.object(civ6_play, "screenshot"), \
                 mock.patch.object(civ6_play, "click_at"), \
                 contextlib.redirect_stdout(io.StringIO()):
                started = civ6_play.configure_and_start(
                    WINDOW, SimpleNamespace(difficulty="DIFFICULTY_DEITY", map="Continents.lua",
                                            map_size="MAPSIZE_SMALL", speed="GAMESPEED_ONLINE",
                                            leader="LEADER_ELEANOR_FRANCE", game_mode=[]),
                    Path(temporary), fault)
            return started, fault

        self.assertEqual(configure(leader_ok=False), (False, {"step": "leader"}))
        # A dropdown, or a game mode still on, is not the leader's failure.
        self.assertEqual(configure(leader_ok=True, dropdown_ok=False), (False, {}))
        self.assertEqual(configure(leader_ok=True), (False, {}))

    def test_two_rounds_on_the_leader_end_the_bootstrap(self) -> None:
        started, args, mocks, said = bootstrap(self.leader_fails)
        self.assertFalse(started)
        self.assertEqual(civ6_play.LEADER_UNVERIFIED_ROUNDS, 2)
        self.assertEqual(mocks.setup.call_count, 2)
        # Backed out once, between the rounds; the round that ends the launch
        # does not walk back to a menu it will never use.
        self.assertEqual(mocks.back.call_count, 1)
        mocks.intro.assert_not_called()
        self.assertEqual(args.setup_fault, {
            "reason": "leader_unverified", "leader": "LEADER_ELEANOR_FRANCE",
            "label": ELEANOR, "rounds": 2})
        self.assertEqual(civ6_play.leader_unverified_fault(args), args.setup_fault)
        self.assertEqual(said.count("setup could not be verified; backing out"), 1)
        self.assertIn(f"attempt 2: requested leader {ELEANOR} (LEADER_ELEANOR_FRANCE) "
                      "could not be verified on the Create Game screen in 2 setup rounds; "
                      "ending this launch (leader_unverified)", said)

    def test_the_rounds_need_not_be_consecutive(self) -> None:
        failures = iter(["leader", "difficulty", "leader", "leader"])

        def configure(bounds, args, run_dir, fault=None) -> bool:
            if next(failures) == "leader":
                fault["step"] = "leader"
            return False

        started, args, mocks, _ = bootstrap(configure)
        self.assertFalse(started)
        self.assertEqual(mocks.setup.call_count, 3)
        self.assertEqual(args.setup_fault["rounds"], 2)

    def test_any_other_setup_failure_keeps_the_whole_budget(self) -> None:
        """Only the leader is bounded: a dropdown that will not verify still
        gets every bootstrap attempt, and ends as it always did."""
        started, args, mocks, said = bootstrap(lambda *_: False)
        self.assertFalse(started)
        self.assertEqual(mocks.setup.call_count, civ6_play.BOOTSTRAP_ATTEMPTS)
        self.assertIsNone(civ6_play.leader_unverified_fault(args))
        self.assertEqual(said.count("setup could not be verified; backing out"),
                         civ6_play.BOOTSTRAP_ATTEMPTS)

    def test_a_leader_that_verifies_on_the_second_round_starts_the_game(self) -> None:
        rounds = iter([False, True])

        def configure(bounds, args, run_dir, fault=None) -> bool:
            ok = next(rounds)
            if not ok:
                fault["step"] = "leader"
            return ok

        with mock.patch.object(civ6_play, "wait_for_agent_start", return_value=True), \
             mock.patch.object(civ6_play.env, "game_pids", return_value=[4242]):
            started, args, mocks, _ = bootstrap(configure)
        self.assertTrue(started)
        self.assertEqual(mocks.setup.call_count, 2)
        self.assertIsNone(civ6_play.leader_unverified_fault(args))

    def test_the_summary_is_a_no_board_record_with_its_own_reason(self) -> None:
        config = {"Difficulty": "DIFFICULTY_DEITY", "MapSize": "MAPSIZE_SMALL",
                  "GameSpeed": "GAMESPEED_ONLINE", "MaxTurns": 250}
        state = {"turn": -1, "score": -1}
        fault = {"reason": "leader_unverified", "leader": "LEADER_ELEANOR_FRANCE",
                 "label": ELEANOR, "rounds": 2}
        summary = civ6_play.leader_unverified_summary("civvis-test", config, state, fault)
        stopped = civ6_play.partial_summary("civvis-test", config, state)
        self.assertEqual(summary["reason"], "leader_unverified")
        self.assertEqual(stopped["reason"], "killed")
        self.assertEqual((summary["leader_asked"], summary["leader_label"],
                          summary["setup_rounds"]), ("LEADER_ELEANOR_FRANCE", ELEANOR, 2))
        # Every field the ledger reads is the stopped launch's, `partial` included.
        for key in set(stopped) - {"reason", "finished_utc"}:
            self.assertEqual(summary[key], stopped[key], key)
        self.assertIs(summary["partial"], True)

    def test_the_exit_status_is_its_own_and_follows_the_child_stop(self) -> None:
        source = Path(civ6_play.__file__).read_text()
        self.assertEqual(civ6_play.EXIT_LEADER_UNVERIFIED, 7)
        # No other ending of the harness returns it.
        self.assertNotIn("return 7", source)
        self.assertEqual(source.count("return EXIT_LEADER_UNVERIFIED"), 1)
        failure = source.index('"could not start a game from the main menu", file=sys.stderr)')
        tail = source[failure:source.index("return 5", failure)]
        stop = tail.index('launcher.stop_owned_failed_startup(owned_launch["process"])')
        written = tail.index("leader_unverified_summary(args.tag, config, state, leader_fault)")
        left = tail.index("return EXIT_LEADER_UNVERIFIED")
        self.assertLess(stop, written)
        self.assertLess(written, left)
        self.assertIn('path = run_dir / "summary.json"', tail[stop:written])

    def test_a_restarted_game_is_not_relaunched_to_ask_again(self) -> None:
        source = inspect.getsource(civ6_play._play)
        self.assertIn("bootstrapped = restart_in_place(tail, record, run_dir, args, handoff)\n",
                      source)
        self.assertIn("if not bootstrapped and leader_unverified_fault(args) is None:", source)

    def test_the_climb_files_it_as_a_launch_that_reached_no_board(self) -> None:
        """What the climb does with a no-board launch is unchanged: the row
        carries the summary's reason and no played turn, it spends the attempt
        (`last_turn` is -1, not None, as for a stopped launch), and the batch
        moves on."""
        import civ6_civvis_climb

        config = {"Difficulty": "DIFFICULTY_DEITY", "MapSize": "MAPSIZE_SMALL",
                  "GameSpeed": "GAMESPEED_ONLINE", "MaxTurns": 250}
        fault = {"reason": "leader_unverified", "leader": "LEADER_ELEANOR_FRANCE",
                 "label": ELEANOR, "rounds": 2}
        rows = {}
        with tempfile.TemporaryDirectory() as temporary:
            for tag, summary in {
                "civvis-unverified": civ6_play.leader_unverified_summary(
                    "civvis-unverified", config, {"turn": -1, "score": -1}, fault),
                "civvis-stopped": civ6_play.partial_summary(
                    "civvis-stopped", config, {"turn": -1, "score": -1}),
            }.items():
                (Path(temporary) / tag).mkdir()
                (Path(temporary) / tag / "summary.json").write_text(json.dumps(summary))
            with mock.patch.object(civ6_civvis_climb, "RUN_ROOT", Path(temporary)):
                for tag in ("civvis-unverified", "civvis-stopped"):
                    rows[tag] = civ6_civvis_climb.outcome_of(tag)
        unverified, stopped = rows["civvis-unverified"], rows["civvis-stopped"]
        self.assertEqual(unverified["reason"], "leader_unverified")
        self.assertEqual(stopped["reason"], "killed")
        for key in ("last_turn", "last_score", "partial", "seat", "leader", "victory"):
            self.assertEqual(unverified.get(key), stopped.get(key), key)
        self.assertEqual(unverified["last_turn"], -1)
        self.assertFalse(civ6_civvis_climb.is_native_crash("exited", unverified))


if __name__ == "__main__":
    unittest.main()
