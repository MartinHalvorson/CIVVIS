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

from civ6_control.orders import orders_db_path

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


def civvis_orders(binary: Path, run_dir: Path, turn: int, victory: str,
                  strategy: str | None = None, civ: str | None = None) -> list[tuple]:
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
                 war_from_plan: bool = False, strategy: str | None = "auto"):
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
        self.civ: str | None = None
        self.proc: subprocess.Popen | None = None

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
        why = (self.run_dir / "why.log").open("a", buffering=1)
        self.proc = subprocess.Popen(
            self.command(),
            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=why, text=True, bufsize=1,
        )
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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run-dir", required=True)
    ap.add_argument("--orders-db", default=None,
                    help="SQLite path shared with the live game; defaults to "
                         "<run-dir>/orders.sqlite")
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
    # ★★★★ WHICH STRATEGY ACTUALLY PLAYS, which nothing ever chose.
    #
    # `civvis_orders` has taken `--strategy` for a while and NO harness script passed
    # it, so every Civ 6 run has been `AdvancedAi::new` -- the decider's own banner
    # reads `{"strategy":"stock","source":"AdvancedAi::new"}`. The operator's standing
    # brief asks for "whatever the provably highest ELO player-strategy CIVVIS has".
    #
    # `auto` ranks on `league::strategy_strength`, the outright-win LOWER BOUND, not
    # the placement rating -- and the two disagree sharply, which is why the default
    # was wrong rather than merely unset:
    #
    #     strategy         rating   games  wins   winrate
    #     adv-religious      1601     116    58     50.0%   <- what `auto` picks
    #     advanced           1703     331    91     27.5%   <- what actually played
    #
    # The higher-RATED strategy wins barely half as often. Placement Glicko answers
    # "who should be matched with whom"; it is not a strength ordering.
    #
    # ⚠ TRANSFER TO THIS BRIDGE IS UNMEASURED. Those games are CIVVIS-vs-CIVVIS, and
    # this project has already watched a champion genome go +48 in compact evaluation
    # and -53 deployed. Treat the first Civ 6 runs under this as the measurement, not
    # as a settled improvement -- and if outcomes worsen, `--strategy ""` restores the
    # old behaviour exactly.
    ap.add_argument("--strategy", default="auto",
                    help="strategy for the decider to load; `auto` takes the "
                         "strongest by outright-win lower bound. Empty string keeps "
                         "the built-in AdvancedAi")
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

    orders_db = orders_db_path(run_dir, args.orders_db)
    conn = connect(orders_db)
    print(f"[brain] mode={args.mode} run={run_tag} db={orders_db} "
          f"decider={'server' if args.server else 'per-turn'}", flush=True)
    strategy = None if args.strategy.strip().lower() in {"", "stock", "none"} else args.strategy
    decider = (Decider(binary, run_dir, args.victory, args.war_from_plan, strategy)
               if args.mode == "civvis" and args.server else None)

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
                rows = civvis_orders(binary, run_dir, turn, args.victory, strategy, seat_civ)
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
