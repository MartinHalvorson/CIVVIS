#!/usr/bin/env python3
"""Run the CIVVIS strategy league on every machine that happens to be up.

`civvis league` already knows how to share one rating period between many
simulators: rounds are immutable manifests, jobs are claimed atomically, and
finalization is deterministic and idempotent, so two workers that see the same
complete set of results compute byte-identical ratings. What it assumes is a
shared filesystem. A real fleet does not have one — machines come up, go down,
and get rebuilt.

This tool supplies the missing half:

* it **probes** every configured host and quietly skips the ones that are
  down, so a dead machine costs one timeout and nothing else;
* it **deploys** a private detached worktree at `origin/main` and a release
  build to each reachable host, never touching a development checkout;
* it **replicates** the league directory between hosts with rsync, which is
  safe precisely because results are immutable and finalization is
  deterministic;
* it **supervises** a league worker per host, restarting what dies, adopting
  machines the moment they come back;
* and it **audits whether the league is still learning anything**, because a
  self-improving system that cannot notice it has stopped improving is just
  an expensive way to heat a room.

    tools/civvis_fleet.py probe
    tools/civvis_fleet.py deploy
    tools/civvis_fleet.py run --rounds 200
    tools/civvis_fleet.py status

Hosts live in `~/.civvis-fleet.json` (override with `--config`):

    {
      "home": "local",
      "league_dir": "/Users/me/civvis-fleet/league",
      "hosts": [
        {"name": "local", "transport": "local", "root": "/Users/me/civvis-fleet"},
        {"name": "spark", "transport": "ssh", "ssh": "spark",
         "root": "/home/me/civvis-fleet", "jobs": 8}
      ]
    }

With no config file the fleet is just this machine, which is a valid fleet.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import time
from typing import Dict, List, Optional, Sequence, Tuple

REPOSITORY = "https://github.com/MartinHalvorson/CIVVIS.git"
DEFAULT_CONFIG = Path.home() / ".civvis-fleet.json"
DEFAULT_ROOT = Path.home() / "civvis-fleet"
# A host that does not answer this fast is treated as down. It costs the fleet
# one timeout per cycle, never a stall.
PROBE_TIMEOUT = 8
DEPLOY_TIMEOUT = 45 * 60
# Leave a couple of cores for the operator's own machine to stay usable.
RESERVED_CORES = 2


class FleetError(RuntimeError):
    pass


# ---------------------------------------------------------------------------
# Hosts
# ---------------------------------------------------------------------------


@dataclasses.dataclass
class Host:
    name: str
    transport: str = "local"
    ssh: str = ""
    root: str = ""
    jobs: int = 0
    enabled: bool = True

    @property
    def is_local(self) -> bool:
        return self.transport == "local"

    def command(self, script: str) -> List[str]:
        """Wrap `script` so it runs on this host, in a login-ish shell.

        Cargo installs to `~/.cargo/bin`, which is not on a non-interactive
        PATH on either macOS or Linux, so it is added explicitly rather than
        depending on whatever the remote profile happens to do.
        """
        prelude = 'export PATH="$HOME/.cargo/bin:$PATH"; '
        if self.is_local:
            return ["/bin/sh", "-c", prelude + script]
        return [
            "ssh",
            "-o",
            "BatchMode=yes",
            f"-o=ConnectTimeout={PROBE_TIMEOUT}",
            self.ssh or self.name,
            prelude + script,
        ]


@dataclasses.dataclass
class HostStatus:
    host: Host
    reachable: bool = False
    detail: str = ""
    cores: int = 0
    revision: str = ""
    built: bool = False
    load: float = 0.0
    worker_running: bool = False

    @property
    def jobs(self) -> int:
        if self.host.jobs > 0:
            return self.host.jobs
        return max(1, self.cores - RESERVED_CORES)


@dataclasses.dataclass
class FleetConfig:
    hosts: List[Host]
    home: str = "local"
    league_dir: str = ""
    rounds: int = 0
    games: int = 24
    players: int = 6
    turns: int = 250
    seed: int = 1
    pop: int = 12
    evolve_every: int = 4
    # The league leases a claimed game for an hour by default, which is right
    # for a machine that crashed and wrong for a fleet that restarts workers
    # on purpose: a supervised restart mid-round otherwise strands its claims
    # and the round cannot finalize until the hour is up. A 250-turn game
    # takes minutes, so this is still far longer than any game in flight.
    lease_seconds: int = 900

    def host(self, name: str) -> Optional[Host]:
        for host in self.hosts:
            if host.name == name:
                return host
        return None

    @property
    def home_host(self) -> Host:
        host = self.host(self.home)
        if host is None:
            raise FleetError(f"home host {self.home!r} is not in the fleet")
        return host


def default_config() -> FleetConfig:
    """A fleet of one is still a fleet, and needs no configuration file."""
    return FleetConfig(
        hosts=[Host(name="local", transport="local", root=str(DEFAULT_ROOT))],
        home="local",
        league_dir=str(DEFAULT_ROOT / "league"),
    )


def load_config(path: Optional[Path]) -> FleetConfig:
    if path is None:
        path = DEFAULT_CONFIG
    if not path.exists():
        return default_config()
    try:
        raw = json.loads(path.read_text())
    except (OSError, ValueError) as error:
        raise FleetError(f"cannot read fleet config {path}: {error}") from error
    return parse_config(raw)


def parse_config(raw: Dict) -> FleetConfig:
    hosts: List[Host] = []
    for entry in raw.get("hosts", []):
        if not isinstance(entry, dict) or not entry.get("name"):
            raise FleetError(f"each host needs a name: {entry!r}")
        transport = entry.get("transport", "local")
        if transport not in ("local", "ssh"):
            raise FleetError(f"host {entry['name']}: unknown transport {transport!r}")
        hosts.append(
            Host(
                name=str(entry["name"]),
                transport=transport,
                ssh=str(entry.get("ssh", "")),
                root=str(entry.get("root") or DEFAULT_ROOT),
                jobs=int(entry.get("jobs", 0)),
                enabled=bool(entry.get("enabled", True)),
            )
        )
    if not hosts:
        return default_config()
    base = default_config()
    home = str(raw.get("home", hosts[0].name))
    league_dir = str(raw.get("league_dir") or Path(hosts[0].root) / "league")
    return FleetConfig(
        hosts=hosts,
        home=home,
        league_dir=league_dir,
        rounds=int(raw.get("rounds", base.rounds)),
        games=int(raw.get("games", base.games)),
        players=int(raw.get("players", base.players)),
        turns=int(raw.get("turns", base.turns)),
        seed=int(raw.get("seed", base.seed)),
        pop=int(raw.get("pop", base.pop)),
        evolve_every=int(raw.get("evolve_every", base.evolve_every)),
        lease_seconds=max(60, int(raw.get("lease_seconds", base.lease_seconds))),
    )


# ---------------------------------------------------------------------------
# Running things
# ---------------------------------------------------------------------------


def run_on(
    host: Host, script: str, *, timeout: int = PROBE_TIMEOUT, check: bool = False
) -> Tuple[int, str, str]:
    """Run `script` on `host`. Never raises for an unreachable machine."""
    try:
        proc = subprocess.run(
            host.command(script),
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        return 124, "", f"timed out after {timeout}s"
    except OSError as error:
        return 127, "", str(error)
    if check and proc.returncode != 0:
        raise FleetError(
            f"{host.name}: command failed ({proc.returncode}): "
            f"{proc.stderr.strip() or proc.stdout.strip()}"
        )
    return proc.returncode, proc.stdout, proc.stderr


PROBE_SCRIPT = r"""
set -e
root={root}
cores=$( (nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null) | head -1 )
echo "cores=${{cores:-1}}"
load=$(uptime | sed 's/.*averages*: *//' | awk '{{print $1}}' | tr -d ,)
echo "load=${{load:-0}}"
if [ -d "$root/src/.git" ] || [ -f "$root/src/.git" ]; then
  echo "revision=$(git -C "$root/src" rev-parse --short HEAD 2>/dev/null || echo none)"
else
  echo "revision=none"
fi
if [ -x "$root/src/target/release/civvis" ]; then echo "built=yes"; else echo "built=no"; fi
if pgrep -f "civvis league --dir $root" >/dev/null 2>&1; then
  echo "worker=yes"
else
  echo "worker=no"
fi
"""


def probe(host: Host) -> HostStatus:
    status = HostStatus(host=host)
    if not host.enabled:
        status.detail = "disabled in config"
        return status
    code, out, err = run_on(host, PROBE_SCRIPT.format(root=shlex.quote(host.root)))
    if code != 0:
        status.detail = (err or out).strip().splitlines()[-1] if (err or out).strip() else "unreachable"
        return status
    status.reachable = True
    for line in out.splitlines():
        key, _, value = line.partition("=")
        value = value.strip()
        if key == "cores":
            status.cores = _int(value, 1)
        elif key == "load":
            status.load = _float(value, 0.0)
        elif key == "revision":
            status.revision = value
        elif key == "built":
            status.built = value == "yes"
        elif key == "worker":
            status.worker_running = value == "yes"
    return status


def _int(value: str, default: int) -> int:
    try:
        return int(float(value))
    except (TypeError, ValueError):
        return default


def _float(value: str, default: float) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def probe_fleet(cfg: FleetConfig) -> List[HostStatus]:
    return [probe(host) for host in cfg.hosts]


# ---------------------------------------------------------------------------
# Deployment
# ---------------------------------------------------------------------------


def deploy_script(root: str, repository: str) -> str:
    """Put a private detached worktree at `origin/main` on a host and build it.

    This checkout belongs to the fleet. It is never a development checkout,
    so the hard reset here cannot destroy anyone's work — the repository's
    own rules forbid an automated build from mutating a development tree.
    """
    root_q = shlex.quote(root)
    repo_q = shlex.quote(repository)
    return f"""
set -e
mkdir -p {root_q}
if [ ! -d {root_q}/src/.git ]; then
  rm -rf {root_q}/src
  git clone --quiet {repo_q} {root_q}/src
fi
cd {root_q}/src
git fetch --quiet origin main
git checkout --quiet --detach origin/main
git reset --quiet --hard origin/main
echo "revision=$(git rev-parse --short HEAD)"
cargo build --release --locked --quiet
echo "built=yes"
"""


def deploy(host: Host, repository: str = REPOSITORY) -> Tuple[bool, str]:
    code, out, err = run_on(
        host, deploy_script(host.root, repository), timeout=DEPLOY_TIMEOUT
    )
    if code != 0:
        return False, (err or out).strip()[-400:]
    revision = ""
    for line in out.splitlines():
        if line.startswith("revision="):
            revision = line.split("=", 1)[1].strip()
    return True, revision


# ---------------------------------------------------------------------------
# Replication
# ---------------------------------------------------------------------------


def rsync_spec(host: Host, path: str) -> str:
    if host.is_local:
        return path
    return f"{host.ssh or host.name}:{path}"


def replicate(
    source: Host, dest: Host, source_dir: str, dest_dir: str, *, delete: bool = False
) -> Tuple[bool, str]:
    """Copy a league directory between hosts.

    Safe under eventual consistency because of what it is copying: manifests
    and results are immutable and publish exactly once, and finalization is
    deterministic, so two hosts that both see a complete round compute the
    same `league.json`. Nothing here needs a distributed lock; it needs the
    bytes to arrive eventually, which is what rsync guarantees.
    """
    if source.is_local and dest.is_local and source_dir == dest_dir:
        return True, "same directory"
    cmd = ["rsync", "-a", "--partial"]
    if delete:
        cmd.append("--delete")
    cmd += [rsync_spec(source, source_dir.rstrip("/") + "/"), rsync_spec(dest, dest_dir)]
    if not source.is_local and not dest.is_local:
        return False, "host-to-host replication needs a local hop"
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    except (subprocess.TimeoutExpired, OSError) as error:
        return False, str(error)
    if proc.returncode != 0:
        return False, proc.stderr.strip()[-300:]
    return True, ""


# ---------------------------------------------------------------------------
# Workers
# ---------------------------------------------------------------------------


def league_command(cfg: FleetConfig, status: HostStatus) -> str:
    host = status.host
    league_dir = league_dir_for(cfg, host)
    parts = [
        f"{shlex.quote(host.root)}/src/target/release/civvis",
        "league",
        "--dir",
        shlex.quote(league_dir),
        "--worker",
        shlex.quote(host.name),
        "--jobs",
        str(status.jobs),
        "--games",
        str(cfg.games),
        "--players",
        str(cfg.players),
        "--turns",
        str(cfg.turns),
        "--seed",
        str(cfg.seed),
        "--pop",
        str(cfg.pop),
        "--evolve-every",
        str(cfg.evolve_every),
        "--lease-seconds",
        str(cfg.lease_seconds),
        "--rounds",
        str(cfg.rounds if cfg.rounds > 0 else 1),
        "--quiet",
    ]
    return " ".join(parts)


def league_dir_for(cfg: FleetConfig, host: Host) -> str:
    """Where this host keeps its copy of the league."""
    if host.name == cfg.home:
        return cfg.league_dir
    return str(Path(host.root) / "league")


def start_worker(cfg: FleetConfig, status: HostStatus) -> Tuple[bool, str]:
    """Start one detached league worker. Idempotent: a host that already has
    one keeps it, because two workers on one host is exactly what `--jobs`
    is for."""
    if status.worker_running:
        return True, "already running"
    host = status.host
    league_dir = league_dir_for(cfg, host)
    log = f"{host.root}/league-worker.log"
    # All three of the worker's descriptors have to leave the pipe this
    # command is being read through, or the caller blocks until the worker
    # exits — which for a league worker is hours. `&&` would also put the
    # whole compound in the background, holding stdout open the same way.
    script = (
        f"mkdir -p {shlex.quote(host.root)} {shlex.quote(league_dir)}; "
        f"nohup {league_command(cfg, status)} "
        f">> {shlex.quote(log)} 2>&1 </dev/null & "
        f"echo started $!"
    )
    code, out, err = run_on(host, script, timeout=30)
    if code != 0:
        return False, (err or out).strip()[-200:]
    return True, out.strip()


def stop_workers(host: Host) -> Tuple[bool, str]:
    code, out, err = run_on(
        host, f"pkill -f 'civvis league --dir {host.root}' || true", timeout=30
    )
    return code == 0, (err or out).strip()


# ---------------------------------------------------------------------------
# Is the league still learning?
# ---------------------------------------------------------------------------


@dataclasses.dataclass
class Health:
    games: int = 0
    seats: float = 0.0
    information: float = 0.0
    distinct_players: int = 0
    verdict: str = ""
    detail: str = ""

    @property
    def learning(self) -> bool:
        return self.information > 0.0


def audit_league(binary: str, league_dir: str, seats: int = 0) -> Health:
    """Ask `civvis rating` whether the games being played still separate the
    strategies playing them.

    This is the check that matters for a self-improving system. A league whose
    roster has converged keeps producing rounds, ratings, and leaderboards
    while its games decide nothing — the failure is invisible in every artifact
    it publishes and obvious the moment a forecast is scored.
    """
    health = Health()
    cmd = [binary, "rating", "--dir", league_dir, "--backtest"]
    if seats:
        cmd += ["--seats", str(seats)]
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
    except (subprocess.TimeoutExpired, OSError) as error:
        health.verdict = "unknown"
        health.detail = str(error)
        return health
    if proc.returncode != 0:
        health.verdict = "unknown"
        health.detail = (proc.stderr or proc.stdout).strip().splitlines()[-1:] and (
            proc.stderr or proc.stdout
        ).strip().splitlines()[-1] or "rating failed"
        return health
    return parse_health(proc.stdout)


def parse_health(report: str) -> Health:
    """Read the best information/game any candidate system achieved."""
    health = Health()
    best = None
    for line in report.splitlines():
        if "games," in line and "seats on average" in line:
            words = line.replace("(", " ").replace(",", " ").split()
            for i, word in enumerate(words):
                if word == "games" and i:
                    health.games = _int(words[i - 1], 0)
                if word == "seats" and i:
                    health.seats = _float(words[i - 1], 0.0)
        parts = line.split()
        if len(parts) >= 5 and not line.startswith(" "):
            try:
                info = float(parts[-3])
            except ValueError:
                continue
            if "guess" in line or "uniform" in line:
                continue
            best = info if best is None else max(best, info)
    if best is None:
        health.verdict = "unknown"
        health.detail = "no scored rows in the backtest"
        return health
    health.information = best
    if best > 0.05:
        health.verdict = "learning"
        health.detail = "games separate the strategies playing them"
    elif best > 0.0:
        health.verdict = "weak"
        health.detail = (
            "games barely separate the roster; widen it or lengthen the games"
        )
    else:
        health.verdict = "stalled"
        health.detail = (
            "no system beats guessing on these games: the roster has converged, "
            "seating is confounded, or games are ending on the turn cap. "
            "Fix the experiment, not the estimator (see docs/RATING.md)"
        )
    return health


# ---------------------------------------------------------------------------
# Experiments: does this change to a strategy actually make it stronger?
# ---------------------------------------------------------------------------

DEFAULT_WEIGHTS_RE = re.compile(r"impl Default for Weights \{(.*?)\n\}\n", re.S)
GENE_RE = re.compile(r"^\s*(\w+): ([-\d.]+),", re.M)
BOUNDS_RE = re.compile(r"pub fn bounds\(\) -> \[\(f64, f64\); \d+\] \{\s*\[(.*?)\]\s*\}", re.S)
PAIR_RE = re.compile(r"\(\s*([-\d.eE]+)\s*,\s*([-\d.eE]+)\s*\)")


def read_genome_defaults(src_root: str) -> Tuple[Dict[str, float], Dict[str, Tuple[float, float]]]:
    """The shipped `Weights` defaults and their legal ranges, read from source.

    Parsed rather than duplicated so an experiment can never quietly test a
    genome the engine would refuse, and so adding a gene does not silently
    leave this behind.
    """
    text = Path(src_root, "src", "ai.rs").read_text()
    body = DEFAULT_WEIGHTS_RE.search(text)
    if not body:
        raise FleetError(f"cannot find Weights defaults in {src_root}/src/ai.rs")
    defaults = {name: float(value) for name, value in GENE_RE.findall(body.group(1))}
    bounds_body = BOUNDS_RE.search(text)
    if not bounds_body:
        raise FleetError(f"cannot find Weights::bounds in {src_root}/src/ai.rs")
    pairs = [(float(lo), float(hi)) for lo, hi in PAIR_RE.findall(bounds_body.group(1))]
    if len(pairs) != len(defaults):
        raise FleetError(
            f"{len(defaults)} genes but {len(pairs)} bounds — refusing to guess the order"
        )
    return defaults, dict(zip(defaults.keys(), pairs))


def entrant(name: str, username: str, kind: Dict, anchor: bool = False) -> Dict:
    return {
        "name": name,
        "username": username,
        "kind": kind,
        "rating": 1500.0,
        "rd": 350.0,
        "vol": 0.06,
        "games": 0,
        "wins": 0,
        "civ_elo": {},
        "born_round": 0,
        "parents": [],
        "retired": False,
        "anchor": anchor,
    }


def gene_sweep_roster(
    defaults: Dict[str, float], bounds: Dict[str, Tuple[float, float]], genes: Sequence[str]
) -> List[Dict]:
    """A roster that varies one gene at a time around the shipped default.

    Two anchors pin the scale and a `control` carrying the exact shipped
    genome sits beside the built-in `advanced` that uses it — if those two do
    not converge to the same rating, the experiment is measuring noise and
    nothing it says about a gene should be believed.
    """
    roster = [
        entrant("advanced", "JackOfAllTrades", {"Builtin": {"ai": "advanced"}}, anchor=True),
        entrant("basic", "TrainingWheels", {"Builtin": {"ai": "basic"}}, anchor=True),
        entrant("control", "Control", {"Advanced": {"weights": dict(defaults), "target": None}}),
    ]
    for gene in genes:
        if gene not in defaults:
            raise FleetError(f"unknown gene {gene!r}")
        lo, hi = bounds[gene]
        for label, value in (("lo", lo), ("hi", hi)):
            if abs(value - defaults[gene]) < 1e-9:
                continue  # the default already sits on this bound
            weights = dict(defaults)
            weights[gene] = value
            name = f"{gene}_{label}"
            roster.append(
                entrant(name, name[:22], {"Advanced": {"weights": weights, "target": None}})
            )
    return roster


def seed_experiment(league_dir: str, roster: Sequence[Dict]) -> None:
    Path(league_dir).mkdir(parents=True, exist_ok=True)
    path = Path(league_dir, "league.json")
    if path.exists():
        raise FleetError(f"{path} already exists; delete it to start a new experiment")
    path.write_text(
        json.dumps(
            {
                "round": 0,
                "strategies": list(roster),
                "calibration": {"comparisons": 0, "brier_sum": 0.0, "log_loss_sum": 0.0},
            },
            indent=1,
        )
    )


@dataclasses.dataclass
class Variant:
    name: str
    rating: float
    rd: float
    games: int
    wins: int


def read_variants(league_dir: str) -> List[Variant]:
    try:
        data = json.loads(Path(league_dir, "league.json").read_text())
    except (OSError, ValueError):
        return []
    return [
        Variant(
            name=s.get("name", "?"),
            rating=float(s.get("rating", 1500.0)),
            rd=float(s.get("rd", 350.0)),
            games=int(s.get("games", 0)),
            wins=int(s.get("wins", 0)),
        )
        for s in data.get("strategies", [])
    ]


def separation(variants: Sequence[Variant], control: str = "control") -> List[Tuple[Variant, float, str]]:
    """Each variant's gap from the control, and whether it is real.

    A gap is only reported as real when it clears the combined 95% interval of
    both ratings. Anything else is `noise` however suggestive it looks, which
    is the whole point of running the experiment instead of eyeballing a
    leaderboard.
    """
    base = next((v for v in variants if v.name == control), None)
    if base is None:
        return []
    out = []
    for v in variants:
        if v.name == control:
            continue
        gap = v.rating - base.rating
        margin = 1.96 * (v.rd * v.rd + base.rd * base.rd) ** 0.5
        if v.games < 30:
            verdict = "too few games"
        elif abs(gap) > margin:
            verdict = "stronger" if gap > 0 else "weaker"
        else:
            verdict = f"noise (±{margin:.0f})"
        out.append((v, gap, verdict))
    out.sort(key=lambda row: -row[1])
    return out


def league_roster(league_dir: str) -> Tuple[int, int]:
    """(active strategies, round) from a league checkpoint, or (0, 0)."""
    path = Path(league_dir) / "league.json"
    try:
        data = json.loads(path.read_text())
    except (OSError, ValueError):
        return 0, 0
    active = [s for s in data.get("strategies", []) if not s.get("retired")]
    return len(active), int(data.get("round", 0))


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------


def cmd_probe(cfg: FleetConfig, args: argparse.Namespace) -> int:
    statuses = probe_fleet(cfg)
    print(f"{'host':<14}{'state':<12}{'cores':>6}{'jobs':>6}{'load':>7}  {'revision':<10}build  worker")
    up = 0
    for s in statuses:
        if s.reachable:
            up += 1
            print(
                f"{s.host.name:<14}{'up':<12}{s.cores:>6}{s.jobs:>6}{s.load:>7.1f}  "
                f"{s.revision or '-':<10}{'yes' if s.built else 'no':<7}"
                f"{'yes' if s.worker_running else 'no'}"
            )
        else:
            print(f"{s.host.name:<14}{'down':<12}{'':>6}{'':>6}{'':>7}  {s.detail[:48]}")
    total = sum(s.jobs for s in statuses if s.reachable and s.built)
    print(f"\n{up}/{len(statuses)} hosts up, {total} simulator slots ready")
    return 0 if up else 1


def cmd_deploy(cfg: FleetConfig, args: argparse.Namespace) -> int:
    failures = 0
    for status in probe_fleet(cfg):
        if not status.reachable:
            print(f"{status.host.name:<14} skipped ({status.detail[:60]})")
            continue
        print(f"{status.host.name:<14} deploying...", flush=True)
        ok, detail = deploy(status.host, args.repository)
        if ok:
            print(f"{status.host.name:<14} ready at {detail}")
        else:
            failures += 1
            print(f"{status.host.name:<14} FAILED: {detail}")
    return 1 if failures else 0


def cmd_status(cfg: FleetConfig, args: argparse.Namespace) -> int:
    home = cfg.home_host
    binary = args.binary or f"{home.root}/src/target/release/civvis"
    active, rnd = league_roster(cfg.league_dir)
    print(f"league {cfg.league_dir}")
    print(f"  round {rnd}, {active} active strategies")
    health = audit_league(binary, cfg.league_dir, seats=args.seats)
    print(f"  {health.games} games scored, {health.seats:.1f} seats on average")
    print(f"  best information per game: {health.information:+.4f} nats")
    print(f"  verdict: {health.verdict.upper()} — {health.detail}")
    return 0 if health.verdict in ("learning", "weak") else 1


def cmd_run(cfg: FleetConfig, args: argparse.Namespace) -> int:
    """Keep a league worker alive on every host that is up, forever."""
    cfg = dataclasses.replace(cfg, rounds=args.rounds or cfg.rounds)
    home = cfg.home_host
    Path(cfg.league_dir).mkdir(parents=True, exist_ok=True)
    cycle = 0
    deadline = time.time() + args.max_seconds if args.max_seconds else None
    while True:
        cycle += 1
        statuses = probe_fleet(cfg)
        reachable = [s for s in statuses if s.reachable]
        print(
            f"[cycle {cycle}] {len(reachable)}/{len(statuses)} hosts up",
            flush=True,
        )
        for status in statuses:
            host = status.host
            if not status.reachable:
                print(f"  {host.name:<12} down ({status.detail[:50]})", flush=True)
                continue
            if not status.built:
                if args.no_deploy:
                    print(f"  {host.name:<12} not built, --no-deploy set", flush=True)
                    continue
                print(f"  {host.name:<12} building...", flush=True)
                ok, detail = deploy(host, args.repository)
                if not ok:
                    print(f"  {host.name:<12} build FAILED: {detail[:80]}", flush=True)
                    continue
                status.built = True
            if host.name != cfg.home:
                # Give the remote the current league, take back what it played.
                replicate(home, host, cfg.league_dir, league_dir_for(cfg, host))
            ok, detail = start_worker(cfg, status)
            print(f"  {host.name:<12} worker: {detail[:60]}", flush=True)
        # Collect evidence from everyone who has any.
        for status in reachable:
            if status.host.name == cfg.home:
                continue
            ok, detail = replicate(
                status.host, home, league_dir_for(cfg, status.host), cfg.league_dir
            )
            if not ok:
                print(f"  {status.host.name:<12} collect failed: {detail[:60]}", flush=True)
        active, rnd = league_roster(cfg.league_dir)
        print(f"  league at round {rnd}, {active} active strategies", flush=True)
        if args.once:
            return 0
        if deadline and time.time() >= deadline:
            print("reached --max-seconds, stopping", flush=True)
            return 0
        time.sleep(max(10, args.interval))


def cmd_experiment(cfg: FleetConfig, args: argparse.Namespace) -> int:
    """Seed, or read, an experiment that varies one gene at a time.

    Evolution can only find strength in the directions its genome can express.
    Before spending more rounds searching, it is worth knowing which genes move
    a rating at all — the ones that do not are where a self-improving loop
    burns its budget for nothing.
    """
    league_dir = args.dir or str(Path(cfg.league_dir).parent / "experiment")
    if args.report:
        variants = read_variants(league_dir)
        if not variants:
            print(f"no experiment at {league_dir}/league.json", file=sys.stderr)
            return 1
        rows = separation(variants, args.control)
        control = next((v for v in variants if v.name == args.control), None)
        print(f"experiment {league_dir}")
        if control:
            print(
                f"  control {control.rating:.0f} ±{control.rd:.0f} "
                f"over {control.games} games\n"
            )
        print(f"{'variant':<24}{'elo':>8}{'±rd':>6}{'gap':>8}{'games':>7}  verdict")
        real = 0
        for v, gap, verdict in rows:
            if verdict in ("stronger", "weaker"):
                real += 1
            print(
                f"{v.name:<24}{v.rating:>8.0f}{v.rd:>6.0f}{gap:>+8.0f}{v.games:>7}  {verdict}"
            )
        print(f"\n{real} of {len(rows)} variants separate from the control")
        return 0

    src_root = args.src or f"{cfg.home_host.root}/src"
    defaults, bounds = read_genome_defaults(src_root)
    genes = [g.strip() for g in args.genes.split(",") if g.strip()] if args.genes else list(defaults)
    roster = gene_sweep_roster(defaults, bounds, genes)
    seed_experiment(league_dir, roster)
    print(f"seeded {len(roster)} entrants at {league_dir}")
    print(f"  {len(genes)} genes varied to each bound, plus 2 anchors and 1 control")
    print(f"\nrun it with:\n  {cfg.home_host.root}/src/target/release/civvis league \\")
    print(
        f"    --dir {league_dir} --rounds {args.rounds} --games {cfg.games} "
        f"--players {cfg.players} \\\n    --turns {cfg.turns} --evolve-every 0 --quiet"
    )
    print(f"\nthen read it with:\n  civvis_fleet.py experiment --dir {league_dir} --report")
    return 0


def cmd_stop(cfg: FleetConfig, args: argparse.Namespace) -> int:
    for host in cfg.hosts:
        ok, detail = stop_workers(host)
        print(f"{host.name:<14}{'stopped' if ok else 'unreachable'} {detail[:50]}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--config", type=Path, default=None, help="fleet config JSON")
    parser.add_argument("--repository", default=REPOSITORY)
    sub = parser.add_subparsers(dest="command", required=True)

    sub.add_parser("probe", help="report which hosts are up and ready")
    sub.add_parser("deploy", help="check out origin/main and build on every host")
    sub.add_parser("stop", help="stop league workers everywhere")

    run = sub.add_parser("run", help="keep league workers running across the fleet")
    run.add_argument("--rounds", type=int, default=0, help="rounds per worker start")
    run.add_argument("--interval", type=int, default=120, help="seconds between cycles")
    run.add_argument("--once", action="store_true", help="one cycle, then exit")
    run.add_argument("--no-deploy", action="store_true")
    run.add_argument("--max-seconds", type=int, default=0)

    status = sub.add_parser("status", help="is the league still learning anything?")
    status.add_argument("--binary", default="", help="civvis binary to audit with")
    status.add_argument("--seats", type=int, default=0, help="only games of this size")

    exp = sub.add_parser(
        "experiment", help="vary one gene at a time and see which ones move a rating"
    )
    exp.add_argument("--dir", default="", help="experiment league directory")
    exp.add_argument("--genes", default="", help="comma-separated genes (default: all)")
    exp.add_argument("--rounds", type=int, default=40)
    exp.add_argument("--src", default="", help="repo root to read the genome from")
    exp.add_argument("--report", action="store_true", help="read an experiment instead")
    exp.add_argument("--control", default="control")
    return parser


def main(argv: Optional[Sequence[str]] = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        cfg = load_config(args.config)
    except FleetError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    handlers = {
        "probe": cmd_probe,
        "deploy": cmd_deploy,
        "run": cmd_run,
        "status": cmd_status,
        "experiment": cmd_experiment,
        "stop": cmd_stop,
    }
    try:
        return handlers[args.command](cfg, args)
    except FleetError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    except KeyboardInterrupt:
        return 130


if __name__ == "__main__":
    sys.exit(main())
