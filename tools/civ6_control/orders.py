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
