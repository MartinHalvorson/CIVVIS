"""Carry CIVVIS's decisions into a running Civilization VI game, one turn at a time.

The mod publishes the board to `events.jsonl` and then waits. This reads that
board, asks CIVVIS what to do, and writes the answer into the SQLite file the mod
has ATTACHed. That closes the loop the project had recorded as impossible: see the
`civvis-civ6-inbound-channel-is-sqlite-attach` memory for what is measured dead
(`ModUserData`, `io`, the clipboard getter) and why ATTACH is what survives.

    python3 tools/civ6_brain.py --run-dir ~/civvis-civ6-runs/control/<tag> --mode civvis

⚠ `ready` IS WRITTEN LAST, ALWAYS. The mod polls `ready` to learn that a turn's
orders are complete, so writing it before the rows would let a half-written turn be
actuated. That ordering is the whole synchronisation protocol.

⚠ MODES ARE NOT INTERCHANGEABLE. `--mode stub` exists to prove the plumbing —
channel, actuation, counters — and decides nothing worth defending; it writes one
research order from a fixed list. Any run used to evaluate CIVVIS must be
`--mode civvis`, and the `orders_source` field in the turn record is what proves
which one actually drove the game.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import os
import re
import shutil
import sqlite3
import subprocess
import sys
import threading
import time
from pathlib import Path

from civ6_control.orders import orders_db_path
# One list, three launchers. `civ6_civvis_climb.py` forwards `--victory` to
# `civ6_play.py --civvis-victory`, which forwards it here, which forwards it to
# `civvis_orders --victory`; each restatement of the names was a place for the
# chain to reject a lane the far end supports, and one of them did.
from civ6_play import DEFAULT_CIVVIS_VICTORY as DEFAULT_VICTORY, VICTORY_LANES

DEFAULT_STRATEGY = ""
DEFAULT_GITHUB_REFRESH_SECONDS = 30.0
GIT_SHA = re.compile(r"^[0-9a-f]{40}$")

SCHEMA = """
CREATE TABLE IF NOT EXISTS orders (
    run TEXT NOT NULL, turn INTEGER NOT NULL, seq INTEGER NOT NULL,
    kind TEXT NOT NULL, subject INTEGER, verb TEXT, x INTEGER, y INTEGER,
    frame INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (run, turn, seq)
);
CREATE TABLE IF NOT EXISTS ready (
    run TEXT NOT NULL, turn INTEGER NOT NULL, count INTEGER NOT NULL,
    frame INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (run, turn)
);
"""

# A mid-turn combat frame's rows share the turn's primary key space with the
# opening board's; frame N's rows sit at seq FRAME_SEQ_STRIDE*N + i, so a
# database created before the `frame` column existed (see `migrate_frames`)
# keeps its (run, turn, seq) key and never collides.
FRAME_SEQ_STRIDE = 10_000


def migrate_frames(conn: sqlite3.Connection) -> None:
    """Add the `frame` column to a database that predates combat frames.

    `CREATE TABLE IF NOT EXISTS` leaves an existing table alone, so a resumed
    run's database would lack the column the mod filters on. ALTER TABLE ADD
    COLUMN with a default is the whole migration; the primary keys stay.
    """
    for table in ("orders", "ready"):
        columns = {row[1] for row in conn.execute(f"PRAGMA table_info({table})")}
        if "frame" not in columns:
            conn.execute(f"ALTER TABLE {table} ADD COLUMN frame INTEGER NOT NULL DEFAULT 0")
    conn.commit()

# A fixed, boring sequence whose only job is to prove an order was actuated.
STUB_RESEARCH = ["TECH_ANIMAL_HUSBANDRY", "TECH_MINING", "TECH_BRONZE_WORKING"]


def connect(path: Path) -> sqlite3.Connection:
    conn = sqlite3.connect(str(path), timeout=10)
    # WAL so the game's reader and this writer never block each other. A turn
    # blocked on a lock is a turn the game spends staring at us.
    conn.execute("PRAGMA journal_mode=WAL")
    conn.executescript(SCHEMA)
    conn.commit()
    migrate_frames(conn)
    return conn


def stub_orders(state: dict) -> list[tuple]:
    turn = int(state.get("turn", 0))
    tech = STUB_RESEARCH[turn % len(STUB_RESEARCH)]
    return [("research", None, tech, None, None)]


@dataclass(frozen=True)
class LiveRuntime:
    """One verified GitHub revision ready to take over at a turn boundary."""

    revision: str
    binary: Path
    brain: Path


def board_age_seconds(event: dict, now: datetime | None = None) -> float | None:
    """Seconds between the harness receiving this board (`utc`, stamped by
    `civ6_play.record`) and `now`; None for a board that carries no stamp.

    This is the brain-side half of a slow turn: tail latency plus the decider.
    The other half — how long the mod's `await` polls ran before the board
    line left `Automation.log` — is the gap between the polls' own stamps.
    """
    stamp = event.get("utc")
    if not isinstance(stamp, str) or not stamp:
        return None
    for layout in ("%Y-%m-%dT%H:%M:%S.%fZ", "%Y-%m-%dT%H:%M:%SZ"):
        try:
            received = datetime.strptime(stamp, layout).replace(tzinfo=timezone.utc)
            break
        except ValueError:
            continue
    else:
        return None
    now = now or datetime.now(timezone.utc)
    return max(0.0, (now - received).total_seconds())


def _utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


class GitHubRuntimeUpdater:
    """Build ``origin/main`` beside the live game without blocking its turns.

    The running decider is an executable image, not a path lookup: replacing the
    file on disk cannot update it.  This worker therefore builds GitHub's current
    protected main branch in a dedicated detached worktree and publishes a
    revision-named binary.  The foreground brain consumes that offer only before
    it starts a new turn, where re-execing cannot split an orders transaction.

    A fetch or build failure never replaces ``_ready`` and never touches the active
    executable.  The game keeps using its last verified runtime while the worker
    retries, which is safer than falling back to a half-built GitHub tip.
    """

    def __init__(self, repo: Path, current_revision: str | None,
                 refresh_seconds: float = DEFAULT_GITHUB_REFRESH_SECONDS,
                 cache_root: Path | None = None, command_runner=None) -> None:
        self.repo = repo.resolve()
        self.refresh_seconds = max(1.0, float(refresh_seconds))
        self.cache_root = (cache_root or (
            Path.home() / ".cache" / "civvis" / "live-game-runtime"
        )).expanduser().resolve()
        self.source = self.cache_root / "source"
        self.target = self.cache_root / "target"
        self.published = self.cache_root / "published"
        self._command_runner = command_runner or subprocess.run
        self._current_revision = current_revision
        self._offered_revision: str | None = None
        self._ready: LiveRuntime | None = None
        self._lock = threading.Lock()
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self._last_error = ""

    def _checked(self, command: list[str], cwd: Path,
                 timeout: float = 120.0, env: dict[str, str] | None = None) -> str:
        result = self._command_runner(
            command, cwd=str(cwd), capture_output=True, text=True,
            timeout=timeout, env=env,
        )
        if result.returncode != 0:
            detail = (result.stderr or result.stdout or "no output").strip()
            raise RuntimeError(
                f"{' '.join(command[:4])} failed ({result.returncode}): "
                f"{detail[-500:]}"
            )
        return (result.stdout or "").strip()

    def _prepare_source(self, revision: str) -> None:
        self.cache_root.mkdir(parents=True, exist_ok=True)
        if self.source.exists() and self.source.is_dir() and not any(self.source.iterdir()):
            self.source.rmdir()
        if not self.source.exists():
            self._checked(
                ["git", "-C", str(self.repo), "worktree", "add", "--detach",
                 str(self.source), revision],
                self.repo,
            )
            return
        self._checked(
            ["git", "-C", str(self.source), "rev-parse", "--is-inside-work-tree"],
            self.source,
        )
        dirty = self._checked(
            ["git", "-C", str(self.source), "status", "--porcelain"],
            self.source,
        )
        if dirty:
            raise RuntimeError(
                f"dedicated live-runtime worktree is dirty: {dirty[:300]}"
            )
        self._checked(
            ["git", "-C", str(self.source), "checkout", "--detach", "--quiet",
             revision],
            self.source,
        )

    def _build(self, revision: str) -> LiveRuntime:
        self._prepare_source(revision)
        runtime_dir = self.published / revision
        binary = runtime_dir / "civvis_orders"
        brain = self.source / "tools" / "civ6_brain.py"
        if not brain.is_file():
            raise RuntimeError(f"GitHub runtime has no decision worker at {brain}")
        if not binary.is_file() or binary.stat().st_size == 0:
            env = os.environ.copy()
            env["CARGO_TARGET_DIR"] = str(self.target)
            env["GIT_TERMINAL_PROMPT"] = "0"
            self._checked(
                ["cargo", "build", "--release", "--locked", "--bin",
                 "civvis_orders"],
                self.source,
                timeout=1800.0,
                env=env,
            )
            built = self.target / "release" / "civvis_orders"
            if not built.is_file() or built.stat().st_size == 0:
                raise RuntimeError(f"cargo reported success but {built} is absent")
            runtime_dir.mkdir(parents=True, exist_ok=True)
            temporary = runtime_dir / f".civvis_orders.{os.getpid()}.tmp"
            shutil.copy2(built, temporary)
            temporary.chmod(temporary.stat().st_mode | 0o111)
            os.replace(temporary, binary)
        return LiveRuntime(revision=revision, binary=binary, brain=brain)

    def refresh_once(self) -> LiveRuntime | None:
        """Fetch and offer a newer verified main revision, synchronously."""
        env = os.environ.copy()
        env["GIT_TERMINAL_PROMPT"] = "0"
        self._checked(
            ["git", "-C", str(self.repo), "-c", "gc.auto=0", "fetch",
             "--quiet", "origin", "main"],
            self.repo,
            env=env,
        )
        revision = self._checked(
            ["git", "-C", str(self.repo), "rev-parse", "origin/main"],
            self.repo,
        ).lower()
        if not GIT_SHA.fullmatch(revision):
            raise RuntimeError(f"origin/main did not resolve to a full Git SHA: {revision!r}")
        with self._lock:
            if revision in {self._current_revision, self._offered_revision}:
                return None
        runtime = self._build(revision)
        with self._lock:
            self._ready = runtime
            self._offered_revision = revision
        return runtime

    def _worker(self) -> None:
        while not self._stop.is_set():
            try:
                self.refresh_once()
                self._last_error = ""
            except Exception as exc:  # a failed refresh must not stop live decisions
                detail = str(exc)
                if detail != self._last_error:
                    print(f"[brain] GitHub runtime refresh failed; keeping the "
                          f"verified runtime: {detail}", flush=True)
                    self._last_error = detail
            self._write_heartbeat()
            self._stop.wait(self.refresh_seconds)

    def start(self) -> None:
        if self._thread is not None and self._thread.is_alive():
            return
        self._stop.clear()
        self._thread = threading.Thread(
            target=self._worker, name="civvis-github-runtime", daemon=True
        )
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        if self._thread is not None:
            self._thread.join(timeout=5.0)

    def heartbeat_path(self) -> Path:
        return self.cache_root / "heartbeat.json"

    def _write_heartbeat(self) -> None:
        """One small file that says the watcher is alive and what it believes.

        A refresh failure used to be a print line in a log nobody tails; a
        fetch or build that kept failing left the game silently playing old
        code with nothing to alarm on. The heartbeat is written every cycle,
        success or failure, so `civ6_ladder.py check` can floor its age and
        surface its last error — the same make-silence-loud contract as the
        ladder's own staleness check.
        """
        try:
            self.cache_root.mkdir(parents=True, exist_ok=True)
            with self._lock:
                payload = {
                    "utc": _utc_stamp(),
                    "current_revision": self._current_revision,
                    "offered_revision": self._offered_revision,
                    "last_error": self._last_error,
                }
            temporary = self.cache_root / f".heartbeat.{os.getpid()}.tmp"
            temporary.write_text(json.dumps(payload, sort_keys=True) + "\n")
            os.replace(temporary, self.heartbeat_path())
        except OSError:
            pass  # the heartbeat must never hurt the game it watches

    def take_ready(self) -> LiveRuntime | None:
        with self._lock:
            runtime = self._ready
            self._ready = None
            if runtime is not None:
                self._current_revision = runtime.revision
                self._offered_revision = None
            return runtime


def write_disabled_heartbeat(cache_root: Path | None = None,
                             revision: str | None = None) -> None:
    """Stamp the runtime heartbeat when the GitHub watcher is deliberately off.

    The live loop launches with ``--github-refresh-seconds 0``: the game plays
    exactly the binary the supervisor built for this cycle, and no updater
    thread runs. Without this stamp the last enabled run's heartbeat stayed
    frozen on disk (2026-08-19) and ``civ6_ladder.py check`` alarmed on its age
    forever — an alarm that always fires cannot catch real staleness. The
    stamp says the silence is chosen, so the check can hold its fire while
    keeping the missing-file and erroring cases loud. Staleness of the binary
    itself is then the supervisor's per-cycle checkout's contract, not this
    watcher's.
    """
    root = (cache_root or (
        Path.home() / ".cache" / "civvis" / "live-game-runtime"
    )).expanduser()
    try:
        root.mkdir(parents=True, exist_ok=True)
        payload = {
            "utc": _utc_stamp(),
            "refresh": "disabled",
            "current_revision": revision,
            "last_error": "",
        }
        temporary = root / f".heartbeat.{os.getpid()}.tmp"
        temporary.write_text(json.dumps(payload, sort_keys=True) + "\n")
        os.replace(temporary, root / "heartbeat.json")
    except OSError:
        pass  # the heartbeat must never hurt the game it watches


def civvis_orders(binary: Path, run_dir: Path, turn: int, victory: str,
                  strategy: str | None = None, civ: str | None = None,
                  without: list[str] | None = None,
                  with_: list[str] | None = None) -> list[tuple]:
    """Ask CIVVIS. Its stdout is a JSON array of orders; anything else is an error.

    ⚠ A non-zero exit or unparseable stdout returns NO orders rather than a guess.
    The mod then falls back and records `fallback`, which is visible — inventing
    orders here would put my heuristics back in the game under CIVVIS's name.
    """
    try:
        command = [str(binary), "--mirror", str(run_dir), "--turn", str(turn),
                   "--victory", victory]
        if strategy:
            command.extend(["--strategy", strategy])
        if civ:
            command.extend(["--civ", civ])
        for treatment in with_ or []:
            command.extend(["--with", treatment])
        for treatment in without or []:
            command.extend(["--without", treatment])
        proc = subprocess.run(
            command,
            capture_output=True, text=True, timeout=60,
        )
    except (subprocess.SubprocessError, OSError) as exc:
        print(f"[brain] civvis-orders failed to run: {exc}", flush=True)
        return []
    if proc.returncode != 0:
        print(f"[brain] civvis-orders exit {proc.returncode}: "
              f"{proc.stderr.strip()[:300]}", flush=True)
        return []
    try:
        payload = json.loads(proc.stdout)
    except ValueError:
        print(f"[brain] civvis-orders stdout not JSON: "
              f"{proc.stdout.strip()[:200]}", flush=True)
        return []
    rows: list[tuple] = []
    for order in payload.get("orders", []):
        rows.append((
            str(order.get("kind", "")),
            order.get("subject"),
            order.get("verb"),
            order.get("x"),
            order.get("y"),
        ))
    if payload.get("note"):
        print(f"[brain] civvis: {payload['note']}", flush=True)
    return rows


def filter_orders(rows: list[tuple], skip_kinds: set[str], skip_verbs: set[str],
                  one_per_unit: bool) -> list[tuple]:
    """Drop or thin CIVVIS's orders, for bisecting a crash — not for normal play.

    ⚠ EVERY USE OF THIS MAKES THE RUN A WORSE MEASUREMENT OF CIVVIS. It exists
    because the game dies with an identical stack at turn 37 on different maps, and
    the only way to find which order does it is to remove candidates one at a time.
    A run that used any of these is a bisect, not an attempt.

    `one_per_unit` keeps a unit's LAST positional order. CIVVIS legitimately moves a
    unit in several steps within one turn, but each step is a separate
    `RequestOperation` on a unit the previous step may have killed — a melee move
    onto a defended plot IS the attack.
    """
    kept: list[tuple] = []
    for kind, subject, verb, x, y in rows:
        if kind in skip_kinds or (verb or "") in skip_verbs:
            continue
        kept.append((kind, subject, verb, x, y))
    if not one_per_unit:
        return kept
    last_positional: dict[int, int] = {}
    for index, (kind, subject, verb, x, y) in enumerate(kept):
        if kind == "unit" and x is not None:
            last_positional[subject] = index
    out = []
    for index, row in enumerate(kept):
        kind, subject, verb, x, y = row
        if kind == "unit" and x is not None and last_positional.get(subject) != index:
            continue
        out.append(row)
    return out


def guard_government_orders(state: dict, rows: list[tuple],
                            seen_governments: set[str]) -> tuple[list[tuple], list[str]]:
    """Keep a rebuilt mirror from repeatedly putting the live empire in anarchy.

    The decider's board is fresh each turn, so it cannot remember which Firaxis
    governments this seat has already used.  Firaxis can: returning to an earlier
    government costs anarchy.  The missing history produced a measured live loop:
    Merchant Republic -> an old government -> no current government -> request
    Merchant Republic again, every three turns.  Science and Culture were both zero
    for the final 101 turns of that game.

    The append-only state stream gives the brain the missing history.  A government
    not seen before is still a normal progression and remains legal.  A different
    government already seen in this game is a return switch, so drop it.  Once a
    previously governed seat reports no current government, it is already in the
    transition; drop every government request until Firaxis reports one again rather
    than restarting or redirecting that transition.  The opening choice is preserved:
    an empty current government is allowed while no government has ever been seen.
    """
    current_value = state.get("government")
    current = str(current_value).strip() if current_value is not None else ""
    if current:
        seen_governments.add(current)

    kept: list[tuple] = []
    blocked: list[str] = []
    for row in rows:
        kind, _subject, verb, _x, _y = row
        if kind != "government":
            kept.append(row)
            continue
        target = str(verb or "").strip()
        if not current and seen_governments:
            blocked.append(f"{target or '(unknown)'}: government transition in progress")
            continue
        if current and target and target != current and target in seen_governments:
            blocked.append(f"{target}: return to a previously used government")
            continue
        kept.append(row)
    return kept, blocked


def seat_civ(run_dir: Path) -> str | None:
    """The civilization Civilization VI dealt this seat, as the league names it.

    ★★★★ THE OTHER HALF OF THE OPERATOR'S BRIEF — "the provably highest ELO
    player-strategy CIVVIS has THAT MAPS TO THE CORRECT CIV". `--strategy auto` alone
    answers only the first half and reports `per_civ:false`, because Civ 6 DEALS the
    civ and nothing knew it. The seat event carries it and lands early (line 25 of a
    real run), while the decider starts lazily on the first turn — so by the time it
    is needed it is already on disk.

    Why it is worth passing: the per-civ table changes the pick and RAISES the
    confidence bound where it has history.

        --civ Rome    -> g56-48         per_civ=True   bound=0.510
        --civ China   -> adv-religious  per_civ=False  bound=0.410   (falls back)
        --civ Egypt   -> adv-religious  per_civ=False  bound=0.410
        --civ Greece  -> adv-religious  per_civ=False  bound=0.410

    ⚠ The league rates only FOUR civs, so most deals fall back to the overall pick —
    which is correct, not a failure. `resolve_strategy` narrows only where that pair
    has history, so a civ it has never seen degrades to exactly today's behaviour.

    ⚠ AND THIS PARTLY ANSWERS A CONFOUND IN #752. `adv-religious` — what `auto` picks
    overall — has 116 games and **zero** per-civ pairs, while `advanced` and `g20-21`
    have all four. The strategies were not rated on the same civ pool, so the headline
    "50.0% against 27.5%" is not a like-for-like comparison. Narrowing by civ compares
    within one pool, which is the stronger claim available.

    Name mapping is deliberately dumb: strip `CIVILIZATION_` and title-case, which is
    exact for the four rated civs (Rome, China, Egypt, Greece). A wrong guess costs
    nothing — the decider finds no history and falls back.
    """
    events = run_dir / "events.jsonl"
    if not events.exists():
        return None
    for line in events.read_text(errors="replace").splitlines():
        if '"seat"' not in line:
            continue
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if event.get("kind") != "seat":
            continue
        civ = event.get("civ") or ""
        if civ.startswith("CIVILIZATION_"):
            return civ[len("CIVILIZATION_"):].title()
        return civ or None
    return None


class Decider:
    """A long-lived `civvis-orders --serve --fresh-board` process.

    ★★★★★ WHY A SERVER RATHER THAN ONE INVOCATION PER TURN. Spawning the binary each
    turn gives CIVVIS a brand-new agent that has never seen this world, so its
    strategic plan — grand strategy, war target, city target — is re-derived from
    scratch every turn and never matures. Measured cost: units huddled within 7 tiles
    of the capital for a whole game, `met` stopped at 2, no rival city was ever seen,
    and a settler oscillated between two tiles for twenty turns.

    Keeping the AGENT alive fixes that. The BOARD is still rebuilt every turn
    (`--fresh-board`), because `Ai::take_turn` needs a turn that has advanced through
    the engine's own private `begin_turn`; reusing the board returns zero orders. That
    combination — fresh board, persistent agent — is the only one of the four that
    both works and carries a plan.

    ⚠ If the process dies the brain falls back to one invocation per turn, so a crash
    costs plan continuity rather than the run. `orders_source` still reads `civvis`
    either way, so the note records which mode answered.
    """

    def __init__(self, binary: Path, run_dir: Path, victory: str,
                 war_from_plan: bool = False, strategy: str | None = None,
                 without: list[str] | None = None, with_: list[str] | None = None):
        self.binary = binary
        self.run_dir = run_dir
        self.victory = victory
        # See the `--strategy` note in main(). Empty means the built-in AdvancedAi,
        # which is what every run before this used without anyone choosing it.
        self.strategy = strategy
        # ⚠ Declares war when CIVVIS's PLAN names a target but its own diplomatic
        # bookkeeping cannot fire. That bookkeeping wants a casus belli, or a
        # denouncement matured over five turns, and NOTHING matures in a board rebuilt
        # each turn — measured: 81 replayed turns, `strategy = conquest` on 26 of them,
        # ZERO declarations. So the decline is an artefact of the reconstruction
        # rather than a judgement about the war.
        self.war_from_plan = war_from_plan
        # The live ledger's comparisons need both a control (`--without`) and
        # a deliberately labelled restoration of one ledger-held row
        # (`--with`). Keep both lists on the persistent process, or an arm
        # would work only on the first non-server turn and quietly disappear
        # under the normal live controller.
        self.without = list(without or [])
        self.with_ = list(with_ or [])
        self.civ: str | None = None
        self.proc: subprocess.Popen | None = None
        self.why = None

    def command(self) -> list[str]:
        """The precise decider invocation for the current seat identity."""
        command = [str(self.binary), "--mirror", str(self.run_dir), "--serve",
                   "--fresh-board", "--explain", "--victory", self.victory]
        if self.strategy:
            command.extend(["--strategy", self.strategy])
        if self.civ:
            command.extend(["--civ", self.civ])
        if self.war_from_plan:
            command.append("--war-from-plan")
        for treatment in self.with_:
            command.extend(["--with", treatment])
        for treatment in self.without:
            command.extend(["--without", treatment])
        return command

    def set_civ(self, civ: object) -> None:
        """Restart before a decision if the run tells us which civ the seat received."""
        value = str(civ).strip() if civ is not None else ""
        value = value or None
        if value == self.civ:
            return
        # A genome is selected at process startup.  Do not let a generic process
        # answer a turn after the seat event gave us the actual civilization.
        self.stop()
        self.civ = value

    def start(self) -> None:
        # ★★★★ KEEP CIVVIS'S REASONING. This used to send the decider's stderr to
        # DEVNULL, so a live run recorded WHAT was ordered and never WHY — and the two
        # questions this project keeps having to answer are "did it choose that" and
        # "did it ever reach the question", which only the journal separates. Every
        # diagnosis tonight came from replaying turns with `--explain` after the fact;
        # this makes the same account available for the run as it happens, including
        # the decider's own crash output, which DEVNULL was also swallowing.
        self.why = (self.run_dir / "why.log").open("a", buffering=1)
        try:
            self.proc = subprocess.Popen(
                self.command(),
                stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=self.why, text=True, bufsize=1,
            )
        except Exception:
            self.why.close()
            self.why = None
            raise
        print("[brain] decider server up (fresh board, persistent agent, "
              f"strategy={self.strategy or 'stock'} civ={self.civ or 'unknown'}, "
              f"explaining into {self.run_dir / 'why.log'})", flush=True)

    def ask(self, turn: int) -> tuple[list[tuple], str]:
        if self.proc is None or self.proc.poll() is not None:
            self.start()
        assert self.proc is not None and self.proc.stdin and self.proc.stdout
        try:
            self.proc.stdin.write(f"{turn}\n")
            self.proc.stdin.flush()
            line = self.proc.stdout.readline()
        except (OSError, ValueError) as exc:
            print(f"[brain] decider died mid-turn: {exc}", flush=True)
            self.proc = None
            return [], "decider died"
        if not line:
            print("[brain] decider closed its output", flush=True)
            self.proc = None
            return [], "decider closed"
        try:
            payload = json.loads(line)
        except ValueError:
            return [], f"unparseable: {line.strip()[:120]}"
        # ★★★★★ A LINE THAT IS NOT A RESPONSE MUST NOT BE READ AS AN EMPTY ONE.
        #
        # `--serve` is one line in, one line out, and this used to trust that
        # absolutely: any JSON object was accepted and `payload.get("orders", [])`
        # turned one without that key into "CIVVIS chose nothing". A single stray
        # println in the decider therefore shifted every turn by one and read as a
        # silent, total abdication -- the run kept going, reported
        # `orders_source: "fallback"`, and the hand-written ladder played the game.
        # That happened: the genome report went to stdout, and a run that had been
        # 236 turns of CIVVIS flipped the moment the new binary was swapped in.
        #
        # So a line without `orders` is skipped and LOGGED, and the real response is
        # read behind it. Recursion depth is bounded by the fact that the decider
        # emits one response per request; a decider that only ever emitted noise would
        # block on `readline` instead, which is a visible hang rather than a quiet
        # wrong answer.
        if "orders" not in payload:
            print(f"[brain] IGNORING non-response line on the decider's stdout: "
                  f"{line.strip()[:160]}", flush=True)
            return self.ask(turn)
        rows = [
            (str(o.get("kind", "")), o.get("subject"), o.get("verb"),
             o.get("x"), o.get("y"))
            for o in payload.get("orders", [])
        ]
        return rows, str(payload.get("note", ""))

    def stop(self) -> None:
        if self.proc is not None and self.proc.poll() is None:
            try:
                if self.proc.stdin:
                    self.proc.stdin.close()
                self.proc.wait(timeout=10)
            except (subprocess.SubprocessError, OSError):
                self.proc.kill()
        self.proc = None
        if getattr(self, "why", None) is not None:
            self.why.close()
            self.why = None

    def use_runtime(self, binary: Path) -> bool:
        """Move the next decision to ``binary`` without splitting this turn."""
        binary = binary.resolve()
        if binary == self.binary.resolve():
            return False
        self.stop()
        self.binary = binary
        return True


def write_turn(conn: sqlite3.Connection, run: str, turn: int,
               rows: list[tuple], frame: int = 0) -> int:
    """Write one answer — the turn's opening board (frame 0) or a mid-turn
    combat frame's (frame N) — and signal it complete.

    A frame's rows replace only that frame's; the `ready` row is one per turn
    and names the newest frame answered, which is the one the mod is waiting
    on (an earlier frame's answer was consumed before the next frame opened).
    """
    conn.execute("DELETE FROM orders WHERE run = ? AND turn = ? AND frame = ?",
                 (run, turn, frame))
    base = FRAME_SEQ_STRIDE * frame
    conn.executemany(
        "INSERT OR REPLACE INTO orders (run, turn, seq, kind, subject, verb, x, y, frame) "
        "VALUES (?,?,?,?,?,?,?,?,?)",
        [(run, turn, base + i, k, s, v, x, y, frame)
         for i, (k, s, v, x, y) in enumerate(rows)],
    )
    conn.commit()
    # LAST, and in its own commit: this is the mod's signal that the turn above is
    # complete. Any other order lets a partial turn be actuated.
    conn.execute("INSERT OR REPLACE INTO ready (run, turn, count, frame) VALUES (?,?,?,?)",
                 (run, turn, len(rows), frame))
    conn.commit()
    return len(rows)


def completed_turns(conn: sqlite3.Connection, run: str) -> set[int]:
    """Turns whose complete order batch was durably signalled to the game."""
    return {int(turn) for (turn,) in conn.execute(
        "SELECT turn FROM ready WHERE run = ?", (run,)
    )}


def completed_game_turns(events: Path, run: str) -> set[int]:
    """Recover turns the game has already completed from its append-only journal.

    ``ready`` is the normal restart checkpoint.  The game's ``turn`` record is a
    second, narrower recovery checkpoint: it is emitted only after a turn has
    been actuated and ended.  If an operator replaces the SQLite database while
    the game remains open, replaying every old ``state`` would rewrite history
    before reaching the live turn.  These records let a new brain skip only turns
    the game itself proves are already over.
    """
    done: set[int] = set()
    try:
        with events.open("r", errors="replace") as handle:
            for raw in handle:
                try:
                    event = json.loads(raw)
                except ValueError:
                    continue
                if event.get("kind") != "turn" or event.get("run") != run:
                    continue
                try:
                    turn = int(event.get("turn"))
                except (TypeError, ValueError):
                    continue
                if turn >= 0:
                    done.add(turn)
    except OSError:
        pass
    return done



def record_note(run_dir: Path, turn: int, note: str) -> None:
    """Append CIVVIS's per-turn diagnostic to a durable file beside the events.

    ⚠ A SEPARATE FILE, not `events.jsonl`. That file is written by the log tail that
    follows Civilization VI's own output; a second writer would interleave partial
    lines into it. `civvis_notes.jsonl` sits in the same run directory and is read
    the same way.

    Failures are swallowed deliberately: a diagnostic that can stall the turn loop is
    worse than no diagnostic.
    """
    try:
        line = json.dumps({"kind": "civvis_note", "turn": turn, "note": note})
        with (run_dir / "civvis_notes.jsonl").open("a") as handle:
            handle.write(line + "\n")
    except OSError:
        pass


def local_revision(repo: Path) -> str | None:
    """Return this launcher's exact source revision without trusting git errors."""
    try:
        result = subprocess.run(
            ["git", "-C", str(repo), "rev-parse", "HEAD"],
            capture_output=True, text=True, timeout=10,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    revision = (result.stdout or "").strip().lower()
    return revision if result.returncode == 0 and GIT_SHA.fullmatch(revision) else None


def binary_sha256(binary: Path) -> str | None:
    """Return the digest of the executable image that will make decisions."""
    try:
        digest = hashlib.sha256()
        with binary.expanduser().resolve().open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
        return digest.hexdigest()
    except (OSError, ValueError):
        return None


def binary_provenance(binary: Path) -> tuple[str | None, str]:
    """Find the source revision, if any, that produced ``binary``.

    A supplied binary may come from a different worktree than this Python
    bridge.  Looking up ``__file__`` in that case stamped the bridge checkout
    onto a game played by the other checkout.  Prefer the nearest Git worktree
    around the executable, then recognize the revision-named binaries emitted
    by ``GitHubRuntimeUpdater``.  The digest remains the identity when neither
    convention can prove a source revision.
    """
    try:
        resolved = binary.expanduser().resolve()
    except OSError:
        resolved = binary.expanduser()

    published = resolved.parent
    if (GIT_SHA.fullmatch(published.name)
            and published.parent.name == "published"):
        return published.name, "published-path"

    candidate = resolved.parent
    while True:
        if (candidate / ".git").exists():
            revision = local_revision(candidate)
            return (revision,
                    "binary-checkout" if revision
                    else "binary-checkout-unverified")
        if candidate == candidate.parent:
            break
        candidate = candidate.parent
    return None, "unverified-binary"


def launch_provenance(binary: Path | None, requested_revision: str | None,
                      launcher_repo: Path) -> tuple[str | None, str]:
    """Choose the revision/source to stamp when this brain opens a run."""
    if requested_revision:
        return requested_revision, "runtime-argument"
    if binary is not None:
        return binary_provenance(binary)
    return local_revision(launcher_repo), "launcher-checkout"


def record_runtime_event(run_dir: Path, status: str, turn: int | None,
                         from_revision: str | None, runtime: LiveRuntime,
                         detail: str | None = None,
                         source: str = "origin/main") -> None:
    """Durably name every mid-game GitHub handoff and any failed re-exec."""
    binary_revision, binary_source = binary_provenance(runtime.binary)
    payload = {
        "kind": "runtime_update",
        "status": status,
        "utc": _utc_stamp(),
        "turn": turn,
        "from_revision": from_revision,
        "to_revision": runtime.revision,
        "source": source,
        "binary": str(runtime.binary),
        "binary_revision": binary_revision,
        "binary_source": binary_source,
        "binary_sha256": binary_sha256(runtime.binary),
    }
    if detail:
        payload["detail"] = detail
    try:
        with (run_dir / "runtime_updates.jsonl").open("a") as handle:
            handle.write(json.dumps(payload, sort_keys=True) + "\n")
    except OSError:
        pass


def _replace_cli_option(arguments: list[str], option: str, value: str) -> None:
    for index, argument in enumerate(arguments):
        if argument == option:
            if index + 1 < len(arguments):
                arguments[index + 1] = value
            else:
                arguments.append(value)
            return
        if argument.startswith(option + "="):
            arguments[index] = f"{option}={value}"
            return
    arguments.extend([option, value])


def runtime_exec_command(runtime: LiveRuntime, argv: list[str]) -> list[str]:
    """Recreate this brain's invocation from the newly fetched source tree."""
    arguments = list(argv[1:])
    _replace_cli_option(arguments, "--bin", str(runtime.binary))
    _replace_cli_option(arguments, "--runtime-revision", runtime.revision)
    return [sys.executable, str(runtime.brain), *arguments]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run-dir", required=True)
    ap.add_argument("--orders-db", default=None,
                    help="SQLite path shared with the live game; defaults to "
                         "<run-dir>/orders.sqlite")
    ap.add_argument("--mode", choices=["stub", "civvis"], default="civvis")
    ap.add_argument("--bin", default=None,
                    help="path to the civvis-orders binary (--mode civvis)")
    ap.add_argument("--runtime-revision", default=None,
                    help=argparse.SUPPRESS)
    ap.add_argument(
        "--github-refresh-seconds", type=float,
        default=DEFAULT_GITHUB_REFRESH_SECONDS,
        help="seconds between background origin/main checks; 0 disables the "
             "turn-boundary live upgrade (default: 30)",
    )
    ap.add_argument("--github-cache", default=None,
                    help="private worktree/build cache for live GitHub updates")
    # ⚠⚠ DOMINATION IS CURRENTLY UNREACHABLE, AND UNTIL 2026-08-14 IT WAS THE
    # DEFAULT.
    #
    # Domination needs a captured capital, and `findWarTarget` needs a rival city
    # plot to be REVEALED before it will target one -- correctly, or the seat would
    # attack a capital it has never seen. But meeting a civilization reveals none of
    # its land, so the revealed gate binds forever. Measured 2026-07-31 across a full
    # day of Settler runs: `met: 1` with `their cities_SEEN: 0` at t125, `met: 0` on
    # two Duel seeds, and zero war declarations in every unforced run.
    #
    # So a run left on this default spends the whole game planning toward a victory
    # whose target set is empty, and every measurement taken from it is a
    # measurement of that, not of how CIVVIS plays. That cost most of a session
    # before anybody noticed the flag.
    #
    # `science` and `score` need no contact at all and are reachable today.
    # `civvis` lets the agent choose. Until reconnaissance can cross water and
    # reveal a city -- see the frontier and probe notes in CivvisControlAgent.lua --
    # prefer one of those and pass `domination` deliberately, not by default.
    #
    # `civvis` is the strongest native controller setting, but its measured edge
    # is mostly Religious Victory (48 of 60 wins at this profile). The Firaxis
    # verification seat is now deliberately trying the strongest REACHABLE
    # non-religious lane instead: Science needs no revealed rival city, while
    # Domination does and recorded zero victories in its 60-game native arm.
    # This pins only our controller's objective; it does not alter the host game's
    # enabled victory conditions, so a rival religious win remains a real loss.
    # Rows from direct-brain runs either side of this change are NOT comparable.
    #
    # ★★★★★ AND THE LIST ITSELF WAS THE BINDING CONSTRAINT, not the default.
    # Culture, Religion and Diplomacy are three of `VictoryTarget`'s six variants
    # and all three are implemented in `advanced.rs`, but this `choices` list
    # named four, so argparse refused them here — between `civ6_play.py`, which
    # forwards the string, and `civvis_orders`, which parses it. Every reading
    # above about "the strongest REACHABLE lane" was taken from a menu that hid
    # half the lanes. The names now come from `civ6_play.VICTORY_LANES`, which
    # `test_civ6_play.py` pins against the Rust enum.
    ap.add_argument("--victory", default=DEFAULT_VICTORY,
                    choices=VICTORY_LANES,
                    help="which victory CIVVIS plays for; `civvis` lets it choose. "
                         "⚠ domination is unreachable while no rival city is ever "
                         "revealed -- see the note above; pass it deliberately, "
                         "never by default")
    # ★★★★ WHICH STRATEGY ACTUALLY PLAYS, which nothing ever chose.
    #
    # `civvis_orders` has taken `--strategy` for a while and NO harness script passed
    # it, so every Civ 6 run has been `AdvancedAi::new` -- the decider's own banner
    # reads `{"strategy":"stock","source":"AdvancedAi::new"}`. The operator's standing
    # brief asks for "whatever the provably highest ELO player-strategy CIVVIS has".
    #
    # `auto` ranks on `league::strategy_strength`, the outright-win LOWER BOUND, not
    # the placement rating -- and the two disagree sharply:
    #
    #     strategy         rating   games  wins   winrate
    #     adv-religious      1601     116    58     50.0%   <- what `auto` picks
    #     advanced           1703     331    91     27.5%   <- what actually played
    #
    # The higher-RATED strategy wins barely half as often. Placement Glicko answers
    # "who should be matched with whom"; it is not a strength ordering.
    #
    # ⚠ TRANSFER TO THIS BRIDGE IS UNMEASURED. Those games are CIVVIS-vs-CIVVIS,
    # every completed local Firaxis run inspected under `auto` lost, and this
    # project has already watched a champion genome go +48 in compact evaluation
    # and -53 deployed. Stock `AdvancedAi` is the deployment-tested controller;
    # an explicit strategy remains available as an experiment, never the default.
    ap.add_argument("--strategy", default=DEFAULT_STRATEGY,
                    help="strategy for the decider to load; empty keeps the "
                         "deployment-tested stock AdvancedAi. `auto` is opt-in")
    ap.add_argument("--skip-kinds", default="",
                    help="comma-separated order kinds to drop (bisect only)")
    ap.add_argument("--skip-verbs", default="",
                    help="comma-separated order verbs to drop (bisect only)")
    ap.add_argument("--one-order-per-unit", action="store_true", default=False,
                    help="keep only a unit's last positional order (bisect only)")
    ap.add_argument("--war-from-plan", action="store_true", default=False,
                    help="declare on CIVVIS's plan target when its own casus-belli "
                         "bookkeeping cannot mature in a rebuilt board")
    ap.add_argument("--without", action="append", default=[], metavar="TREATMENT",
                    help="withhold one live treatment from the decider, "
                         "repeatable — the control arm of a live A/B. Names "
                         "are civvis_orders' own; an unknown one is a hard "
                         "error there rather than a silent no-op")
    ap.add_argument("--with", dest="with_", action="append", default=[],
                    metavar="TREATMENT",
                    help="restore one ledger-held live treatment in a labelled "
                         "verification arm; repeatable and validated by "
                         "civvis_orders")
    ap.add_argument("--server", action="store_true", default=True,
                    help="keep one CIVVIS agent alive across turns (plan continuity)")
    ap.add_argument("--no-server", dest="server", action="store_false",
                    help="spawn civvis-orders per turn; loses plan continuity")
    ap.add_argument("--seconds", type=float, default=7200.0)
    args = ap.parse_args()

    run_dir = Path(args.run_dir).expanduser()
    run_tag = run_dir.name
    events = run_dir / "events.jsonl"
    binary = Path(args.bin).expanduser() if args.bin else None
    if args.mode == "civvis" and (binary is None or not binary.exists()):
        print(f"[brain] --mode civvis needs --bin pointing at civvis-orders "
              f"(got {binary})", file=sys.stderr)
        return 2

    skip_kinds = {k.strip() for k in args.skip_kinds.split(",") if k.strip()}
    skip_verbs = {v.strip() for v in args.skip_verbs.split(",") if v.strip()}
    if skip_kinds or skip_verbs or args.one_order_per_unit:
        print(f"[brain] ⚠ BISECT MODE: skip_kinds={sorted(skip_kinds)} "
              f"skip_verbs={sorted(skip_verbs)} one_per_unit={args.one_order_per_unit}"
              " — this run is not a clean measurement of CIVVIS", flush=True)

    orders_db = orders_db_path(run_dir, args.orders_db)
    conn = connect(orders_db)
    repo_root = Path(__file__).resolve().parent.parent
    runtime_revision, runtime_source = launch_provenance(
        binary, args.runtime_revision, repo_root
    )
    print(f"[brain] mode={args.mode} run={run_tag} db={orders_db} "
          f"decider={'server' if args.server else 'per-turn'} "
          f"revision={runtime_revision or 'unverified'} "
          f"binary_source={runtime_source} "
          f"forced={args.with_ or 'none'} withheld={args.without or 'none'}", flush=True)
    strategy = None if args.strategy.strip().lower() in {"", "stock", "none"} else args.strategy
    decider = (Decider(binary, run_dir, args.victory, args.war_from_plan, strategy,
                       without=args.without, with_=args.with_)
               if args.mode == "civvis" and args.server else None)
    updater = None
    if args.mode == "civvis" and args.github_refresh_seconds > 0:
        updater = GitHubRuntimeUpdater(
            repo=repo_root,
            current_revision=runtime_revision,
            refresh_seconds=args.github_refresh_seconds,
            cache_root=(Path(args.github_cache).expanduser()
                        if args.github_cache else None),
        )
        updater.start()
        print(f"[brain] watching GitHub origin/main every "
              f"{args.github_refresh_seconds:g}s; verified builds publish from "
              f"{updater.cache_root}", flush=True)
    elif args.mode == "civvis":
        # Refresh 0 is a choice, not a death: stamp the heartbeat so the
        # ladder check stops reading the last enabled run's frozen file as
        # "the game may be playing old code". See `write_disabled_heartbeat`.
        write_disabled_heartbeat(
            cache_root=(Path(args.github_cache).expanduser()
                        if args.github_cache else None),
            revision=runtime_revision,
        )
    if args.mode == "civvis" and binary is not None:
        # The first row of runtime_updates.jsonl names the revision the run
        # OPENED on, so a run with zero handoffs still records what code
        # played it and the ledger can carry the whole revision history. A
        # handoff re-execs this brain, whose fresh start writes the next
        # start row with the new revision — consecutive duplicates are the
        # reader's to collapse.
        record_runtime_event(
            run_dir, "start", None, None,
            LiveRuntime(revision=runtime_revision or "unverified",
                        binary=binary, brain=Path(__file__).resolve()),
            source=runtime_source,
        )

    deadline = time.time() + args.seconds
    offset = 0
    # A brain is intentionally time-bounded so an operator can upgrade or
    # restart it during a long game. `ready` is written only after the full batch
    # is committed, which makes it the authoritative resume checkpoint.  When an
    # operator has replaced the DB, completed game records recover old turns so
    # replay cannot rewrite the history before reaching the live state.
    served = completed_turns(conn, run_tag)
    journaled = completed_game_turns(events, run_tag)
    recovered = journaled - served
    served.update(journaled)
    seat_civ: str | None = None
    if recovered:
        print(f"[brain] recovered {len(recovered)} completed turn(s) from the "
              f"game journal after the SQLite checkpoint was absent; "
              f"latest={max(recovered)}", flush=True)
    if served:
        print(f"[brain] resuming after {len(served)} completed turn(s); "
              f"latest={max(served)}", flush=True)
    # Reconstructed by replaying every state in the append-only journal, including
    # already-served turns.  That makes a restarted brain retain the Firaxis history
    # the fresh-board decider necessarily loses.
    seen_governments: set[str] = set()
    # (turn, frame) pairs already answered this session; frames are never
    # recovered across a restart — a frame's board is gone with the turn.
    served_frames: set[tuple[int, int]] = set()
    while time.time() < deadline:
        if not events.exists():
            time.sleep(0.5)
            continue
        with events.open("r", errors="replace") as handle:
            handle.seek(offset)
            fresh = handle.readlines()
            offset = handle.tell()
        for raw in fresh:
            try:
                event = json.loads(raw)
            except ValueError:
                continue
            if event.get("kind") == "seat":
                seat_civ = str(event.get("civ") or "").strip() or None
                if decider is not None:
                    decider.set_civ(seat_civ)
                continue
            if event.get("kind") != "state":
                continue
            current_government = event.get("government")
            if current_government is not None and str(current_government).strip():
                seen_governments.add(str(current_government).strip())
            turn = int(event.get("turn", -1))
            # A mid-turn combat frame re-plans the same turn on a newer board:
            # frame 0 is the opening board and is served once; frame N is
            # served once too, keyed apart, and never counts as the turn's
            # opening answer.
            frame = int(event.get("frame", 0) or 0)
            if turn < 0 or (frame == 0 and turn in served) or (frame > 0 and (turn, frame) in served_frames):
                continue
            runtime = updater.take_ready() if updater is not None else None
            if runtime is not None:
                previous = runtime_revision
                record_runtime_event(
                    run_dir, "handoff", turn, previous, runtime
                )
                print(f"[brain] turn {turn}: GitHub origin/main advanced "
                      f"{(previous or 'unverified')[:12]} -> "
                      f"{runtime.revision[:12]}; restarting the persistent "
                      "decider before this turn", flush=True)
                updater.stop()
                if decider is not None:
                    decider.stop()
                command = runtime_exec_command(runtime, sys.argv)
                try:
                    os.execv(command[0], command)
                except OSError as exc:
                    # The Rust runtime is still useful even if Python could not
                    # re-exec.  Move the next decision to it and keep the game
                    # alive, while recording that the bridge itself stayed old.
                    record_runtime_event(
                        run_dir, "reexec_failed", turn, previous, runtime,
                        str(exc),
                    )
                    print(f"[brain] could not re-exec the GitHub decision worker: "
                          f"{exc}; using its verified Rust runtime in this process",
                          flush=True)
                    binary = runtime.binary
                    runtime_revision = runtime.revision
                    if decider is not None:
                        decider.use_runtime(binary)
                    updater.start()
            if frame == 0:
                served.add(turn)
            else:
                served_frames.add((turn, frame))
            started = time.time()
            if args.mode == "stub":
                rows = stub_orders(event)
            elif decider is not None:
                rows, note = decider.ask(turn)
                if note:
                    print(f"[brain] civvis: {note[:220]}", flush=True)
                    # ★★★★ WRITE IT DOWN. This note is the richest diagnostic in the
                    # pipeline -- it carries `skipped` (actions that had no
                    # counterpart or named a unit the bridge could not map),
                    # `unmapped`, `plan=none`, and how many units could still move --
                    # and it went ONLY to this console. Nothing durable recorded it,
                    # so afterwards there was no way to tell "CIVVIS ordered nothing"
                    # apart from "CIVVIS's order was dropped in translation".
                    #
                    # That gap is why a unit parked for 171 consecutive turns could
                    # not be explained from a finished run.
                    record_note(run_dir, turn, note)
            else:
                rows = civvis_orders(binary, run_dir, turn, args.victory, strategy,
                                     seat_civ, args.without, args.with_)
            rows, government_blocks = guard_government_orders(
                event, rows, seen_governments
            )
            if government_blocks:
                detail = "; ".join(government_blocks)
                print(f"[brain] turn {turn}: blocked government order(s): {detail}",
                      flush=True)
                record_note(run_dir, turn, f"live government guard: {detail}")
            before = len(rows)
            rows = filter_orders(rows, skip_kinds, skip_verbs, args.one_order_per_unit)
            if len(rows) != before:
                print(f"[brain] turn {turn}: bisect dropped {before - len(rows)} "
                      f"of {before} orders", flush=True)
            count = write_turn(conn, run_tag, turn, rows, frame)
            answered = datetime.now(timezone.utc)
            age = board_age_seconds(event, answered)
            print(f"[brain] turn {turn}{f' frame {frame}' if frame else ''}: {count} orders in "
                  f"{time.time() - started:.2f}s at {answered:%H:%M:%S.%f}"[:-3] + "Z"
                  + (f", board received {age:.1f}s earlier" if age is not None else ""),
                  flush=True)
        time.sleep(0.1)
    if decider is not None:
        decider.stop()
    if updater is not None:
        updater.stop()
    print("[brain] done", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
