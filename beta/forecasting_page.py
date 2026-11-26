#!/usr/bin/env python3
"""The AI Forecasting page's record: how far ahead an AI agent sees.

    forecasting_page.py <source_tree>              print the record as JSON
    forecasting_page.py <source_tree> <page.html>  write it into the page
    forecasting_page.py --self-play <batches_dir> <out.json>
                                                rebuild the snapshot

The AI Forecasting page (`beta/forecasting.html`, served at
`<lane>/charts/ai-forecasting/`, the address it states itself) is the third
page under Charts/ (operator 2026-11-25: "rename ai insights to: the shape of
asi. add an ai intrepertability and an ai forecasting charts. then civ ai
interpretability and the rest"). It is for what the games show about an AI's
forecasts, a chart at a time, each a forecast set against what happened.

Its first chart is the forecast a CIVVIS AI agent makes of its own win. A
seat with the `victory-portfolio` gene on works out, for the victory it is
going for, the turn it expects to win on
(`src/ai/advanced/victory_portfolio.rs`), and writes it into the trace of its
plan: a point whenever the plan changes and at least every 17 turns
(`victory_portfolio.trace[].expected_finish`; empty while the seat cannot
yet support a finish turn). The game then says when it did win. So for every
seat that won, each point of its trace is a forecast with an answer:

- **bands**, ten turns of lead to a band (`BAND`), the nearest first: a
  point made 0 to 9 turns before the seat's win, 10 to 19, and so on up to
  `SHOWN`. Each holds the points made at that lead (`points`), how many of
  them carried a finish turn (`with`), how many of those were for a turn
  before the win (`early`), and how late the forecasts were, in turns, at
  five places through them in order: a tenth, a quarter, the middle, three
  quarters and nine tenths of the way (`late`, the forecast turn less the
  turn of the win, so a forecast for a turn too early is below zero).
- **beyond**, the same for every point made `SHOWN` turns or more before the
  win, in one.

A point counts when the victory the seat was going for at that turn (its
`primary`) is the victory it went on to win by: a Science clock is not a
forecast of a Diplomatic win. Only winners have an answer to be held to, so
the chart is of the forecasts of seats that won, and says nothing of how
sure a forecast makes a win.

The seats' rows are on the machine that plays them, so the page reads a
snapshot, `data/forecasting/self_play_forecasts.json`, which `--self-play`
rebuilds from the batch directories (the directory the other Charts/
snapshots are rebuilt from). It reads the newest finished batch alone, as the
Civ AI Interpretability snapshot does: a forecast is a fact about one build
of the AI, and a batch is one build.

Like the site's other pages this runs inside the script that publishes the
whole site: what it cannot read costs the page its figures, said on stderr,
and never the publish.
"""

from __future__ import annotations

import datetime
import json
import pathlib
import re
import sys

import self_play_rows as rows_of

SLOT = re.compile(r'(<script type="application/json" id="forecasting-data">)(.*?)(</script>)', re.S)

#: The snapshot of the seats' forecasts, in the source tree.
FORECASTS = pathlib.Path("data/forecasting/self_play_forecasts.json")

#: Turns of lead to a band, and the lead the bands stop at: further ahead
#: than this fewer than half the seats have a forecast to chart, and a band
#: holds too few of them to place its quarters.
BAND = 10
SHOWN = 120

#: Where through a band's forecasts, in order, its lateness is read.
PLACES = [0.1, 0.25, 0.5, 0.75, 0.9]

#: The victories a game ends in. A seat's trace names the one it is going
#: for (`VictoryTarget`) by the name its row gives the one it won by.
VICTORIES = ["science", "culture", "religious", "diplomatic", "domination", "score"]


def band(points: int, late: list[float]) -> dict:
    """One band as the snapshot holds it, from the points made in it and the
    lateness of each that carried a finish turn."""
    late = sorted(late)
    return {
        "points": points,
        "with": len(late),
        "early": sum(1 for turns in late if turns < 0),
        "late": [round(rows_of.quantile(late, place), 1) for place in PLACES] if late else None,
    }


def batch_forecasts(directory: pathlib.Path) -> dict | None:
    """One batch's forecasts against the wins they were of. `None` for a
    batch that does not play `SETUP`, or in which no winning seat forecast."""
    said: dict = {}
    seats = forecasting = winners = 0
    seeds = set()
    made = [0] * (rows_of.TURNS // BAND + 1)
    late: list[list[float]] = [[] for _ in made]
    try:
        for row in rows_of.seats(directory, said):
            portfolio = row.get("victory_portfolio")
            trace = portfolio.get("trace") if isinstance(portfolio, dict) else None
            end = row.get("turn")
            if not isinstance(trace, list) or not trace or not isinstance(end, int):
                continue
            seats += 1
            seeds.add(row.get("seed"))
            if portfolio.get("enabled") is not True:
                continue
            forecasting += 1
            going_for = row.get("victory")
            if row.get("win") is not True or going_for not in VICTORIES:
                continue
            winners += 1
            for point in trace:
                if not isinstance(point, dict) or not isinstance(point.get("turn"), int):
                    continue
                lead = end - point["turn"]
                if lead < 0 or point.get("primary") != going_for:
                    continue
                at = min(lead // BAND, len(made) - 1)
                made[at] += 1
                finish = point.get("expected_finish")
                if isinstance(finish, (int, float)):
                    late[at].append(float(finish) - end)
    except rows_of.OtherTable:
        return None
    shown = SHOWN // BAND
    if not any(late[:shown]):
        return None
    return {
        "commit": said.get("commit", ""),
        "games": len(seeds),
        "seats": seats,
        "forecasting": forecasting,
        "winners": winners,
        "bands": [{"from": index * BAND, **band(made[index], late[index])} for index in range(shown)],
        "beyond": {"from": SHOWN, **band(sum(made[shown:]), [turns for each in late[shown:] for turns in each])},
    }


def snapshot(batches: pathlib.Path) -> dict:
    """The newest finished batch under a gene-screen state directory's
    `batches/` that plays `SETUP` and has forecasts, as the snapshot file
    holds it."""
    for directory in rows_of.finished(batches):
        forecasts = batch_forecasts(directory)
        if forecasts is None:
            continue
        return {
            "about": "The forecast each winning seat of the newest finished gene-screen batch made of the turn it "
            "would win, against the turn it won, by how far ahead the forecast was made. "
            "Rebuilt by `beta/forecasting_page.py --self-play`; read by the AI Forecasting page.",
            "setup": "6 players, Continents, Online speed, 250 turns, every seat a CIVVIS AI agent",
            "taken": datetime.datetime.now(datetime.timezone.utc).date().isoformat(),
            "batch": directory.name,
            "began": rows_of.began(directory),
            "band": BAND,
            "places": PLACES,
            **forecasts,
        }
    raise SystemExit(f"forecasting_page: no finished batch with a forecast under {batches}")


def own_win(source: pathlib.Path) -> dict | None:
    """The first chart's record: the snapshot, when it is one the page can
    draw."""
    try:
        taken = json.loads((source / FORECASTS).read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        print(f"forecasting_page: no snapshot of the seats' forecasts ({error})", file=sys.stderr)
        return None
    bands = taken.get("bands") if isinstance(taken, dict) else None
    if not isinstance(bands, list) or not any(isinstance(b, dict) and b.get("late") for b in bands):
        print(f"forecasting_page: {FORECASTS} holds no forecast", file=sys.stderr)
        return None
    return {key: taken.get(key) for key in (
        "taken", "batch", "began", "commit", "games", "seats", "forecasting", "winners", "band", "places", "bands", "beyond")}


def record(source: pathlib.Path) -> dict | None:
    """The page's record, or `None` when no chart has figures."""
    out = {"own_win": own_win(source)}
    return out if out["own_win"] else None


def fill(page: str, data: dict | None) -> str | None:
    """The page with its slot filled, or `None` when it has no slot."""
    # `<` is escaped so no name or sentence can close the script element.
    text = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003c")
    filled, count = SLOT.subn(lambda m: m[1] + text + m[3], page, count=1)
    return filled if count else None


def main(argv: list[str]) -> int:
    if len(argv) == 4 and argv[1] == "--self-play":
        out = pathlib.Path(argv[3])
        taken = snapshot(pathlib.Path(argv[2]))
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(taken, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        print(f"forecasting_page: wrote {taken['winners']} winning seats' forecasts ({taken['batch']}) to {out}", file=sys.stderr)
        return 0
    if len(argv) not in (2, 3):
        print(__doc__, file=sys.stderr)
        return 2
    data = record(pathlib.Path(argv[1]))
    if len(argv) == 2:
        json.dump(data, sys.stdout, ensure_ascii=False, indent=2)
        print()
        return 0
    page = pathlib.Path(argv[2])
    filled = fill(page.read_text(encoding="utf-8"), data)
    if filled is None:
        print(f"forecasting_page: {page} has no forecasting slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(filled, encoding="utf-8")
    if data is None:
        print(f"forecasting_page: no figures to write into {page}", file=sys.stderr)
    else:
        print(f"forecasting_page: wrote {data['own_win']['winners']} winning seats' forecasts into {page}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
