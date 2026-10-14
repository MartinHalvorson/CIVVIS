"""Stable filesystem ownership for the Civ VI inbound-order database."""

from __future__ import annotations

from pathlib import Path


def orders_db_path(run_dir: Path, configured: str | None = None) -> Path:
    """Return the one SQLite path shared by a run's game and its brain.

    An attached SQLite database remains bound to its original inode.  A global
    path lets a later run unlink and recreate that file while a live game still
    reads the old one, so new order batches become invisible.  Keeping the
    default inside the run directory gives every live game an immutable path.
    """
    return Path(configured).expanduser() if configured else Path(run_dir) / "orders.sqlite"


def reset_orders_db(path: Path) -> None:
    """Remove a run-local database before its game has attached it.

    Callers must stop the game first.  SQLite's sidecars belong to the same
    database generation and cannot safely survive into a new game.
    """
    for suffix in ("", "-wal", "-shm"):
        try:
            Path(f"{path}{suffix}").unlink()
        except FileNotFoundError:
            pass

#: Sequence and frame for the out-of-band retire row.  The frame is far above
#: any replan frame the decider writes, so the row can never be mistaken for
#: part of a real batch by `fetchOrders`, which filters on an exact frame.
RETIRE_SEQ = 99_000
RETIRE_FRAME = 990


def request_retire(path: Path, run_tag: str, turn: int, reason: str = "operator") -> bool:
    """Ask the running game to Retire, so the attempt is filed as a real loss.

    Killing the harness leaves the game unfinished — no `TeamVictory`, no
    defeat, nothing for `tools/civ6_ladder.py` to record — so a game we
    genuinely lost is indistinguishable from one that crashed.  The mod polls
    the orders channel for a `retire` row and answers it with the shipped
    `UI.RequestAction(ActionTypes.ACTION_RETIRE)`.

    Matched on the run alone, deliberately: the mod's ordinary fetch only sees
    the turn and frame it is currently reading, and a retire is asked for at a
    moment nobody scheduled.  `turn` and the frame are recorded for the ledger
    rather than for routing, and the sentinel frame keeps the row out of any
    real batch.

    Returns whether the row was written.  A missing or unwritable database is
    not fatal: the caller still stops the game, it is simply filed the old way.
    """
    try:
        import sqlite3

        with sqlite3.connect(str(path), timeout=5.0) as db:
            db.execute(
                "INSERT OR REPLACE INTO orders"
                " (run, turn, seq, kind, subject, verb, x, y, frame)"
                " VALUES (?, ?, ?, 'retire', NULL, ?, NULL, NULL, ?)",
                (run_tag, int(turn), RETIRE_SEQ, str(reason), RETIRE_FRAME),
            )
        return True
    except Exception:
        return False


#: Sequence and frame for the out-of-band pause row, beside the retire row's.
#: `write_turn` replaces only the (run, turn, frame) it is answering, so a row
#: on this sentinel frame survives every batch the decider writes around it.
PAUSE_SEQ = 99_001
PAUSE_FRAME = 991


#: Sequence and frame for the out-of-band watch-pace row, beside the pause's.
PACE_SEQ = 99_002
PACE_FRAME = 992


def set_operator_pace(path: Path, run_tag: str, ms: int) -> bool:
    """Hold each turn of the live game to the page's Watch pace.

    One `pace` row per run, its budget in milliseconds in `x`; the control mod
    holds each turn's opening until the previous opening is that long ago.
    ``0`` (Lightning) deletes the row: no wait at all, the lane's own speed.
    See `civ6_control.live_pause.requested_pace_ms` for where it comes from.

    Returns whether the database now says what was asked, exactly as
    `set_operator_pause` does.
    """
    try:
        import sqlite3

        uri = Path(path).absolute().as_uri() + "?mode=rw"
        with sqlite3.connect(uri, uri=True, timeout=2.0) as db:
            db.execute("DELETE FROM orders WHERE run = ? AND kind = 'pace'", (run_tag,))
            if int(ms) > 0:
                db.execute(
                    "INSERT OR REPLACE INTO orders"
                    " (run, turn, seq, kind, subject, verb, x, y, frame)"
                    " VALUES (?, 0, ?, 'pace', NULL, 'operator', ?, NULL, ?)",
                    (run_tag, PACE_SEQ, int(ms), PACE_FRAME),
                )
        return True
    except Exception:
        return False


def set_operator_pause(path: Path, run_tag: str, turn: int | None, paused: bool) -> bool:
    """Hold (or release) the live game at the opening of our next turn.

    The control mod polls the orders channel for a `pause` row on this run and,
    while one exists, stops playing the seat once the turn's board has gone
    out — Civilization VI then simply waits on our turn.  Resuming deletes
    every pause row for the run, whatever turn it was written on.  See
    `civ6_control.live_pause` for where the request comes from.

    Returns whether the database now says what was asked.  A database the brain
    has not created yet is not an error: the caller retries on its next poll.
    """
    try:
        import sqlite3

        # `mode=rw`: never create the file. An empty database made here would
        # be the one the game attaches, ahead of the brain's schema.
        uri = Path(path).absolute().as_uri() + "?mode=rw"
        with sqlite3.connect(uri, uri=True, timeout=2.0) as db:
            if paused:
                db.execute(
                    "INSERT OR REPLACE INTO orders"
                    " (run, turn, seq, kind, subject, verb, x, y, frame)"
                    " VALUES (?, ?, ?, 'pause', NULL, 'operator', NULL, NULL, ?)",
                    (run_tag, int(turn) if isinstance(turn, int) else 0,
                     PAUSE_SEQ, PAUSE_FRAME),
                )
            else:
                db.execute("DELETE FROM orders WHERE run = ? AND kind = 'pause'",
                           (run_tag,))
        return True
    except Exception:
        return False
