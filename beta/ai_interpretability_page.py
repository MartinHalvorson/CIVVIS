#!/usr/bin/env python3
"""The AI Interpretability page's record: reading an AI agent from outside.

    ai_interpretability_page.py <source_tree>              print the record as JSON
    ai_interpretability_page.py <source_tree> <page.html>  write it into the page
    ai_interpretability_page.py --self-play <batches_dir> <out.json>
                                                rebuild the snapshot

The AI Interpretability page (`beta/ai_interpretability.html`, served at
`<lane>/charts/ai-interpretability/`, the address it states itself) is the
second page under Charts/ (operator 2026-11-25: "rename ai insights to: the
shape of asi. add an ai intrepertability and an ai forecasting charts. then
civ ai interpretability and the rest"). The page after it, Civ AI
Interpretability (`beta/interpretability.html`, which had this name and
address until 2026-11-24), opens the CIVVIS AI agent up and reads its rules.
This one is for what the games show about reading an AI without opening it,
a chart at a time.

Its first chart asks how early an agent's aim shows in what it does. In a
gene-screen batch every seat but the ones left to choose is told which of the
six victories to go for (`player_target` on its row), and its row keeps what
its empire looked like at every 25th turn (`trajectory`). A reader is given
only that look, seven numbers (`FEATURES`), and has to say which aim the seat
was given:

- **read**, a line per mark (`MARKS`): the seats still playing at that turn
  (`seats`), how many of them the reader named the aim of (`right`), and the
  same two per aim (`aims`). Guessing would be right one time in six.

The reader is the plainest there is, so that what it reads is in the empires
and not in the reader (`read_aims`): it learns the average empire of each aim
from half the games, and names for each seat of the other half the aim whose
average its empire is nearest to, each number first put on the scale of how
much seats differ in it at that turn. Then the halves change places, so every
seat is read once by a reader that never saw its game. A game goes whole to
one half: seats of one game share a map and a history.

The seats' rows are on the machine that plays them, so the page reads a
snapshot, `data/ai_interpretability/self_play_aims.json`, which `--self-play`
rebuilds from the batch directories (the directory the other Charts/
snapshots are rebuilt from). It reads the newest finished batch alone, as the
Civ AI Interpretability snapshot does: how an agent plays is a fact about
one build of the AI, and a batch is one build.

Like the site's other pages this runs inside the script that publishes the
whole site: what it cannot read costs the page its figures, said on stderr,
and never the publish.
"""

from __future__ import annotations

import datetime
import json
import math
import pathlib
import re
import statistics
import sys

import self_play_rows as rows_of

SLOT = re.compile(r'(<script type="application/json" id="ai-interpretability-data">)(.*?)(</script>)', re.S)

#: The snapshot of how the seats' aims were read, in the source tree.
AIMS_READ = pathlib.Path("data/ai_interpretability/self_play_aims.json")

#: The aims a seat can be given, in the site's order of the victories, and
#: what the reader is shown of its empire: everything a row's `trajectory`
#: keeps of a turn but whether the seat was alive.
AIMS = ["science", "culture", "religious", "diplomatic", "domination", "score"]
FEATURES = ["cities", "techs", "civics", "science", "culture", "military", "wars"]

#: The turns the seats are read at. A row keeps every 25th turn; past turn
#: 200 most games are over and too few seats are left to read.
MARKS = [25, 50, 75, 100, 125, 150, 175, 200]


def looked(point: dict) -> list[float]:
    """A seat's empire at one turn as the reader is shown it. Each number is
    taken as its logarithm: yields and armies grow many times over through a
    game, and what tells two seats apart is how many times bigger, not how
    much."""
    return [math.log1p(max(0.0, float(point.get(name) or 0))) for name in FEATURES]


def read_aims(seats: list[tuple]) -> dict:
    """How many of one turn's seats a reader names the aim of. `seats` are
    `(game, aim, look)`. Returns `{"seats", "right", "aims": {aim: [seats,
    right]}}`."""
    games = sorted({game for game, _, _ in seats}, key=str)
    half = {game: index % 2 for index, game in enumerate(games)}
    counted = {aim: [0, 0] for aim in AIMS}
    for learned in (0, 1):
        known = [seat for seat in seats if half[seat[0]] == learned]
        asked = [seat for seat in seats if half[seat[0]] != learned]
        if not known or not asked:
            continue
        width = range(len(FEATURES))
        middle = [statistics.fmean(look[i] for _, _, look in known) for i in width]
        spread = [statistics.pstdev([look[i] for _, _, look in known]) or 1.0 for i in width]
        scaled = lambda look: [(look[i] - middle[i]) / spread[i] for i in width]  # noqa: E731
        average = {}
        for aim in AIMS:
            of_aim = [scaled(look) for _, given, look in known if given == aim]
            if of_aim:
                average[aim] = [statistics.fmean(look[i] for look in of_aim) for i in width]
        if not average:
            continue
        for _, given, look in asked:
            at = scaled(look)
            named = min(average, key=lambda aim: sum((at[i] - average[aim][i]) ** 2 for i in width))
            counted[given][0] += 1
            counted[given][1] += named == given
    return {
        "seats": sum(seats_ for seats_, _ in counted.values()),
        "right": sum(right for _, right in counted.values()),
        "aims": counted,
    }


def batch_aims(directory: pathlib.Path) -> dict | None:
    """One batch's seats read at every mark. `None` for a batch that does
    not play `SETUP`, or with no seat that was given an aim."""
    said: dict = {}
    at_mark: dict[int, list[tuple]] = {mark: [] for mark in MARKS}
    seeds, seats = set(), 0
    try:
        for row in rows_of.seats(directory, said):
            aim = row.get("player_target")
            trajectory = row.get("trajectory")
            if aim not in AIMS or not isinstance(trajectory, list):
                continue
            seats += 1
            seeds.add(row.get("seed"))
            for point in trajectory:
                if isinstance(point, dict) and point.get("turn") in at_mark and point.get("alive"):
                    at_mark[point["turn"]].append((row.get("seed"), aim, looked(point)))
    except rows_of.OtherTable:
        return None
    if not seats:
        return None
    return {
        "commit": said.get("commit", ""),
        "games": len(seeds),
        "seats": seats,
        "read": [{"turn": mark, **read_aims(at_mark[mark])} for mark in MARKS if at_mark[mark]],
    }


def snapshot(batches: pathlib.Path) -> dict:
    """The newest finished batch under a gene-screen state directory's
    `batches/` that plays `SETUP` and gives its seats aims, as the snapshot
    file holds it."""
    for directory in rows_of.finished(batches):
        aims = batch_aims(directory)
        if aims is None:
            continue
        return {
            "about": "How many seats of the newest finished gene-screen batch a reader named the aim of, at every "
            "25th turn, from seven numbers about the seat's empire. "
            "Rebuilt by `beta/ai_interpretability_page.py --self-play`; read by the AI Interpretability page.",
            "setup": "6 players, Continents, Online speed, 250 turns, every seat a CIVVIS AI agent",
            "taken": datetime.datetime.now(datetime.timezone.utc).date().isoformat(),
            "batch": directory.name,
            "began": rows_of.began(directory),
            "aims": AIMS,
            "features": FEATURES,
            **aims,
        }
    raise SystemExit(f"ai_interpretability_page: no finished batch whose seats were given aims under {batches}")


def aim_read(source: pathlib.Path) -> dict | None:
    """The first chart's record: the snapshot, when it is one the page can
    draw."""
    try:
        taken = json.loads((source / AIMS_READ).read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        print(f"ai_interpretability_page: no snapshot of the seats' aims ({error})", file=sys.stderr)
        return None
    read = taken.get("read") if isinstance(taken, dict) else None
    if not isinstance(read, list) or not any(isinstance(mark, dict) and mark.get("seats") for mark in read):
        print(f"ai_interpretability_page: {AIMS_READ} holds no seat that was read", file=sys.stderr)
        return None
    return {key: taken.get(key) for key in (
        "taken", "batch", "began", "commit", "games", "seats", "aims", "features", "read")}


def record(source: pathlib.Path) -> dict | None:
    """The page's record, or `None` when no chart has figures."""
    out = {"aim_read": aim_read(source)}
    return out if out["aim_read"] else None


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
        print(f"ai_interpretability_page: wrote {taken['seats']} seats' aims as read ({taken['batch']}) to {out}", file=sys.stderr)
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
        print(f"ai_interpretability_page: {page} has no AI interpretability slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(filled, encoding="utf-8")
    if data is None:
        print(f"ai_interpretability_page: no figures to write into {page}", file=sys.stderr)
    else:
        print(f"ai_interpretability_page: wrote {data['aim_read']['seats']} seats' aims as read into {page}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
