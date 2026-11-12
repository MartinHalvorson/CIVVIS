#!/usr/bin/env python3
"""The host AI's tactical habits, fitted to live games, for the battle solver's enemy.

The battle solver (`src/ai/advanced/battle_solver/`, `docs/BATTLE_SOLVER.md`)
plays each local fight forward against a modelled enemy, and its own tuning
names that model as the binding limit: a greedy, always-swinging enemy read
-100 on the bench, one that acts half the time read +262. This tool replaces
the guess with a fit to what the real host AI did in recorded games.

Two steps, both read-only on the logs:

``extract`` reads each run's ``events.jsonl`` and writes one row per
*enemy unit-turn*: a visible, at-war, land military unit standing at the end
of our turn, the units of ours it could strike in its coming turn (reach is
approximated, see ``reaches``), whether it struck one of ours, whom, and where
it stood at the start of our next turn.

``fit`` fits three small tables on those rows and scores each on games held
out of the fit (five folds, split by game, never by row):

* P(strikes one of ours | one or more in reach): a logistic on one-hot
  buckets the solver's model can compute for itself (class, owner, hit
  points, beside its own city, best strength edge, targets in reach, whether
  one is in reach without moving);
* the target, given that it strikes: a conditional logit over the targets in
  reach (target hit points, expected damage, whether the blow is expected to
  kill, a shooter target, distance, no move needed);
* P(advances | nothing in reach): a bucket table by owner and gap.

Each is reported against the solver's current rule and a constant. With
``--doc`` it prints the tables for ``docs/HOST_ENEMY_MODEL.md`` and with
``--rust`` the constants for ``battle_solver/host_enemy.rs``.

Usage::

    python3 tools/fit_host_enemy.py extract --out rows.jsonl [RUN_DIR ...]
    python3 tools/fit_host_enemy.py fit rows.jsonl [--json fit.json] [--doc] [--rust]
    python3 tools/fit_host_enemy.py rates rows.jsonl   # the table host_tactics.rs prints

With no run directories ``extract`` reads every non-continuation run under
``~/civvis-civ6-runs/control`` matching ``--glob`` (default
``civvis-2026111*T*``).
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import math
import sys
from pathlib import Path
from typing import Any, Iterable, Iterator

REPO = Path(__file__).resolve().parent.parent
RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"
UNITS_JSON = REPO / "data" / "units.json"

BARBARIAN_PLAYER = 63
FREE_CITIES_PLAYER = 62

CLASSES = ("melee", "ranged", "siege", "cavalry")
OWNERS = ("major", "city_state", "barbarian")
HP_BUCKETS = ("hp<=33", "hp34-66", "hp67-100")
EDGE_BUCKETS = ("edge<-10", "edge-10..-1", "edge0..9", "edge>=10")
TARGET_BUCKETS = ("1 target", "2 targets", "3+ targets")
GAP_BUCKETS = ("gap<=3", "gap4-5", "gap6-8")
#: Where a unit that struck nothing stood: one of ours in reach (it declined),
#: or nothing in reach and the nearest unit of ours this far off.
SITUATIONS = ("declined", "gap<=3", "gap4-5", "gap6-8")
ADVANCE_HP = ("hp<=33", "hp>33")
#: Beyond this many tiles a unit is not in the solver's fight
#: (`extract::ENGAGE_RADIUS` 5 plus the ground it keeps around a unit).
MAX_GAP = 8

# Promotion class -> the four classes the solver distinguishes. Anything not
# listed (naval, air, support, religious, civilian) is not a land fighter.
PROMOTION_CLASS = {
    "melee": "melee",
    "anti_cavalry": "melee",
    "recon": "melee",
    "giant_death_robot": "melee",
    "monk": "melee",
    "ranged": "ranged",
    "siege": "siege",
    "light_cavalry": "cavalry",
    "heavy_cavalry": "cavalry",
}
#: `Game::ranged_pair_bases`: a bombard strength fires at land units at -17.
SIEGE_VS_UNITS = 17.0


# ------------------------------------------------------------------ geometry
def _cube(x: int, y: int) -> tuple[int, int, int]:
    q = x - (y - (y & 1)) // 2
    return q, y, -q - y


def hex_distance(a: tuple[int, int], b: tuple[int, int]) -> int:
    """Distance between two host offset (odd-r) plots, in hexes."""
    ax, ay, az = _cube(*a)
    bx, by, bz = _cube(*b)
    return max(abs(ax - bx), abs(ay - by), abs(az - bz))


# -------------------------------------------------------------- combat math
def effective_strength(base: float, hp: int) -> float:
    """`dice::effective_strength`: the wound penalty, one point per ten hp
    lost, rounded half away from zero as Rust's `f32::round` does (Python's
    `round` would take 75 hp to a penalty of 2, the engine to 3)."""
    wounded = math.floor(10.0 - max(0, min(100, hp)) / 10.0 + 0.5)
    return max(0.0, base - wounded)


def mean_damage(att: float, defence: float) -> float:
    """`dice::mean_damage`: 30 * e^(delta / 25), clamped to [1, 100]."""
    return max(1.0, min(100.0, 30.0 * math.exp((att - defence) / 25.0)))


# ------------------------------------------------------------------- units
_UNIT_DATA: dict[str, dict[str, Any]] | None = None


def unit_data() -> dict[str, dict[str, Any]]:
    global _UNIT_DATA
    if _UNIT_DATA is None:
        try:
            _UNIT_DATA = json.loads(UNITS_JSON.read_text())
        except (OSError, ValueError):
            _UNIT_DATA = {}
    return _UNIT_DATA


def _spec(kind: str | None, base: str | None = None) -> dict[str, Any]:
    data = unit_data()
    for name in (kind, base):
        if isinstance(name, str) and name.startswith("UNIT_"):
            key = name[5:].lower()
            if key in data:
                return data[key]
    return {}


def unit_class(unit: dict[str, Any]) -> str | None:
    """`melee` / `ranged` / `siege` / `cavalry`, or None for anything that is
    not a land fighter (ships, aircraft, support, civilians, embarked)."""
    if unit.get("embarked"):
        return None
    kind = unit.get("kind") or unit.get("type")
    spec = _spec(kind, unit.get("base"))
    exported = unit.get("class")
    if isinstance(exported, str) and exported.startswith("PROMOTION_CLASS_"):
        promotion = exported[len("PROMOTION_CLASS_"):].lower()
    else:
        promotion = str(spec.get("promotion_class") or "")
    if promotion:
        return PROMOTION_CLASS.get(promotion)
    if spec.get("domain") in ("sea", "air"):
        return None
    if spec.get("siege"):
        return "siege"
    if (unit.get("ranged") or 0) > 0:
        return "ranged"
    if (unit.get("combat") or 0) > 0:
        return "melee"
    return None


def attack_strength(unit: dict[str, Any], klass: str) -> float:
    """Base attack against a land unit, before wounds and terrain."""
    spec = _spec(unit.get("kind") or unit.get("type"), unit.get("base"))
    if klass == "siege":
        bombard = spec.get("bombard_strength") or unit.get("ranged") or 0
        return max(0.0, float(bombard) - SIEGE_VS_UNITS)
    if klass == "ranged":
        return float(unit.get("ranged") or spec.get("ranged_strength") or 0)
    return float(unit.get("combat") or spec.get("strength") or 0)


def defence_strength(unit: dict[str, Any]) -> float:
    """Base defence against melee and ranged blows alike: the melee strength."""
    spec = _spec(unit.get("kind") or unit.get("type"), unit.get("base"))
    return float(unit.get("combat") or spec.get("strength") or 0)


def attack_range(unit: dict[str, Any], klass: str) -> int:
    if klass in ("melee", "cavalry"):
        return 1
    observed = unit.get("range")
    if type(observed) is int and observed > 0:
        return observed
    spec = _spec(unit.get("kind") or unit.get("type"), unit.get("base"))
    return int(spec.get("range") or 2)


def max_moves(unit: dict[str, Any]) -> int:
    moves = unit.get("max_moves")
    if isinstance(moves, (int, float)) and moves > 0:
        return int(moves)
    spec = _spec(unit.get("kind") or unit.get("type"), unit.get("base"))
    return int(spec.get("moves") or 2)


def moves_and_fires(unit: dict[str, Any], klass: str) -> bool:
    """Siege fires only from where it stands unless promoted (Expert Crew)."""
    if klass != "siege":
        return True
    return any("EXPERT_CREW" in str(p) for p in unit.get("promotions") or [])


def reaches(attacker: dict[str, Any], klass: str, distance: int) -> tuple[bool, bool]:
    """`(in reach this turn, in reach without moving)`.

    ⚠ The approximation: open ground. A melee or cavalry unit reaches a target
    `d` tiles off when `d <= max_moves` (it walks `d - 1` tiles and strikes
    with movement left); a shooter when `d <= range + max_moves - 1`; siege
    only from where it stands. Terrain costs, rivers, zones of control and
    units in the way are not read, so this over-counts reach in rough ground.
    The solver's model floods real movement, which is why the fit also
    carries "in reach without moving", a fact the approximation gets right.
    """
    rng = attack_range(attacker, klass)
    moves = max_moves(attacker)
    if klass in ("melee", "cavalry"):
        return distance <= max(1, moves), distance == 1
    still = distance <= rng
    if not moves_and_fires(attacker, klass):
        return still, still
    return distance <= rng + max(0, moves - 1), still


def hp_bucket(hp: int) -> str:
    return HP_BUCKETS[0] if hp <= 33 else HP_BUCKETS[1] if hp <= 66 else HP_BUCKETS[2]


def edge_bucket(edge: float) -> str:
    if edge < -10:
        return EDGE_BUCKETS[0]
    if edge < 0:
        return EDGE_BUCKETS[1]
    if edge < 10:
        return EDGE_BUCKETS[2]
    return EDGE_BUCKETS[3]


def target_bucket(n: int) -> str:
    return TARGET_BUCKETS[min(n, 3) - 1]


def gap_bucket(gap: int) -> str:
    return GAP_BUCKETS[0] if gap <= 3 else GAP_BUCKETS[1] if gap <= 5 else GAP_BUCKETS[2]


# ---------------------------------------------------------------- the board
def _plot(record: dict[str, Any]) -> tuple[int, int] | None:
    x, y = record.get("x"), record.get("y")
    if isinstance(x, int) and isinstance(y, int):
        return x, y
    return None


def _owner_type(player: Any, free: Any, group: str) -> str:
    if player == BARBARIAN_PLAYER:
        return "barbarian"
    if group == "minors" or player == FREE_CITIES_PLAYER or free:
        return "city_state"
    return "major" if group == "rivals" else "barbarian"


def board_of(state: dict[str, Any]) -> dict[str, Any]:
    """The parts of one `state` frame this fit reads, keyed for updates."""
    ours: dict[int, dict[str, Any]] = {}
    for unit in state.get("units") or []:
        uid, plot = unit.get("id"), _plot(unit)
        if isinstance(uid, int) and plot is not None:
            ours[uid] = dict(unit)
    our_cities = {p for p in (_plot(c) for c in state.get("cities") or []) if p is not None}
    enemies: dict[tuple[int, int], dict[str, Any]] = {}
    enemy_cities: dict[int, set[tuple[int, int]]] = collections.defaultdict(set)
    for group in ("rivals", "minors"):
        for actor in state.get(group) or []:
            if not actor.get("at_war"):
                continue
            player = actor.get("player")
            if not isinstance(player, int):
                continue
            for city in actor.get("cities") or []:
                plot = _plot(city)
                if plot is not None:
                    enemy_cities[player].add(plot)
            for unit in actor.get("units") or []:
                uid = unit.get("id")
                if isinstance(uid, int) and _plot(unit) is not None:
                    row = dict(unit)
                    row["player"] = player
                    row["owner"] = _owner_type(player, unit.get("free"), group)
                    enemies[(player, uid)] = row
    for unit in state.get("hostiles") or []:
        uid, player = unit.get("id"), unit.get("player")
        if isinstance(uid, int) and isinstance(player, int) and _plot(unit) is not None:
            if (player, uid) in enemies:
                continue
            row = dict(unit)
            row["owner"] = _owner_type(player, unit.get("free"), "hostiles")
            enemies[(player, uid)] = row
    return {
        "ours": ours,
        "our_cities": our_cities,
        "enemies": enemies,
        "enemy_cities": {k: sorted(v) for k, v in enemy_cities.items()},
    }


def apply_our_combat(board: dict[str, Any], row: dict[str, Any]) -> None:
    """One of our strikes after the frame: wounds and deaths on both sides."""
    attacker = row.get("attacker") or {}
    defender = row.get("defender") or {}
    aid = attacker.get("id")
    if aid in board["ours"]:
        if row.get("attacker_killed"):
            board["ours"].pop(aid, None)
        else:
            end = row.get("attacker_hp_end")
            if not isinstance(end, (int, float)):
                end = (attacker.get("hp") or 0) - (row.get("damage_to_attacker") or 0)
            board["ours"][aid]["hp"] = int(end)
    key = (defender.get("player"), defender.get("id"))
    if defender.get("type") == "unit" and key in board["enemies"]:
        if row.get("defender_killed"):
            board["enemies"].pop(key, None)
        else:
            hp = (defender.get("hp") or 0) - (row.get("damage_to_defender") or 0)
            board["enemies"][key]["hp"] = int(max(1, hp))


def enemy_rows(
    game: str,
    turn: int,
    board: dict[str, Any],
    strikes: list[dict[str, Any]],
    following: dict[str, Any] | None,
) -> Iterator[dict[str, Any]]:
    """One row per visible at-war land fighter at the end of our turn."""
    # Our land fighters a blow could land on: not embarked, not garrisoned
    # (a blow at a unit in its city lands on the city, `Sim::garrisoned`).
    targets = []
    for uid, unit in board["ours"].items():
        klass = unit_class(unit)
        plot = _plot(unit)
        if klass is None or plot is None or plot in board["our_cities"]:
            continue
        targets.append((uid, unit, klass, plot))
    our_plots = [_plot(u) for u in board["ours"].values() if _plot(u) is not None]

    struck: dict[tuple[Any, Any], dict[str, Any]] = {}
    for row in strikes:
        attacker = row.get("attacker") or {}
        if attacker.get("type") != "unit":
            continue
        key = (attacker.get("player"), attacker.get("id"))
        # A unit that struck us and someone else is read as striking us.
        if key not in struck or (row.get("against_us") and not struck[key].get("against_us")):
            struck[key] = row

    later: dict[tuple[int, int], dict[str, Any]] = following["enemies"] if following else {}

    for key, enemy in board["enemies"].items():
        klass = unit_class(enemy)
        plot = _plot(enemy)
        if klass is None or plot is None:
            continue
        hp = int(enemy.get("hp") or 100)
        att = attack_strength(enemy, klass)
        if att <= 0:
            continue
        own = board["enemy_cities"].get(key[0], [])
        home = any(hex_distance(plot, c) <= 1 for c in own)
        in_reach = []
        for uid, unit, tclass, tplot in targets:
            d = hex_distance(plot, tplot)
            ok, still = reaches(enemy, klass, d)
            if not ok:
                continue
            thp = int(unit.get("hp") or 100)
            a = effective_strength(att, hp)
            dfn = effective_strength(defence_strength(unit), thp)
            dealt = mean_damage(a, dfn)
            taken = 0.0 if klass in ("ranged", "siege") else mean_damage(dfn, a)
            in_reach.append(
                {
                    "id": uid,
                    "kind": unit.get("kind"),
                    "class": tclass,
                    "d": d,
                    "still": still,
                    "hp": thp,
                    "edge": round(a - dfn, 2),
                    "dealt": round(dealt, 2),
                    "taken": round(taken, 2),
                }
            )
        gap = min((hex_distance(plot, p) for p in our_plots), default=None)
        outcome = struck.get(key)
        defender = (outcome or {}).get("defender") or {}
        if outcome is None:
            hit = None
        elif not outcome.get("against_us"):
            hit = "other"
        elif defender.get("type") == "unit":
            hit = defender.get("id")
        else:
            hit = "city"
        after = later.get(key)
        moved = None
        if after is not None and _plot(after) is not None and gap is not None:
            new = _plot(after)
            new_gap = min(hex_distance(new, p) for p in our_plots)
            if new == plot:
                moved = "hold"
            elif new_gap < gap:
                moved = "closer"
            elif new_gap > gap:
                moved = "away"
            else:
                moved = "level"
        yield {
            "game": game,
            "turn": turn,
            "player": key[0],
            "id": key[1],
            "kind": enemy.get("kind") or enemy.get("type"),
            "owner": enemy["owner"],
            "class": klass,
            "hp": hp,
            "moves": max_moves(enemy),
            "home": home,
            "gap": gap,
            "targets": in_reach,
            "struck": hit,
            "struck_at": (
                [(outcome.get("attacker") or {}).get("x"), (outcome.get("attacker") or {}).get("y")]
                if outcome
                else None
            ),
            "moved": moved,
        }


def read_run(path: Path) -> Iterator[dict[str, Any]]:
    """Stream one run's events, keeping only the kinds this fit reads."""
    wanted = ('"kind": "state"', '"kind": "combat"', '"kind": "host_move"', '"kind": "end_turn_wait"')
    with path.open("r", errors="replace") as handle:
        for raw in handle:
            if not any(w in raw for w in wanted):
                continue
            try:
                event = json.loads(raw)
            except ValueError:
                continue
            if isinstance(event, dict):
                yield event


def _junk(row: dict[str, Any]) -> bool:
    """The mod's district-vs-district id -1 row (`civ6_tactics_ledger._is_junk_combat`)."""
    a, d = row.get("attacker") or {}, row.get("defender") or {}
    return a.get("type") == "district" and d.get("type") == "district" and a.get("id") == -1 and d.get("id") == -1


def extract_game(game: str, events: Iterable[dict[str, Any]], stats: collections.Counter) -> Iterator[dict[str, Any]]:
    """Rows for one game. The end of our turn is the turn's last `state`
    frame, updated by our moves (`host_move`) and strikes after it; the
    enemy's turn is every `combat` row with `our_turn` false between that and
    the next turn's first frame, which is also where the enemy is seen next."""
    turn = None
    board = None
    frozen = False
    strikes: list[dict[str, Any]] = []
    for event in events:
        kind = event.get("kind")
        t = event.get("turn")
        if kind == "state" and isinstance(t, int):
            if turn is None or t > turn:
                following = board_of(event)
                if board is not None:
                    yield from enemy_rows(game, turn, board, strikes, following)
                    stats["turns"] += 1
                    for row in strikes:
                        a, d = row.get("attacker") or {}, row.get("defender") or {}
                        if not row.get("against_us") or a.get("type") != "unit" or d.get("type") != "unit":
                            continue
                        stats["unit strikes on our units"] += 1
                        if (a.get("player"), a.get("id")) not in board["enemies"]:
                            stats["  by a unit not in sight at our turn's end"] += 1
                turn, board, strikes, frozen = t, following, [], False
            elif t == turn and not frozen:
                board = board_of(event)
            continue
        if turn is None or t != turn or board is None:
            continue
        if kind == "end_turn_wait":
            frozen = True
        elif kind == "host_move" and not frozen:
            unit = board["ours"].get(event.get("unit"))
            if unit is not None and isinstance(event.get("x"), int):
                unit["x"], unit["y"] = event["x"], event["y"]
        elif kind == "combat" and not _junk(event):
            if event.get("our_turn") is False:
                frozen = True
                strikes.append(event)
                stats["enemy-turn combat rows"] += 1
            elif event.get("ours") and not frozen:
                apply_our_combat(board, event)


def game_dirs(root: Path, pattern: str) -> list[Path]:
    return sorted(
        p for p in root.glob(pattern) if p.is_dir() and "-cont" not in p.name and (p / "events.jsonl").is_file()
    )


def cmd_extract(args: argparse.Namespace) -> int:
    dirs = [Path(d) for d in args.runs] or game_dirs(Path(args.root).expanduser(), args.glob)
    stats: collections.Counter = collections.Counter()
    rows = 0
    with open(args.out, "w") as out:
        for d in dirs:
            for row in extract_game(d.name, read_run(d / "events.jsonl"), stats):
                out.write(json.dumps(row, separators=(",", ":")) + "\n")
                rows += 1
            stats["games"] += 1
            if not args.quiet:
                print(f"{d.name}: {rows} rows so far", file=sys.stderr)
    print(json.dumps({"rows": rows, **stats}, sort_keys=True))
    return 0


# ------------------------------------------------------------------ fitting
def solve_linear(a: list[list[float]], b: list[float]) -> list[float]:
    """Gaussian elimination with partial pivoting; `a` is small and dense."""
    n = len(b)
    m = [row[:] + [b[i]] for i, row in enumerate(a)]
    for col in range(n):
        pivot = max(range(col, n), key=lambda r: abs(m[r][col]))
        m[col], m[pivot] = m[pivot], m[col]
        if abs(m[col][col]) < 1e-12:
            continue
        for r in range(n):
            if r != col and m[r][col] != 0.0:
                f = m[r][col] / m[col][col]
                for c in range(col, n + 1):
                    m[r][c] -= f * m[col][c]
    return [m[i][n] / m[i][i] if abs(m[i][i]) > 1e-12 else 0.0 for i in range(n)]


def sigmoid(z: float) -> float:
    if z >= 0:
        return 1.0 / (1.0 + math.exp(-z))
    e = math.exp(z)
    return e / (1.0 + e)


def fit_logistic(cells: list[tuple[list[float], float, float]], ridge: float = 1.0, iters: int = 50) -> list[float]:
    """Binomial logistic regression on aggregated cells `(x, trials, successes)`,
    Newton's method, an L2 penalty on every weight but the first (intercept)."""
    k = len(cells[0][0])
    w = [0.0] * k
    for _ in range(iters):
        grad = [0.0] * k
        hess = [[0.0] * k for _ in range(k)]
        for x, n, y in cells:
            p = sigmoid(sum(wi * xi for wi, xi in zip(w, x)))
            r = y - n * p
            v = n * p * (1.0 - p)
            for i in range(k):
                if x[i] == 0.0:
                    continue
                grad[i] += r * x[i]
                for j in range(k):
                    if x[j] != 0.0:
                        hess[i][j] += v * x[i] * x[j]
        for i in range(1, k):
            grad[i] -= ridge * w[i]
            hess[i][i] += ridge
        step = solve_linear(hess, grad)
        w = [wi + si for wi, si in zip(w, step)]
        if max(abs(s) for s in step) < 1e-8:
            break
    return w


def fit_choice(sets: list[tuple[list[list[float]], int]], ridge: float = 1.0, iters: int = 50) -> list[float]:
    """Conditional logit: each set is `(alternatives' features, chosen index)`."""
    k = len(sets[0][0][0])
    w = [0.0] * k
    for _ in range(iters):
        grad = [-ridge * wi for wi in w]
        hess = [[ridge if i == j else 0.0 for j in range(k)] for i in range(k)]
        for xs, chosen in sets:
            us = [sum(wi * xi for wi, xi in zip(w, x)) for x in xs]
            top = max(us)
            es = [math.exp(u - top) for u in us]
            z = sum(es)
            ps = [e / z for e in es]
            mean = [sum(p * x[i] for p, x in zip(ps, xs)) for i in range(k)]
            for i in range(k):
                grad[i] += xs[chosen][i] - mean[i]
            for p, x in zip(ps, xs):
                for i in range(k):
                    di = x[i] - mean[i]
                    if di == 0.0:
                        continue
                    for j in range(k):
                        hess[i][j] += p * di * (x[j] - mean[j])
        step = solve_linear(hess, grad)
        w = [wi + si for wi, si in zip(w, step)]
        if max(abs(s) for s in step) < 1e-8:
            break
    return w


def fold_of(game: str, folds: int) -> int:
    return int(hashlib.sha256(game.encode()).hexdigest(), 16) % folds


# Strike model: intercept, then one-hot levels with the first level of each
# factor as the reference.
STRIKE_FACTORS: list[tuple[str, tuple[str, ...]]] = [
    ("class", CLASSES),
    ("owner", OWNERS),
    ("hp", HP_BUCKETS),
    ("home", ("away", "home")),
    ("edge", EDGE_BUCKETS),
    ("targets", TARGET_BUCKETS),
    ("still", ("must move", "no move needed")),
]


def strike_levels(row: dict[str, Any]) -> dict[str, str]:
    targets = row["targets"]
    return {
        "class": row["class"],
        "owner": row["owner"],
        "hp": hp_bucket(row["hp"]),
        "home": "home" if row["home"] else "away",
        "edge": edge_bucket(max(t["edge"] for t in targets)),
        "targets": target_bucket(len(targets)),
        "still": "no move needed" if any(t["still"] for t in targets) else "must move",
    }


def strike_vector(levels: dict[str, str]) -> list[float]:
    x = [1.0]
    for name, values in STRIKE_FACTORS:
        x.extend(1.0 if levels[name] == v else 0.0 for v in values[1:])
    return x


def strike_names() -> list[str]:
    names = ["intercept"]
    for name, values in STRIKE_FACTORS:
        names.extend(f"{name}={v}" for v in values[1:])
    return names


def struck_us(row: dict[str, Any]) -> bool:
    """It struck a unit of ours (`struck` is that unit's id); "city" and
    "other" (a third party) are strikes that were not at a unit of ours."""
    return isinstance(row["struck"], int)


TARGET_FEATURES = ("hp", "dealt", "kills", "shooter", "distance", "no move")


def target_vector(t: dict[str, Any]) -> list[float]:
    return [
        t["hp"] / 100.0,
        min(t["dealt"], t["hp"]) / 100.0,
        1.0 if t["dealt"] >= t["hp"] else 0.0,
        1.0 if t["class"] in ("ranged", "siege") else 0.0,
        float(t["d"]),
        1.0 if t["still"] else 0.0,
    ]


def greedy_pick(row: dict[str, Any]) -> int:
    """The stock enemy's pick: the best `blow_value` in hammers, with each
    unit's production cost as its value (`model::Sim::blow_value`)."""
    best, best_v = 0, -1e9
    me_value = float(_spec(row["kind"]).get("cost") or 100)
    for k, t in enumerate(row["targets"]):
        value = float(_spec(t["kind"]).get("cost") or 100)
        killed = t["dealt"] >= t["hp"]
        dies = t["taken"] >= row["hp"] and not killed
        v = min(t["dealt"], t["hp"]) / 100.0 * value + (0.8 * value if killed else 0.0)
        v -= min(t["taken"], row["hp"]) / 100.0 * me_value + (0.8 * me_value if dies else 0.0)
        if v > best_v:
            best, best_v = k, v
    return best


def greedy_value(row: dict[str, Any]) -> float:
    me_value = float(_spec(row["kind"]).get("cost") or 100)
    t = row["targets"][greedy_pick(row)]
    value = float(_spec(t["kind"]).get("cost") or 100)
    killed = t["dealt"] >= t["hp"]
    dies = t["taken"] >= row["hp"] and not killed
    v = min(t["dealt"], t["hp"]) / 100.0 * value + (0.8 * value if killed else 0.0)
    return v - min(t["taken"], row["hp"]) / 100.0 * me_value - (0.8 * me_value if dies else 0.0)


def _ll(p: float, y: bool) -> float:
    p = min(1.0 - 1e-6, max(1e-6, p))
    return math.log(p if y else 1.0 - p)


def evaluate_strike(rows: list[dict[str, Any]], folds: int, ridge: float) -> dict[str, Any]:
    contact = [r for r in rows if r["targets"]]
    by_fold = collections.defaultdict(list)
    for r in contact:
        by_fold[fold_of(r["game"], folds)].append(r)
    totals = collections.Counter()
    calibration = [[0, 0.0, 0] for _ in range(10)]
    for f in range(folds):
        train = [r for g, rs in by_fold.items() if g != f for r in rs]
        test = by_fold.get(f, [])
        if not test or not train:
            continue
        w = fit_logistic(aggregate(train), ridge)
        base = sum(struck_us(r) for r in train) / len(train)
        for r in test:
            y = struck_us(r)
            p = sigmoid(sum(a * b for a, b in zip(w, strike_vector(strike_levels(r)))))
            totals["fitted"] += _ll(p, y)
            totals["constant"] += _ll(base, y)
            totals["stock 0.5"] += _ll(0.5, y)
            gated = 0.5 if greedy_value(r) > -12.0 else 0.02
            totals["stock 0.5, value gate"] += _ll(gated, y)
            totals["n"] += 1
            band = min(int(p * 10), 9)
            calibration[band][0] += 1
            calibration[band][1] += p
            calibration[band][2] += y
    w = fit_logistic(aggregate(contact), ridge)
    return {
        "rows": len(contact),
        "struck": sum(struck_us(r) for r in contact),
        "struck_city": sum(1 for r in contact if r["struck"] == "city"),
        "struck_other": sum(1 for r in contact if r["struck"] == "other"),
        "held_out_ll": {k: v / max(1, totals["n"]) for k, v in totals.items() if k != "n"},
        "held_out_rows": totals["n"],
        "held_out_calibration": [
            {"band": f"{b / 10:.1f}-{(b + 1) / 10:.1f}", "rows": n, "predicted": p / n, "struck": y / n}
            for b, (n, p, y) in enumerate(calibration)
            if n
        ],
        "weights": dict(zip(strike_names(), w)),
    }


def aggregate(rows: list[dict[str, Any]]) -> list[tuple[list[float], float, float]]:
    cells: dict[tuple, list[float]] = {}
    for r in rows:
        x = tuple(strike_vector(strike_levels(r)))
        cell = cells.setdefault(x, [0.0, 0.0])
        cell[0] += 1
        cell[1] += 1 if struck_us(r) else 0
    return [(list(x), n, y) for x, (n, y) in cells.items()]


def choice_sets(rows: list[dict[str, Any]]) -> list[tuple[dict[str, Any], int]]:
    out = []
    for r in rows:
        if not struck_us(r) or len(r["targets"]) < 2:
            continue
        ids = [t["id"] for t in r["targets"]]
        if r["struck"] in ids:
            out.append((r, ids.index(r["struck"])))
    return out


def evaluate_target(rows: list[dict[str, Any]], folds: int, ridge: float) -> dict[str, Any]:
    sets = choice_sets(rows)
    by_fold = collections.defaultdict(list)
    for s in sets:
        by_fold[fold_of(s[0]["game"], folds)].append(s)
    totals = collections.Counter()
    for f in range(folds):
        train = [s for g, ss in by_fold.items() if g != f for s in ss]
        test = by_fold.get(f, [])
        if not test or not train:
            continue
        w = fit_choice([([target_vector(t) for t in r["targets"]], c) for r, c in train], ridge)
        # The greedy rule's best smoothing on the training games: it puts
        # 1 - eps on its pick and spreads eps over the rest.
        hits = sum(greedy_pick(r) == c for r, c in train)
        eps = min(0.99, max(0.01, 1.0 - hits / len(train)))
        for r, c in test:
            xs = [target_vector(t) for t in r["targets"]]
            us = [sum(a * b for a, b in zip(w, x)) for x in xs]
            top = max(us)
            z = sum(math.exp(u - top) for u in us)
            n = len(xs)
            totals["fitted"] += us[c] - top - math.log(z)
            totals["uniform"] += -math.log(n)
            g = greedy_pick(r)
            totals["stock greedy"] += math.log(1.0 - eps if g == c else eps / (n - 1))
            totals["fitted top-1"] += 1 if max(range(n), key=lambda k: us[k]) == c else 0
            totals["greedy top-1"] += 1 if g == c else 0
            totals["uniform top-1"] += 1.0 / n
            totals["n"] += 1
    w = fit_choice([([target_vector(t) for t in r["targets"]], c) for r, c in sets], ridge)
    n = max(1, totals["n"])
    return {
        "sets": len(sets),
        "mean_alternatives": sum(len(r["targets"]) for r, _ in sets) / max(1, len(sets)),
        "held_out_ll": {k: totals[k] / n for k in ("fitted", "stock greedy", "uniform")},
        "held_out_top1": {k: totals[k] / n for k in ("fitted top-1", "greedy top-1", "uniform top-1")},
        "held_out_sets": totals["n"],
        "weights": dict(zip(TARGET_FEATURES, w)),
    }


def situation(row: dict[str, Any]) -> str:
    if row["targets"]:
        return SITUATIONS[0]
    return SITUATIONS[1 + GAP_BUCKETS.index(gap_bucket(row["gap"]))]


def advance_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Unit-turns that struck nothing, seen again at the start of our next
    turn, in reach of one of ours or within `MAX_GAP` of one."""
    return [
        r
        for r in rows
        if r["struck"] is None
        and r["moved"]
        and r["gap"] is not None
        and (r["targets"] or r["gap"] <= MAX_GAP)
    ]


def evaluate_advance(rows: list[dict[str, Any]], folds: int) -> dict[str, Any]:
    """P(closes on us | struck nothing), by its own hit points and where it
    stood, smoothed toward the situation's own rate (a pseudo-count of 5).
    "Closer" is a smaller gap to the nearest unit of ours at the start of our
    next turn; hold, a sideways step and a step back are all "did not close".
    Owner and class keyings were tried and read no better on held-out games."""
    free = advance_rows(rows)

    def key(r: dict[str, Any]) -> tuple[str, str]:
        return (ADVANCE_HP[0] if r["hp"] <= 33 else ADVANCE_HP[1], situation(r))

    def table(train: list[dict[str, Any]]) -> dict[tuple[str, str], float]:
        n = collections.Counter()
        y = collections.Counter()
        for r in train:
            for k in (situation(r), key(r)):
                n[k] += 1
                y[k] += r["moved"] == "closer"
        overall = sum(r["moved"] == "closer" for r in train) / max(1, len(train))
        out = {}
        for where in SITUATIONS:
            prior = (y[where] + 5 * overall) / (n[where] + 5)
            for hp in ADVANCE_HP:
                out[(hp, where)] = (y[(hp, where)] + 5 * prior) / (n[(hp, where)] + 5)
        return out

    by_fold = collections.defaultdict(list)
    for r in free:
        by_fold[fold_of(r["game"], folds)].append(r)
    totals = collections.Counter()
    for f in range(folds):
        train = [r for g, rs in by_fold.items() if g != f for r in rs]
        test = by_fold.get(f, [])
        if not test or not train:
            continue
        t = table(train)
        base = sum(r["moved"] == "closer" for r in train) / len(train)
        for r in test:
            y = r["moved"] == "closer"
            totals["fitted"] += _ll(t[key(r)], y)
            totals["constant"] += _ll(base, y)
            totals["stock 0.5"] += _ll(0.5, y)
            totals["n"] += 1
    full = table(free)
    counts = collections.Counter(key(r) for r in free)
    moves = collections.Counter(r["moved"] for r in free)
    return {
        "rows": len(free),
        "moves": dict(moves),
        "held_out_ll": {k: v / max(1, totals["n"]) for k, v in totals.items() if k != "n"},
        "held_out_rows": totals["n"],
        "table": {f"{h}|{w}": round(full[(h, w)], 4) for h in ADVANCE_HP for w in SITUATIONS},
        "counts": {f"{h}|{w}": counts[(h, w)] for h in ADVANCE_HP for w in SITUATIONS},
    }


def summary_rates(rows: list[dict[str, Any]]) -> dict[str, Any]:
    """The plain rate a visible enemy with one of ours in reach strikes one
    of ours, by owner: what other code can price unit risk with today."""
    out = {}
    for owner in OWNERS:
        rs = [r for r in rows if r["targets"] and r["owner"] == owner]
        out[owner] = {"rows": len(rs), "struck": sum(struck_us(r) for r in rs)}
        out[owner]["rate"] = out[owner]["struck"] / max(1, len(rs))
    return out


def describe(rows: list[dict[str, Any]]) -> dict[str, Any]:
    games = sorted({r["game"] for r in rows})
    contact = [r for r in rows if r["targets"]]
    declined = collections.Counter(str(r["moved"]) for r in contact if not struck_us(r))
    # The stock enemy retreats at 25 hp or less; what the host's did instead.
    low = collections.Counter(str(r["moved"]) for r in rows if r["hp"] <= 25 and r["struck"] is None)
    # How well the reach approximation covers what the host actually hit.
    hits = [r for r in rows if struck_us(r)]
    covered = sum(1 for r in hits if r["struck"] in {t["id"] for t in r["targets"]})
    cells = collections.Counter()
    for r in contact:
        lv = strike_levels(r)
        for name, _ in STRIKE_FACTORS:
            cells[(name, lv[name])] += 1
            cells[(name, lv[name], "y")] += struck_us(r)
    return {
        "games": len(games),
        "unit_turns": len(rows),
        "with_target_in_reach": len(contact),
        "struck_ours": len(hits),
        "struck_a_target_in_reach": covered,
        "declined_then": dict(declined),
        "hp25_struck_nothing_then": dict(low),
        "levels": {
            f"{name}={v}": {"rows": cells[(name, v)], "struck": cells[(name, v, "y")]}
            for name, values in STRIKE_FACTORS
            for v in values
        },
    }


def load_rows(path: str) -> list[dict[str, Any]]:
    with open(path) as handle:
        return [json.loads(line) for line in handle if line.strip()]


def fit_all(rows: list[dict[str, Any]], folds: int = 5, ridge: float = 1.0) -> dict[str, Any]:
    return {
        "data": describe(rows),
        "strike": evaluate_strike(rows, folds, ridge),
        "target": evaluate_target(rows, folds, ridge),
        "advance": evaluate_advance(rows, folds),
        "summary": summary_rates(rows),
        "folds": folds,
        "ridge": ridge,
    }


def rust_constants(fit: dict[str, Any]) -> str:
    """The tables as `host_enemy.rs` states them, four decimals."""
    w = fit["strike"]["weights"]
    t = fit["target"]["weights"]
    a = fit["advance"]["table"]
    s = fit["summary"]
    lines = [f"pub const STRIKE_INTERCEPT: f32 = {w['intercept']:.4f};"]
    for name, values in STRIKE_FACTORS:
        coeffs = [0.0] + [w[f"{name}={v}"] for v in values[1:]]
        const = f"STRIKE_{name.upper()}"
        lines.append(f"pub const {const}: [f32; {len(values)}] = [{', '.join(f'{c:.4f}' for c in coeffs)}];")
    lines.append(f"pub const TARGET_WEIGHTS: [f32; {len(TARGET_FEATURES)}] = [{', '.join(f'{t[k]:.4f}' for k in TARGET_FEATURES)}];")
    rows = []
    for hp in ADVANCE_HP:
        rows.append("[" + ", ".join(f"{a[f'{hp}|{w}']:.4f}" for w in SITUATIONS) + "]")
    lines.append(f"pub const ADVANCE: [[f32; {len(SITUATIONS)}]; {len(ADVANCE_HP)}] = [{', '.join(rows)}];")
    lines.append(f"pub const STRIKE_RATE_MAJOR: f32 = {s['major']['rate']:.4f};")
    lines.append(f"pub const STRIKE_RATE_CITY_STATE: f32 = {s['city_state']['rate']:.4f};")
    lines.append(f"pub const STRIKE_RATE_BARBARIAN: f32 = {s['barbarian']['rate']:.4f};")
    return "\n".join(lines)


def doc_tables(fit: dict[str, Any]) -> str:
    """The tables `docs/HOST_ENEMY_MODEL.md` carries, in the form its Rust
    test reads back: one `| name | value |` row per constant."""
    w = fit["strike"]["weights"]
    out = ["| strike term | log-odds |", "|---|---|", f"| intercept | {w['intercept']:+.4f} |"]
    for name, values in STRIKE_FACTORS:
        out.append(f"| {name}={values[0]} | +0.0000 |")
        for v in values[1:]:
            out.append(f"| {name}={v} | {w[f'{name}={v}']:+.4f} |")
    out += ["", "| target term | weight |", "|---|---|"]
    for k in TARGET_FEATURES:
        out.append(f"| {k} | {fit['target']['weights'][k]:+.4f} |")
    out += ["", "| advance | " + " | ".join(SITUATIONS) + " |", "|---" * (len(SITUATIONS) + 1) + "|"]
    for hp in ADVANCE_HP:
        cells = [
            f"{fit['advance']['table'][f'{hp}|{w}']:.4f} ({fit['advance']['counts'][f'{hp}|{w}']})"
            for w in SITUATIONS
        ]
        out.append(f"| {hp} | " + " | ".join(cells) + " |")
    out += ["", "| owner | unit-turns in reach | struck one of ours | rate |", "|---|---|---|---|"]
    for owner in OWNERS:
        s = fit["summary"][owner]
        out.append(f"| {owner} | {s['rows']} | {s['struck']} | {s['rate']:.4f} |")
    return "\n".join(out)


def rate_table(rows: list[dict[str, Any]]) -> dict[str, Any]:
    """The behaviour table `src/host_tactics.rs` prints for simulated rival
    chairs, read the same way from live rows, so the two sit side by side.

    Each entry is `[numerator, denominator]`. Target choice reads the strikes
    on a unit in reach with two or more in reach; a tie for the nearest,
    weakest or most damaged counts as a hit, and `chance` is the share a
    uniform pick would hit (tied best / alternatives, averaged)."""
    t: dict[str, list[float]] = collections.defaultdict(lambda: [0.0, 0.0])

    def add(key: str, hit: bool) -> None:
        t[key][0] += 1 if hit else 0
        t[key][1] += 1

    for r in rows:
        targets = r["targets"]
        if targets:
            still = any(x["still"] for x in targets)
            add("strike | in reach", struck_us(r))
            add("strike | in reach without moving" if still else "strike | must move first", struck_us(r))
            ids = [x["id"] for x in targets]
            if struck_us(r) and len(targets) >= 2 and r["struck"] in ids:
                hit = targets[ids.index(r["struck"])]
                n = len(targets)
                for name, key, best in (
                    ("nearest", "d", min),
                    ("weakest", "hp", min),
                    ("most damaged", "dealt", max),
                ):
                    top = best(x[key] for x in targets)
                    tied = sum(1 for x in targets if x[key] == top)
                    add(f"target: {name}", hit[key] == top)
                    t[f"target: {name} (chance)"][0] += tied / n
                    t[f"target: {name} (chance)"][1] += 1
        if r["struck"] is None and r["moved"] and r["gap"] is not None and (targets or r["gap"] <= MAX_GAP):
            add("closes | struck nothing", r["moved"] == "closer")
            add("closes | declined" if targets else "closes | nothing in reach", r["moved"] == "closer")
            if r["hp"] <= 25:
                add("steps away | hp<=25, struck nothing", r["moved"] == "away")
                add("holds | hp<=25, struck nothing", r["moved"] == "hold")
            else:
                add("steps away | hp>25, struck nothing", r["moved"] == "away")
    return {k: [round(v[0], 3), int(v[1])] for k, v in sorted(t.items())}


def cmd_rates(args: argparse.Namespace) -> int:
    table = rate_table(load_rows(args.rows))
    for key, (hit, n) in table.items():
        print(f"{key:<40} {hit / max(1, n):.3f}  ({hit:g} of {n})")
    return 0


def cmd_fit(args: argparse.Namespace) -> int:
    rows = load_rows(args.rows)
    fit = fit_all(rows, args.folds, args.ridge)
    if args.json:
        Path(args.json).write_text(json.dumps(fit, indent=1, sort_keys=True))
    if args.doc:
        print(doc_tables(fit))
    if args.rust:
        print(rust_constants(fit))
    if not args.doc and not args.rust:
        print(json.dumps(fit, indent=1, sort_keys=True))
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = parser.add_subparsers(dest="cmd", required=True)
    ex = sub.add_parser("extract")
    ex.add_argument("runs", nargs="*")
    ex.add_argument("--out", required=True)
    ex.add_argument("--root", default=str(RUN_ROOT))
    ex.add_argument("--glob", default="civvis-2026111*T*")
    ex.add_argument("--quiet", action="store_true")
    ex.set_defaults(func=cmd_extract)
    fi = sub.add_parser("fit")
    fi.add_argument("rows")
    fi.add_argument("--folds", type=int, default=5)
    fi.add_argument("--ridge", type=float, default=1.0)
    fi.add_argument("--json")
    fi.add_argument("--doc", action="store_true")
    fi.add_argument("--rust", action="store_true")
    fi.set_defaults(func=cmd_fit)
    ra = sub.add_parser("rates")
    ra.add_argument("rows")
    ra.set_defaults(func=cmd_rates)
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
