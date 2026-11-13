"""Tests for tools/fit_host_enemy.py on a hand-written event fixture and on
synthetic rows with known answers. No live logs are read."""

from __future__ import annotations

import collections
import math
import random
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import fit_host_enemy as F  # noqa: E402

OUR_WARRIOR = 1001
OUR_ARCHER = 1002
RIVAL = 1
RIVAL_WARRIOR = 7
RIVAL_ARCHER = 8
BARB_WARRIOR = 7  # the same id as the rival's: ids are per player


def unit(uid, kind, x, y, hp=100, **extra):
    data = {
        "UNIT_WARRIOR": (20, 0, "PROMOTION_CLASS_MELEE"),
        "UNIT_ARCHER": (15, 25, "PROMOTION_CLASS_RANGED"),
        "UNIT_BUILDER": (0, 0, None),
    }[kind]
    row = {"id": uid, "kind": kind, "x": x, "y": y, "hp": hp, "combat": data[0], "ranged": data[1], "max_moves": 2}
    if data[2]:
        row["class"] = data[2]
    row.update(extra)
    return row


def state(turn, frame, ours, rival_units, barbarians, at_war=True, rival_cities=()):
    return {
        "kind": "state",
        "turn": turn,
        "frame": frame,
        "units": ours,
        "cities": [{"id": 1, "x": 0, "y": 0}],
        "rivals": [
            {
                "player": RIVAL,
                "at_war": at_war,
                "units": rival_units,
                "cities": [{"x": x, "y": y} for x, y in rival_cities],
            }
        ],
        "minors": [],
        "hostiles": barbarians,
    }


def fixture():
    """Turn 5: our warrior beside a rival warrior, our archer stepping up
    after the frame, a barbarian warrior five tiles off. Our warrior wounds
    the rival; the rival strikes our warrior back on its turn; the barbarian
    closes; a junk row is ignored. Turn 6's first frame closes the turn."""
    ours = [
        unit(OUR_WARRIOR, "UNIT_WARRIOR", 5, 5),
        unit(OUR_ARCHER, "UNIT_ARCHER", 8, 5),
        unit(1003, "UNIT_BUILDER", 4, 5),
    ]
    rivals = [
        unit(RIVAL_WARRIOR, "UNIT_WARRIOR", 6, 5),
        unit(RIVAL_ARCHER, "UNIT_ARCHER", 5, 12),
        unit(9, "UNIT_WARRIOR", 3, 9, embarked=True),
    ]
    barbs = [unit(BARB_WARRIOR, "UNIT_WARRIOR", 12, 5, player=63, type="UNIT_WARRIOR")]
    del barbs[0]["kind"]
    return [
        state(5, 0, ours, rivals, barbs, rival_cities=[(7, 4)]),
        {"kind": "host_move", "turn": 5, "unit": OUR_ARCHER, "x": 7, "y": 5},
        {
            "kind": "combat",
            "turn": 5,
            "our_turn": True,
            "ours": True,
            "attacker": {"type": "unit", "id": OUR_WARRIOR, "player": 0, "hp": 100},
            "defender": {"type": "unit", "id": RIVAL_WARRIOR, "player": RIVAL, "hp": 100},
            "attacker_hp_end": 75,
            "damage_to_defender": 30,
            "defender_killed": False,
        },
        {"kind": "end_turn_wait", "turn": 5},
        {
            "kind": "combat",
            "turn": 5,
            "our_turn": False,
            "against_us": True,
            "attacker": {"type": "unit", "id": RIVAL_WARRIOR, "player": RIVAL, "x": 6, "y": 5},
            "defender": {"type": "unit", "id": OUR_WARRIOR, "player": 0},
        },
        {
            "kind": "combat",
            "turn": 5,
            "our_turn": False,
            "against_us": True,
            "attacker": {"type": "district", "id": -1, "player": -1},
            "defender": {"type": "district", "id": -1, "player": -1},
        },
        state(
            6,
            0,
            ours,
            [unit(RIVAL_WARRIOR, "UNIT_WARRIOR", 6, 5, hp=50)],
            [dict(barbs[0], x=10, y=5)],
        ),
    ]


class GeometryAndCombat(unittest.TestCase):
    def test_hex_distance_is_odd_r(self):
        self.assertEqual(F.hex_distance((5, 5), (6, 5)), 1)
        self.assertEqual(F.hex_distance((5, 5), (5, 6)), 1)
        self.assertEqual(F.hex_distance((5, 5), (8, 5)), 3)
        self.assertEqual(F.hex_distance((0, 0), (0, 4)), 4)

    def test_the_wound_penalty_and_damage_are_the_engines(self):
        self.assertEqual(F.effective_strength(30, 40), 24)
        self.assertEqual(F.effective_strength(30, 100), 30)
        self.assertEqual(F.effective_strength(30, 75), 27)
        self.assertEqual(F.effective_strength(30, 85), 28)
        self.assertAlmostEqual(F.mean_damage(30, 30), 30.0)
        self.assertEqual(F.mean_damage(10, 100), 1.0)
        self.assertEqual(F.mean_damage(90, 10), 100.0)

    def test_reach_on_open_ground(self):
        warrior = unit(1, "UNIT_WARRIOR", 0, 0)
        self.assertEqual(F.reaches(warrior, "melee", 1), (True, True))
        self.assertEqual(F.reaches(warrior, "melee", 2), (True, False))
        self.assertEqual(F.reaches(warrior, "melee", 3), (False, False))
        archer = unit(2, "UNIT_ARCHER", 0, 0)
        self.assertEqual(F.reaches(archer, "ranged", 2), (True, True))
        self.assertEqual(F.reaches(archer, "ranged", 3), (True, False))
        self.assertEqual(F.reaches(archer, "ranged", 4), (False, False))
        catapult = {"kind": "UNIT_CATAPULT", "max_moves": 2}
        self.assertEqual(F.reaches(catapult, "siege", 3), (False, False))
        crew = dict(catapult, promotions=["PROMOTION_EXPERT_CREW"])
        self.assertEqual(F.reaches(crew, "siege", 3), (True, False))

    def test_classes(self):
        self.assertEqual(F.unit_class(unit(1, "UNIT_WARRIOR", 0, 0)), "melee")
        self.assertEqual(F.unit_class(unit(1, "UNIT_ARCHER", 0, 0)), "ranged")
        self.assertEqual(F.unit_class({"kind": "UNIT_CATAPULT"}), "siege")
        self.assertEqual(F.unit_class({"kind": "UNIT_HORSEMAN"}), "cavalry")
        self.assertIsNone(F.unit_class(unit(1, "UNIT_BUILDER", 0, 0)))
        self.assertIsNone(F.unit_class(unit(1, "UNIT_WARRIOR", 0, 0, embarked=True)))
        self.assertIsNone(F.unit_class({"kind": "UNIT_GALLEY", "class": "PROMOTION_CLASS_NAVAL_MELEE"}))


class Extraction(unittest.TestCase):
    def setUp(self):
        stats = collections.Counter()
        self.rows = {(r["player"], r["id"]): r for r in F.extract_game("g", fixture(), stats)}
        self.stats = stats

    def test_one_row_per_visible_land_fighter_keyed_by_player(self):
        self.assertEqual(set(self.rows), {(RIVAL, RIVAL_WARRIOR), (RIVAL, RIVAL_ARCHER), (63, BARB_WARRIOR)})
        self.assertEqual(self.stats["turns"], 1)

    def test_our_strike_after_the_frame_wounds_the_rival(self):
        self.assertEqual(self.rows[(RIVAL, RIVAL_WARRIOR)]["hp"], 70)

    def test_the_enemy_strike_and_its_targets(self):
        row = self.rows[(RIVAL, RIVAL_WARRIOR)]
        self.assertEqual(row["struck"], OUR_WARRIOR)
        self.assertEqual(row["owner"], "major")
        self.assertTrue(row["home"])
        ids = {t["id"]: t for t in row["targets"]}
        # Our archer moved up after the frame and is now beside it; the
        # Builder is no target.
        self.assertEqual(set(ids), {OUR_WARRIOR, OUR_ARCHER})
        self.assertTrue(ids[OUR_ARCHER]["still"])
        # Our warrior is wounded to 75 by its own strike, the rival to 70:
        # both lose 3 strength (2.5 rounds away from zero, as in the engine).
        self.assertEqual(ids[OUR_WARRIOR]["hp"], 75)
        self.assertAlmostEqual(ids[OUR_WARRIOR]["edge"], 0.0)
        self.assertTrue(F.struck_us(row))

    def test_the_barbarian_closed_and_the_far_archer_is_out_of_reach(self):
        barb = self.rows[(63, BARB_WARRIOR)]
        self.assertEqual(barb["owner"], "barbarian")
        self.assertEqual(barb["targets"], [])
        self.assertEqual(barb["gap"], 5)
        self.assertEqual(barb["moved"], "closer")
        self.assertIsNone(barb["struck"])
        archer = self.rows[(RIVAL, RIVAL_ARCHER)]
        self.assertEqual(archer["targets"], [])
        self.assertIsNone(archer["moved"])

    def test_peace_makes_no_rows(self):
        events = fixture()
        events[0] = state(5, 0, events[0]["units"], events[0]["rivals"][0]["units"], [], at_war=False)
        rows = list(F.extract_game("g", events, collections.Counter()))
        self.assertEqual(rows, [])


def synthetic_rows(games=30, per_game=120, seed=3):
    """Rows whose strike follows a known logistic in `still` and `owner`, and
    whose target is the nearest unit, from fixed coefficients."""
    rng = random.Random(seed)
    rows = []
    for g in range(games):
        for k in range(per_game):
            n = rng.randint(1, 3)
            targets = []
            for t in range(n):
                d = rng.randint(1, 3)
                targets.append(
                    {
                        "id": 100 + t,
                        "kind": "UNIT_WARRIOR",
                        "class": "melee",
                        "d": d,
                        "still": d == 1,
                        "hp": rng.choice([40, 70, 100]),
                        "edge": rng.uniform(-15, 15),
                        "dealt": rng.uniform(10, 60),
                        "taken": rng.uniform(10, 60),
                    }
                )
            if rng.random() < 0.2:
                targets = []
            owner = rng.choice(F.OWNERS)
            still = any(t["still"] for t in targets)
            z = -2.0 + (2.0 if still else 0.0) + (1.0 if owner == "barbarian" else 0.0)
            hit = bool(targets) and rng.random() < 1 / (1 + math.exp(-z))
            struck = None
            if hit:
                weights = [math.exp(-1.5 * t["d"]) for t in targets]
                r = rng.random() * sum(weights)
                for t, w in zip(targets, weights):
                    r -= w
                    if r < 0:
                        struck = t["id"]
                        break
                struck = struck if struck is not None else targets[-1]["id"]
            rows.append(
                {
                    "game": f"game-{g}",
                    "turn": k,
                    "player": 1,
                    "id": k,
                    "kind": "UNIT_WARRIOR",
                    "owner": owner,
                    "class": "melee",
                    "hp": rng.choice([20, 60, 100]),
                    "moves": 2,
                    "home": rng.random() < 0.1,
                    "gap": rng.randint(1, 8),
                    "targets": targets,
                    "struck": struck,
                    "moved": None if hit else rng.choice(["closer", "hold", "hold", "level"]),
                }
            )
    return rows


class Fitting(unittest.TestCase):
    def test_logistic_recovers_known_weights(self):
        rng = random.Random(1)
        cells = collections.defaultdict(lambda: [0, 0])
        for _ in range(20000):
            x = (1.0, float(rng.random() < 0.5), float(rng.random() < 0.3))
            p = 1 / (1 + math.exp(-(-1.0 + 1.5 * x[1] - 0.8 * x[2])))
            cells[x][0] += 1
            cells[x][1] += rng.random() < p
        w = F.fit_logistic([(list(x), n, y) for x, (n, y) in cells.items()], ridge=0.0)
        for got, want in zip(w, (-1.0, 1.5, -0.8)):
            self.assertAlmostEqual(got, want, delta=0.12)

    def test_choice_recovers_known_weights(self):
        rng = random.Random(2)
        sets = []
        for _ in range(3000):
            xs = [[rng.random(), float(rng.random() < 0.5)] for _ in range(rng.randint(2, 4))]
            us = [2.0 * x[0] - 1.0 * x[1] for x in xs]
            ws = [math.exp(u) for u in us]
            r = rng.random() * sum(ws)
            chosen = len(xs) - 1
            for k, w in enumerate(ws):
                r -= w
                if r < 0:
                    chosen = k
                    break
            sets.append((xs, chosen))
        w = F.fit_choice(sets, ridge=0.0)
        self.assertAlmostEqual(w[0], 2.0, delta=0.25)
        self.assertAlmostEqual(w[1], -1.0, delta=0.15)

    def test_folds_split_by_game_not_row(self):
        rows = synthetic_rows(games=10, per_game=5)
        by_game = collections.defaultdict(set)
        for r in rows:
            by_game[r["game"]].add(F.fold_of(r["game"], 5))
        self.assertTrue(all(len(f) == 1 for f in by_game.values()))

    def test_the_fit_beats_both_baselines_on_held_out_games(self):
        fit = F.fit_all(synthetic_rows(), folds=5)
        strike = fit["strike"]["held_out_ll"]
        self.assertGreater(strike["fitted"], strike["constant"])
        self.assertGreater(strike["fitted"], strike["stock 0.5"])
        self.assertGreater(fit["strike"]["weights"]["still=no move needed"], 1.0)
        target = fit["target"]["held_out_ll"]
        self.assertGreater(target["fitted"], target["uniform"])
        self.assertLess(fit["target"]["weights"]["distance"], -0.5)
        self.assertEqual(fit["strike"]["held_out_rows"], fit["strike"]["rows"])
        for owner in F.OWNERS:
            s = fit["summary"][owner]
            self.assertAlmostEqual(s["rate"], s["struck"] / s["rows"])

    def test_the_doc_and_rust_tables_name_what_host_enemy_rs_reads(self):
        fit = F.fit_all(synthetic_rows(games=10, per_game=60), folds=5)
        doc = F.doc_tables(fit)
        for name in ("intercept", "class=cavalry", "still=no move needed", "no move", "hp<=33", "hp>33", "major", "barbarian"):
            self.assertIn(f"| {name} |", doc)
        rust = F.rust_constants(fit)
        for const in ("STRIKE_INTERCEPT", "STRIKE_STILL: [f32; 2]", "TARGET_WEIGHTS: [f32; 6]", "ADVANCE: [[f32; 4]; 2]", "STRIKE_RATE_BARBARIAN"):
            self.assertIn(const, rust)



class Rates(unittest.TestCase):
    def test_the_rate_table_reads_strikes_targets_and_moves(self):
        def target(tid, d, hp, dealt):
            return {"id": tid, "d": d, "hp": hp, "dealt": dealt, "still": d == 1}

        rows = [
            # Struck the nearest of two, which is not the weakest.
            {"targets": [target(1, 1, 100, 30), target(2, 2, 40, 30)], "struck": 1, "moved": None, "gap": 1, "hp": 90},
            # Declined from a distance, then closed.
            {"targets": [target(3, 2, 100, 30)], "struck": None, "moved": "closer", "gap": 2, "hp": 90},
            # Nothing in reach, wounded, stepped away.
            {"targets": [], "struck": None, "moved": "away", "gap": 4, "hp": 20},
            # Struck a city: not a strike on ours, not a closing row.
            {"targets": [target(4, 1, 100, 30)], "struck": "city", "moved": "hold", "gap": 1, "hp": 90},
        ]
        table = F.rate_table(rows)
        self.assertEqual(table["strike | in reach"], [1, 3])
        self.assertEqual(table["strike | in reach without moving"], [1, 2])
        self.assertEqual(table["strike | must move first"], [0, 1])
        self.assertEqual(table["target: nearest"], [1, 1])
        self.assertEqual(table["target: weakest"], [0, 1])
        self.assertEqual(table["target: most damaged"], [1, 1])
        self.assertEqual(table["target: most damaged (chance)"], [1.0, 1])
        self.assertEqual(table["target: nearest (chance)"], [0.5, 1])
        self.assertEqual(table["closes | struck nothing"], [1, 2])
        self.assertEqual(table["closes | declined"], [1, 1])
        self.assertEqual(table["steps away | hp<=25, struck nothing"], [1, 1])

if __name__ == "__main__":
    unittest.main()
