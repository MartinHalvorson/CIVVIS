#!/usr/bin/env python3
"""The self-play benchmark hour: what the self-play tournament on this
machine finishes in one hour, taken every six hours.

    self_play_hour.py run --state-dir DIR [--minutes 60] [--record]
    self_play_hour.py publish --state-dir DIR     send readings the ledger lacks
    self_play_hour.py show                        the readings on the ledger
    self_play_hour.py install --state-dir DIR [--hours 2,8,14,20]
    self_play_hour.py uninstall

Operator, 2026-11-24: "we should be benchmarking civvis self-play tournament
games as well. maybe every 6 hrs or so we can do 1 hr of civvis self-play
games on this machine", the machine that plays the verification games and
the self-play tournament both.

**What an hour is.** `run` watches the tournament the continuous batch
scheduler plays (`tools/continuous_batch_scheduler.py`, its `--state-dir`)
for `--minutes` and reads what it finished in that time. Nothing is started
or stopped: the games are the tournament's own, six CIVVIS agents on a
Continents map at Online speed to turn 250, and they count for the
tournament as every game of it does.

**How the hour's games are told apart.** A row carries no finish time, but
the batch's rows file (`batches/<batch>/rows-continuous.jsonl`) is
append-only and a finished game's six rows are written to it in one piece,
in finishing order. So the games of the hour are the rows between the file's
length when the hour began and its length when it ended. A batch that was
cut during the hour leaves the rest of its file and the start of the next
one's, and both are read. A game is a seed, and its first row is the game.

**The reading.** One JSON object:

- `taken` (the moment the hour began, UTC), `minutes` (how long it ran by
  the wall clock, which is longer than asked if the machine slept or the
  hour ran on, below) and `asked` (the minutes it was asked to watch);
- `games` and `games_an_hour`;
- `p`: the wall-clock seconds of a game at the 10th, 50th and 90th
  percentile, nearest rank. A game's seconds include any time its worker was
  held by the scheduler's governor, so on a busy machine they are long;
- `cpu_a_turn` and `cpu_a_game`: the middle game's processor seconds for one
  turn and for the whole game (`cpu_secs`, the game thread's own clock),
  which the machine's load does not stretch, and `turns`, its length;
- the conditions, each a mean over samples taken through the hour:
  `running` (workers running of `workers`), `cores` (the cores they were
  given), `host_busy` (percent of the whole machine in use), `load` (the
  one-minute load), `civ6` (the share of the hour Civilization VI was
  running) and `battery` (the share on battery);
- `reserved`: whether the hour was set aside for self-play. `false` for an
  hour taken beside everything else the machine was doing;
- `build` and `batch` (the tournament's source commit and batch), `host`,
  `machine` and `machine_cores`.

**An hour the tournament was held through.** The scheduler's governor stops
the tournament's workers while the machine is busy, and it can stop all of
them for an hour: the scheduled hour of 2026-11-24 20:00Z finished one game,
with no worker running at any sample and a load of 17. One game says nothing
of how fast a game is played. So with `--min-games N` an hour that ends with
fewer than N finished games runs on until N have finished, and no longer
than `--max-minutes`, which keeps a reading from running into the next one.
`minutes` is then the whole time watched and `games_an_hour` the rate over
it. `install` schedules the hour with both (`LEAST_GAMES`,
`LONGEST_MINUTES`); `run` by hand watches exactly its minutes unless told.

**Where it goes.** Every reading is a line of `self-play-hours.jsonl` in the
state directory, the journal. `--record` (and `publish`) add the journal's
lines to `benchmarks/self_play_hours.jsonl` on the `ledger` branch, oldest
first, with the plumbing `tools/civ6_ladder.py` publishes a run with: a
commit on the branch's tip built in a scratch index, pushed as a fast
forward, and rebuilt on the new tip if a finished game got there first. The
CIVVIS Performance page reads that file when the site is published.

`install` schedules `run --record` with launchd at the given local hours,
under the machine's canonical fleet name when `--host` gives it
(`docs/MACHINES.md`; without one a reading takes git's `civvis.machine`).
`docs/SELF_PLAY_HOUR.md` is the write-up.
"""

from __future__ import annotations

import argparse
import datetime
import json
import math
import os
import platform
import plistlib
import subprocess
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_ladder  # noqa: E402

REPO = Path(__file__).resolve().parent.parent

#: The rows file of a batch, in its directory under `<state>/batches/`.
ROWS = "rows-continuous.jsonl"

#: The journal of readings, in the state directory.
JOURNAL = "self-play-hours.jsonl"

#: The readings on the `ledger` branch, a line each, oldest first.
LEDGER_PATH = "benchmarks/self_play_hours.jsonl"

#: The percentiles of a game's wall seconds a reading carries.
PERCENTILES = [10, 50, 90]

#: The launchd job `install` writes.
LABEL = "com.civvis.self-play-hour"

#: The local hours `install` schedules by default: six apart.
HOURS = [2, 8, 14, 20]

#: The games a scheduled hour waits for past its minutes, and how long it
#: may run in all: under the six hours to the next one.
LEAST_GAMES = 20
LONGEST_MINUTES = 240.0

#: Civilization VI's processes, by the exact names the scheduler's governor
#: looks for.
CIV6 = ("Civ6_Exe_Child", "Civ6_Exe")


def utc(moment: float) -> str:
    return datetime.datetime.fromtimestamp(moment, datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def percentile(ordered: list[float], p: int) -> float:
    """The nearest-rank percentile of a sorted list
    (`beta/performance_page.py` `percentile`)."""
    rank = max(1, math.ceil(p * len(ordered) / 100))
    return ordered[min(len(ordered), rank) - 1]


def middle(values: list[float]) -> float | None:
    return percentile(sorted(values), 50) if values else None


def status(state_dir: Path) -> dict:
    """The scheduler's `status.json`, or `{}` when it cannot be read (it is
    rewritten every ten seconds, so a read can land mid-write)."""
    try:
        read = json.loads((state_dir / "status.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    return read if isinstance(read, dict) else {}


def mark(state_dir: Path, now: float | None = None) -> dict:
    """Where the tournament stands: the moment, the batch being played and
    the length of its rows file. Raises when the scheduler names no batch,
    because then there is nothing to time."""
    said = status(state_dir)
    batch = said.get("batch")
    if not isinstance(batch, str) or not batch:
        raise RuntimeError(f"{state_dir / 'status.json'} names no batch; is the tournament running?")
    rows = state_dir / "batches" / batch / ROWS
    return {
        "at": time.time() if now is None else now,
        "batch": batch,
        "offset": rows.stat().st_size if rows.is_file() else 0,
        "commit": str(said.get("source_commit") or ""),
    }


def whole_lines(path: Path, begin: int, end: int | None) -> list[bytes]:
    """The complete lines of a file between two lengths of it. A span that
    opens inside a line starts at the next one, and a last line with no end
    is left for the next reading."""
    try:
        with path.open("rb") as rows:
            opened_inside = False
            if begin > 0:
                rows.seek(begin - 1)
                opened_inside = rows.read(1) != b"\n"
            rows.seek(begin)
            data = rows.read() if end is None else rows.read(max(0, end - begin))
    except OSError:
        return []
    lines = data.split(b"\n")
    lines.pop()  # what follows the last newline: nothing, or half a line
    return lines[1:] if opened_inside and lines else lines


def spans(state_dir: Path, start: dict, end: dict) -> list[tuple[Path, int, int | None]]:
    """The stretches of rows files written between two marks, in order: one
    stretch of one file, or, when the batch was cut in between, the rest of
    the first batch's file, any batch played whole in between, and the start
    of the last one's."""
    batches = state_dir / "batches"
    if start["batch"] == end["batch"]:
        return [(batches / start["batch"] / ROWS, start["offset"], end["offset"])]
    between = sorted(
        entry.name for entry in batches.iterdir() if entry.is_dir() and start["batch"] < entry.name < end["batch"]
    ) if batches.is_dir() else []
    return (
        [(batches / start["batch"] / ROWS, start["offset"], None)]
        + [(batches / name / ROWS, 0, None) for name in between]
        + [(batches / end["batch"] / ROWS, 0, end["offset"])]
    )


def finished(state_dir: Path, start: dict, end: dict) -> list[dict]:
    """The games whose rows were written between two marks, a game per seed,
    in finishing order: `secs` (wall), `cpu_secs` (or `None` on a build from
    before rows carried it), `turn` and `victory`."""
    games: dict[tuple[str, object], dict] = {}
    for path, begin, upto in spans(state_dir, start, end):
        for line in whole_lines(path, begin, upto):
            try:
                row = json.loads(line)
            except ValueError:
                continue
            if not isinstance(row, dict) or row.get("kind") != "game":
                continue
            key = (path.parent.name, row.get("seed"))
            secs, turn, cpu = row.get("secs"), row.get("turn"), row.get("cpu_secs")
            if key in games or not real(secs) or not isinstance(turn, int) or isinstance(turn, bool) or turn < 1:
                continue
            games[key] = {"secs": float(secs), "cpu_secs": float(cpu) if real(cpu) else None, "turn": turn, "victory": str(row.get("victory") or "")}
    return list(games.values())


def real(value: object) -> bool:
    """A reading that is a positive, finite number."""
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value) and value > 0


def civ6_running() -> bool:
    """Whether Civilization VI is up, by its exact process names."""
    for name in CIV6:
        try:
            if subprocess.run(["pgrep", "-x", name], capture_output=True, check=False).returncode == 0:
                return True
        except OSError:
            return False
    return False


def sample(state_dir: Path) -> dict:
    """The machine as it is now: the one-minute load, whether Civilization VI
    is running, and what the scheduler's governor last saw (the workers it
    has and is running, the cores they were given, the share of the machine
    in use, whether it is on battery). A part that cannot be read is left
    out."""
    seen: dict = {"civ6": civ6_running()}
    try:
        seen["load"] = os.getloadavg()[0]
    except OSError:
        pass
    governor = status(state_dir).get("governor")
    if isinstance(governor, dict):
        for name, key in (("workers", "workers"), ("running", "workers_running"), ("cores", "games_cores"), ("host_busy", "host_busy_percent")):
            value = governor.get(key)
            if isinstance(value, (int, float)) and not isinstance(value, bool):
                seen[name] = value
        if isinstance(governor.get("on_battery"), bool):
            seen["battery"] = governor["on_battery"]
    return seen


def mean(samples: list[dict], key: str, places: int = 1) -> float | None:
    values = [float(seen[key]) for seen in samples if key in seen]
    return round(sum(values) / len(values), places) if values else None


def machine() -> dict:
    """The machine the hour was taken on: its processor and its cores."""
    try:
        chip = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, check=False).stdout.decode().strip()
    except OSError:
        chip = ""
    return {"machine": chip or platform.processor() or platform.machine(), "machine_cores": os.cpu_count()}


def reading(start: dict, end: dict, games: list[dict], samples: list[dict], *, host: str = "", reserved: bool = False, asked: float | None = None) -> dict:
    """The hour as one line of the ledger; see the module's own account."""
    minutes = max(0.0, end["at"] - start["at"]) / 60
    secs = sorted(game["secs"] for game in games)
    timed = [game for game in games if game["cpu_secs"]]
    cpu_a_turn = middle([game["cpu_secs"] / game["turn"] for game in timed])
    cpu_a_game = middle([game["cpu_secs"] for game in timed])
    turns = middle([game["turn"] for game in games])
    workers = [seen["workers"] for seen in samples if "workers" in seen]
    return {
        "taken": utc(start["at"]),
        "minutes": round(minutes, 1),
        **({"asked": round(asked, 1)} if asked is not None else {}),
        "games": len(games),
        "games_an_hour": round(len(games) * 60 / minutes, 1) if minutes > 0 else None,
        "p": [round(percentile(secs, p), 1) for p in PERCENTILES] if secs else None,
        "cpu_a_turn": round(cpu_a_turn, 4) if cpu_a_turn else None,
        "cpu_a_game": round(cpu_a_game, 1) if cpu_a_game else None,
        "turns": int(turns) if turns else None,
        "workers": int(max(workers)) if workers else None,
        "running": mean(samples, "running"),
        "cores": mean(samples, "cores"),
        "host_busy": mean(samples, "host_busy"),
        "load": mean(samples, "load"),
        "civ6": mean(samples, "civ6", 2),
        "battery": mean(samples, "battery", 2),
        "reserved": bool(reserved),
        "build": (end.get("commit") or start.get("commit") or "")[:9],
        "batch": end["batch"],
        "host": host,
        **machine(),
    }


def watch(
    state_dir: Path, minutes: float, every: float, *, least_games: int = 0, longest: float | None = None, clock=time.time, sleep=time.sleep
) -> tuple[dict, dict, list[dict]]:
    """Watch the tournament for `minutes`: the mark as it begins, the mark
    as it ends, and a sample every `every` seconds in between. The end is a
    moment of the wall clock, so a machine that slept through part of the
    hour ends as soon as it wakes, and the reading says how long it was.

    With `least_games`, an hour that ends with fewer finished games runs on,
    sampled as before, until that many have finished or `longest` minutes
    have passed since it began (the module's account says why)."""
    start = mark(state_dir, clock())
    until = start["at"] + minutes * 60
    last = start["at"] + max(minutes, longest or minutes) * 60
    samples = [sample(state_dir)]
    while True:
        now = clock()
        if now < until:
            left = until - now
        elif now >= last or len(finished(state_dir, start, mark(state_dir, now))) >= least_games:
            break
        else:
            left = last - now
        sleep(min(every, left))
        samples.append(sample(state_dir))
    return start, mark(state_dir, clock()), samples


def journal(state_dir: Path) -> list[dict]:
    """The readings taken on this machine, oldest first."""
    try:
        lines = (state_dir / JOURNAL).read_text(encoding="utf-8").splitlines()
    except OSError:
        return []
    return parse(lines)


def parse(lines: list[str]) -> list[dict]:
    """Readings from lines of JSON, oldest first; a line that is not a
    reading is left out."""
    out = []
    for line in lines:
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if isinstance(row, dict) and isinstance(row.get("taken"), str):
            out.append(row)
    return sorted(out, key=key)


def key(row: dict) -> tuple[str, str]:
    """What makes a reading the same reading: when and where it was taken."""
    return (str(row.get("taken")), str(row.get("host") or ""))


def written(rows: list[dict]) -> str:
    """The ledger file for these readings: oldest first, a compact line
    each, so the same readings are always the same bytes."""
    return "".join(json.dumps(row, sort_keys=True, separators=(",", ":")) + "\n" for row in sorted(rows, key=key))


def on_ledger(repo: Path, tip: str | None, env: dict | None = None) -> list[dict]:
    """The readings the branch holds at `tip`; none before its first."""
    if tip is None or not civ6_ladder._git(repo, "rev-parse", "-q", "--verify", f"{tip}:{LEDGER_PATH}", env=env, check=False):
        return []
    return parse(civ6_ladder._git(repo, "show", f"{tip}:{LEDGER_PATH}", env=env).splitlines())


def publish(rows: list[dict], *, repo: Path | None = None, remote: str = "origin", env: dict | None = None, attempts: int = 3) -> str:
    """Add the readings the `ledger` branch lacks and push it. "published",
    or "already" when it has them all. A reading on the branch is never
    rewritten: the branch's line stands and ours is dropped.

    The branch moves with every finished verification game, so a push can
    lose the race: then the tip is read again and the commit rebuilt on it,
    up to `attempts` times. Never forced. A machine whose remote has no
    `ledger` branch does not make one."""
    repo = Path(repo or REPO)
    env = civ6_ladder._ledger_identity(repo, dict(env or {}))
    last: Exception | None = None
    for _ in range(max(1, attempts)):
        tip = civ6_ladder.ledger_tip(repo, remote, env=env)
        if tip is None:
            raise RuntimeError(f"{remote} has no `{civ6_ladder.LEDGER_BRANCH}` branch; the readings stay in the journal")
        held = on_ledger(repo, tip, env=env)
        have = {key(row) for row in held}
        fresh = []
        for row in sorted(rows, key=key):
            if key(row) not in have:
                have.add(key(row))
                fresh.append(row)
        if not fresh:
            return "already"
        text = written(held + fresh)
        with tempfile.TemporaryDirectory() as scratch:
            commit = civ6_ladder._ledger_commit(
                repo, tip, [(LEDGER_PATH, text.encode())],
                f"ledger: self-play hour {fresh[-1].get('taken')} ({fresh[-1].get('games')} games)", Path(scratch), env,
            )
        try:
            civ6_ladder._git(repo, "push", "-q", remote, f"{commit}:refs/heads/{civ6_ladder.LEDGER_BRANCH}", env=env)
        except RuntimeError as error:  # the tip moved under us: read it again
            last = error
            continue
        civ6_ladder._git(repo, "update-ref", f"refs/remotes/{remote}/{civ6_ladder.LEDGER_BRANCH}", commit, env=env)
        return "published"
    raise RuntimeError(f"the readings did not reach the ledger after {attempts} attempts: {last}")


def default_host(repo: Path) -> str:
    """The fleet's name for this machine, as the scheduler takes it."""
    try:
        return civ6_ladder._git(repo, "config", "civvis.machine", check=False)
    except OSError:
        return ""


def run(args: argparse.Namespace) -> int:
    state_dir = Path(args.state_dir).expanduser()
    host = args.host or default_host(Path(args.repo))
    start, end, samples = watch(state_dir, args.minutes, args.sample_seconds, least_games=args.min_games, longest=args.max_minutes)
    row = reading(start, end, finished(state_dir, start, end), samples, host=host, asked=args.minutes)
    with (state_dir / JOURNAL).open("a", encoding="utf-8") as out:
        out.write(json.dumps(row, sort_keys=True, separators=(",", ":")) + "\n")
    print(json.dumps(row, sort_keys=True), flush=True)
    if not args.record:
        return 0
    return send(state_dir, Path(args.repo))


def send(state_dir: Path, repo: Path) -> int:
    """Publish the journal. A failure is said and leaves the journal as it
    is, for the next hour's publish to send."""
    try:
        print(f"ledger: {publish(journal(state_dir), repo=repo)}", flush=True)
    except Exception as error:  # noqa: BLE001 - the reading is in the journal; the next publish sends it
        print(f"ledger: not published ({error}); `self_play_hour.py publish` sends it", file=sys.stderr, flush=True)
        return 1
    return 0


def show(repo: Path) -> int:
    env = civ6_ladder._ledger_identity(repo, {})
    rows = on_ledger(repo, civ6_ladder.ledger_tip(repo, env=env), env=env)
    print(f"{'taken':<21}{'min':>5}{'games':>7}{'/hour':>8}{'p50 s':>8}{'cpu/turn':>10}{'running':>9}{'load':>6}{'civ6':>6}  build")
    for row in rows:
        p = row.get("p") or [None, None, None]
        cell = lambda value, form: format(value, form) if isinstance(value, (int, float)) else "-"
        print(
            f"{str(row.get('taken')):<21}{cell(row.get('minutes'), '.0f'):>5}{cell(row.get('games'), 'd'):>7}{cell(row.get('games_an_hour'), '.0f'):>8}"
            f"{cell(p[1], '.0f'):>8}{cell(row.get('cpu_a_turn'), '.3f'):>10}{cell(row.get('running'), '.1f'):>9}{cell(row.get('load'), '.1f'):>6}"
            f"{cell(row.get('civ6'), '.2f'):>6}  {row.get('build') or '-'}{'  reserved' if row.get('reserved') else ''}"
        )
    return 0


def agent(
    state_dir: Path, hours: list[int], minutes: float, repo: Path, host: str = "", *, least_games: int = LEAST_GAMES, longest: float = LONGEST_MINUTES
) -> dict:
    """The launchd job: `run --record` at each of `hours`, local time, under
    the machine's fleet name when one is given (`docs/MACHINES.md`). An hour
    that ends short of `least_games` runs on, to `longest` minutes at most."""
    log = str(Path.home() / "Library/Logs/civvis-self-play-hour.log")
    return {
        "Label": LABEL,
        "ProgramArguments": [
            "/usr/bin/python3", str(repo / "tools" / "self_play_hour.py"), "run",
            "--state-dir", str(state_dir), "--minutes", f"{minutes:g}", "--min-games", str(least_games), "--max-minutes", f"{max(minutes, longest):g}",
            "--record", "--repo", str(repo),
            *(["--host", host] if host else []),
        ],
        "StartCalendarInterval": [{"Hour": hour, "Minute": 0} for hour in sorted(set(hours))],
        "EnvironmentVariables": {"HOME": str(Path.home()), "PATH": "/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin"},
        "ProcessType": "Background",
        "LowPriorityIO": True,
        "StandardOutPath": log,
        "StandardErrorPath": log,
    }


def install(args: argparse.Namespace) -> int:
    state_dir = Path(args.state_dir).expanduser().resolve()
    hours = [int(hour) for hour in str(args.hours).split(",") if hour.strip()]
    if not hours or any(not 0 <= hour <= 23 for hour in hours):
        print("--hours is a list of local hours, 0 to 23", file=sys.stderr)
        return 2
    mark(state_dir)  # refuse a directory that is not a running tournament's
    target = Path.home() / "Library/LaunchAgents" / f"{LABEL}.plist"
    target.parent.mkdir(parents=True, exist_ok=True)
    domain = f"gui/{os.getuid()}"
    subprocess.run(["launchctl", "bootout", f"{domain}/{LABEL}"], capture_output=True, check=False)
    with target.open("wb") as out:
        plistlib.dump(agent(state_dir, hours, args.minutes, Path(args.repo).resolve(), args.host, least_games=args.min_games, longest=args.max_minutes), out)
    loaded = subprocess.run(["launchctl", "bootstrap", domain, str(target)], capture_output=True, check=False)
    if loaded.returncode != 0:
        print(f"launchctl bootstrap failed: {loaded.stderr.decode(errors='replace').strip()}", file=sys.stderr)
        return 1
    print(
        f"{LABEL}: {args.minutes:g} minutes at {', '.join(f'{hour:02d}:00' for hour in sorted(set(hours)))} local time,"
        f" on to {args.min_games} games within {max(args.minutes, args.max_minutes):g} minutes ({target})"
    )
    return 0


def uninstall() -> int:
    subprocess.run(["launchctl", "bootout", f"gui/{os.getuid()}/{LABEL}"], capture_output=True, check=False)
    target = Path.home() / "Library/LaunchAgents" / f"{LABEL}.plist"
    if target.is_file():
        target.unlink()
    print(f"{LABEL}: removed")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)

    def common(command: argparse.ArgumentParser, state: bool = True) -> None:
        if state:
            command.add_argument("--state-dir", required=True, help="the continuous batch scheduler's state directory")
        command.add_argument("--repo", default=str(REPO), help="the checkout whose `origin` holds the ledger branch")

    took = commands.add_parser("run", help="watch the tournament for an hour and write the reading")
    common(took)
    took.add_argument("--minutes", type=float, default=60.0)
    took.add_argument("--min-games", type=int, default=0, help="run on past the minutes until this many games have finished")
    took.add_argument("--max-minutes", type=float, default=LONGEST_MINUTES, help="the longest an hour may run on to")
    took.add_argument("--sample-seconds", type=float, default=30.0)
    took.add_argument("--record", action="store_true", help="publish the journal to the ledger branch")
    took.add_argument("--host", default="", help="this machine's fleet name (default: git config civvis.machine)")
    common(commands.add_parser("publish", help="send the journal's readings the ledger lacks"))
    common(commands.add_parser("show", help="print the readings on the ledger"), state=False)
    placed = commands.add_parser("install", help="schedule `run --record` with launchd")
    common(placed)
    placed.add_argument("--hours", default=",".join(str(hour) for hour in HOURS), help="local hours, comma separated")
    placed.add_argument("--minutes", type=float, default=60.0)
    placed.add_argument("--min-games", type=int, default=LEAST_GAMES, help="the games an hour waits for past its minutes")
    placed.add_argument("--max-minutes", type=float, default=LONGEST_MINUTES, help="the longest an hour may run on to")
    placed.add_argument("--host", default="", help="this machine's fleet name, for every reading the job takes")
    commands.add_parser("uninstall", help="remove the launchd job")
    args = parser.parse_args(argv)
    if args.command == "run":
        if not args.minutes > 0 or not args.sample_seconds > 0:
            parser.error("--minutes and --sample-seconds are positive")
        if args.min_games < 0 or not args.max_minutes > 0:
            parser.error("--min-games is not negative and --max-minutes is positive")
        return run(args)
    if args.command == "publish":
        return send(Path(args.state_dir).expanduser(), Path(args.repo))
    if args.command == "show":
        return show(Path(args.repo))
    if args.command == "install":
        if not args.minutes > 0 or args.min_games < 0 or not args.max_minutes > 0:
            parser.error("--minutes and --max-minutes are positive and --min-games is not negative")
        return install(args)
    return uninstall()


if __name__ == "__main__":
    sys.exit(main())
