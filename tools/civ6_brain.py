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
import json
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

SCHEMA = """
CREATE TABLE IF NOT EXISTS orders (
    run TEXT NOT NULL, turn INTEGER NOT NULL, seq INTEGER NOT NULL,
    kind TEXT NOT NULL, subject INTEGER, verb TEXT, x INTEGER, y INTEGER,
    PRIMARY KEY (run, turn, seq)
);
CREATE TABLE IF NOT EXISTS ready (
    run TEXT NOT NULL, turn INTEGER NOT NULL, count INTEGER NOT NULL,
    PRIMARY KEY (run, turn)
);
"""

# A fixed, boring sequence whose only job is to prove an order was actuated.
STUB_RESEARCH = ["TECH_ANIMAL_HUSBANDRY", "TECH_MINING", "TECH_BRONZE_WORKING"]


def connect(path: Path) -> sqlite3.Connection:
    conn = sqlite3.connect(str(path), timeout=10)
    # WAL so the game's reader and this writer never block each other. A turn
    # blocked on a lock is a turn the game spends staring at us.
    conn.execute("PRAGMA journal_mode=WAL")
    conn.executescript(SCHEMA)
    conn.commit()
    return conn


def stub_orders(state: dict) -> list[tuple]:
    turn = int(state.get("turn", 0))
    tech = STUB_RESEARCH[turn % len(STUB_RESEARCH)]
    return [("research", None, tech, None, None)]


def civvis_orders(binary: Path, run_dir: Path, turn: int, victory: str) -> list[tuple]:
    """Ask CIVVIS. Its stdout is a JSON array of orders; anything else is an error.

    ⚠ A non-zero exit or unparseable stdout returns NO orders rather than a guess.
    The mod then falls back and records `fallback`, which is visible — inventing
    orders here would put my heuristics back in the game under CIVVIS's name.
    """
    try:
        proc = subprocess.run(
            [str(binary), "--mirror", str(run_dir), "--turn", str(turn),
             "--victory", victory],
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
                 war_from_plan: bool = False):
        self.binary = binary
        self.run_dir = run_dir
        self.victory = victory
        # ⚠ Declares war when CIVVIS's PLAN names a target but its own diplomatic
        # bookkeeping cannot fire. That bookkeeping wants a casus belli, or a
        # denouncement matured over five turns, and NOTHING matures in a board rebuilt
        # each turn — measured: 81 replayed turns, `strategy = conquest` on 26 of them,
        # ZERO declarations. So the decline is an artefact of the reconstruction
        # rather than a judgement about the war.
        self.war_from_plan = war_from_plan
        self.proc: subprocess.Popen | None = None

    def start(self) -> None:
        # ★★★★ KEEP CIVVIS'S REASONING. This used to send the decider's stderr to
        # DEVNULL, so a live run recorded WHAT was ordered and never WHY — and the two
        # questions this project keeps having to answer are "did it choose that" and
        # "did it ever reach the question", which only the journal separates. Every
        # diagnosis tonight came from replaying turns with `--explain` after the fact;
        # this makes the same account available for the run as it happens, including
        # the decider's own crash output, which DEVNULL was also swallowing.
        why = (self.run_dir / "why.log").open("a", buffering=1)
        self.proc = subprocess.Popen(
            [str(self.binary), "--mirror", str(self.run_dir), "--serve",
             "--fresh-board", "--explain", "--victory", self.victory]
            + (["--war-from-plan"] if self.war_from_plan else []),
            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=why, text=True, bufsize=1,
        )
        print("[brain] decider server up (fresh board, persistent agent, explaining "
              f"into {self.run_dir / 'why.log'})", flush=True)

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


def write_turn(conn: sqlite3.Connection, run: str, turn: int,
               rows: list[tuple]) -> int:
    conn.execute("DELETE FROM orders WHERE run = ? AND turn = ?", (run, turn))
    conn.executemany(
        "INSERT OR REPLACE INTO orders (run, turn, seq, kind, subject, verb, x, y) "
        "VALUES (?,?,?,?,?,?,?,?)",
        [(run, turn, i, k, s, v, x, y) for i, (k, s, v, x, y) in enumerate(rows)],
    )
    conn.commit()
    # LAST, and in its own commit: this is the mod's signal that the turn above is
    # complete. Any other order lets a partial turn be actuated.
    conn.execute("INSERT OR REPLACE INTO ready (run, turn, count) VALUES (?,?,?)",
                 (run, turn, len(rows)))
    conn.commit()
    return len(rows)



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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run-dir", required=True)
    ap.add_argument("--orders-db", default=str(Path.home() / "civvis-civ6-runs" / "orders.sqlite"))
    ap.add_argument("--mode", choices=["stub", "civvis"], default="civvis")
    ap.add_argument("--bin", default=None,
                    help="path to the civvis-orders binary (--mode civvis)")
    # ⚠⚠ DOMINATION IS CURRENTLY UNREACHABLE, AND IT IS THE DEFAULT.
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
    ap.add_argument("--victory", default="domination",
                    choices=["domination", "science", "score", "civvis"],
                    help="which victory CIVVIS plays for; `civvis` lets it choose. "
                         "⚠ domination is unreachable while no rival city is ever "
                         "revealed -- see the note above")
    ap.add_argument("--skip-kinds", default="",
                    help="comma-separated order kinds to drop (bisect only)")
    ap.add_argument("--skip-verbs", default="",
                    help="comma-separated order verbs to drop (bisect only)")
    ap.add_argument("--one-order-per-unit", action="store_true", default=False,
                    help="keep only a unit's last positional order (bisect only)")
    ap.add_argument("--war-from-plan", action="store_true", default=False,
                    help="declare on CIVVIS's plan target when its own casus-belli "
                         "bookkeeping cannot mature in a rebuilt board")
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

    conn = connect(Path(args.orders_db).expanduser())
    print(f"[brain] mode={args.mode} run={run_tag} db={args.orders_db} "
          f"decider={'server' if args.server else 'per-turn'}", flush=True)
    decider = (Decider(binary, run_dir, args.victory, args.war_from_plan)
               if args.mode == "civvis" and args.server else None)

    deadline = time.time() + args.seconds
    offset = 0
    served: set[int] = set()
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
            if event.get("kind") != "state":
                continue
            turn = int(event.get("turn", -1))
            if turn < 0 or turn in served:
                continue
            served.add(turn)
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
                rows = civvis_orders(binary, run_dir, turn, args.victory)
            before = len(rows)
            rows = filter_orders(rows, skip_kinds, skip_verbs, args.one_order_per_unit)
            if len(rows) != before:
                print(f"[brain] turn {turn}: bisect dropped {before - len(rows)} "
                      f"of {before} orders", flush=True)
            count = write_turn(conn, run_tag, turn, rows)
            print(f"[brain] turn {turn}: {count} orders in "
                  f"{time.time() - started:.2f}s", flush=True)
        time.sleep(0.1)
    if decider is not None:
        decider.stop()
    print("[brain] done", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
