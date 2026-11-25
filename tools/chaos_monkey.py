#!/usr/bin/env python3
"""Chaos Monkey's record: what worlds played under a very high frequency of
natural disasters did, and what they teach.

    chaos_monkey.py <records.jsonl> [...]                 print the record as JSON
    chaos_monkey.py --write <records.jsonl> [...]         write data/chaos_monkey.json
    chaos_monkey.py --write --commit <sha> --date <day> <records.jsonl> [...]

Chaos engineering breaks a running system on purpose, to find where it is
fragile before a real failure does (operator 2026-11-25: "A great thought
today is chaos engineering society in simulation to try to learn any lessons
we can, without having to go through the event ourselves to learn ... Here we
can run civvis sims with a very high frequency of natural disasters. what can
we learn about avoiding horrible outcomes?").

The games are `civvis chaos-monkey`'s (`src/chaos_monkey.rs`): every seed is
played once at each disaster frequency by the same agents, so the worlds at
one seed differ in their disasters and in nothing else, and a world under
chaos has its own calm self (1x, Civilization VI as shipped) to be read
against. This script reads those games, one JSON record a line, and writes
the record the Chaos Monkey page draws (`beta/chaos.html`, filled by
`beta/chaos_page.py`):

- **doses**, one per frequency, lowest first: the games, the disasters a game
  and by kind, what the civilizations lost to them, the world at its end,
  that end as a share of the same seeds played calm (`kept`), the horrible
  outcomes (`outcomes`), how the games ended, and the world turn by turn.
- **factors**, the differences between civilizations that went with keeping
  more of their people under the storm (the frequencies in `STORM`), each as
  two groups split at the middle.
- **lessons**, what the games say, each a figure and a sentence written from
  the record's own numbers by a rule that says when it holds. A lesson the
  games do not bear out is not written, so the page cannot say something its
  games stopped saying; `lessons()` is the whole list of what may be said.

It reads games and writes a record; it plays nothing. The same seeds on the
same commit give the same games, so the record names the commit, the seeds
and the command that played them rather than keeping the games themselves.
"""

from __future__ import annotations

import json
import pathlib
import statistics
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RECORD = ROOT / "data" / "chaos_monkey.json"

#: The frequencies at which a world is under the storm: where the factors are
#: read. Below them the disasters are too few to tell civilizations apart.
STORM = 10.0

#: A civilization counts as having had people to lose once it reached this
#: many, so a capital that never grew is not a collapse.
PEOPLE_TO_LOSE = 10

#: The horrible outcomes, in the order the page draws them. `share` is a
#: share of the civilizations (or of the games); the rest are counts a game.
OUTCOMES = [
    ("collapsed", "Lost half their people", "share of civilizations", "share"),
    ("eliminated", "Wiped out", "share of civilizations", "share"),
    ("captures", "Cities that changed hands", "a game", "count"),
    ("wars", "Wars started", "a game", "count"),
    ("nukes", "Nuclear strikes", "a game", "count"),
]

VICTORIES = ["science", "culture", "diplomatic", "religious", "domination", "score"]


def load(paths: list[str]) -> list[dict]:
    """Every game in the files, one per seed and frequency (the last read
    wins, so a rerun replaces what it reran)."""
    games: dict[tuple[int, float], dict] = {}
    for path in paths:
        for line in pathlib.Path(path).read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line.startswith("{"):
                continue
            game = json.loads(line)
            games[(int(game["seed"]), float(game["frequency"]))] = game
    return [games[key] for key in sorted(games)]


def grid(game: dict, turns: list[int], read) -> list[float]:
    """One figure a listed turn for a game: `read(sample index)` at the last
    sample taken on or before the turn, so a game that ended early stands as
    it ended."""
    sampled = game["sample_turns"]
    out, at = [], 0
    for turn in turns:
        while at + 1 < len(sampled) and sampled[at + 1] <= turn:
            at += 1
        out.append(read(at))
    return out


def world(game: dict, key: str, at: int) -> float:
    """A series summed over the game's civilizations at one sample."""
    return float(sum(civ["series"][key][at] for civ in game["civs"]))


def collapsed(civ: dict) -> bool:
    """Whether a civilization lost half its people: its population fell to
    half its own earlier peak or less."""
    return civ["peak_pop"] >= PEOPLE_TO_LOSE and civ["low_pop"] * 2 <= civ["peak_pop"]


def mean(values) -> float:
    values = list(values)
    return sum(values) / len(values) if values else 0.0


def end(game: dict) -> dict:
    """The world as the game ended: its last sample, summed."""
    last = len(game["sample_turns"]) - 1
    improved = world(game, "improved", last)
    pillaged = world(game, "pillaged", last)
    return {
        "people": world(game, "pop", last),
        "cities": world(game, "cities", last),
        "science": world(game, "techs", last),
        "army": world(game, "army", last),
        "land": improved - pillaged,
        "improved": improved,
        "pillaged": pillaged,
    }


def victory(game: dict) -> str:
    label = (game.get("victory") or "").lower()
    for kind in VICTORIES:
        if kind in label:
            return kind
    return "none" if not game.get("winner") else "score"


def dose(games: list[dict], calm: dict[int, dict], turns: list[int]) -> dict:
    """One frequency's games, summed up. `calm` is the 1x game by seed."""
    ends = [end(game) for game in games]
    twins = [(end(game), end(calm[game["seed"]])) for game in games if game["seed"] in calm]
    kept = {
        key: round(sum(now[key] for now, _ in twins) / max(1e-9, sum(then[key] for _, then in twins)), 4)
        for key in ("people", "cities", "science", "army", "land")
    } if twins else {}
    civs = [civ for game in games for civ in game["civs"]]
    classes: dict[str, float] = {}
    for game in games:
        for kind, count in game["disasters"].items():
            classes[kind] = classes.get(kind, 0.0) + count / len(games)
    victories: dict[str, int] = {}
    for game in games:
        kind = victory(game)
        victories[kind] = victories.get(kind, 0) + 1
    wrecked = lambda game, at: world(game, "pillaged", at) / max(1.0, world(game, "improved", at))
    return {
        "frequency": int(games[0]["frequency"]) if float(games[0]["frequency"]).is_integer() else games[0]["frequency"],
        "games": len(games),
        "disasters": round(sum(classes.values()), 1),
        "classes": {kind: round(count, 2) for kind, count in sorted(classes.items())},
        "lost": {
            key: round(mean(sum(civ["lost"][key] for civ in game["civs"]) for game in games), 1)
            for key in ("citizens", "tiles", "units")
        },
        "end": {key: round(mean(e[key] for e in ends), 1) for key in ("people", "cities", "science", "army", "land")},
        "ended": int(statistics.median(game["ended"] for game in games)),
        "kept": kept,
        "outcomes": {
            "collapsed": round(mean(collapsed(civ) for civ in civs), 4),
            "eliminated": round(mean(not civ["alive"] for civ in civs), 4),
            "captures": round(mean(sum(civ["cities_lost"] for civ in game["civs"]) for game in games), 2),
            "wars": round(mean(game["wars"] for game in games), 2),
            "nukes": round(mean(game["nuclear_strikes"] for game in games), 2),
        },
        "victories": {kind: victories[kind] for kind in [*VICTORIES, "none"] if kind in victories},
        "series": {
            "people": [round(mean(column), 1) for column in zip(*(grid(g, turns, lambda at, g=g: world(g, "pop", at)) for g in games))],
            "wrecked": [round(mean(column), 4) for column in zip(*(grid(g, turns, lambda at, g=g: wrecked(g, at)) for g in games))],
        },
    }


# ---------------------------------------------------------------- factors

def at_turn(game: dict, turn: int) -> int:
    """The index of the last sample on or before a turn."""
    at = 0
    for index, sampled in enumerate(game["sample_turns"]):
        if sampled <= turn:
            at = index
    return at


def averaged(civ: dict, key: str) -> float:
    return mean(civ["series"][key])


#: The differences between civilizations the record looks at: a name, what
#: is compared, the two groups' labels, and the figure a civilization is
#: placed by. `now` is the civilization under the storm, `then` the same
#: civilization in the same world played calm, `game` the storm game.
FACTORS = [
    (
        "spread",
        "Spread over more cities",
        "cities the same start grows in a calm world by turn 100",
        ("more cities", "fewer cities"),
        lambda now, then, game, calm: then["series"]["cities"][at_turn(calm, 100)],
    ),
    (
        "builders",
        "Kept Builders at hand",
        "Builders a city, over the game under the storm",
        ("more Builders", "fewer Builders"),
        lambda now, then, game, calm: averaged(now, "builders") / max(1.0, averaged(now, "cities")),
    ),
    (
        "army",
        "Kept a standing army",
        "military units a city, over the game under the storm",
        ("bigger army", "smaller army"),
        lambda now, then, game, calm: averaged(now, "army") / max(1.0, averaged(now, "cities")),
    ),
    (
        "peace",
        "Stayed out of wars",
        "turns at war with another civilization under the storm",
        ("more war", "less war"),
        lambda now, then, game, calm: now["war_turns"],
    ),
    (
        "treasury",
        "Held gold in reserve",
        "gold in the treasury, over the game under the storm",
        ("more gold", "less gold"),
        lambda now, then, game, calm: averaged(now, "gold"),
    ),
    (
        "leaders",
        "Led the calm world",
        "score at the end of the same world played calm",
        ("the stronger half", "the weaker half"),
        lambda now, then, game, calm: then["series"]["score"][-1],
    ),
    (
        "coast",
        "Lived on the coast",
        "share of its cities beside the sea when the calm world ended",
        ("more coastal", "more inland"),
        lambda now, then, game, calm: then["shelter"]["on_coast"] / max(1, then["shelter"]["cities"]),
    ),
    (
        "rivers",
        "Farmed the floodplains",
        "floodplain tiles a city when the calm world ended",
        ("more floodplain", "less floodplain"),
        lambda now, then, game, calm: then["shelter"]["floodplains"] / max(1, then["shelter"]["cities"]),
    ),
]


def pairs(games: list[dict]) -> list[tuple[dict, dict, dict, dict]]:
    """Every civilization under the storm with its calm twin: the same
    civilization in the same seed's 1x game."""
    calm = {game["seed"]: game for game in games if float(game["frequency"]) == 1.0}
    out = []
    for game in games:
        if float(game["frequency"]) < STORM or game["seed"] not in calm:
            continue
        twins = {civ["civ"]: civ for civ in calm[game["seed"]]["civs"]}
        for civ in game["civs"]:
            then = twins.get(civ["civ"])
            if then and then["series"]["pop"][-1] >= PEOPLE_TO_LOSE:
                out.append((civ, then, game, calm[game["seed"]]))
    return out


def factors(games: list[dict]) -> list[dict]:
    """Each difference as two groups, split at the middle within a frequency
    so neither group is the harsher worlds, with the share of their calm
    selves' people each group still had at the end. Widest gap first."""
    paired = pairs(games)
    out = []
    for key, name, of, (more_label, fewer_label), place in FACTORS:
        groups: dict[bool, list[tuple[dict, dict]]] = {True: [], False: []}
        for frequency in sorted({float(game["frequency"]) for _, _, game, _ in paired}):
            rows = [row for row in paired if float(row[2]["frequency"]) == frequency]
            placed = sorted(((place(*row), index) for index, row in enumerate(rows)), key=lambda item: item[0])
            half = len(placed) // 2
            # The lower half and the upper half; with an odd number the one
            # in the middle belongs to neither.
            for rank, (_, index) in enumerate(placed):
                if rank < half:
                    groups[False].append(rows[index][:2])
                elif rank >= len(placed) - half:
                    groups[True].append(rows[index][:2])
        if min(len(groups[True]), len(groups[False])) < 12:
            continue
        share = lambda rows: sum(now["series"]["pop"][-1] for now, _ in rows) / max(1e-9, sum(then["series"]["pop"][-1] for _, then in rows))
        out.append(
            {
                "key": key,
                "name": name,
                "of": of,
                "more": {"label": more_label, "kept": round(share(groups[True]), 4), "n": len(groups[True])},
                "fewer": {"label": fewer_label, "kept": round(share(groups[False]), 4), "n": len(groups[False])},
            }
        )
    out.sort(key=lambda factor: -abs(factor["more"]["kept"] - factor["fewer"]["kept"]))
    return out


# ---------------------------------------------------------------- lessons

def pct(share: float) -> str:
    return f"{round(share * 100)}%"


def times(frequency) -> str:
    return f"{frequency:g}x"


def count(value: float) -> str:
    return f"{value:,.0f}" if value >= 10 else f"{value:.1f}".rstrip("0").rstrip(".")


def lessons(doses: list[dict], found: list[dict]) -> list[dict]:
    """What the games say. Each lesson is written only when its own rule
    holds on the record, from the record's own figures."""
    out = []
    calm, worst = doses[0], doses[-1]
    chaos = doses[1:]
    if calm["frequency"] != 1 or not chaos:
        return out
    kept = lambda d, key: d["kept"][key]

    # 1. How much disaster a world absorbs: the highest frequency that still
    # leaves it nine tenths of its people.
    absorbed = [d for d in chaos if kept(d, "people") >= 0.9]
    if absorbed:
        most = absorbed[-1]
        out.append(
            {
                "key": "absorbs",
                "figure": times(most["frequency"]),
                "of": f"disasters, and the world still ends with {pct(kept(most, 'people'))} of its people",
                "title": "A world absorbs far more than it is ever dealt",
                "text": f"{count(most['disasters'])} disasters a game instead of {count(calm['disasters'])} and the world is "
                f"nearly whole. Routine shocks are already priced in.",
            }
        )
    # 2. Where it breaks: the first frequency that costs a quarter of the people.
    broken = [d for d in chaos if kept(d, "people") <= 0.75]
    if broken:
        first = broken[0]
        before = doses[doses.index(first) - 1]
        out.append(
            {
                "key": "breaks",
                "figure": times(first["frequency"]),
                "of": f"is where it gives: {pct(1 - kept(first, 'people'))} of the people are gone by the end",
                "title": "Then it falls off an edge",
                "text": f"At {times(before['frequency'])} the world keeps {pct(kept(before, 'people'))} of its people; at "
                f"{times(first['frequency'])}, {pct(kept(first, 'people'))}. The damage is not in proportion to the dose: "
                f"repair keeps up until it cannot.",
            }
        )
    # 3. What gives way first and what holds, at the highest frequency.
    names = {"people": "people", "cities": "cities", "science": "science", "army": "armies", "land": "working land"}
    ranked = sorted(names, key=lambda key: kept(worst, key))
    weakest, strongest = ranked[0], ranked[-1]
    if kept(worst, strongest) - kept(worst, weakest) >= 0.15:
        out.append(
            {
                "key": "first",
                "figure": pct(kept(worst, weakest)),
                "of": f"of the {names[weakest]} are left at {times(worst['frequency'])}, against {pct(kept(worst, strongest))} of the {names[strongest]}",
                "title": f"The {names[weakest]} go first; the {names[strongest]} hold",
                "text": f"What a society has built is not equally fragile. Under the worst storm the {names[strongest]} "
                f"are the last thing lost and the {names[weakest]} the first.",
            }
        )
    # 4. Disasters do not finish a civilization; nobody is wiped out more often.
    if worst["outcomes"]["eliminated"] <= calm["outcomes"]["eliminated"] + 0.02 and worst["outcomes"]["collapsed"] >= calm["outcomes"]["collapsed"] + 0.1:
        out.append(
            {
                "key": "survive",
                "figure": pct(worst["outcomes"]["collapsed"]),
                "of": f"of civilizations lose half their people at {times(worst['frequency'])}; {pct(worst['outcomes']['eliminated'])} are wiped out",
                "title": "Disasters cripple; they do not finish",
                "text": f"Even at {times(worst['frequency'])} no more civilizations die than in a calm world "
                f"({pct(calm['outcomes']['eliminated'])}). The horrible outcome is the long poverty, not the end.",
            }
        )
    # 5. War: does the storm bring more of it, or less?
    wars = [d["outcomes"]["wars"] for d in doses]
    if calm["outcomes"]["wars"] > 0:
        change = worst["outcomes"]["wars"] / calm["outcomes"]["wars"]
        if change >= 1.25:
            out.append(
                {
                    "key": "war",
                    "figure": f"{change:.1f}x",
                    "of": f"the wars at {times(worst['frequency'])}: {count(worst['outcomes']['wars'])} a game against {count(calm['outcomes']['wars'])}",
                    "title": "A battered world fights more",
                    "text": "Weakened neighbours are targets. The disaster is the first blow; the second comes from next door.",
                }
            )
        elif change <= 0.8:
            out.append(
                {
                    "key": "war",
                    "figure": pct(1 - change),
                    "of": f"fewer wars at {times(worst['frequency'])}: {count(worst['outcomes']['wars'])} a game against {count(calm['outcomes']['wars'])}",
                    "title": "A battered world fights less",
                    "text": "Armies are among the first things a storm takes, and a civilization rebuilding has none to spare.",
                }
            )
    # 6. Who comes through: the widest difference between civilizations.
    if found:
        top = found[0]
        better, worse = (top["more"], top["fewer"]) if top["more"]["kept"] >= top["fewer"]["kept"] else (top["fewer"], top["more"])
        if better["kept"] - worse["kept"] >= 0.08:
            out.append(
                {
                    "key": "who",
                    "figure": f"{pct(better['kept'])} vs {pct(worse['kept'])}",
                    "of": f"of their people kept under the storm: civilizations with {better['label']}, against {worse['label']}",
                    "title": f"What went with coming through: {better['label']}",
                    "text": f"Counted as {top['of']}. Of everything compared, this separated the survivors most.",
                }
            )
    return out


def record(games: list[dict], played: dict | None = None) -> dict | None:
    """The page's record, or `None` when there are no games to read."""
    if not games:
        return None
    frequencies = sorted({float(game["frequency"]) for game in games})
    if 1.0 not in frequencies:
        raise ValueError("no games at 1x: a world under chaos needs its calm self to be read against")
    calm = {game["seed"]: game for game in games if float(game["frequency"]) == 1.0}
    limit = max(game["max_turns"] for game in games)
    turns = [1, *range(10, limit + 1, 10)]
    doses = [dose([g for g in games if float(g["frequency"]) == f], calm, turns) for f in frequencies]
    found = factors(games)
    seeds = sorted({game["seed"] for game in games})
    first = games[0]
    setup = (
        f"{first['players']} civilizations, every seat a CIVVIS AI agent, {limit} turns, "
        f"seeds {seeds[0]} to {seeds[-1]}, each played at {', '.join(times(d['frequency']) for d in doses)}"
    )
    drawn = [
        {"key": key, "name": name, "of": of, "unit": unit}
        for key, name, of, unit in OUTCOMES
        if any(d["outcomes"][key] for d in doses)
    ]
    return {
        "played": {"games": len(games), "seeds": [seeds[0], seeds[-1]], "setup": setup, **(played or {})},
        "turns": turns,
        "doses": doses,
        "outcomes": drawn,
        "factors": found[:5],
        "factors_of": (
            f"Civilizations at {times(STORM)} and over, by the share of their people they still had at the end, "
            f"against the same civilization in the same world played calm."
        ),
        "lessons": lessons(doses, found),
    }


def main(argv: list[str]) -> int:
    args = argv[1:]
    write = "--write" in args
    played: dict = {}
    for flag in ("--commit", "--date", "--command"):
        if flag in args:
            played[flag[2:]] = args[args.index(flag) + 1]
            del args[args.index(flag) : args.index(flag) + 2]
    paths = [arg for arg in args if arg != "--write"]
    if not paths:
        print(__doc__, file=sys.stderr)
        return 2
    data = record(load(paths), played)
    text = json.dumps(data, ensure_ascii=False, indent=1) + "\n"
    if write:
        RECORD.write_text(text, encoding="utf-8")
        said = ", ".join(f"{times(d['frequency'])} {d['games']}" for d in data["doses"]) if data else "no games"
        print(f"chaos_monkey: wrote {RECORD.relative_to(ROOT)} ({said}; {len(data['lessons']) if data else 0} lessons)", file=sys.stderr)
    else:
        sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
