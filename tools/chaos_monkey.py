#!/usr/bin/env python3
"""Chaos Monkey's record: what worlds played under a very high frequency of
natural disasters did, said only as far as the games settle it.

    chaos_monkey.py <records.jsonl> [...]                 print the record as JSON
    chaos_monkey.py --write <records.jsonl> [...]         write data/chaos_monkey.json
    chaos_monkey.py --write --commit <sha> --date <day> <records.jsonl> [...]

Chaos engineering breaks a running system on purpose, to find where it is
fragile before a real failure does (operator 2026-11-25: "A great thought
today is chaos engineering society in simulation to try to learn any lessons
we can, without having to go through the event ourselves to learn ... Here we
can run civvis sims with a very high frequency of natural disasters. what can
we learn about avoiding horrible outcomes?"). The page keeps only what the
statistics back strongly (operator 2026-11-25: "clean up the chaos
engineering stuf ffor now please. the charts specifically. only things that
are highly backed by statistics").

The games are `civvis chaos-monkey`'s (`src/chaos_monkey.rs`): every seed is
played once at each disaster frequency by the same agents, so the worlds at
one seed differ in their disasters and in nothing else, and a world under
chaos has its own calm self (1x, Civilization VI as shipped) to be read
against. This script reads those games, one JSON record a line, and writes
the record the Chaos Monkey page draws (`beta/chaos.html`, filled by
`beta/chaos_page.py`):

- **doses**, one per frequency, lowest first: the games and the disasters a
  game, then the figures. `kept` is the world as a share of the same seeds
  played calm, in people, cities, science, army and working land. A world is
  read against its calm self **at the same turn**: the last tenth turn both
  games reached (`common_turn`). Games end when somebody wins, a calm world
  and its stormy twin seldom on the same turn, and a world that played forty
  turns longer has forty turns more of everything. `wars`, `captures` and
  `won` are wars started a game, cities that changed hands a game, and the
  share of games won outright rather than on score at the turn limit.
- **Every figure under chaos is tested** against the same seeds played calm
  (`tested`). `low` and `high` are where 95 in 100 of `REDRAWS` redraws of
  the worlds put it, whole worlds at a time. `p` is an exact paired test
  (Wilcoxon's signed ranks; on a yes or no it is the sign test). A figure is
  `sure` when `p` is under `SURE`, one in a thousand: with `TESTS` figures
  tested, the chance that even one sure figure is luck stays under one in
  twenty.
- **lessons**, what the games say, each a figure, its range and a sentence
  written from the record's own numbers by a rule in `lessons()`. A lesson
  rests only on sure figures, so the page cannot say what its games do not
  settle.

It reads games and writes a record; it plays nothing. The same seeds on the
same commit give the same games, so the record names the commit, the seeds
and the command that played them rather than keeping the games themselves.
"""

from __future__ import annotations

import json
import math
import pathlib
import random
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RECORD = ROOT / "data" / "chaos_monkey.json"

#: The five things measured in a world against its calm self, in the order
#: the page draws them: the key, the name a lesson gives it, and whether that
#: name is plural ("armies go", "science goes").
KEPT = {
    "people": ("people", True),
    "cities": ("cities", True),
    "science": ("science", False),
    "army": ("armies", True),
    "land": ("working land", False),
}

#: The figures a game, beside the world's shares.
OUTCOMES = ("wars", "captures", "won")

#: How many times the worlds are drawn again, with replacement, whole worlds
#: at a time, to find how far each figure moves by the luck of which worlds
#: were played; and the share of those redraws the range holds.
REDRAWS = 10000
LEVEL = 0.95

#: A figure is sure when the chance of a difference this big from the calm
#: worlds by luck alone is under this.
SURE = 0.001

#: How many figures each frequency past 1x tests: the five shares and the
#: three outcomes. With five frequencies that is 40, and 40 tests at one in a
#: thousand keep the chance of even one wrong sure figure under 5%.
TESTS_A_FREQUENCY = len(KEPT) + len(OUTCOMES)


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


def world(game: dict, key: str, at: int) -> float:
    """A series summed over the game's civilizations at one sample."""
    return float(sum(civ["series"][key][at] for civ in game["civs"]))


def at_turn(game: dict, turn: int) -> int:
    """The index of the last sample on or before a turn."""
    at = 0
    for index, sampled in enumerate(game["sample_turns"]):
        if sampled <= turn:
            at = index
    return at


def common_turn(game: dict, calm: dict) -> int:
    """The last sampled turn both a game and its calm twin reached: the turn
    the two worlds are compared on."""
    turn = min(game["ended"], calm["ended"])
    return max(1, turn // 10 * 10)


def end(game: dict, turn: int | None = None) -> dict:
    """The world as the game ended, or as it stood on a turn: one sample,
    summed over its civilizations."""
    last = len(game["sample_turns"]) - 1 if turn is None else at_turn(game, turn)
    return {
        "people": world(game, "pop", last),
        "cities": world(game, "cities", last),
        "science": world(game, "techs", last),
        "army": world(game, "army", last),
        "land": world(game, "improved", last) - world(game, "pillaged", last),
    }


def won(game: dict) -> bool:
    """Whether somebody won the game outright, rather than on score when the
    turns ran out."""
    return bool(game.get("winner")) and "score" not in (game.get("victory") or "").lower()


def outcome(game: dict, key: str) -> float:
    if key == "wars":
        return float(game["wars"])
    if key == "captures":
        return float(sum(civ["cities_lost"] for civ in game["civs"]))
    return float(won(game))


# ------------------------------------------------------------- statistics

def signed_rank_p(differences: list[float]) -> float:
    """The exact two-sided p of Wilcoxon's signed-rank test that paired
    differences centre on nothing. Ties share their mean rank; a difference
    of nothing is set aside. On differences that are all the same size (a
    yes or no against a yes or no) it is the exact sign test."""
    moved = [d for d in differences if abs(d) > 1e-12]
    if not moved:
        return 1.0
    order = sorted(range(len(moved)), key=lambda i: abs(moved[i]))
    ranks = [0.0] * len(moved)
    start = 0
    while start < len(order):
        stop = start
        while stop + 1 < len(order) and abs(abs(moved[order[stop + 1]]) - abs(moved[order[start]])) < 1e-12:
            stop += 1
        for at in range(start, stop + 1):
            ranks[order[at]] = (start + stop) / 2 + 1
        start = stop + 1
    # Ranks doubled, so a shared half rank is still a whole number.
    doubled = [round(2 * rank) for rank in ranks]
    observed = sum(rank for rank, d in zip(doubled, moved) if d > 0)
    ways = {0: 1}
    for rank in doubled:
        grown: dict[int, int] = {}
        for total, count in ways.items():
            grown[total] = grown.get(total, 0) + count
            grown[total + rank] = grown.get(total + rank, 0) + count
        ways = grown
    every = 2 ** len(moved)
    below = sum(count for total, count in ways.items() if total <= observed) / every
    above = sum(count for total, count in ways.items() if total >= observed) / every
    return min(1.0, 2 * min(below, above))


def redraws(seeds: list[int]) -> list[list[int]]:
    """The worlds drawn again `REDRAWS` times, with replacement, the same
    draws for every figure."""
    draw = random.Random(20261125)
    return [[draw.choice(seeds) for _ in seeds] for _ in range(REDRAWS)]


def spread(values: list[float]) -> tuple[float, float]:
    """Where `LEVEL` of the redrawn figures fall."""
    ordered = sorted(values)
    cut = int(round(len(ordered) * (1 - LEVEL) / 2))
    return ordered[cut], ordered[len(ordered) - 1 - cut]


def tested(value: float, redrawn: list[float], p: float, digits: int) -> dict:
    low, high = spread(redrawn)
    return {
        "value": round(value, digits),
        "low": round(low, digits),
        "high": round(high, digits),
        "p": float(f"{p:.3g}"),
        "sure": p < SURE,
    }


# ------------------------------------------------------------------ doses

def dose(frequency: float, games: dict[int, dict], calm: dict[int, dict], draws: list[list[int]] | None) -> dict:
    """One frequency's games, by seed, against the same seeds played calm.
    `draws` is `None` for the calm worlds themselves, which are the
    reference and are not tested."""
    seeds = sorted(games)
    disasters = sum(sum(game["disasters"].values()) for game in games.values()) / len(seeds)
    out = {
        "frequency": int(frequency) if float(frequency).is_integer() else frequency,
        "games": len(seeds),
        "disasters": round(disasters, 1),
        "kept": {},
    }
    if draws is None:
        out["kept"] = {key: {"value": 1.0} for key in KEPT}
        for key in OUTCOMES:
            out[key] = {"value": round(sum(outcome(games[s], key) for s in seeds) / len(seeds), 4 if key == "won" else 2)}
        return out
    for key in KEPT:
        pair = {}
        for seed in seeds:
            turn = common_turn(games[seed], calm[seed])
            pair[seed] = (end(games[seed], turn)[key], end(calm[seed], turn)[key])
        share = lambda sample: sum(pair[s][0] for s in sample) / max(1e-9, sum(pair[s][1] for s in sample))
        p = signed_rank_p([math.log((now + 1) / (then + 1)) for now, then in pair.values()])
        out["kept"][key] = tested(share(seeds), [share(sample) for sample in draws], p, 4)
    for key in OUTCOMES:
        pair = {seed: (outcome(games[seed], key), outcome(calm[seed], key)) for seed in seeds}
        average = lambda sample: sum(pair[s][0] for s in sample) / len(sample)
        p = signed_rank_p([now - then for now, then in pair.values()])
        out[key] = tested(average(seeds), [average(sample) for sample in draws], p, 4 if key == "won" else 2)
    return out


# ---------------------------------------------------------------- lessons

def pct(share: float) -> str:
    return f"{round(share * 100)}%"


def times(frequency) -> str:
    return f"{frequency:,g}x"


def count(value: float) -> str:
    return f"{value:,.0f}" if value >= 10 else f"{value:.1f}".rstrip("0").rstrip(".")


def lessons(doses: list[dict]) -> list[dict]:
    """What the games say. Each lesson is written only when its own rule
    holds on the record, and every figure it quotes is sure, so the list is
    as long as the games settle and no longer."""
    out: list[dict] = []
    if len(doses) < 2 or doses[0]["frequency"] != 1:
        return out
    calm, chaos, worst = doses[0], doses[1:], doses[-1]
    sure = lambda figure: bool(figure.get("sure"))
    say = lambda key, figure, of, within, title, text: out.append(
        {"key": key, "figure": figure, "of": of, "range": within, "title": title, "text": text}
    )

    # 1. Mild chaos pays: frequencies at which the world has surely more
    # people than its calm self, the biggest gain first.
    gains = sorted(
        (d for d in chaos if sure(d["kept"]["people"]) and d["kept"]["people"]["value"] > 1),
        key=lambda d: -d["kept"]["people"]["value"],
    )
    if gains:
        best = gains[0]["kept"]["people"]
        also = "".join(f" {times(d['frequency'])} too: +{pct(d['kept']['people']['value'] - 1)}." for d in gains[1:])
        say(
            "pays",
            f"+{pct(best['value'] - 1)}",
            f"more people at {times(gains[0]['frequency'])} than in the same worlds played calm, on the same turn",
            f"+{pct(best['low'] - 1)} to +{pct(best['high'] - 1)}",
            "Mild chaos pays",
            f"In Civilization VI's rules a flood leaves silt and an eruption leaves soil.{also}",
        )
    # 2. Where it breaks: the first frequency that surely costs a quarter of
    # the people.
    broken = next((d for d in chaos if sure(d["kept"]["people"]) and d["kept"]["people"]["value"] <= 0.75), None)
    if broken:
        people = broken["kept"]["people"]
        before = doses[doses.index(broken) - 1]["kept"]["people"]
        text = (
            f"One step down the dial, at {times(doses[doses.index(broken) - 1]['frequency'])}, the loss was {pct(1 - before['value'])}."
            if sure(before) and before["value"] < 1
            else f"One step down the dial it could not be told from the calm world."
        )
        say(
            "breaks",
            times(broken["frequency"]),
            f"is where it breaks: {pct(1 - people['value'])} fewer people than the same worlds played calm",
            f"{pct(1 - people['high'])} to {pct(1 - people['low'])} fewer",
            "Then it breaks",
            text,
        )
    # 3. What gives way first and what holds, at the highest frequency: the
    # two ends, when both are sure and their ranges do not meet.
    held = {key: worst["kept"][key] for key in KEPT if sure(worst["kept"][key])}
    if len(held) >= 2:
        ranked = sorted(held, key=lambda key: held[key]["value"])
        weakest, strongest = ranked[0], ranked[-1]
        if held[weakest]["high"] < held[strongest]["low"]:
            (weak, weak_plural), (strong, strong_plural) = KEPT[weakest], KEPT[strongest]
            between = ", ".join(f"{KEPT[key][0]} {pct(held[key]['value'])}" for key in ranked[1:-1])
            say(
                "first",
                f"{pct(held[weakest]['value'])} / {pct(held[strongest]['value'])}",
                f"of the {weak} and of the {strong} left at {times(worst['frequency'])}",
                f"{pct(held[weakest]['low'])} to {pct(held[weakest]['high'])} and {pct(held[strongest]['low'])} to {pct(held[strongest]['high'])}",
                f"{weak.capitalize()} {'go' if weak_plural else 'goes'} first; {strong} {'hold' if strong_plural else 'holds'}",
                f"Between them: {between}." if between else "",
            )
    # 4. War: does the storm surely bring more of it, or less?
    wars, then = worst["wars"], calm["wars"]["value"]
    if sure(wars) and then > 0:
        change = wars["value"] / then
        figures = f"{count(wars['value'])} wars a game at {times(worst['frequency'])}, against {count(then)} calm"
        within = f"{count(wars['low'])} to {count(wars['high'])} wars a game"
        taken = worst["captures"]
        conquest = (
            f" Cities changing hands: {count(taken['value'])} a game, against {count(calm['captures']['value'])}."
            if sure(taken)
            else ""
        )
        if change >= 1.25:
            say("war", f"{change:.1f}x", figures, within, "A battered world fights more", conquest.strip())
        elif change <= 0.8:
            say("war", f"-{pct(1 - change)}", figures, within, "A battered world fights less", conquest.strip())
    # 5. Victories stop: games won outright, not on score at the turn limit.
    for d in chaos:
        if sure(d["won"]) and calm["won"]["value"] - d["won"]["value"] >= 0.25:
            gone = d["won"]
            later = [e for e in chaos if e["frequency"] > d["frequency"] and sure(e["won"])]
            say(
                "won",
                f"{pct(gone['value'])} / {pct(calm['won']['value'])}",
                f"of games won outright at {times(d['frequency'])}, and calm",
                f"{pct(gone['low'])} to {pct(gone['high'])} at {times(d['frequency'])}",
                "Nobody wins outright any more",
                "The rest run out the clock and are decided on score."
                + "".join(f" {times(e['frequency'])}: {pct(e['won']['value'])}." for e in later),
            )
            break
    return out


def record(games: list[dict], played: dict | None = None) -> dict | None:
    """The page's record, or `None` when there are no games to read."""
    if not games:
        return None
    frequencies = sorted({float(game["frequency"]) for game in games})
    if 1.0 not in frequencies:
        raise ValueError("no games at 1x: a world under chaos needs its calm self to be read against")
    calm = {game["seed"]: game for game in games if float(game["frequency"]) == 1.0}
    at = {f: {game["seed"]: game for game in games if float(game["frequency"]) == f and game["seed"] in calm} for f in frequencies}
    draws = redraws(sorted(calm))
    doses = []
    for f in frequencies:
        if f != 1.0 and sorted(at[f]) != sorted(calm):
            # A redraw is of whole worlds: every seed at every frequency.
            raise ValueError(f"the games at {times(f)} are not the calm worlds' seeds")
        doses.append(dose(f, at[f], calm, None if f == 1.0 else draws))
    seeds = sorted(calm)
    first = games[0]
    limit = max(game["max_turns"] for game in games)
    setup = (
        f"{first['players']} civilizations, every seat a CIVVIS AI agent, {limit} turns, "
        f"seeds {seeds[0]} to {seeds[-1]}, each played at {', '.join(times(d['frequency']) for d in doses)}"
    )
    return {
        "played": {"games": len(games), "seeds": [seeds[0], seeds[-1]], "setup": setup, **(played or {})},
        "tested": {
            "figures": TESTS_A_FREQUENCY * (len(doses) - 1),
            "sure": sum(
                bool(figure.get("sure"))
                for d in doses[1:]
                for figure in [*d["kept"].values(), *(d[key] for key in OUTCOMES)]
            ),
            "p": SURE,
            "level": LEVEL,
            "redraws": REDRAWS,
        },
        "doses": doses,
        "lessons": lessons(doses),
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
