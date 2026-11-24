#!/usr/bin/env python3
"""The Performance page's figures: how long the simulator takes to play a game.

    performance_page.py <source_tree>              print the record as JSON
    performance_page.py <source_tree> <page.html>  write it into the page
    performance_page.py --self-play <batches_dir> <out.json>
                                                rebuild the timing snapshot

The Performance page (`beta/performance.html`, served at
`<lane>/charts/civvis-performance/`, the address it states itself) is
CIVVIS's own page in the site list: the entry that
opens under CIVVIS (operator 2026-11-21: "performance should be a page
similar to the home/ and other similar pages (musings/research/) etc. we want
the civvis performance charts there. fastest game video. p1, p10, p50, p90,
p99 times for games for self-play games."). It carries five things:

- **CIVVIS computer control: game speed over time**, at its top (operator
  2026-11-24: "we should be benchmarking our civvis computer control games
  time end to end and that number show flow through to a performance chart
  (civvis comptuer control games speed over time) and be a top chart for
  civvis performance"): how long one game takes with CIVVIS at the controls
  of Civilization VI itself, end to end, day by day. `control_times.control`
  works it out of the run ledger (`results.jsonl` on the `ledger` branch) as
  it stands when the site is published, so it moves with every finished game
  and nothing of it is committed.
- **how many times faster than a game played by hand**, under it
  (operator 2026-11-22: "at the top of civvis performance we want to
  calculate a multiplier on the theoretical and experimetnal throughput of
  the simualtor. and the calculations. baeline is 5 hrs for manually played
  game."): the readings of `data/performance/speedup_timeline.json`, the
  ledger the home page's speed timeline is drawn from, as two tables with
  the sum beside every figure. `speedup_timeline.worked` works them out;
  nothing of them is written in the page. Under them the same benchmarks
  stand processor by processor (operator 2026-11-22: "we should have a
  table in civvis performance that has our benchmarks (and age) across each
  processor. we should also list the single thread score and multithread
  score from the website"): the processors of
  `data/performance/processors.json`, the one the benchmarks were run on as
  measured and every other worked out from the website's two scores.
- **each way of playing over time** (operator 2026-11-24: "in civvis
  performance, we should have charts studying the improvements rates of each
  of these over time"): a small chart for every stop of the home page's line
  after the baseline, its readings by the day they were taken and the pace
  they have kept. `speedup_timeline.history` builds them from the same
  ledger, with the self-play tournament's batches beside the self-play
  benchmark.
- **the path through one game** (operator 2026-11-24: "we should try to
  reduce the path of civvis through civ 6 down to the computatoinal
  irreducability and should have a chart tracking the size/num of steps"):
  the actions of one self-play game from its first turn to its last, counted
  by `tools/path_steps.py` into `data/performance/path_steps.json`: how many
  steps, how many bytes, how many of the steps a unit's walk taken as one
  step would save, and what the same path costs with no agent deciding it.
  `path` reads the ledger. `docs/PATH_STEPS.md` is the write-up.
- **the fastest game on video**: the record videos of the site playing a
  complete game in real time, listed in
  `data/performance/fastest_game_videos.json`. The one with the shortest time
  is the page's video and the rest stand under it as the earlier records. A
  video is hosted on YouTube and the page links it (operator 2026-11-21:
  "website is a fast and light surface layer ... store the big stuff
  elsewhere"), so a new record is a line added to that file.
- **how long a self-play game takes**, at the 1st, 10th, 50th, 90th and 99th
  percentile: the wall-clock seconds of the gene-screen tournament batches'
  games, every seat a CIVVIS AI agent, six players on a Continents map at
  Online speed, 250 turns. The p10 is the time one game in ten beats, the p50
  the middle game, the p99 the time one game in a hundred runs past.
- **game time batch by batch**: the same five percentiles for every batch, so
  the chart shows the simulator getting faster build by build.

The games are rows on the machine that plays them, not in the repository, so
the figures come from a snapshot, `data/performance/self_play_times.json`,
which `--self-play` rebuilds from the batch directories. A game is a seed:
every seat of a game writes a row with the same `secs`, the game's whole wall
time, and the first row of a seed is the game. A batch counts once it is
finished (its `analysis.json` is written); the batch still being played is
left out, because a few hundred of its games are too few for a 1st or a 99th
percentile to mean anything. Percentiles are nearest-rank on the games
themselves, taken when the snapshot is: the file holds the answers and a
coarse count of games per span of time for the chart, not the games.

These are a tournament's times, not a benchmark's: seven games are played at
once on one machine that is doing other work too, so a time moves with the
machine's load as well as with the code. The page says so.

Like the home page's tables this runs inside the script that publishes the
whole site, so what it cannot read costs the page its figures, said on
stderr, and never costs the site its publish. A lane is filled by its own
revision's copy of this script from its own revision's snapshot and record
videos (`fill` in `beta/publish.sh`): a refreshed snapshot or a new record
is on /test once it is on main, and on the front page once it is promoted.
"""

from __future__ import annotations

import datetime
import json
import math
import pathlib
import re
import sys

import control_times
import speedup_timeline

SLOT = re.compile(r'(<script type="application/json" id="performance-data">)(.*?)(</script>)', re.S)

#: The snapshot of the self-play games' times, in the source tree.
SELF_PLAY = pathlib.Path("data/performance/self_play_times.json")

#: The path through one game, reading by reading, in the source tree
#: (`tools/path_steps.py --record`).
PATH = pathlib.Path("data/performance/path_steps.json")

#: How many kinds of step the page's chart of the newest reading names; the
#: rest stand together under one bar.
PATH_KINDS = 7

#: The record videos, in the source tree.
VIDEOS = pathlib.Path("data/performance/fastest_game_videos.json")

#: The percentiles the page shows, in its order (operator 2026-11-21: "p1,
#: p10, p50, p90, p99 times for games").
PERCENTILES = [1, 10, 50, 90, 99]

#: The windows, newest first: the newest finished batch, the batches that
#: finished in the last seven days counted back from it, and every batch.
WINDOWS = ["latest", "7", "all"]

#: The chart counts games in spans of time that each run 12% longer than the
#: one before, twenty to a tenfold: span `i` is 10^(i/20) seconds up to
#: 10^((i+1)/20). Game times spread over two and three tenfolds, and on even
#: spans of seconds the fast games would share one column.
SPANS_PER_TENFOLD = 20

#: A batch directory is named for the moment it began: `20261121T083532Z-db65`.
BATCH = re.compile(r"(\d{4})(\d{2})(\d{2})T\d{6}Z-[0-9a-f]+")

#: The table the snapshot keeps: a batch whose header says anything else is
#: left out, so the times are all of one kind of game.
SETUP = {"speed": "online", "turns": 250, "players": 6, "map": "continents", "all_seats": True}

#: A record video is a video on YouTube, named by its eleven-character id in
#: either of the two addresses YouTube gives for one.
VIDEO = re.compile(r"https://(?:www\.youtube\.com/watch\?v=|youtu\.be/)([\w-]{11})(?:[?&#].*)?")


def percentile(ordered: list[float], p: int) -> float:
    """The nearest-rank percentile of a sorted list: the smallest value with
    at least `p` percent of the values at or under it."""
    rank = max(1, math.ceil(p * len(ordered) / 100))
    return ordered[min(len(ordered), rank) - 1]


def span(secs: float) -> int:
    """The chart span a time falls in (see `SPANS_PER_TENFOLD`)."""
    return math.floor(SPANS_PER_TENFOLD * math.log10(secs))


def summary(games: list[tuple[float, int, str]]) -> dict:
    """What the page shows of a set of games, each `(secs, turn, victory)`:
    the five percentiles, the fastest game, the middle game's length in turns
    and in seconds a turn, and the games in each span of time."""
    ordered = sorted(secs for secs, _, _ in games)
    fastest = min(games)
    a_turn = sorted(secs / turn for secs, turn, _ in games)
    spans: dict[int, int] = {}
    for secs in ordered:
        spans[span(secs)] = spans.get(span(secs), 0) + 1
    return {
        "games": len(ordered),
        "p": [round(percentile(ordered, p), 1) for p in PERCENTILES],
        "fastest": {"secs": round(fastest[0], 1), "turn": fastest[1], "victory": fastest[2]},
        "slowest": round(ordered[-1], 1),
        "turns": int(percentile(sorted(turn for _, turn, _ in games), 50)),
        "a_turn": round(percentile(a_turn, 50), 3),
        "spans": {str(i): n for i, n in sorted(spans.items())},
    }


def batch_games(directory: pathlib.Path) -> tuple[list[tuple[float, int, str]], str] | None:
    """One batch's games as `(secs, turn, victory)`, a game per seed, and the
    commit its build was made from. `None` for a batch whose header is not
    the table in `SETUP`, or with no game that has a time."""
    games: dict[int, tuple[float, int, str]] = {}
    commit = ""
    for rows in sorted(directory.glob("rows*.jsonl")):
        with rows.open(encoding="utf-8") as lines:
            for line in lines:
                try:
                    row = json.loads(line)
                except ValueError:
                    continue
                if row.get("kind") == "header":
                    if any(row.get(key) != value for key, value in SETUP.items()):
                        return None
                    build = row.get("build")
                    commit = commit or str((build if isinstance(build, dict) else {}).get("commit") or "")
                    continue
                if row.get("kind") != "game" or row.get("seed") in games:
                    continue
                secs, turn = row.get("secs"), row.get("turn")
                if isinstance(secs, (int, float)) and math.isfinite(secs) and secs > 0 and isinstance(turn, int) and turn >= 1:
                    games[row["seed"]] = (float(secs), turn, str(row.get("victory") or ""))
    return (list(games.values()), commit) if games else None


def snapshot(batches: pathlib.Path) -> dict:
    """Every finished batch under a gene-screen state directory's `batches/`,
    oldest first, and the three windows, as the snapshot file holds them."""
    out, played = [], []
    for directory in sorted(p for p in batches.iterdir() if p.is_dir() and BATCH.fullmatch(p.name)):
        if not (directory / "analysis.json").is_file():
            continue  # still being played
        read = batch_games(directory)
        if not read:
            continue
        games, commit = read
        m = BATCH.fullmatch(directory.name)
        began = f"{m[1]}-{m[2]}-{m[3]}"
        newest = max(p.stat().st_mtime for p in directory.glob("rows*.jsonl"))
        ended = datetime.datetime.fromtimestamp(newest, datetime.timezone.utc).date().isoformat()
        entry = {"batch": directory.name, "began": began, "ended": max(began, ended), "commit": commit[:9], **summary(games)}
        # A batch is one point of the timeline; only a window is drawn as a
        # distribution, so only a window keeps its spans.
        del entry["spans"]
        out.append(entry)
        played.append((entry, games))
    windows = {}
    if played:
        newest = max(entry["ended"] for entry, _ in played)
        since = (datetime.date.fromisoformat(newest) - datetime.timedelta(days=6)).isoformat()
        latest = max(played, key=lambda pair: (pair[0]["ended"], pair[0]["batch"]))[0]["batch"]
        kept = {
            "latest": [pair for pair in played if pair[0]["batch"] == latest],
            "7": [pair for pair in played if pair[0]["ended"] >= since],
            "all": played,
        }
        for key in WINDOWS:
            games = [game for _, batch in kept[key] for game in batch]
            fastest = min(kept[key], key=lambda pair: min(pair[1]))[0]["batch"]
            windows[key] = {
                "from": min(entry["began"] for entry, _ in kept[key]),
                "to": max(entry["ended"] for entry, _ in kept[key]),
                "batches": [entry["batch"] for entry, _ in kept[key]],
                **summary(games),
            }
            windows[key]["fastest"]["batch"] = fastest
    return {
        "about": "How long each self-play game took, in wall-clock seconds: the 1st, 10th, 50th, 90th and 99th percentile "
        "of every finished batch and of three windows, a game per seed. `spans` counts a window's games in spans of time "
        "twenty to a tenfold (span i is 10^(i/20) s up to 10^((i+1)/20) s). "
        "Rebuilt by `beta/performance_page.py --self-play`; read by the Performance page.",
        "setup": "6 players, Continents, Online speed, 250 turns, every seat a CIVVIS AI agent; seven games at a time on one machine",
        "taken": datetime.datetime.now(datetime.timezone.utc).date().isoformat(),
        "percentiles": PERCENTILES,
        "spans_per_tenfold": SPANS_PER_TENFOLD,
        "windows": windows,
        "batches": out,
    }


def written(data: dict) -> str:
    """The snapshot as its file holds it: a line per window and per batch, so
    a refresh's diff is the windows and the batches it added."""
    head = {key: value for key, value in data.items() if key not in ("windows", "batches")}
    flat = lambda value: json.dumps(value, separators=(",", ":"))
    windows = [f'  {json.dumps(key)}: {flat(value)}' for key, value in data["windows"].items()]
    batches = [flat(batch) for batch in data["batches"]]
    return (
        json.dumps(head, indent=1)[:-2]
        + ',\n "windows": {\n' + ",\n".join(windows) + "\n },"
        + '\n "batches": [\n' + ",\n".join(batches) + "\n ]\n}\n"
    )


def videos(listed: object) -> list[dict]:
    """The record videos, fastest first (the newest first among equals): each
    its YouTube watch page, its id, the time it shows a game played in, as
    seconds and as its title words it, the day it was posted, and its title.
    An entry that is not a YouTube link with a time is left out, said on
    stderr."""
    out = []
    for entry in listed if isinstance(listed, list) else []:
        found = VIDEO.fullmatch(str(entry.get("video") or "")) if isinstance(entry, dict) else None
        seconds = entry.get("seconds") if isinstance(entry, dict) else None
        if not found or isinstance(seconds, bool) or not isinstance(seconds, (int, float)) or not seconds > 0:
            print(f"performance_page: a record video is not a YouTube link with a time; left out ({entry!r})", file=sys.stderr)
            continue
        out.append(
            {
                "id": found[1],
                "video": f"https://www.youtube.com/watch?v={found[1]}",
                "seconds": seconds,
                "time": str(entry.get("time") or f"{seconds:g} seconds"),
                "posted": str(entry.get("posted") or ""),
                "title": str(entry.get("title") or ""),
            }
        )
    return sorted(out, key=lambda v: (v["seconds"], [-ord(c) for c in v["posted"]]))


def path(source: pathlib.Path) -> dict | None:
    """The path through one game, for the page: `readings`, oldest first,
    and the newest reading's steps by kind. `None` when the revision has no
    ledger, or one that cannot be read.

    A reading has the day it was `taken`, the `build` it was taken on (ten
    characters of the commit), the `games` it is the mean of, their `turns`,
    the `steps` of the path and the `bytes` it takes written down, `kept`,
    the steps there would be with each unit's walk as one step, and `saved`,
    the steps that would save (`kept + saved` is `steps`). Where the reading
    has them it has the `instructions` the game took and the
    `replay_instructions` the same path took with no agent deciding, with
    `replay_share`, the second over the first.

    `kinds` are the newest reading's steps by kind, most first, the
    `PATH_KINDS` most common by name and the rest as one row (`rest: true`,
    with how many `kinds` it stands for). A one-hex move's row is split: the
    steps that stay (`kept`) and the ones a walk as one step would save
    (`saved`).
    """
    try:
        book = json.loads((source / PATH).read_text(encoding="utf-8"))
        readings = []
        for taken in sorted(book["readings"], key=lambda reading: reading["taken"]):
            steps, saved = int(taken["steps"]), int(taken["hops"]) - int(taken["walks"])
            if steps <= 0 or saved < 0 or saved > steps:
                raise ValueError("a reading whose steps do not add up")
            row = {
                "taken": str(taken["taken"])[:10],
                "build": str(taken.get("build") or "")[:10],
                "games": int(taken["games"]),
                "turns": float(taken["turns"]),
                "steps": steps,
                "bytes": int(taken["bytes"]),
                "kept": steps - saved,
                "saved": saved,
            }
            if taken.get("instructions") and taken.get("replay_instructions"):
                row.update(
                    instructions=int(taken["instructions"]),
                    replay_instructions=int(taken["replay_instructions"]),
                    replay_share=round(taken["replay_instructions"] / taken["instructions"], 4),
                )
            readings.append(row)
        if not readings:
            return None
        newest = sorted(book["readings"], key=lambda reading: reading["taken"])[-1]
        counted = sorted(newest["kinds"].items(), key=lambda pair: -pair[1])
        kinds = []
        for kind, count in counted[:PATH_KINDS]:
            row = {"kind": str(kind), "steps": int(count)}
            if kind == "move":
                saved = readings[-1]["saved"]
                row.update(kept=int(count) - saved, saved=saved)
            kinds.append(row)
        rest = counted[PATH_KINDS:]
        if rest:
            kinds.append({"rest": True, "kinds": len(rest), "steps": int(sum(count for _, count in rest))})
        return {"readings": readings, "kinds": kinds, "setup": str(newest.get("setup") or "")}
    except (OSError, ValueError, KeyError, TypeError, AttributeError):
        return None


def performance(source: pathlib.Path) -> dict | None:
    """The record, or `None` when neither the times, nor a video, nor the
    speedup, nor the computer control games can be read."""
    out: dict = {"percentiles": PERCENTILES, "spans_per_tenfold": SPANS_PER_TENFOLD}
    # The computer control games are the run ledger's, read as it stands now.
    # A clone that never fetched the `ledger` branch has none, and anything
    # odd in the ledger costs the page this one part.
    try:
        out["control"] = control_times.control(source)
    except Exception as error:  # noqa: BLE001 - the ledger is written by every game, the site must still publish
        print(f"performance_page: the computer control games could not be timed ({error!r})", file=sys.stderr)
        out["control"] = None
    if out["control"] is None:
        print("performance_page: no computer control game times", file=sys.stderr)
    out["speedup"] = speedup_timeline.worked(source)
    if out["speedup"] is None:
        print("performance_page: no speedup readings", file=sys.stderr)
    out["history"] = speedup_timeline.history(source)
    out["path"] = path(source)
    if out["path"] is None:
        print("performance_page: no path readings", file=sys.stderr)
    try:
        taken = json.loads((source / SELF_PLAY).read_text(encoding="utf-8"))
        windows = {key: taken["windows"][key] for key in WINDOWS if taken["windows"].get(key)}
        if not windows.get("all") or not taken["batches"]:
            raise ValueError("no finished batch")
        if taken.get("percentiles") != PERCENTILES or taken.get("spans_per_tenfold") != SPANS_PER_TENFOLD:
            raise ValueError("the snapshot was taken with other percentiles or spans; rebuild it")
        out.update(windows=windows, batches=taken["batches"], taken=str(taken.get("taken") or ""))
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"performance_page: no self-play times ({error})", file=sys.stderr)
        out.update(windows={}, batches=[], taken="")
    try:
        out["videos"] = videos(json.loads((source / VIDEOS).read_text(encoding="utf-8")).get("videos"))
    except (OSError, ValueError, AttributeError) as error:
        print(f"performance_page: no record videos ({error})", file=sys.stderr)
        out["videos"] = []
    return out if out["windows"] or out["videos"] or out["speedup"] or out["control"] else None


def fill(page: str, data: dict | None) -> str | None:
    """The page with its slot filled, or `None` when it has no slot."""
    # `<` is escaped so no name or sentence can close the script element.
    text = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003c")
    filled, count = SLOT.subn(lambda m: m[1] + text + m[3], page, count=1)
    return filled if count else None


def main(argv: list[str]) -> int:
    if len(argv) == 4 and argv[1] == "--self-play":
        data = snapshot(pathlib.Path(argv[2]))
        out = pathlib.Path(argv[3])
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(written(data), encoding="utf-8")
        games = sum(b["games"] for b in data["batches"])
        print(f"performance_page: {games} self-play games in {len(data['batches'])} finished batches into {out}", file=sys.stderr)
        return 0
    if len(argv) not in (2, 3):
        print(__doc__, file=sys.stderr)
        return 2
    data = performance(pathlib.Path(argv[1]))
    if len(argv) == 2:
        json.dump(data, sys.stdout, ensure_ascii=False, indent=2)
        print()
        return 0
    page = pathlib.Path(argv[2])
    filled = fill(page.read_text(encoding="utf-8"), data)
    if filled is None:
        print(f"performance_page: {page} has no performance slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(filled, encoding="utf-8")
    if data is None:
        print(f"performance_page: no figures to write into {page}", file=sys.stderr)
    else:
        games = data["windows"].get("all", {}).get("games", 0)
        speedup = ((data.get("speedup") or {}).get("answer") or {}).get("all") or "no"
        control = ((data.get("control") or {}).get("all") or {}).get("games", 0)
        print(
            f"performance_page: wrote {control} computer control games' times, {games} self-play games' times in"
            f" {len(data['batches'])} batches, {len(data['videos'])} videos and {speedup} speedup into {page}",
            file=sys.stderr,
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
