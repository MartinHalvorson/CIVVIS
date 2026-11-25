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
  and by kind, what the civilizations lost to them, the world as a share of
  the same seeds played calm (`kept`), the horrible outcomes (`outcomes`),
  how the games ended, and the world turn by turn.

  A world is read against its calm self **at the same turn**: the last
  tenth turn both games reached (`common_turn`). Games end when somebody
  wins, a calm world and its stormy twin seldom on the same turn, and a
  world that played forty turns longer has forty turns more of everything.
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
#: read. Below it a world shrugs the disasters off (it has as many people at
#: 30x as calm), so there is nothing yet to tell civilizations apart by.
STORM = 100.0

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
    ("leader", "The largest civilization's share", "of the world's people", "share"),
    ("climate", "Climate change", "phases of warming reached", "count"),
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


def common_turn(game: dict, calm: dict) -> int:
    """The last sampled turn both a game and its calm twin reached: the turn
    the two worlds are compared on."""
    turn = min(game["ended"], calm["ended"])
    return max(1, turn // 10 * 10)


def end(game: dict, turn: int | None = None) -> dict:
    """The world as the game ended, or as it stood on a turn: one sample,
    summed over its civilizations."""
    last = len(game["sample_turns"]) - 1 if turn is None else at_turn(game, turn)
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


def running(games: list[dict], turns: list[int], read, digits: int) -> list[float]:
    """A figure a listed turn, the mean over the games still being played on
    it, for as long as at least half of them are. A game that somebody has
    won is not carried on as a world standing still."""
    out = []
    for turn in turns:
        playing = [game for game in games if game["ended"] >= turn]
        if len(playing) * 2 < len(games):
            break
        out.append(round(mean(read(game, at_turn(game, turn)) for game in playing), digits))
    return out


def leader_share(game: dict) -> float:
    """The largest civilization's share of the world's people as the game
    ended."""
    people = [civ["series"]["pop"][-1] for civ in game["civs"]]
    return max(people) / max(1, sum(people))


def victory(game: dict) -> str:
    label = (game.get("victory") or "").lower()
    for kind in VICTORIES:
        if kind in label:
            return kind
    return "none" if not game.get("winner") else "score"


def dose(games: list[dict], calm: dict[int, dict], turns: list[int]) -> dict:
    """One frequency's games, summed up. `calm` is the 1x game by seed."""
    ends = [end(game) for game in games]
    twins = [
        (end(game, common_turn(game, calm[game["seed"]])), end(calm[game["seed"]], common_turn(game, calm[game["seed"]])))
        for game in games
        if game["seed"] in calm
    ]
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
        # How often the civilization that wins the calm world wins this one.
        "same_winner": round(
            mean(game.get("winner") == calm[game["seed"]].get("winner") for game in games if game["seed"] in calm), 4
        ),
        "outcomes": {
            "collapsed": round(mean(collapsed(civ) for civ in civs), 4),
            "eliminated": round(mean(not civ["alive"] for civ in civs), 4),
            "captures": round(mean(sum(civ["cities_lost"] for civ in game["civs"]) for game in games), 2),
            "wars": round(mean(game["wars"] for game in games), 2),
            "nukes": round(mean(game["nuclear_strikes"] for game in games), 2),
            "leader": round(mean(leader_share(game) for game in games), 4),
            "climate": round(mean(game["climate_phase"] for game in games), 2),
        },
        "victories": {kind: victories[kind] for kind in [*VICTORIES, "none"] if kind in victories},
        "series": {
            "people": running(games, turns, lambda game, at: world(game, "pop", at), 1),
            "wrecked": running(games, turns, wrecked, 4),
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


#: The differences between civilizations the record looks at: a name, what
#: is compared, the two groups' labels, and the figure a civilization is
#: placed by. `now` is the civilization in the game being read, `then` the
#: same civilization in the same world played calm, `game` and `calm` the two
#: games.
#:
#: Only differences that are settled before the damage is done are here: the
#: land a start comes to hold and the empire it grows into (read off the calm
#: world, which no storm touched), and what a civilization had at hand on
#: turn 50 of the game itself. What a civilization has at the END of a storm
#: (its gold, its Aqueducts, its wars) is as much the storm's doing as its
#: own, and says nothing about what protects.
EARLY = 50
FACTORS = [
    (
        "coast",
        "Living on the coast",
        "share of its cities beside the sea in the calm world",
        ("more coastal", "more inland"),
        lambda now, then, game, calm: then["shelter"]["on_coast"] / max(1, then["shelter"]["cities"]),
    ),
    (
        "rivers",
        "Farming the floodplains",
        "floodplain tiles a city in the calm world",
        ("more floodplain", "less floodplain"),
        lambda now, then, game, calm: then["shelter"]["floodplains"] / max(1, then["shelter"]["cities"]),
    ),
    (
        "spread",
        "Spreading over more cities",
        "cities the same start has by turn 100 of the calm world",
        ("more cities", "fewer cities"),
        lambda now, then, game, calm: then["series"]["cities"][at_turn(calm, 100)],
    ),
    (
        "leaders",
        "Leading the calm world",
        "score in the calm world",
        ("the stronger half", "the weaker half"),
        lambda now, then, game, calm: then["series"]["score"][at_turn(calm, common_turn(game, calm))],
    ),
    (
        "builders",
        "Builders at hand early",
        f"Builders a city on turn {EARLY}",
        ("more Builders", "fewer Builders"),
        lambda now, then, game, calm: early(now, game, "builders") / max(1, early(now, game, "cities")),
    ),
    (
        "army",
        "A standing army early",
        f"military units a city on turn {EARLY}",
        ("bigger army", "smaller army"),
        lambda now, then, game, calm: early(now, game, "army") / max(1, early(now, game, "cities")),
    ),
    (
        "treasury",
        "Gold in reserve early",
        f"gold in the treasury on turn {EARLY}",
        ("more gold", "less gold"),
        lambda now, then, game, calm: early(now, game, "gold"),
    ),
]


def early(civ: dict, game: dict, key: str) -> float:
    """What a civilization had on turn `EARLY` of its game."""
    return civ["series"][key][at_turn(game, EARLY)]


def people(civ: dict, game: dict, turn: int) -> int:
    """A civilization's people on a turn of its game."""
    return civ["series"]["pop"][at_turn(game, turn)]


def pairs(games: list[dict], storm: bool = True) -> list[tuple[dict, dict, dict, dict]]:
    """Every civilization with its calm twin (the same civilization in the
    same seed's 1x game), in the worlds under the storm (`STORM` and over)
    or, for `storm=False`, in the mild ones between: turned past 1x and not
    yet hurt."""
    calm = {game["seed"]: game for game in games if float(game["frequency"]) == 1.0}
    out = []
    for game in games:
        frequency = float(game["frequency"])
        wanted = frequency >= STORM if storm else 1.0 < frequency < STORM
        if not wanted or game["seed"] not in calm:
            continue
        twins = {civ["civ"]: civ for civ in calm[game["seed"]]["civs"]}
        for civ in game["civs"]:
            then = twins.get(civ["civ"])
            if then and people(then, calm[game["seed"]], common_turn(game, calm[game["seed"]])) >= PEOPLE_TO_LOSE:
                out.append((civ, then, game, calm[game["seed"]]))
    return out


def halves(paired: list[tuple], place) -> dict[bool, list[tuple]]:
    """The civilizations with more of something and those with less: split
    at the middle within a frequency, so neither half is the harsher worlds.
    With an odd number the one in the middle belongs to neither."""
    groups: dict[bool, list[tuple]] = {True: [], False: []}
    for frequency in sorted({float(game["frequency"]) for _, _, game, _ in paired}):
        rows = [row for row in paired if float(row[2]["frequency"]) == frequency]
        placed = sorted(((place(*row), index) for index, row in enumerate(rows)), key=lambda item: item[0])
        half = len(placed) // 2
        for rank, (_, index) in enumerate(placed):
            if rank < half:
                groups[False].append(rows[index])
            elif rank >= len(placed) - half:
                groups[True].append(rows[index])
    return groups


def share(rows: list[tuple]) -> float:
    """The people a group of civilizations has, as a share of their calm
    selves', each on the turn both its worlds reached."""
    now = sum(people(civ, game, common_turn(game, calm)) for civ, _, game, calm in rows)
    then = sum(people(twin, calm, common_turn(game, calm)) for _, twin, game, calm in rows)
    return now / max(1e-9, then)


def factors(games: list[dict]) -> list[dict]:
    """Each difference as two halves of the civilizations under the storm,
    with the share of their calm selves' people each half still has.

    The same split is made in the mild worlds too, where the storm does no
    harm (`mild`). A civilization that led its calm world falls back in any
    replay, storm or none, and a half that is simply luckier is luckier
    there as well: what the storm adds is the gap under the storm less the
    gap in the mild worlds (`edge`). Widest edge first."""
    stormy, mild = pairs(games, storm=True), pairs(games, storm=False)
    out = []
    for key, name, of, (more_label, fewer_label), place in FACTORS:
        under, beside = halves(stormy, place), halves(mild, place)
        if min(len(under[True]), len(under[False]), len(beside[True]), len(beside[False])) < 12:
            continue
        more, fewer = share(under[True]), share(under[False])
        mild_more, mild_fewer = share(beside[True]), share(beside[False])
        out.append(
            {
                "key": key,
                "name": name,
                "of": of,
                "more": {"label": more_label, "kept": round(more, 4), "n": len(under[True])},
                "fewer": {"label": fewer_label, "kept": round(fewer, 4), "n": len(under[False])},
                "mild": {"more": round(mild_more, 4), "fewer": round(mild_fewer, 4)},
                "edge": round((more - fewer) - (mild_more - mild_fewer), 4),
            }
        )
    out.sort(key=lambda factor: -abs(factor["edge"]))
    return out


# ---------------------------------------------------------------- lessons

def pct(share: float) -> str:
    return f"{round(share * 100)}%"


def times(frequency) -> str:
    return f"{frequency:,g}x"


def count(value: float) -> str:
    return f"{value:,.0f}" if value >= 10 else f"{value:.1f}".rstrip("0").rstrip(".")


#: The five things measured in a world, as a lesson names them: the noun,
#: and whether it is plural ("armies go", "science goes").
MEASURES = {
    "people": ("people", True),
    "cities": ("cities", True),
    "science": ("science", False),
    "army": ("armies", True),
    "land": ("working land", False),
}


def lessons(doses: list[dict], found: list[dict]) -> list[dict]:
    """What the games say. Each lesson is written only when its own rule
    holds on the record, from the record's own figures, so the list is as
    long as the games bear out and no longer."""
    out: list[dict] = []
    if len(doses) < 2 or doses[0]["frequency"] != 1:
        return out
    calm, chaos, worst = doses[0], doses[1:], doses[-1]
    kept = lambda d, key: d["kept"][key]
    say = lambda key, figure, of, title, text: out.append({"key": key, "figure": figure, "of": of, "title": title, "text": text})

    # 1. How much disaster a world takes without losing a tenth of its
    # people: the highest frequency below which every step holds.
    absorbed = None
    for d in chaos:
        if kept(d, "people") < 0.9:
            break
        absorbed = d
    if absorbed:
        say(
            "absorbs",
            times(absorbed["frequency"]),
            f"the disasters, and the world has {pct(kept(absorbed, 'people'))} of the people of its calm self",
            "A world absorbs far more than it is ever dealt",
            f"{count(absorbed['disasters'])} disasters a game instead of {count(calm['disasters'])}. Ordinary shocks "
            f"are already priced in.",
        )
    # 2. Disasters that pay: a frequency at which the world has more people
    # than its calm self.
    richest = max(chaos, key=lambda d: kept(d, "people"))
    if kept(richest, "people") >= 1.05:
        say(
            "pays",
            f"+{pct(kept(richest, 'people') - 1)}",
            f"more people at {times(richest['frequency'])} than in the same worlds played calm",
            "Shocks that leave something behind can pay",
            "Floods leave silt and eruptions leave soil. What is rebuilt on richer ground outgrows what was lost.",
        )
    # 3. Where it breaks: the first frequency that costs a quarter of the people.
    broken = next((d for d in chaos if kept(d, "people") <= 0.75), None)
    if broken:
        before = doses[doses.index(broken) - 1]
        say(
            "breaks",
            times(broken["frequency"]),
            f"is where it gives: {pct(1 - kept(broken, 'people'))} fewer people than the calm world",
            "Then repair falls behind, and it breaks",
            f"At {times(before['frequency'])} it still had {pct(kept(before, 'people'))}. By now "
            f"{pct(broken['series']['wrecked'][-1])} of the improved land lies wrecked: damage outruns rebuilding.",
        )
    # 4. What gives way first and what holds, at the highest frequency.
    ranked = sorted(MEASURES, key=lambda key: kept(worst, key))
    weakest, strongest = ranked[0], ranked[-1]
    if kept(worst, strongest) - kept(worst, weakest) >= 0.15:
        (weak, weak_plural), (strong, strong_plural) = MEASURES[weakest], MEASURES[strongest]
        say(
            "first",
            f"{pct(kept(worst, weakest))} / {pct(kept(worst, strongest))}",
            f"of the {weak} and of the {strong} left at {times(worst['frequency'])}",
            f"{weak.capitalize()} {'go' if weak_plural else 'goes'} first; {strong} {'hold' if strong_plural else 'holds'}",
            "What needs upkeep every turn is lost first; what is founded or learned once goes last.",
        )
    # 5. Disasters cripple; they do not finish. Nobody is wiped out more
    # often, however many are halved.
    if worst["outcomes"]["eliminated"] <= calm["outcomes"]["eliminated"] + 0.02:
        say(
            "survive",
            pct(worst["outcomes"]["eliminated"]),
            f"of civilizations wiped out at {times(worst['frequency'])}, against {pct(calm['outcomes']['eliminated'])} in the calm worlds",
            "Disasters cripple; they do not finish",
            f"Only {pct(worst['outcomes']['collapsed'])} ever lose half the people they had. The horrible outcome is "
            f"a world that never grows, not one that ends.",
        )
    # 6. War: does the storm bring more of it, or less?
    if calm["outcomes"]["wars"] > 0:
        change = worst["outcomes"]["wars"] / calm["outcomes"]["wars"]
        figures = f"{count(worst['outcomes']['wars'])} wars a game at {times(worst['frequency'])}, against {count(calm['outcomes']['wars'])} calm"
        if change >= 1.25:
            say(
                "war",
                f"{change:.1f}x",
                figures,
                "A battered world fights more",
                "A weakened neighbour is a target: the second blow comes from next door.",
            )
        elif change <= 0.8:
            say(
                "war",
                f"-{pct(1 - change)}",
                figures,
                "A battered world fights less",
                f"Armies go early ({pct(kept(worst, 'army'))} left). A civilization rebuilding its farms has none to send.",
            )
    # 7. History turns on the weather: how often the calm world's winner
    # still wins, over every world the dial was turned on. Pooled, not the
    # worst frequency's: twelve worlds at one frequency is a coarse figure.
    same = sum(d["same_winner"] * d["games"] for d in chaos) / sum(d["games"] for d in chaos)
    if same <= 0.5:
        mildest = chaos[0]
        say(
            "winner",
            pct(same),
            "of worlds under any storm are won by the civilization that wins them calm",
            "Who wins is decided by the weather",
            f"Same map, same start, same players. Even at {times(mildest['frequency'])} it is "
            f"{pct(mildest['same_winner'])}. One in six would be pure chance.",
        )
    # 8. The price is paid in progress: games that end in a victory somebody
    # earned, not at the clock.
    earned = lambda d: 1 - (d["victories"].get("score", 0) + d["victories"].get("none", 0)) / d["games"]
    if earned(calm) - earned(worst) >= 0.25:
        say(
            "progress",
            f"{pct(earned(worst))} / {pct(earned(calm))}",
            f"of games end in a victory somebody reached at {times(worst['frequency'])}, and calm",
            "The cost is what never gets built",
            "A world busy rebuilding does not reach the stars. It runs out the clock.",
        )
    # 9. Who comes through: the difference between civilizations the storm
    # itself widens most, over the same split where the storm does no harm.
    if found and abs(found[0]["edge"]) >= 0.08:
        top = found[0]
        sides = ("more", "fewer") if top["edge"] > 0 else ("fewer", "more")
        better, worse = top[sides[0]], top[sides[1]]
        say(
            "who",
            f"{pct(better['kept'])} / {pct(worse['kept'])}",
            f"of their people kept under the storm by civilizations with {better['label']}, and with {worse['label']}",
            f"What went with coming through: {better['label']}",
            f"Where the storm does no harm the same halves stand at {pct(top['mild'][sides[0]])} and "
            f"{pct(top['mild'][sides[1]])}. Counted as {top['of']}.",
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
            f"Civilizations at {times(STORM)} and over, in two halves by one difference at a time: the share of "
            f"their calm selves' people each half still has."
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
