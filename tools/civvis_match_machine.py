#!/usr/bin/env python3
"""Run a terminal-bound, resource-capped CIVVIS match fleet.

The match machine keeps one visible game and a pool of fast headless games on
the same mutable league.  Every game is an eight-player, Standard-size,
Standard-speed Continents free-for-all under the stock rules.  CIVVIS itself
does the rating: the spectator server samples each civilization's top three
eligible players with 3:2:1 rank weights and atomically appends a completed
game to ``matches.csv`` and ``league.json``.

The operator fetches ``origin/main`` into a private detached worktree, drains
old games at a revision boundary, builds the new HEAD, and launches every new
game from that immutable binary.  It never edits or updates a development
checkout.  CPU, memory, disk and Apple-GPU use are sampled host-wide; game
process groups are stopped and resumed before the configured ceiling.  macOS
thermal pressure is an additional hard gate.

The process is intentionally foreground and watches the shell PID supplied by
``--watch-pid``.  Closing that terminal kills the fleet.  While it is alive,
its ``caffeinate`` child prevents idle/system sleep on AC power.
"""

from __future__ import annotations

import argparse
import csv
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import subprocess
import sys
import time
from typing import Any
from urllib.error import URLError
from urllib.request import Request, urlopen


PLAYERS = 8
WIDTH = 84
HEIGHT = 54
CITY_STATES = 12
DEFAULT_SPEED = "online"
SPEED_TURNS = {
    "online": 250,
    "quick": 330,
    "standard": 500,
    "epic": 750,
    "marathon": 1500,
}
MAP = "continents"
SHAPE = "flat"
POLES = "poles"
RESULT_HOLD_SECONDS = 10.0
SHED_ONE_MARGIN = 15.0
SHED_HEADLESS_MARGIN = 10.0
RESUME_MARGIN = 20.0
RESUME_COOLDOWN_SECONDS = 15.0
RESUME_STEP_SECONDS = 5.0


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def append_jsonl(path: Path, event: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(event, sort_keys=True) + "\n")


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.replace(temporary, path)


def command(
    *args: str,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    timeout: float | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
        check=False,
    )


def process_identity(pid: int) -> str | None:
    if pid <= 0:
        return None
    result = command("ps", "-o", "lstart=", "-p", str(pid))
    value = result.stdout.strip()
    return value if result.returncode == 0 and value else None


def process_is_same(pid: int, identity: str | None) -> bool:
    return identity is not None and process_identity(pid) == identity


def parse_top_cpu(text: str) -> float | None:
    matches = re.findall(r"CPU usage:.*?([0-9.]+)% idle", text)
    return max(0.0, min(100.0, 100.0 - float(matches[-1]))) if matches else None


def cpu_percent() -> float | None:
    if sys.platform == "darwin":
        # macOS top accepts only whole-second sample intervals.
        result = command("top", "-l", "2", "-n", "0", "-s", "1", timeout=4)
        return parse_top_cpu(result.stdout)
    try:
        first = Path("/proc/stat").read_text(encoding="utf-8").splitlines()[0].split()[1:]
        time.sleep(0.2)
        second = Path("/proc/stat").read_text(encoding="utf-8").splitlines()[0].split()[1:]
        a, b = [int(value) for value in first], [int(value) for value in second]
        total = sum(b) - sum(a)
        idle = (b[3] + b[4]) - (a[3] + a[4])
        return 100.0 * (total - idle) / total if total > 0 else None
    except (OSError, ValueError, IndexError):
        return None


def physical_memory_bytes() -> int | None:
    if sys.platform == "darwin":
        result = command("sysctl", "-n", "hw.memsize")
        try:
            return int(result.stdout.strip())
        except ValueError:
            return None
    try:
        for line in Path("/proc/meminfo").read_text(encoding="utf-8").splitlines():
            if line.startswith("MemTotal:"):
                return int(line.split()[1]) * 1024
    except OSError:
        pass
    return None


def memory_percent() -> float | None:
    total = physical_memory_bytes()
    if not total:
        return None
    result = command("ps", "-axo", "rss=")
    try:
        rss = sum(int(line.strip()) for line in result.stdout.splitlines() if line.strip()) * 1024
    except ValueError:
        return None
    return max(0.0, min(100.0, 100.0 * rss / total))


def gpu_percent() -> float | None:
    if sys.platform != "darwin" or shutil.which("ioreg") is None:
        return None
    result = command("ioreg", "-r", "-d", "1", "-w", "0", "-c", "AGXAccelerator")
    values = re.findall(r'"Device Utilization %"=([0-9]+)', result.stdout)
    return max(map(float, values)) if values else None


def thermal_pressure() -> bool | None:
    if sys.platform != "darwin" or shutil.which("pmset") is None:
        return None
    result = command("pmset", "-g", "therm")
    if result.returncode != 0:
        return None
    normal = "No thermal warning level has been recorded" in result.stdout
    normal = normal and "No performance warning level has been recorded" in result.stdout
    return not normal


@dataclass(frozen=True)
class Resources:
    cpu: float | None
    memory: float | None
    disk: float
    gpu: float | None
    thermal_pressure: bool | None

    def maximum(self) -> float:
        values = [self.disk]
        values.extend(value for value in (self.cpu, self.memory, self.gpu) if value is not None)
        return max(values)

    def overloaded(self, limit: float) -> bool:
        return self.thermal_pressure is True or self.maximum() >= limit

    def comfortably_below(self, limit: float, margin: float = 10.0) -> bool:
        return self.thermal_pressure is not True and self.maximum() < limit - margin


def resources(runtime: Path) -> Resources:
    usage = shutil.disk_usage(runtime)
    return Resources(
        cpu=cpu_percent(),
        memory=memory_percent(),
        disk=100.0 * usage.used / usage.total,
        gpu=gpu_percent(),
        thermal_pressure=thermal_pressure(),
    )


def http_json(port: int, path: str, *, data: dict[str, Any] | None = None) -> dict[str, Any] | None:
    try:
        payload = None if data is None else json.dumps(data).encode()
        request = Request(
            f"http://127.0.0.1:{port}{path}",
            data=payload,
            headers={"Content-Type": "application/json"} if payload else {},
            method="POST" if payload else "GET",
        )
        with urlopen(request, timeout=1.5) as response:
            value = json.load(response)
        return value if isinstance(value, dict) else None
    except (OSError, URLError, ValueError):
        return None


def free_port(start: int, used: set[int]) -> int:
    for port in range(start, start + 1000):
        if port in used:
            continue
        with socket.socket() as candidate:
            try:
                candidate.bind(("127.0.0.1", port))
            except OSError:
                continue
        return port
    raise RuntimeError("no free match-machine port")


def game_command(
    binary: Path,
    league: Path,
    seed: int,
    port: int,
    *,
    visible: bool,
    speed: str = DEFAULT_SPEED,
    turns: int | None = None,
) -> list[str]:
    turns = SPEED_TURNS[speed] if turns is None else turns
    args = [
        str(binary),
        "play",
        "--players", str(PLAYERS),
        "--width", str(WIDTH),
        "--height", str(HEIGHT),
        "--city-states", str(CITY_STATES),
        "--turns", str(turns),
        "--speed", speed,
        "--map", MAP,
        "--shape", SHAPE,
        "--poles", POLES,
        "--leader-pool", "civ6",
        "--seed", str(seed),
        "--port", str(port),
        "--spectate",
        "--supervised",
        "--league", str(league),
        "--league-record",
    ]
    if not visible:
        args.append("--no-open")
    return args


def stop_process(process: subprocess.Popen[str], timeout: float = 8.0) -> None:
    if process.poll() is not None:
        return
    try:
        os.killpg(process.pid, signal.SIGCONT)
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=timeout)
    except (OSError, subprocess.TimeoutExpired):
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except OSError:
            pass


def set_paused(process: subprocess.Popen[str], paused: bool) -> bool:
    if process.poll() is not None:
        return False
    try:
        os.killpg(process.pid, signal.SIGSTOP if paused else signal.SIGCONT)
        return True
    except OSError:
        return False


def match_row(league: Path, seed: int) -> dict[str, str] | None:
    path = league / "matches.csv"
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            rows = list(csv.DictReader(handle))
    except OSError:
        return None
    return next((row for row in reversed(rows) if row.get("seed") == str(seed)), None)


def winner_placement(row: dict[str, str] | None) -> str | None:
    if row is None:
        return None
    return next(
        (placement for placement in row.get("placements", "").split("|") if placement.endswith("@0")),
        None,
    )


def resource_action(sample: Resources, limit: float) -> str:
    if sample.overloaded(limit):
        return "shed_all"
    if sample.maximum() >= limit - SHED_HEADLESS_MARGIN:
        return "shed_headless"
    if sample.maximum() >= limit - SHED_ONE_MARGIN:
        return "shed_one"
    if sample.comfortably_below(limit, margin=RESUME_MARGIN):
        return "resume"
    return "hold"


@dataclass
class GameProcess:
    process: subprocess.Popen[str]
    seed: int
    port: int
    revision: str
    visible: bool
    started_monotonic: float
    started_utc: str
    log: str
    paused: bool = False
    ready: bool = False
    winner_seen: float | None = None
    last_status: dict[str, Any] = field(default_factory=dict)


class MatchMachine:
    def __init__(self, args: argparse.Namespace):
        self.args = args
        self.repo = args.repo.resolve()
        self.runtime = args.runtime.resolve()
        self.source = args.source.resolve()
        self.league = self.runtime / "league"
        self.logs = self.runtime / "logs"
        self.events = self.runtime / "events.jsonl"
        self.resource_log = self.runtime / "resources.jsonl"
        self.state_path = self.runtime / "state.json"
        self.ranking = self.runtime / "AI_PLAYER_ELO_RANKINGS.md"
        self.runtime.mkdir(parents=True, exist_ok=True)
        self.logs.mkdir(parents=True, exist_ok=True)
        self.games: list[GameProcess] = []
        self.current_revision: str | None = None
        self.binary: Path | None = None
        self.pending_revision: str | None = None
        self.next_sync = 0.0
        self.next_resource_log = 0.0
        self.completed = 0
        self.failed = 0
        self.visible_started = False
        self.visible_completed = False
        self.next_seed = args.seed
        self.started_monotonic = time.monotonic()
        self.deadline = self.started_monotonic + args.duration
        self.stopping = False
        self.watch_identity = process_identity(args.watch_pid) if args.watch_pid else None
        self.caffeinate: subprocess.Popen[str] | None = None
        self.maxima = {"cpu": 0.0, "memory": 0.0, "disk": 0.0, "gpu": 0.0}
        self.resume_not_before = 0.0

    def event(self, kind: str, **values: Any) -> None:
        event = {"at": utc_now(), "kind": kind, **values}
        append_jsonl(self.events, event)
        print(f"[match-machine] {kind}: " + " ".join(f"{k}={v}" for k, v in values.items()), flush=True)

    def persist(self, reason: str = "heartbeat") -> None:
        matches = self.league / "matches.csv"
        match_count = 0
        try:
            match_count = max(0, len(matches.read_text(encoding="utf-8").splitlines()) - 1)
        except OSError:
            pass
        state = {
            "updated_at": utc_now(),
            "reason": reason,
            "pid": os.getpid(),
            "watch_pid": self.args.watch_pid,
            "deadline_utc": datetime.fromtimestamp(
                time.time() + max(0.0, self.deadline - time.monotonic()), timezone.utc
            ).isoformat(timespec="seconds"),
            "revision": self.current_revision,
            "pending_revision": self.pending_revision,
            "visible_started": self.visible_started,
            "visible_completed": self.visible_completed,
            "completed_this_run": self.completed,
            "failed_this_run": self.failed,
            "match_log_rows": match_count,
            "active": [
                {
                    "pid": game.process.pid,
                    "seed": game.seed,
                    "port": game.port,
                    "visible": game.visible,
                    "paused": game.paused,
                    "revision": game.revision,
                    "turn": game.last_status.get("turn"),
                }
                for game in self.games
            ],
            "resource_maxima_percent": self.maxima,
            "rules": {
                "players": PLAYERS,
                "map_size": "standard",
                "dimensions": [WIDTH, HEIGHT],
                "city_states": CITY_STATES,
                "map": MAP,
                "speed": getattr(self.args, "speed", DEFAULT_SPEED),
                "turns": getattr(
                    self.args,
                    "turns",
                    SPEED_TURNS[getattr(self.args, "speed", DEFAULT_SPEED)],
                ),
                "teams": None,
                "ruleset": "stock Civ 6 defaults",
            },
        }
        atomic_json(self.state_path, state)

    def keep_awake(self) -> None:
        if sys.platform == "darwin" and shutil.which("caffeinate"):
            self.caffeinate = subprocess.Popen(
                ["caffeinate", "-i", "-s", "-w", str(os.getpid())],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                start_new_session=True,
                text=True,
            )
            self.event("sleep_prevention_started", pid=self.caffeinate.pid, mode="idle+AC-system")

    def wait_for_capacity(self, purpose: str, *, cpu_reservation: float = 0.0) -> Resources:
        """Wait without starting work until every measured resource has headroom."""
        next_notice = 0.0
        while True:
            if self.stopping or time.monotonic() >= self.deadline:
                raise RuntimeError("operator window ended while waiting for capacity")
            if self.args.watch_pid and not process_is_same(self.args.watch_pid, self.watch_identity):
                raise RuntimeError("watched terminal closed while waiting for capacity")
            sample = resources(self.runtime)
            now = time.monotonic()
            non_cpu = [sample.disk]
            non_cpu.extend(value for value in (sample.memory, sample.gpu) if value is not None)
            cpu_ceiling = self.args.limit - cpu_reservation - 5.0
            cpu_safe = sample.cpu is not None and sample.cpu < cpu_ceiling
            other_safe = max(non_cpu) < self.args.limit - 10.0
            if cpu_safe and other_safe and sample.thermal_pressure is not True:
                return sample
            if now >= next_notice:
                self.event("resource_gate", purpose=purpose, resources=asdict(sample))
                next_notice = now + self.args.resource_log_interval
            time.sleep(self.args.poll)

    def ensure_source(self) -> None:
        if not (self.repo / ".git").exists():
            raise RuntimeError(f"{self.repo} is not the CIVVIS repository")
        if not (self.source / ".git").exists():
            self.source.parent.mkdir(parents=True, exist_ok=True)
            result = command(
                "git", "worktree", "add", "--detach", str(self.source), "origin/main", cwd=self.repo
            )
            if result.returncode != 0:
                raise RuntimeError(f"cannot create private source worktree: {result.stdout}")

    def fetch(self) -> str:
        result = command("git", "fetch", "--prune", "origin", "main", cwd=self.repo, timeout=120)
        if result.returncode != 0:
            self.event("fetch_failed", output=result.stdout[-1000:])
        revision = command("git", "rev-parse", "origin/main", cwd=self.repo).stdout.strip()
        if not re.fullmatch(r"[0-9a-f]{40}", revision):
            raise RuntimeError("cannot resolve origin/main")
        return revision

    def initialize_league(self) -> None:
        self.league.mkdir(parents=True, exist_ok=True)
        destination = self.league / "league.json"
        if destination.exists():
            return
        source = self.source / "data" / "league" / "league.json"
        shutil.copy2(source, destination)
        # Opt into safe admission of new builtin controller families without
        # ever resetting the evidence accumulated here.
        (self.league / ".civvis-managed-roster").write_text("match-machine\n", encoding="utf-8")
        self.event("league_initialized", source=str(source))

    def build(self, revision: str) -> None:
        self.event("build_started", revision=revision, jobs=self.args.build_jobs)
        reset = command("git", "reset", "--hard", revision, cwd=self.source)
        if reset.returncode != 0:
            raise RuntimeError(f"cannot reset private worktree: {reset.stdout}")
        environment = os.environ.copy()
        environment["CARGO_BUILD_JOBS"] = str(self.args.build_jobs)
        environment["CIVVIS_COMMIT"] = revision
        result = command(
            "cargo", "build", "--release", "--locked", "--bin", "civvis",
            cwd=self.source,
            env=environment,
            timeout=self.args.build_timeout,
        )
        if result.returncode != 0:
            self.event("build_failed", revision=revision, output=result.stdout[-4000:])
            raise RuntimeError(f"CIVVIS build failed at {revision[:12]}")
        built = self.source / "target" / "release" / "civvis"
        binaries = self.runtime / "bin"
        binaries.mkdir(parents=True, exist_ok=True)
        promoted = binaries / f"civvis-{revision}"
        temporary = promoted.with_suffix(".tmp")
        shutil.copy2(built, temporary)
        temporary.chmod(0o755)
        os.replace(temporary, promoted)
        check = command(str(promoted), "validate", cwd=self.source, timeout=180)
        if check.returncode != 0:
            promoted.unlink(missing_ok=True)
            self.event("validation_failed", revision=revision, output=check.stdout[-4000:])
            raise RuntimeError(f"CIVVIS validation failed at {revision[:12]}")
        self.binary = promoted
        self.current_revision = revision
        self.pending_revision = None
        self.initialize_league()
        self.refresh_ranking()
        self.event("build_ready", revision=revision, binary=str(promoted))

    def refresh_ranking(self) -> None:
        script = self.source / "tools" / "update_ai_player_elo_rankings.py"
        if not script.exists() or not (self.league / "league.json").exists():
            return
        result = command(
            sys.executable,
            str(script),
            "--league", str(self.league / "league.json"),
            "--output", str(self.ranking),
            cwd=self.source,
            timeout=60,
        )
        if result.returncode != 0:
            self.event("ranking_refresh_failed", output=result.stdout[-2000:])

    def launch(self, *, visible: bool, seed: int | None = None) -> None:
        assert self.binary is not None and self.current_revision is not None
        seed = self.next_seed if seed is None else seed
        if seed == self.next_seed:
            self.next_seed += 1
        used = {game.port for game in self.games}
        port = free_port(self.args.port, used)
        kind = "visible" if visible else "headless"
        log = self.logs / f"{kind}-{seed}-{self.current_revision[:12]}.log"
        handle = log.open("w", encoding="utf-8")
        speed = getattr(self.args, "speed", DEFAULT_SPEED)
        turns = getattr(self.args, "turns", SPEED_TURNS[speed])
        process = subprocess.Popen(
            game_command(
                self.binary,
                self.league,
                seed,
                port,
                visible=visible,
                speed=speed,
                turns=turns,
            ),
            cwd=self.binary.parent,
            stdout=handle,
            stderr=subprocess.STDOUT,
            text=True,
            start_new_session=True,
        )
        handle.close()
        game = GameProcess(
            process=process,
            seed=seed,
            port=port,
            revision=self.current_revision,
            visible=visible,
            started_monotonic=time.monotonic(),
            started_utc=utc_now(),
            log=str(log),
        )
        self.games.append(game)
        if visible:
            self.visible_started = True
        self.event(
            "game_started",
            game_kind=kind,
            seed=seed,
            pid=process.pid,
            port=port,
            revision=self.current_revision,
        )

    def finish(self, game: GameProcess, *, failed: bool, reason: str) -> None:
        status = game.last_status
        row = match_row(self.league, game.seed)
        stop_process(game.process)
        if game in self.games:
            self.games.remove(game)
        if failed:
            self.failed += 1
        else:
            self.completed += 1
            if game.visible:
                self.visible_completed = True
        self.event(
            "game_failed" if failed else "game_completed",
            game_kind="visible" if game.visible else "headless",
            seed=game.seed,
            revision=game.revision,
            reason=reason,
            turn=row.get("turns") if row else status.get("turn"),
            winner=status.get("winner") if row is None else None,
            winner_placement=winner_placement(row),
            victory=row.get("victory") if row else status.get("victory_type"),
            match_row=row,
            elapsed_seconds=round(time.monotonic() - game.started_monotonic, 1),
            log=game.log,
        )

    def poll_games(self) -> bool:
        changed = False
        now = time.monotonic()
        for game in list(self.games):
            code = game.process.poll()
            if code is not None:
                self.finish(game, failed=True, reason=f"process exited {code}")
                changed = True
                continue
            recorded = match_row(self.league, game.seed) is not None
            if recorded:
                if game.winner_seen is None:
                    game.winner_seen = now
                hold = RESULT_HOLD_SECONDS if game.visible else 0.5
                if now - game.winner_seen >= hold:
                    self.finish(game, failed=False, reason="rated result recorded")
                    changed = True
                continue
            if game.paused:
                continue
            status = http_json(game.port, "/status")
            if status is None:
                if now - game.started_monotonic > self.args.start_timeout and not game.ready:
                    self.finish(game, failed=True, reason="server did not become ready")
                    changed = True
                continue
            game.ready = True
            game.last_status = status
            if status.get("winner") is None:
                if not game.visible:
                    http_json(game.port, "/pace", data={"ms": 0, "paused": False})
                elif status.get("turn", 0) <= 1:
                    http_json(game.port, "/pace", data={"ms": self.args.visible_pace, "paused": False})
                continue
            if game.winner_seen is None:
                game.winner_seen = now
            if now - game.winner_seen >= self.args.record_timeout:
                self.finish(game, failed=True, reason="winner was not appended to matches.csv")
                changed = True
        return changed

    def govern(self, sample: Resources) -> None:
        for name in ("cpu", "memory", "disk", "gpu"):
            value = getattr(sample, name)
            if value is not None:
                self.maxima[name] = max(self.maxima[name], round(value, 1))
        # Keep enough headroom for a host-wide burst: shed one process fifteen
        # points early and every headless process ten points early. Recovery is
        # deliberately slower than shedding so a transient cannot oscillate
        # the whole pool back on before the host has settled.
        action = resource_action(sample, self.args.limit)
        now = time.monotonic()
        if action != "resume":
            self.resume_not_before = max(
                self.resume_not_before, now + RESUME_COOLDOWN_SECONDS
            )
        if action == "shed_all":
            for game in sorted(self.games, key=lambda game: game.visible):
                if game.paused or (
                    game.visible
                    and any(not item.visible and not item.paused for item in self.games)
                ):
                    continue
                if set_paused(game.process, True):
                    game.paused = True
                    self.event("game_paused_for_resources", seed=game.seed, resources=asdict(sample))
        elif action == "shed_headless":
            for game in self.games:
                if game.visible or game.paused:
                    continue
                if set_paused(game.process, True):
                    game.paused = True
                    self.event("game_paused_for_resources", seed=game.seed, resources=asdict(sample))
        elif action == "shed_one":
            candidates = sorted(self.games, key=lambda game: (game.visible, game.paused))
            candidate = next((game for game in candidates if not game.paused), None)
            if candidate and set_paused(candidate.process, True):
                candidate.paused = True
                self.event("game_paused_for_resources", seed=candidate.seed, resources=asdict(sample))
        elif action == "resume" and now >= self.resume_not_before:
            candidate = next((game for game in self.games if game.paused), None)
            if candidate and set_paused(candidate.process, False):
                candidate.paused = False
                self.resume_not_before = now + RESUME_STEP_SECONDS
                self.event("game_resumed", seed=candidate.seed, resources=asdict(sample))

    def fill_slots(self, sample: Resources) -> None:
        # Admit only one process per fresh resource sample. Starting the whole
        # pool from one idle reading could cross the ceiling before the next
        # measurement had a chance to govern it.
        if self.pending_revision or not sample.comfortably_below(
            self.args.limit, margin=RESUME_MARGIN
        ):
            return
        if not self.visible_started:
            self.launch(visible=True)
            return
        headless = sum(not game.visible for game in self.games)
        if headless < self.args.headless and len(self.games) < self.args.max_processes:
            self.launch(visible=False)

    def sync(self) -> None:
        revision = self.fetch()
        self.next_sync = time.monotonic() + self.args.sync_interval
        if revision != self.current_revision:
            self.pending_revision = revision
            self.event("head_changed", current=self.current_revision, target=revision)

    def run(self) -> int:
        self.event(
            "machine_started",
            duration=self.args.duration,
            watch_pid=self.args.watch_pid,
            headless=self.args.headless,
            limit=self.args.limit,
        )
        self.keep_awake()
        self.ensure_source()
        self.sync()
        assert self.pending_revision is not None
        build_reservation = 100.0 * self.args.build_jobs / max(1, os.cpu_count() or 1)
        self.wait_for_capacity("initial build", cpu_reservation=build_reservation)
        self.build(self.pending_revision)
        last_sample = resources(self.runtime)
        try:
            while not self.stopping and time.monotonic() < self.deadline:
                if self.args.watch_pid and not process_is_same(self.args.watch_pid, self.watch_identity):
                    self.event("terminal_closed", watch_pid=self.args.watch_pid)
                    break
                now = time.monotonic()
                if now >= self.next_sync:
                    self.sync()
                changed = self.poll_games()
                last_sample = resources(self.runtime)
                self.govern(last_sample)
                if now >= self.next_resource_log:
                    append_jsonl(
                        self.resource_log,
                        {"at": utc_now(), **asdict(last_sample), "active": len(self.games)},
                    )
                    self.next_resource_log = now + self.args.resource_log_interval
                    self.persist("resource sample")
                if self.pending_revision and not self.games:
                    try:
                        build_reservation = (
                            100.0 * self.args.build_jobs / max(1, os.cpu_count() or 1)
                        )
                        self.wait_for_capacity("HEAD build", cpu_reservation=build_reservation)
                        self.build(self.pending_revision)
                    except RuntimeError as error:
                        self.event("revision_rejected", error=str(error))
                        self.pending_revision = None
                        self.next_sync = now + self.args.build_retry
                self.fill_slots(last_sample)
                if changed:
                    self.refresh_ranking()
                    self.persist("game boundary")
                time.sleep(self.args.poll)
        finally:
            self.stopping = True
            for game in list(self.games):
                self.finish(game, failed=True, reason="match machine stopped before result")
            if self.caffeinate is not None:
                stop_process(self.caffeinate, timeout=2)
            self.refresh_ranking()
            self.persist("stopped")
            self.event("machine_stopped", completed=self.completed, failed=self.failed, resources=asdict(last_sample))
        return 0


def default_paths() -> tuple[Path, Path, Path]:
    repo = Path(__file__).resolve().parents[1]
    runtime = repo / "target" / "match-machine"
    source = repo.parent / f"{repo.name.lower()}-match-machine-src"
    return repo, runtime, source


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    repo, runtime, source = default_paths()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=repo, help="stable CIVVIS checkout")
    parser.add_argument("--runtime", type=Path, default=runtime, help="durable logs and league")
    parser.add_argument("--source", type=Path, default=source, help="private detached build worktree")
    parser.add_argument("--duration", type=float, default=24 * 60 * 60, help="seconds to operate")
    parser.add_argument("--watch-pid", type=int, required=True, help="terminal shell PID; its exit stops all games")
    parser.add_argument("--headless", type=int, default=8, help="concurrent headless games")
    parser.add_argument("--max-processes", type=int, default=8, help="visible plus headless hard cap")
    parser.add_argument("--limit", type=float, default=70.0, help="host resource ceiling percent")
    parser.add_argument(
        "--build-jobs",
        type=int,
        default=max(1, int((os.cpu_count() or 1) * 0.4)),
        help="Cargo workers (default: 40%% of logical CPUs)",
    )
    parser.add_argument("--build-timeout", type=float, default=1800)
    parser.add_argument("--build-retry", type=float, default=60)
    parser.add_argument("--sync-interval", type=float, default=300)
    parser.add_argument("--poll", type=float, default=1)
    parser.add_argument("--resource-log-interval", type=float, default=60)
    parser.add_argument("--record-timeout", type=float, default=45)
    parser.add_argument("--start-timeout", type=float, default=90)
    parser.add_argument(
        "--speed",
        choices=tuple(SPEED_TURNS),
        default=DEFAULT_SPEED,
        help="Civ VI game speed (default: online)",
    )
    parser.add_argument(
        "--turns",
        type=int,
        default=None,
        help="turn limit (default: the stock limit for --speed)",
    )
    parser.add_argument("--visible-pace", type=int, default=250, help="milliseconds per visible turn")
    parser.add_argument("--port", type=int, default=8870)
    parser.add_argument("--seed", type=int, default=int(time.time()) & 0x7FFF_FFFF)
    args = parser.parse_args(argv)
    if args.turns is None:
        args.turns = SPEED_TURNS[args.speed]
    if not 0 < args.limit <= 70:
        parser.error("--limit must be in (0, 70]")
    if args.duration <= 0 or args.headless < 1 or args.max_processes < 2 or args.turns < 1:
        parser.error("duration, headless, max-processes, and turns must be positive")
    if args.build_jobs < 1:
        parser.error("--build-jobs must be positive")
    build_share = 100.0 * args.build_jobs / max(1, os.cpu_count() or 1)
    if build_share >= args.limit - 5.0:
        parser.error("--build-jobs leaves insufficient CPU headroom below --limit")
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    machine = MatchMachine(args)

    def stop(_signum: int, _frame: Any) -> None:
        machine.stopping = True

    signal.signal(signal.SIGINT, stop)
    signal.signal(signal.SIGTERM, stop)
    try:
        return machine.run()
    except (OSError, RuntimeError, subprocess.TimeoutExpired) as error:
        machine.event("fatal", error=str(error))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
