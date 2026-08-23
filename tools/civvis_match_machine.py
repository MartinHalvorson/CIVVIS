#!/usr/bin/env python3
"""Run a terminal-bound, resource-capped CIVVIS match fleet.

The match machine continuously keeps one visible game and a pool of fast
headless games on the same mutable league.  Every game is an eight-player,
Standard-size, Online-speed Continents free-for-all under the stock rules.
CIVVIS itself does the rating: the spectator server samples each
civilization's top three eligible players with 3:2:1 rank weights and
atomically appends a completed game to ``matches.csv`` and ``league.json``.

The operator fetches ``origin/main`` into a private detached worktree, drains
old headless games at a revision boundary, and builds the new HEAD in the
background while keeping one visible match alive.  New games then use the
immutable promoted binary.  It never edits or updates a development checkout.
CPU, memory, disk and Apple-GPU use are sampled host-wide; game and build
process groups are stopped and resumed before the configured ceiling.  macOS
thermal pressure is an additional hard gate.

The process is intentionally foreground and watches the shell PID supplied by
``--watch-pid``.  Closing that terminal kills the fleet.  While it is alive,
its ``caffeinate`` child prevents idle/system sleep on AC power.
"""

from __future__ import annotations

import argparse
from concurrent.futures import Future, ThreadPoolExecutor
import csv
import ctypes
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
import threading
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
VISIBLE_SUCCESSOR_GRACE_SECONDS = 8.0
SHED_ONE_MARGIN = 15.0
SHED_HEADLESS_MARGIN = 10.0
RESUME_MARGIN = 20.0
RESUME_COOLDOWN_SECONDS = 15.0
RESUME_STEP_SECONDS = 5.0
BUILD_CPU_DUTY_CYCLE_SECONDS = 0.4
BUILD_CPU_MAX_DUTY = 0.35
BUILD_CPU_TARGET_MARGIN = SHED_ONE_MARGIN + 2.0
# Every match is pinned to the stock eight-major contract, so measurement
# need is judged on the exact eight-seat win-evidence bucket the league's own
# selection reads.
FOCUS_TABLE_SIZE = 8
_resource_sample_lock = threading.Lock()


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


def visible_browser_already_opened(state_path: Path) -> bool:
    """Carry the one-tab promise across operator process upgrades."""
    try:
        previous = json.loads(state_path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return False
    return bool(previous.get("visible_started"))


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


_darwin_cpu_ticks: tuple[int, int, int, int] | None = None
_darwin_libsystem: Any | None = None
_darwin_host_port: int | None = None
DARWIN_CPU_STATE_IDLE = 2


def darwin_cpu_ticks() -> tuple[int, int, int, int] | None:
    """Return the host-wide user, system, idle, and nice tick totals on macOS."""
    global _darwin_host_port, _darwin_libsystem
    try:
        if _darwin_libsystem is None:
            _darwin_libsystem = ctypes.CDLL("/usr/lib/libSystem.B.dylib")
            _darwin_libsystem.mach_host_self.restype = ctypes.c_uint
            _darwin_libsystem.host_statistics.argtypes = [
                ctypes.c_uint,
                ctypes.c_int,
                ctypes.POINTER(ctypes.c_int),
                ctypes.POINTER(ctypes.c_uint),
            ]
            _darwin_libsystem.host_statistics.restype = ctypes.c_int
        if _darwin_host_port is None:
            _darwin_host_port = _darwin_libsystem.mach_host_self()
        ticks = (ctypes.c_uint * 4)()
        count = ctypes.c_uint(4)
        status = _darwin_libsystem.host_statistics(
            _darwin_host_port,
            3,  # HOST_CPU_LOAD_INFO
            ctypes.cast(ticks, ctypes.POINTER(ctypes.c_int)),
            ctypes.byref(count),
        )
    except (AttributeError, OSError, TypeError):
        return None
    if status != 0 or count.value < 4:
        return None
    return tuple(int(tick) for tick in ticks)


def darwin_cpu_percent() -> float | None:
    """Measure aggregate CPU from native tick deltas without spawning ``top``."""
    global _darwin_cpu_ticks
    current = darwin_cpu_ticks()
    if current is None:
        return None
    previous = _darwin_cpu_ticks
    if previous is None:
        # The host API is cumulative.  A short bootstrap interval gives the
        # first resource gate a valid sample without the costly ``top`` fork.
        time.sleep(0.2)
        previous = current
        current = darwin_cpu_ticks()
        if current is None:
            return None
    _darwin_cpu_ticks = current
    deltas = [((new - old) & 0xFFFFFFFF) for old, new in zip(previous, current)]
    total = sum(deltas)
    return 100.0 * (total - deltas[DARWIN_CPU_STATE_IDLE]) / total if total > 0 else None


def cpu_percent() -> float | None:
    if sys.platform == "darwin":
        native = darwin_cpu_percent()
        if native is not None:
            return native
        # Keep the familiar command path as a conservative fallback if the
        # native host API is unavailable on a future macOS release.
        try:
            result = command("top", "-l", "1", "-n", "0", timeout=4)
        except (OSError, subprocess.TimeoutExpired):
            return None
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

    def governed_maximum(self) -> float:
        # The resources the fleet's own work actually consumes.  The simulator
        # and the build are CPU-only, so shed/resume margins measured against
        # the GPU describe another process's load — pausing fleet work cannot
        # lower it, and holding margin-distance below the ceiling against it
        # starves the whole fleet whenever a real game renders on this host.
        values = [self.disk]
        values.extend(value for value in (self.cpu, self.memory) if value is not None)
        return max(values)

    def overloaded(self, limit: float) -> bool:
        # CPU is the admission-critical signal.  If sampling it fails, pause
        # safely rather than guessing that enough aggregate headroom remains.
        # GPU and thermal stay in this hard gate: at the limit the host is
        # genuinely saturated no matter whose load it is.
        return self.cpu is None or self.thermal_pressure is True or self.maximum() >= limit

    def comfortably_below(self, limit: float, margin: float = 10.0) -> bool:
        # Margins buy headroom for the fleet's next process, so they apply to
        # the governed resources only; the full maximum still has to clear the
        # hard limit itself.
        return (
            self.cpu is not None
            and self.thermal_pressure is not True
            and self.maximum() < limit
            and self.governed_maximum() < limit - margin
        )


def resources(runtime: Path) -> Resources:
    # The background HEAD builder and operator loop can both govern work. Keep
    # native CPU tick deltas ordered and avoid doubling the host-wide helper
    # probes when their sampling windows overlap.
    with _resource_sample_lock:
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


def port_available(port: int) -> bool:
    with socket.socket() as candidate:
        try:
            candidate.bind(("127.0.0.1", port))
        except OSError:
            return False
    return True


def free_port(start: int, used: set[int]) -> int:
    for port in range(start, start + 1000):
        if port in used:
            continue
        if port_available(port):
            return port
    raise RuntimeError("no free match-machine port")


def game_port(base: int, used: set[int], *, visible: bool) -> int | None:
    if visible:
        return base if base not in used and port_available(base) else None
    # The browser follows supervised successors at one stable origin. Never
    # let a headless server occupy that dedicated visible-game port.
    return free_port(base + 1, used)


def game_command(
    binary: Path,
    league: Path,
    seed: int,
    port: int,
    *,
    visible: bool,
    open_browser: bool | None = None,
    speed: str = DEFAULT_SPEED,
    turns: int | None = None,
    focus_strategy: str | None = None,
) -> list[str]:
    if open_browser is None:
        open_browser = visible
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
    if focus_strategy:
        args.extend(("--force-strategy", focus_strategy))
    if not open_browser:
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
    if sample.governed_maximum() >= limit - SHED_HEADLESS_MARGIN:
        return "shed_headless"
    if sample.governed_maximum() >= limit - SHED_ONE_MARGIN:
        return "shed_one"
    if sample.comfortably_below(limit, margin=RESUME_MARGIN):
        return "resume"
    return "hold"


def build_cpu_duty_cycle(cpu: float | None, limit: float) -> float:
    """Return a safe run fraction for a build that may saturate every CPU.

    Cargo's job limit controls concurrent rustc processes, but one optimized
    rustc invocation can still use every core.  Model that worst case and keep
    its cycle-averaged host CPU below the normal shed-one threshold with a
    small additional margin.
    """
    if cpu is None:
        return 0.0
    target = max(0.0, limit - BUILD_CPU_TARGET_MARGIN)
    if cpu >= target:
        return 0.0
    available_fraction = (target - cpu) / max(1.0, 100.0 - cpu)
    return min(BUILD_CPU_MAX_DUTY, max(0.0, available_fraction))


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
    focus_strategy: str | None = None
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
        # Builds reset ``self.source`` in a background worker. Keep the
        # self-contained ranking updater in the runtime so completed games can
        # refresh their durable Elo view while a newer HEAD is compiling.
        self.ranking_updater = self.runtime / "update_ai_player_elo_rankings.py"
        self.runtime.mkdir(parents=True, exist_ok=True)
        self.logs.mkdir(parents=True, exist_ok=True)
        self.games: list[GameProcess] = []
        self.current_revision: str | None = None
        self.binary: Path | None = None
        self.pending_revision: str | None = None
        self.build_revision: str | None = None
        self.build_future: Future[Path] | None = None
        self.build_executor = ThreadPoolExecutor(max_workers=1, thread_name_prefix="civvis-build")
        self.next_sync = 0.0
        self.next_resource_log = 0.0
        self.resume_visible_next = False
        self.completed = 0
        self.failed = 0
        self.visible_started = False
        self.visible_completed = False
        self.visible_completed_count = 0
        self.visible_browser_opened = visible_browser_already_opened(self.state_path)
        self.next_seed = args.seed
        self.started_monotonic = time.monotonic()
        self.deadline = self.started_monotonic + args.duration
        absolute_deadline = getattr(args, "deadline_utc", None)
        if absolute_deadline is not None:
            remaining = (absolute_deadline - datetime.now(timezone.utc)).total_seconds()
            self.deadline = self.started_monotonic + max(0.0, remaining)
        self.stopping = False
        self.stop_signal: str | None = None
        self.watch_identity = process_identity(args.watch_pid) if args.watch_pid else None
        self.caffeinate: subprocess.Popen[str] | None = None
        self.maxima = {"cpu": 0.0, "memory": 0.0, "disk": 0.0, "gpu": 0.0}
        self.resume_not_before = 0.0
        self.strategy_schedule: list[str] = []
        self.strategy_cursor = 0
        self.schedule_roster: frozenset[str] = frozenset()

    def event(self, kind: str, **values: Any) -> None:
        event = {"at": utc_now(), "kind": kind, **values}
        append_jsonl(self.events, event)
        print(f"[match-machine] {kind}: " + " ".join(f"{k}={v}" for k, v in values.items()), flush=True)

    def stop_cause(self, unspent: float) -> str:
        """Why this run ended, in the one field a reader looks at first.

        A signal wins over the clock: a run killed in its final second is
        still a kill, and calling it `window_ended` is what made the
        2026-08-15 outage look like a completed job for a week.
        """
        if self.stop_signal:
            return f"stopped:{self.stop_signal.lower()}"
        return "stopped:window_ended" if unspent <= 0 else "stopped:loop_exit"

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
            "build_revision": self.build_revision,
            "visible_started": self.visible_started,
            "visible_completed": self.visible_completed,
            "visible_completed_count": self.visible_completed_count,
            "visible_active": any(game.visible for game in self.games),
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
                    "focus_strategy": game.focus_strategy,
                }
                for game in self.games
            ],
            "strategy_coverage": {
                "roster_strategies": len(set(self.strategy_schedule)),
                "scheduled_entries": len(self.strategy_schedule),
                "cursor": self.strategy_cursor,
                "all_unretired": True,
            },
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

    def watched_terminal_closed(self) -> bool:
        return bool(
            self.args.watch_pid
            and not process_is_same(self.args.watch_pid, self.watch_identity)
        )

    def stop_for_terminal_close(self) -> None:
        if self.stopping:
            return
        self.stopping = True
        self.event("terminal_closed", watch_pid=self.args.watch_pid)

    def wait_for_capacity(
        self, purpose: str, *, cpu_reservation: float = 0.0
    ) -> Resources | None:
        """Wait without starting work until every measured resource has headroom."""
        next_notice = 0.0
        while True:
            if self.stopping:
                return None
            if time.monotonic() >= self.deadline:
                self.stopping = True
                self.event("operator_window_ended", purpose=purpose)
                return None
            if self.watched_terminal_closed():
                self.stop_for_terminal_close()
                return None
            sample = resources(self.runtime)
            now = time.monotonic()
            if self.capacity_available(sample, cpu_reservation=cpu_reservation):
                return sample
            if now >= next_notice:
                self.event("resource_gate", purpose=purpose, resources=asdict(sample))
                next_notice = now + self.args.resource_log_interval
            time.sleep(self.args.poll)

    def capacity_available(self, sample: Resources, *, cpu_reservation: float = 0.0) -> bool:
        non_cpu = [sample.disk]
        non_cpu.extend(value for value in (sample.memory, sample.gpu) if value is not None)
        cpu_ceiling = self.args.limit - cpu_reservation - 5.0
        cpu_safe = sample.cpu is not None and sample.cpu < cpu_ceiling
        other_safe = max(non_cpu) < self.args.limit - 10.0
        return cpu_safe and other_safe and sample.thermal_pressure is not True

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
        if not destination.exists():
            source = self.source / "data" / "league" / "league.json"
            shutil.copy2(source, destination)
            # Opt into safe admission of new builtin controller families without
            # ever resetting the evidence accumulated here.
            (self.league / ".civvis-managed-roster").write_text(
                "match-machine\n", encoding="utf-8"
            )
            self.event("league_initialized", source=str(source))
        self.refresh_strategy_schedule()

    def refresh_strategy_schedule(self) -> None:
        """Build a deterministic all-roster coverage cycle.

        The normal live selector intentionally samples only the strongest few
        strategies for each civilization. The match machine has a different
        obligation: every unretired strategy must receive repeated evidence,
        while the strongest Elo entries still get extra focus.

        The full pass is ordered by measurement need — fewest games at the
        pinned eight-seat table first, widest rating deviation breaking ties —
        so a newborn the live league just bred is focused within a few
        launches instead of waiting a whole cycle behind well-measured
        entries. The top eight by rating are appended once more as the
        exploitation half of the cycle.
        """
        try:
            roster = json.loads((self.league / "league.json").read_text(encoding="utf-8"))
            strategies = roster.get("strategies", [])
        except (OSError, ValueError):
            self.strategy_schedule = []
            self.schedule_roster = frozenset()
            return
        candidates = [
            strategy
            for strategy in strategies
            if isinstance(strategy, dict)
            and strategy.get("retired") is not True
            and strategy.get("human") is not True
            and isinstance(strategy.get("name"), str)
            and strategy["name"].strip()
        ]

        def games_at_table(strategy: dict) -> int:
            bucket = strategy.get("wins_by_table_size")
            if isinstance(bucket, dict):
                exact = bucket.get(str(FOCUS_TABLE_SIZE))
                if isinstance(exact, dict):
                    return int(exact.get("games", 0))
            return 0

        need_order = sorted(
            candidates,
            key=lambda strategy: (
                games_at_table(strategy),
                -float(strategy.get("rd", 350.0)),
                strategy["name"].casefold(),
            ),
        )
        by_rating = sorted(
            candidates,
            key=lambda strategy: (
                -float(strategy.get("rating", 1500.0)),
                strategy["name"].casefold(),
            ),
        )
        names = [strategy["name"] for strategy in need_order]
        top = [strategy["name"] for strategy in by_rating[: min(8, len(by_rating))]]
        self.strategy_schedule = names + top
        self.strategy_cursor %= max(1, len(self.strategy_schedule))
        self.schedule_roster = frozenset(names)
        self.event(
            "strategy_schedule_ready",
            roster_strategies=len(names),
            scheduled_entries=len(self.strategy_schedule),
            most_needed=names[:8],
            top_strategies=top,
        )

    def next_focus_strategy(self) -> str | None:
        # Selection now breeds and retires from live games, so the roster
        # changes underneath a running operator. The schedule was built once
        # at startup, which meant a newborn never received a focus seat and a
        # retiree kept burning launches on a --force-strategy the server
        # silently drops. Rebuild when the active-name set changes — ratings
        # moving is not a reason, so the cursor keeps its place through
        # ordinary results — and restart from the most-needed entry when it
        # does change, which is exactly where a fresh newborn sorts.
        try:
            roster = json.loads((self.league / "league.json").read_text(encoding="utf-8"))
            active = frozenset(
                strategy["name"]
                for strategy in roster.get("strategies", [])
                if isinstance(strategy, dict)
                and strategy.get("retired") is not True
                and strategy.get("human") is not True
                and isinstance(strategy.get("name"), str)
                and strategy["name"].strip()
            )
        except (OSError, ValueError):
            active = None
        if active is not None and active != self.schedule_roster:
            self.refresh_strategy_schedule()
            self.strategy_cursor = 0
        if not self.strategy_schedule:
            return None
        strategy = self.strategy_schedule[self.strategy_cursor % len(self.strategy_schedule)]
        self.strategy_cursor += 1
        return strategy

    def resource_capped_command(
        self,
        *args: str,
        cwd: Path,
        env: dict[str, str] | None,
        timeout: float,
        purpose: str,
        log_path: Path,
        prefer_visible: bool = False,
    ) -> subprocess.CompletedProcess[str] | None:
        """Run long work while preserving terminal, deadline, and host limits."""
        log_path.parent.mkdir(parents=True, exist_ok=True)
        started = time.monotonic()
        # This loop keeps the process SIGSTOPped for admission, host pressure,
        # visible-game yields, and the build duty cycle, so a wall-clock
        # deadline kills healthy throttled work long before it has spent its
        # budget.  Only unpaused execution ages against ``timeout``; the
        # duty-cycle branch charges its own run pulses explicitly.
        run_seconds = 0.0
        last_check = started
        paused = False
        admission_pending = False
        cpu_throttled = purpose == "build"
        pressure_paused = False
        yielding_for_visible = False
        visible_recovery_not_before: float | None = None
        with log_path.open("w", encoding="utf-8") as output:
            process = subprocess.Popen(
                args,
                cwd=cwd,
                env=env,
                stdout=output,
                stderr=subprocess.STDOUT,
                start_new_session=True,
                text=True,
            )
            # Popen begins executing immediately. Stop every resource-capped
            # process before the first host sample so even a validator cannot
            # burst past the ceiling during its admission check. A very short
            # command may finish in the race between poll and SIGSTOP; that is
            # already complete work, not a failed safety gate.
            if process.poll() is None:
                if not set_paused(process, True):
                    if process.poll() is None:
                        stop_process(process, timeout=2)
                        raise RuntimeError("cannot engage the initial resource gate")
                else:
                    paused = True
                    admission_pending = True
            if cpu_throttled and paused:
                self.event(
                    "work_cpu_throttle_started",
                    purpose=purpose,
                    pid=process.pid,
                    cycle_seconds=BUILD_CPU_DUTY_CYCLE_SECONDS,
                    max_duty=BUILD_CPU_MAX_DUTY,
                )
            while process.poll() is None:
                now = time.monotonic()
                if not paused:
                    run_seconds += now - last_check
                last_check = now
                if self.stopping:
                    stop_process(process, timeout=2)
                    return None
                if self.watched_terminal_closed():
                    self.stop_for_terminal_close()
                    stop_process(process, timeout=2)
                    return None
                if now >= self.deadline:
                    self.stopping = True
                    self.event("operator_window_ended", purpose=purpose)
                    stop_process(process, timeout=2)
                    return None
                if run_seconds >= timeout:
                    stop_process(process, timeout=2)
                    raise subprocess.TimeoutExpired(args, timeout)

                if prefer_visible:
                    visible_games = [game for game in self.games if game.visible]
                    visible_waiting = not visible_games or any(
                        game.paused for game in visible_games
                    )
                    if visible_waiting:
                        # The operator loop and background builder otherwise
                        # race for the same narrow recovery window.  A build
                        # pulse can repeatedly win and leave the sole visible
                        # match stopped even though both processes are alive.
                        if not paused:
                            if not set_paused(process, True):
                                stop_process(process, timeout=2)
                                raise RuntimeError(
                                    "cannot yield background work to visible game"
                                )
                            paused = True
                        if not yielding_for_visible:
                            yielding_for_visible = True
                            self.event(
                                "work_yielded_for_visible",
                                purpose=purpose,
                                pid=process.pid,
                            )
                        visible_recovery_not_before = None
                        time.sleep(self.args.poll)
                        continue
                    if yielding_for_visible:
                        if visible_recovery_not_before is None:
                            visible_recovery_not_before = now + RESUME_STEP_SECONDS
                        if now < visible_recovery_not_before:
                            time.sleep(self.args.poll)
                            continue
                        yielding_for_visible = False
                        visible_recovery_not_before = None
                        self.event(
                            "work_resumed_after_visible",
                            purpose=purpose,
                            pid=process.pid,
                            grace_seconds=RESUME_STEP_SECONDS,
                        )

                sample = resources(self.runtime)
                must_pause = (
                    sample.cpu is None
                    or sample.thermal_pressure is True
                    or sample.maximum() >= self.args.limit
                    or sample.governed_maximum() >= self.args.limit - SHED_ONE_MARGIN
                )
                if cpu_throttled:
                    duty = build_cpu_duty_cycle(sample.cpu, self.args.limit)
                    must_pause = must_pause or duty <= 0.0
                    if must_pause:
                        if not paused:
                            if not set_paused(process, True):
                                stop_process(process, timeout=2)
                                raise RuntimeError("cannot pause resource-capped build")
                            paused = True
                        if not pressure_paused:
                            pressure_paused = True
                            self.event(
                                "work_paused_for_resources",
                                purpose=purpose,
                                pid=process.pid,
                                resources=asdict(sample),
                            )
                        time.sleep(self.args.poll)
                        continue
                    comfortable = sample.comfortably_below(
                        self.args.limit, margin=RESUME_MARGIN
                    )
                    if (pressure_paused or admission_pending) and not comfortable:
                        # Once pressure trips the gate, require the same
                        # recovery headroom as the game governor. The initial
                        # admission gate uses that headroom too. Resuming at the
                        # shed threshold creates stop/start oscillation and
                        # leaves no room for the next build pulse.
                        if not pressure_paused:
                            pressure_paused = True
                            self.event(
                                "work_paused_for_resources",
                                purpose=purpose,
                                pid=process.pid,
                                resources=asdict(sample),
                            )
                        time.sleep(self.args.poll)
                        continue
                    if pressure_paused:
                        pressure_paused = False
                        self.event(
                            "work_resumed",
                            purpose=purpose,
                            pid=process.pid,
                            resources=asdict(sample),
                        )
                    admission_pending = False
                    if paused:
                        if not set_paused(process, False):
                            if process.poll() is None:
                                stop_process(process, timeout=2)
                                raise RuntimeError("cannot resume resource-capped build")
                            continue
                        paused = False
                    time.sleep(BUILD_CPU_DUTY_CYCLE_SECONDS * duty)
                    if process.poll() is None:
                        if not set_paused(process, True):
                            stop_process(process, timeout=2)
                            raise RuntimeError("cannot re-pause resource-capped build")
                        paused = True
                    time.sleep(BUILD_CPU_DUTY_CYCLE_SECONDS * (1.0 - duty))
                    # The pulse ran with ``paused`` True at the next loop top
                    # (the cycle always re-pauses a live process), so charge
                    # its run share here rather than from the loop clock.
                    run_seconds += BUILD_CPU_DUTY_CYCLE_SECONDS * duty
                    continue
                comfortable = sample.comfortably_below(
                    self.args.limit, margin=RESUME_MARGIN
                )
                if must_pause or (
                    (pressure_paused or admission_pending) and not comfortable
                ):
                    if not paused:
                        if not set_paused(process, True):
                            if process.poll() is None:
                                stop_process(process, timeout=2)
                                raise RuntimeError("cannot pause resource-capped work")
                            continue
                        paused = True
                    if not pressure_paused:
                        pressure_paused = True
                        self.event(
                            "work_paused_for_resources",
                            purpose=purpose,
                            pid=process.pid,
                            resources=asdict(sample),
                        )
                elif paused and comfortable:
                    if not set_paused(process, False):
                        if process.poll() is None:
                            stop_process(process, timeout=2)
                            raise RuntimeError("cannot resume resource-capped work")
                        continue
                    paused = False
                    admission_pending = False
                    if pressure_paused:
                        pressure_paused = False
                        self.event(
                            "work_resumed",
                            purpose=purpose,
                            pid=process.pid,
                            resources=asdict(sample),
                        )
                time.sleep(self.args.poll)
            returncode = process.returncode

        captured = log_path.read_text(encoding="utf-8", errors="replace")
        return subprocess.CompletedProcess(args, returncode, captured)

    def compile_build(self, revision: str, prefer_visible: bool = False) -> Path | None:
        self.event("build_started", revision=revision, jobs=self.args.build_jobs)
        reset = command("git", "reset", "--hard", revision, cwd=self.source)
        if reset.returncode != 0:
            raise RuntimeError(f"cannot reset private worktree: {reset.stdout}")
        environment = os.environ.copy()
        environment["CARGO_BUILD_JOBS"] = str(self.args.build_jobs)
        environment.pop("CIVVIS_COMMIT", None)
        result = self.resource_capped_command(
            "cargo", "build", "--release", "--locked", "--bin", "civvis",
            cwd=self.source,
            env=environment,
            timeout=self.args.build_timeout,
            purpose="build",
            log_path=self.runtime / "logs" / f"build-{revision[:12]}.log",
            prefer_visible=prefer_visible,
        )
        if result is None:
            return None
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
        check = self.resource_capped_command(
            str(promoted),
            "validate",
            cwd=self.source,
            env=None,
            timeout=180,
            purpose="validation",
            log_path=self.runtime / "logs" / f"validation-{revision[:12]}.log",
            prefer_visible=prefer_visible,
        )
        if check is None:
            promoted.unlink(missing_ok=True)
            return None
        if check.returncode != 0:
            promoted.unlink(missing_ok=True)
            self.event("validation_failed", revision=revision, output=check.stdout[-4000:])
            raise RuntimeError(f"CIVVIS validation failed at {revision[:12]}")
        return promoted

    def activate_build(self, revision: str, promoted: Path) -> None:
        self.binary = promoted
        self.current_revision = revision
        if self.pending_revision == revision:
            self.pending_revision = None
        self.cache_ranking_updater()
        self.initialize_league()
        self.refresh_ranking()
        self.event("build_ready", revision=revision, binary=str(promoted))
        # A build can take longer than the regular sync interval.  Start that
        # interval again only after a validated promotion so the new revision
        # has time to fill the headless fleet before another HEAD transition
        # begins its drain-and-build cycle.
        self.next_sync = time.monotonic() + self.args.sync_interval

    def build(self, revision: str) -> None:
        promoted = self.compile_build(revision)
        if promoted is not None:
            self.activate_build(revision, promoted)

    def start_head_build(self, revision: str) -> None:
        self.build_revision = revision
        self.build_future = self.build_executor.submit(self.compile_build, revision, True)

    def poll_head_build(self, now: float) -> None:
        future = self.build_future
        if future is None or not future.done():
            return
        revision = self.build_revision
        assert revision is not None
        try:
            promoted = future.result()
            if promoted is not None:
                self.activate_build(revision, promoted)
        except (OSError, RuntimeError, subprocess.TimeoutExpired) as error:
            self.event("revision_rejected", error=str(error))
            if self.pending_revision == revision:
                self.pending_revision = None
            self.next_sync = now + self.args.build_retry
        finally:
            self.build_future = None
            self.build_revision = None

    def cache_ranking_updater(self) -> None:
        """Snapshot the self-contained ranking script after a source reset."""
        source_script = self.source / "tools" / "update_ai_player_elo_rankings.py"
        if not source_script.exists():
            return
        temporary = self.ranking_updater.with_suffix(".tmp")
        try:
            shutil.copy2(source_script, temporary)
            os.replace(temporary, self.ranking_updater)
        except OSError as error:
            temporary.unlink(missing_ok=True)
            self.event("ranking_updater_cache_failed", error=str(error))

    def refresh_ranking(self) -> None:
        source_script = self.source / "tools" / "update_ai_player_elo_rankings.py"
        script = self.ranking_updater if self.ranking_updater.exists() else source_script
        if not script.exists() or not (self.league / "league.json").exists():
            return
        result = command(
            sys.executable,
            str(script),
            "--league", str(self.league / "league.json"),
            "--output", str(self.ranking),
            cwd=self.runtime,
            timeout=60,
        )
        if result.returncode != 0:
            self.event("ranking_refresh_failed", output=result.stdout[-2000:])

    def launch(self, *, visible: bool, seed: int | None = None) -> bool:
        assert self.binary is not None and self.current_revision is not None
        used = {game.port for game in self.games}
        port = game_port(self.args.port, used, visible=visible)
        if port is None:
            return False
        seed = self.next_seed if seed is None else seed
        if seed == self.next_seed:
            self.next_seed += 1
        kind = "visible" if visible else "headless"
        focus_strategy = None if visible else self.next_focus_strategy()
        log = self.logs / f"{kind}-{seed}-{self.current_revision[:12]}.log"
        handle = log.open("w", encoding="utf-8")
        speed = getattr(self.args, "speed", DEFAULT_SPEED)
        turns = getattr(self.args, "turns", SPEED_TURNS[speed])
        environment = os.environ.copy()
        environment["CIVVIS_COMMIT"] = self.current_revision
        process = subprocess.Popen(
            game_command(
                self.binary,
                self.league,
                seed,
                port,
                visible=visible,
                open_browser=visible and not self.visible_browser_opened,
                speed=speed,
                turns=turns,
                focus_strategy=focus_strategy,
            ),
            cwd=self.binary.parent,
            env=environment,
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
            focus_strategy=focus_strategy,
        )
        self.games.append(game)
        if visible:
            self.visible_started = True
            self.visible_browser_opened = True
        self.event(
            "game_started",
            game_kind=kind,
            seed=seed,
            pid=process.pid,
            port=port,
            revision=self.current_revision,
            focus_strategy=focus_strategy,
        )
        return True

    def record_outcome(
        self,
        game: GameProcess,
        *,
        failed: bool,
        reason: str,
        row: dict[str, str] | None,
    ) -> None:
        status = game.last_status
        if failed:
            self.failed += 1
        else:
            self.completed += 1
            if game.visible:
                self.visible_completed = True
                self.visible_completed_count += 1
        self.event(
            "game_failed" if failed else "game_completed",
            game_kind="visible" if game.visible else "headless",
            seed=game.seed,
            revision=game.revision,
            reason=reason,
            turn=row.get("turns") if row else status.get("turn"),
            winner=status.get("winner") if row is None else None,
            winner_placement=winner_placement(row),
            # The rated row already carries the engine's own denotation, which
            # for a Mercy Rule ending names the lane it ended on. An unrated
            # game has no row, so fall back to the same label off `/status`
            # rather than to the bare victory type.
            victory=row.get("victory")
            if row
            else (status.get("victory_label") or status.get("victory_type")),
            match_row=row,
            elapsed_seconds=round(time.monotonic() - game.started_monotonic, 1),
            log=game.log,
        )

    def finish(self, game: GameProcess, *, failed: bool, reason: str) -> None:
        row = match_row(self.league, game.seed)
        # ``matches.csv`` is the authoritative, atomically recorded outcome.
        # A shutdown can arrive during the brief result-hold window after that
        # row has appeared, so never turn a rated result into a failure merely
        # because its process is being stopped.
        if row is not None and failed:
            failed = False
            reason = "rated result recorded before process stopped"
        stop_process(game.process)
        if game in self.games:
            self.games.remove(game)
        self.record_outcome(game, failed=failed, reason=reason, row=row)

    def adopt_visible_successor(
        self,
        game: GameProcess,
        *,
        seed: int,
        row: dict[str, str],
        status: dict[str, Any] | None,
    ) -> None:
        """Track CIVVIS's automatic successor without stopping its server."""
        previous_seed = game.seed
        self.record_outcome(
            game,
            failed=False,
            reason="rated result recorded; automatic successor reused",
            row=row,
        )
        game.seed = seed
        game.started_monotonic = time.monotonic()
        game.started_utc = utc_now()
        game.paused = False
        game.ready = True
        game.winner_seen = None
        game.last_status = status or {}
        if seed == self.next_seed:
            self.next_seed += 1
        self.event(
            "game_started",
            game_kind="visible",
            seed=seed,
            pid=game.process.pid,
            port=game.port,
            revision=game.revision,
            predecessor_seed=previous_seed,
            reused_process=True,
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
            row = match_row(self.league, game.seed)
            if row is not None:
                if game.winner_seen is None:
                    game.winner_seen = now
                if game.visible and game.revision == self.current_revision:
                    runtime = http_json(game.port, "/runtime")
                    successor_seed = runtime.get("seed") if runtime else None
                    if isinstance(successor_seed, int) and successor_seed != game.seed:
                        self.adopt_visible_successor(
                            game,
                            seed=successor_seed,
                            row=row,
                            status=http_json(game.port, "/status"),
                        )
                        changed = True
                        continue
                # At a revision boundary, release the old visible binary as
                # soon as its result is durable.  Otherwise allow the server's
                # built-in ten-second countdown time to start its successor.
                hold = (
                    RESULT_HOLD_SECONDS + VISIBLE_SUCCESSOR_GRACE_SECONDS
                    if game.visible and game.revision == self.current_revision
                    else 0.5
                )
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
            paused = [game for game in self.games if game.paused]
            candidate = paused[0] if paused else None
            if paused and (self.pending_revision or self.build_future is not None):
                # A bursty host may expose only one restart window at a time.
                # Alternating classes prevents the always-first visible game
                # from starving the old headless fleet that must drain before
                # the pending HEAD can build, without starving the spectator
                # while that transition is waiting.
                preferred = next(
                    (
                        game
                        for game in paused
                        if game.visible == self.resume_visible_next
                    ),
                    None,
                )
                if preferred is not None:
                    candidate = preferred
            if candidate and set_paused(candidate.process, False):
                candidate.paused = False
                if self.pending_revision or self.build_future is not None:
                    self.resume_visible_next = not candidate.visible
                self.resume_not_before = now + RESUME_STEP_SECONDS
                self.event("game_resumed", seed=candidate.seed, resources=asdict(sample))

    def fill_slots(self, sample: Resources) -> None:
        # Admit only one process per fresh resource sample. Starting the whole
        # pool from one idle reading could cross the ceiling before the next
        # measurement had a chance to govern it.
        # The visible slot is a stronger lifecycle invariant than headless
        # throughput. During a HEAD handoff the old spectator can finish while
        # the promoted binary is already ready; reserve the dedicated port
        # immediately instead of waiting for the normal 20-point recovery
        # margin. If headless work is using that remaining headroom, pause it
        # first. The hard ceiling still wins: an overloaded host gets no new
        # process, but shed headless work immediately so the visible slot can
        # be reserved on the next safe sample.
        if not any(game.visible for game in self.games):
            if sample.overloaded(self.args.limit) or not sample.comfortably_below(
                self.args.limit, margin=RESUME_MARGIN
            ):
                for game in self.games:
                    if game.visible or game.paused:
                        continue
                    if set_paused(game.process, True):
                        game.paused = True
                        self.event(
                            "game_paused_for_resources",
                            seed=game.seed,
                            resources=asdict(sample),
                        )
                if sample.overloaded(self.args.limit):
                    return
            self.launch(visible=True)
            return
        if not sample.comfortably_below(self.args.limit, margin=RESUME_MARGIN):
            return
        # Recovery comes before growth. A process paused by the governor is
        # already consuming a fleet slot and must be resumed before a fresh
        # game can compete with it for the recovered headroom.
        if any(game.paused for game in self.games):
            return
        # "One visible game" is a concurrency invariant, not a lifetime
        # allowance. Replace the spectator match after either completion or a
        # failed process, while still admitting at most one process per sample.
        # A HEAD transition drains only the headless fleet.  The visible slot
        # stays populated while compilation runs in the background, so the
        # stable spectator origin never disappears for an entire build.
        if self.pending_revision or self.build_future is not None:
            return
        headless = sum(not game.visible for game in self.games)
        if headless < self.args.headless and len(self.games) < self.args.max_processes:
            self.launch(visible=False)

    def sync(self) -> None:
        revision = self.fetch()
        self.next_sync = time.monotonic() + self.args.sync_interval
        if revision == self.current_revision:
            # A queued revision can become unnecessary if upstream is rebased
            # or reverted while the old headless fleet is draining.
            self.pending_revision = None
        elif revision != self.pending_revision:
            self.pending_revision = revision
            self.event("head_changed", current=self.current_revision, target=revision)

    def run(self) -> int:
        last_sample = Resources(None, None, 0.0, None, None)
        try:
            self.event(
                "machine_started",
                duration=self.args.duration,
                watch_pid=self.args.watch_pid,
                headless=self.args.headless,
                limit=self.args.limit,
            )
            if self.watched_terminal_closed():
                self.stop_for_terminal_close()
                return 0
            if time.monotonic() >= self.deadline:
                self.stopping = True
                self.event("operator_window_ended", purpose="startup")
                return 0
            self.keep_awake()
            self.ensure_source()
            self.sync()
            assert self.pending_revision is not None
            build_reservation = 100.0 * self.args.build_jobs / max(1, os.cpu_count() or 1)
            if self.wait_for_capacity("initial build", cpu_reservation=build_reservation) is None:
                return 0
            if self.watched_terminal_closed():
                self.stop_for_terminal_close()
                return 0
            self.build(self.pending_revision)
            last_sample = resources(self.runtime)
            while not self.stopping and time.monotonic() < self.deadline:
                if self.watched_terminal_closed():
                    self.stop_for_terminal_close()
                    break
                now = time.monotonic()
                if now >= self.next_sync and self.build_future is None:
                    self.sync()
                changed = self.poll_games()
                last_sample = resources(self.runtime)
                self.govern(last_sample)
                self.poll_head_build(now)
                if now >= self.next_resource_log:
                    append_jsonl(
                        self.resource_log,
                        {"at": utc_now(), **asdict(last_sample), "active": len(self.games)},
                    )
                    self.next_resource_log = now + self.args.resource_log_interval
                    self.persist("resource sample")
                if (
                    self.pending_revision
                    and self.build_future is None
                    and self.games
                    and all(game.visible and game.ready for game in self.games)
                ):
                    build_reservation = (
                        100.0 * self.args.build_jobs / max(1, os.cpu_count() or 1)
                    )
                    if self.capacity_available(
                        last_sample, cpu_reservation=build_reservation
                    ):
                        # Draining a long game can take several minutes. Fetch
                        # once more at the last safe boundary so we compile the
                        # newest HEAD rather than a revision that was current
                        # only when draining began. Re-sample after network
                        # work before reserving build CPU.
                        self.sync()
                        last_sample = resources(self.runtime)
                        self.govern(last_sample)
                        if self.pending_revision and self.capacity_available(
                            last_sample, cpu_reservation=build_reservation
                        ):
                            self.start_head_build(self.pending_revision)
                self.fill_slots(last_sample)
                # The promoted revision refreshes the ranking after the worker
                # finishes.  Avoid executing a source-tree script while that
                # same detached worktree is being reset and compiled.
                if changed and self.build_future is None:
                    self.refresh_ranking()
                if changed:
                    self.persist("game boundary")
                time.sleep(self.args.poll)
        finally:
            self.stopping = True
            for game in list(self.games):
                self.finish(game, failed=True, reason="match machine stopped before result")
            if self.caffeinate is not None:
                stop_process(self.caffeinate, timeout=2)
            if self.build_future is not None:
                self.build_future.cancel()
            self.build_executor.shutdown(wait=True)
            self.refresh_ranking()
            # `stopped` said nothing a reader could act on. The cause and the
            # unspent time are what separate "the window ended, publish it"
            # from "something killed this, restart it".
            unspent = max(0.0, self.deadline - time.monotonic())
            cause = self.stop_cause(unspent)
            self.persist(cause)
            self.event("machine_stopped", completed=self.completed, failed=self.failed,
                       stop_cause=cause, seconds_unspent=round(unspent, 1),
                       resources=asdict(last_sample))
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
    parser.add_argument(
        "--deadline-utc",
        default=None,
        help="absolute ISO-8601 UTC deadline; preserves the operator window across crash restarts",
    )
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
    parser.add_argument(
        "--build-timeout",
        type=float,
        default=1800,
        help="seconds of unpaused build execution; pauses and duty-cycle stops do not age it",
    )
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
    parser.add_argument(
        "--visible-pace",
        type=int,
        default=0,
        help="milliseconds per visible turn (default: zero-delay simulation)",
    )
    parser.add_argument("--port", type=int, default=8870)
    parser.add_argument("--seed", type=int, default=int(time.time()) & 0x7FFF_FFFF)
    args = parser.parse_args(argv)
    if args.deadline_utc is not None:
        try:
            deadline = datetime.fromisoformat(args.deadline_utc.replace("Z", "+00:00"))
        except ValueError:
            parser.error("--deadline-utc must be an ISO-8601 timestamp")
        if deadline.tzinfo is None:
            parser.error("--deadline-utc must include a timezone")
        args.deadline_utc = deadline.astimezone(timezone.utc)
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

    def stop(signum: int, _frame: Any) -> None:
        # ⚠ RECORD *WHICH* SIGNAL. Discarding it is why the league's week-long
        # outage went unnoticed: on 2026-08-15T08:59:11Z this process was
        # SIGTERM'd when the agent session that had launched it in the
        # background ended, and the only trace it left was
        # `reason: "stopped"` beside a `deadline_utc` 17h51m in the future —
        # a record identical to the one a window that ran to term writes.
        # Every other way this loop can end already names itself
        # (`terminal_closed`, `operator_window_ended`, `fatal`); the one that
        # actually happened was the silent one.
        try:
            machine.stop_signal = signal.Signals(signum).name
        except ValueError:  # a signal number Python does not name
            machine.stop_signal = f"signal {signum}"
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
