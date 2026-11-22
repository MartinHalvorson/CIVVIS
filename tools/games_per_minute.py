#!/usr/bin/env python3
"""Games per minute: how many self-play games one machine finishes in a minute.

The benchmark (operator 2026-11-21: "max games per minute on an m5 max ...
finishing as many self-play civvis games as possible across pretty much all or
nearly all cores in 1 minute"). The write-up is `docs/GAMES_PER_MINUTE.md`;
the recorded runs are `data/performance/games_per_minute.json`.

    tools/games_per_minute.py --binary target/release/gene_screen
    tools/games_per_minute.py --binary B --quiesce --record   # a run for the record
    tools/games_per_minute.py --release DIR/quiesce.json      # undo a quiesce by hand

## The test

One worker process per core, each playing the standard self-play game (the
gene screen's bare defaults: six CIVVIS seats, 74x46 Continents, nine
city-states, Online speed, 250 turns, all six victories) one after another on
one thread, from a fixed block of seeds. A game counts in the minute its rows
reach the disk, which is the moment it is finished. Three figures come out:

- `first_minute`: games finished in the first 60 s after a cold start, every
  worker beginning a new game at second 0. The literal stopwatch minute. It is
  coarse: no game is finished before the shortest one ends, and a game cut off
  by the bell counts for nothing.
- `games_per_minute`: the mean count of the measured minutes that follow the
  first. By then the workers are out of step with one another, so a minute
  holds games begun before it and leaves others unfinished after it, and the
  two cancel. This is the throughput, and the headline.
- `best_minute`: the best single measured minute.

The seeds are fixed, so the same binary plays the same games in the same
order on every run. A different binary may play different games from the same
seeds, and a build whose games end sooner scores higher for that reason as
well as for running faster: the figure is games, not turns. `turns_per_second`
is printed beside it for that.

## A quiet machine

The figure is a maximum only if the cores are free. `--quiesce` makes them
free for the length of the run and puts everything back afterwards:

- it holds the live Civilization VI game through the operator's own Pause
  (`civ6_control/live_pause.py`), and waits until the game reports it is held,
  so no rival turn is starved into a stall;
- it sets the CPU-headroom guard's off switch (`~/.civvis-headroom-off`),
  because that guard parks any batch process once the host is under 20 % idle,
  which is the state this test creates on purpose;
- it stops (SIGSTOP) running compilers and other simulator batches, and
  continues exactly the ones it stopped.

What it did is written to `quiesce.json` in the run directory before the first
worker starts, and `--release` undoes it from that file if the run was killed.
Without `--quiesce` nothing outside the run is touched, and the report shows
how much CPU the workers actually got.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import signal
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

TOOLS = Path(__file__).resolve().parent
REPO = TOOLS.parent
LEDGER = REPO / "data" / "performance" / "games_per_minute.json"

MINUTE = 60.0
#: Far apart, so no two workers ever share a seed however long the run.
SEED_BASE = 20261122000000
SEED_STRIDE = 100_000
#: More games than a worker can finish; the run ends on the clock.
GAMES_PER_WORKER = 10_000
POLL_SECONDS = 0.02

HEADROOM_OFF = Path.home() / ".civvis-headroom-off"
#: Other CPU-bound batch work that a record run stops for its length.
TOOLCHAIN = ("rustc", "clippy-driver", "rust-lld", "ld", "cc", "clang")
SIMULATORS = ("gene_screen", "victory_eval", "strategy_elo", "civvis")
#: A `civvis` process showing a game to somebody is not batch work.
SHOWING = ("--serve", "--mirror", " play ")


# ── the count ───────────────────────────────────────────────────────────────


def worker_seeds(workers: int, base: int = SEED_BASE, stride: int = SEED_STRIDE) -> list[int]:
    """Each worker's first seed."""
    return [base + index * stride for index in range(workers)]


def count_between(times: Iterable[float], start: float, end: float) -> int:
    """Finishes in (start, end]: a game on the bell belongs to the minute it ends."""
    return sum(1 for at in times if start < at <= end)


def summarise(finishes: list[dict[str, Any]], minutes: int, workers: int) -> dict[str, Any]:
    """The three figures from each finished game's time since the start."""
    times = [game["at"] for game in finishes]
    measured = [
        count_between(times, MINUTE * minute, MINUTE * (minute + 1))
        for minute in range(1, minutes + 1)
    ]
    counted = [game for game in finishes if MINUTE < game["at"] <= MINUTE * (minutes + 1)]
    turns = sum(game.get("turn") or 0 for game in counted)
    worker_seconds = sorted(game["secs"] for game in counted if game.get("secs"))
    return {
        "first_minute": count_between(times, 0.0, MINUTE),
        "minutes": measured,
        "games_per_minute": round(sum(measured) / minutes, 2) if minutes else None,
        "best_minute": max(measured) if measured else None,
        "games": len(counted),
        "turns_per_second": round(turns / (MINUTE * minutes), 1) if minutes else None,
        "mean_turns": round(turns / len(counted), 1) if counted else None,
        "median_game_seconds": (
            round(worker_seconds[len(worker_seconds) // 2], 1) if worker_seconds else None
        ),
        "games_per_minute_per_worker": (
            round(sum(measured) / minutes / workers, 3) if minutes and workers else None
        ),
    }


class Rows:
    """One worker's rows file, read as it grows; a new seed is a finished game."""

    def __init__(self, path: Path) -> None:
        self.path = path
        self.offset = 0
        self.tail = b""
        self.seen: set[int] = set()
        self.header: dict[str, Any] | None = None

    def finished(self) -> list[dict[str, Any]]:
        try:
            with self.path.open("rb") as handle:
                handle.seek(self.offset)
                chunk = handle.read()
        except FileNotFoundError:
            return []
        if not chunk:
            return []
        self.offset += len(chunk)
        lines = (self.tail + chunk).split(b"\n")
        self.tail = lines.pop()  # a row still being written
        games = []
        for line in lines:
            if not line.strip():
                continue
            row = json.loads(line)
            if row.get("kind") == "header":
                self.header = row
            elif "seed" in row and row["seed"] not in self.seen:
                self.seen.add(row["seed"])
                games.append(
                    {key: row.get(key) for key in ("seed", "turn", "victory", "secs")}
                )
        return games


# ── the machine ─────────────────────────────────────────────────────────────


def run(*command: str) -> str:
    try:
        return subprocess.run(command, capture_output=True, text=True, timeout=20).stdout
    except (OSError, subprocess.SubprocessError):
        return ""


def machine() -> dict[str, Any]:
    def sysctl(name: str) -> str:
        return run("sysctl", "-n", name).strip()

    cores = os.cpu_count() or 1
    levels = [sysctl(f"hw.perflevel{level}.physicalcpu") for level in (0, 1)]
    return {
        "chip": sysctl("machdep.cpu.brand_string") or platform.processor(),
        "cores": cores,
        "core_levels": [int(count) for count in levels if count.isdigit()],
        "memory_gb": round(int(sysctl("hw.memsize") or 0) / 2**30),
        "os": f"{platform.system()} {platform.release()}",
        "power": "battery" if "Battery Power" in run("pmset", "-g", "batt") else "mains",
    }


def processes() -> list[dict[str, Any]]:
    """Every process: pid, state, %CPU and its program's name."""
    rows = []
    for line in run("ps", "-Ao", "pid=,stat=,pcpu=,comm=").splitlines():
        parts = line.split(None, 3)
        if len(parts) == 4 and parts[0].isdigit():
            rows.append(
                {
                    "pid": int(parts[0]),
                    "stat": parts[1],
                    "cpu": float(parts[2]),
                    "name": parts[3].rsplit("/", 1)[-1],
                }
            )
    return rows


def cpu_sample(own: set[int]) -> dict[str, Any]:
    """Who has the CPU right now: the workers, and everything else."""
    ours = 0.0
    others: dict[str, float] = {}
    for row in processes():
        if row["pid"] in own:
            ours += row["cpu"]
        elif row["cpu"] >= 5.0:
            others[row["name"]] = others.get(row["name"], 0.0) + row["cpu"]
    top = sorted(others.items(), key=lambda item: -item[1])[:4]
    return {"workers": round(ours), "others": round(sum(others.values())), "top_others": top}


# ── a quiet machine ─────────────────────────────────────────────────────────


def batch_work(own: set[int]) -> list[dict[str, Any]]:
    """Running compilers and simulator batches that are not this run's."""
    found = []
    for row in processes():
        if row["pid"] in own or row["pid"] == os.getpid() or row["stat"].startswith("T"):
            continue
        name = row["name"]
        simulator = name in SIMULATORS and not any(
            mark in run("ps", "-o", "args=", "-p", str(row["pid"])).strip() + " " for mark in SHOWING
        )
        if name in TOOLCHAIN or name.startswith("build-script-") or simulator:
            found.append({"pid": row["pid"], "name": name})
    return found


def live_pause():
    """The operator's Pause for the live game, or None where the tree has none."""
    sys.path.insert(0, str(TOOLS))
    try:
        from civ6_control import live_pause as module  # type: ignore
    except ImportError:
        return None
    finally:
        sys.path.pop(0)
    return module


def live_game_up() -> bool:
    return bool(run("pgrep", "-x", "Civ6_Exe_Child").strip())


def quiesce(journal: Path, hold_timeout: float, log) -> dict[str, Any]:
    """Free the cores, writing each step to `journal` before taking the next."""
    done: dict[str, Any] = {"held_live_game": False, "headroom_off": False, "stopped": []}

    def save() -> None:
        journal.write_text(json.dumps(done, indent=1) + "\n")

    save()
    pause = live_pause()
    if live_game_up() and pause is not None:
        if pause.requested():
            log("the live game is already paused by somebody else; leaving that pause alone")
        else:
            done["held_live_game"] = True
            save()
            pause.set_requested(True, source="games_per_minute")
            log("asked the live Civ VI game to hold; waiting for it to report held")
            deadline = time.monotonic() + hold_timeout
            while not pause.holds_game():
                if time.monotonic() > deadline:
                    raise RuntimeError(
                        f"the live game did not hold within {hold_timeout:.0f} s; nothing was run"
                    )
                time.sleep(0.5)
            log("the live game is held")
    if not HEADROOM_OFF.exists():
        done["headroom_off"] = True
        save()
        HEADROOM_OFF.write_text("games_per_minute.py: off for one benchmark run\n")
        log(f"CPU-headroom guard off for the run ({HEADROOM_OFF})")
    for work in batch_work(set()):
        done["stopped"].append(work)
        save()
        try:
            os.kill(work["pid"], signal.SIGSTOP)
        except ProcessLookupError:
            pass
    if done["stopped"]:
        names = sorted({work["name"] for work in done["stopped"]})
        log(f"stopped {len(done['stopped'])} batch process(es): {', '.join(names)}")
    return done


def release(done: dict[str, Any], log) -> None:
    """Put back exactly what `quiesce` changed. Safe to run twice."""
    for work in done.get("stopped", []):
        try:
            os.kill(work["pid"], signal.SIGCONT)
        except ProcessLookupError:
            pass
    if done.get("stopped"):
        log(f"continued {len(done['stopped'])} batch process(es)")
    if done.get("headroom_off"):
        try:
            HEADROOM_OFF.unlink()
        except FileNotFoundError:
            pass
        log("CPU-headroom guard back on")
    if done.get("held_live_game"):
        pause = live_pause()
        if pause is not None:
            pause.set_requested(False)
            log("the live game is released")


# ── the run ─────────────────────────────────────────────────────────────────


def source_tree(binary: Path) -> Path | None:
    """`<tree>/target/<profile>/gene_screen` plays from `<tree>`."""
    for parent in binary.parents:
        if parent.name == "target" and (parent.parent / "Cargo.toml").exists():
            return parent.parent
    return None


def play(args: argparse.Namespace, run_dir: Path, log) -> dict[str, Any]:
    binary = Path(args.binary).resolve()
    cwd = Path(args.cwd).resolve() if args.cwd else source_tree(binary)
    seeds = worker_seeds(args.workers, args.seed_base)
    seconds = MINUTE * (1 + args.minutes)
    workers: list[subprocess.Popen] = []
    logs: list[Any] = []
    rows: list[Rows] = []
    finishes: list[dict[str, Any]] = []
    samples: list[dict[str, Any]] = []
    load_start = os.getloadavg()[0]
    start = time.monotonic()
    started_utc = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    try:
        for index, seed in enumerate(seeds):
            out = run_dir / f"worker-{index:02d}.jsonl"
            command = [
                str(binary), "--games", str(GAMES_PER_WORKER),
                "--target-games", str(GAMES_PER_WORKER), "--start-seed", str(seed),
                "--jobs", "1", "--out", str(out), "--quiet", *args.game_args,
            ]
            logs.append((run_dir / f"worker-{index:02d}.log").open("wb"))
            workers.append(
                subprocess.Popen(command, cwd=cwd, stdout=subprocess.DEVNULL, stderr=logs[-1])
            )
            rows.append(Rows(out))
        spawned = time.monotonic() - start
        own = {worker.pid for worker in workers}
        log(f"{len(workers)} workers started in {spawned * 1000:.0f} ms; {seconds:.0f} s on the clock")
        next_sample = 10.0
        next_note = MINUTE
        while True:
            now = time.monotonic() - start
            for index, reader in enumerate(rows):
                for game in reader.finished():
                    finishes.append({**game, "at": round(now, 3), "worker": index})
            if now >= next_note:
                minute = round(next_note / MINUTE)
                finished = count_between((g["at"] for g in finishes), next_note - MINUTE, next_note)
                log(f"minute {minute}: {finished} games finished")
                next_note += MINUTE
            if now >= seconds:
                break
            if now >= next_sample:
                samples.append(cpu_sample(own))
                next_sample += 10.0
            dead = [worker for worker in workers if worker.poll() is not None]
            if dead:
                raise RuntimeError(
                    f"{len(dead)} worker(s) exited early (code {dead[0].returncode}); "
                    f"see {run_dir}/worker-*.log"
                )
            time.sleep(POLL_SECONDS)
    finally:
        for worker in workers:
            if worker.poll() is None:
                worker.kill()
        for worker in workers:
            worker.wait()
        for handle in logs:
            handle.close()
    header = next((reader.header for reader in rows if reader.header), None) or {}
    result = summarise(finishes, args.minutes, args.workers)
    busy = [sample["workers"] for sample in samples]
    other = [sample["others"] for sample in samples]
    top: dict[str, float] = {}
    for sample in samples:
        for name, cpu in sample["top_others"]:
            top[name] = top.get(name, 0.0) + cpu / len(samples)
    result.update(
        {
            "taken": started_utc,
            "workers": args.workers,
            "machine": machine(),
            "build": {
                key: (header.get("build") or {}).get(key)
                for key in ("commit", "dirty", "binary_sha256")
            },
            "setup": {
                key: header.get(key)
                for key in ("players", "width", "height", "map", "speed", "turns", "city_states")
            },
            "game_args": args.game_args,
            "seed_base": args.seed_base,
            "conditions": {
                "quiesced": bool(args.quiesce),
                "load_before": round(load_start, 1),
                "workers_cpu_cores": round(sum(busy) / len(busy) / 100, 1) if busy else None,
                "others_cpu_cores": round(sum(other) / len(other) / 100, 1) if other else None,
                "top_others": [
                    [name, round(cpu / 100, 1)]
                    for name, cpu in sorted(top.items(), key=lambda item: -item[1])[:4]
                ],
            },
        }
    )
    (run_dir / "finishes.json").write_text(json.dumps(finishes, indent=0) + "\n")
    return result


def record(result: dict[str, Any], ledger: Path = LEDGER) -> None:
    book = json.loads(ledger.read_text()) if ledger.exists() else {
        "about": (
            "Games per minute: self-play games finished per minute with one worker "
            "per core, by `tools/games_per_minute.py --record`. The test is "
            "`docs/GAMES_PER_MINUTE.md`. Newest run last."
        ),
        "runs": [],
    }
    book["runs"].append(result)
    ledger.write_text(json.dumps(book, indent=1) + "\n")


def report(result: dict[str, Any]) -> str:
    box, where = result["machine"], result["conditions"]
    setup = result["setup"]
    lines = [
        f"games per minute: {result['games_per_minute']:g}"
        f"  (minutes {result['minutes']}, best {result['best_minute']},"
        f" first minute from a cold start {result['first_minute']})",
        f"  {result['workers']} workers on {box['chip']}, {box['cores']} cores, {box['power']} power"
        f" · {result['games_per_minute_per_worker']} games a minute per worker",
        f"  {result['games']} games, {result['mean_turns']} turns a game,"
        f" {result['turns_per_second']} turns a second,"
        f" median game {result['median_game_seconds']} s on its worker",
        f"  the workers had {where['workers_cpu_cores']} cores, everything else"
        f" {where['others_cpu_cores']} ({', '.join(f'{n} {c}' for n, c in where['top_others'])})"
        f" · load before {where['load_before']} · quiesced: {where['quiesced']}",
        f"  build {str(result['build'].get('commit'))[:10]}"
        f"{' (dirty)' if result['build'].get('dirty') else ''}"
        f" · {setup.get('players')}p {setup.get('width')}x{setup.get('height')}"
        f" {setup.get('map')} · {setup.get('speed')} · {setup.get('turns')} turns",
    ]
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--binary", help="a release `gene_screen`")
    parser.add_argument("--workers", type=int, default=os.cpu_count() or 1,
                        help="worker processes, one game at a time each (default: every core)")
    parser.add_argument("--minutes", type=int, default=2,
                        help="measured minutes after the first, cold one (default 2)")
    parser.add_argument("--seed-base", type=int, default=SEED_BASE)
    parser.add_argument("--cwd", help="where the workers run (default: the binary's source tree)")
    parser.add_argument("--out", help="run directory (default: a new temporary one)")
    parser.add_argument("--quiesce", action="store_true",
                        help="hold the live game, switch the headroom guard off and stop "
                             "other batch work for the run")
    parser.add_argument("--hold-timeout", type=float, default=180.0,
                        help="seconds to wait for the live game to hold")
    parser.add_argument("--record", action="store_true", help=f"append the run to {LEDGER.name}")
    parser.add_argument("--json", action="store_true", help="print the run as JSON")
    parser.add_argument("--release", metavar="JOURNAL", help="undo a quiesce from its journal")
    parser.add_argument("game_args", nargs="*",
                        help="after `--`: extra gene_screen arguments (a probe, not the standard game)")
    args = parser.parse_args(argv)

    def log(text: str) -> None:
        print(f"[{datetime.now(timezone.utc).strftime('%H:%M:%SZ')}] {text}", file=sys.stderr, flush=True)

    if args.release:
        release(json.loads(Path(args.release).read_text()), log)
        return 0
    if not args.binary or not Path(args.binary).is_file():
        parser.error("--binary must name a built gene_screen")
    if args.workers < 1 or args.minutes < 1:
        parser.error("--workers and --minutes are at least 1")
    if args.record and args.game_args:
        parser.error("a run with extra game arguments is a probe and is not recorded")

    run_dir = Path(args.out) if args.out else Path(tempfile.mkdtemp(prefix="games-per-minute-"))
    run_dir.mkdir(parents=True, exist_ok=True)
    log(f"run directory {run_dir}")

    def stop(signum, _frame):  # a killed run still releases what it took
        raise KeyboardInterrupt(f"signal {signum}")

    for signum in (signal.SIGTERM, signal.SIGHUP):
        signal.signal(signum, stop)

    done: dict[str, Any] = {}
    journal = run_dir / "quiesce.json"
    try:
        if args.quiesce:
            log(f"to undo by hand: {Path(__file__).name} --release {journal}")
            try:
                done = quiesce(journal, args.hold_timeout, log)
            except BaseException:
                done = json.loads(journal.read_text()) if journal.exists() else {}
                raise
        result = play(args, run_dir, log)
    finally:
        if done:
            release(done, log)
    (run_dir / "result.json").write_text(json.dumps(result, indent=1) + "\n")
    if args.record:
        record(result)
        log(f"recorded in {LEDGER}")
    print(json.dumps(result, indent=1) if args.json else report(result))
    return 0


if __name__ == "__main__":
    sys.exit(main())
