#!/usr/bin/env python3
"""Regression checks for the live Civ VI SQLite order-channel path."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control.orders import orders_db_path, reset_orders_db  # noqa: E402


class Civ6OrdersTest(unittest.TestCase):
    def test_default_path_is_unique_to_the_run(self) -> None:
        run = Path("/tmp/civvis-runs/control/one")

        self.assertEqual(orders_db_path(run), run / "orders.sqlite")
        self.assertEqual(
            orders_db_path(run, "/tmp/explicit.sqlite"), Path("/tmp/explicit.sqlite")
        )

    def test_reset_removes_only_database_sidecars(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            database = Path(temporary) / "orders.sqlite"
            unrelated = Path(temporary) / "orders.sqlite.keep"
            for suffix in ("", "-wal", "-shm"):
                Path(f"{database}{suffix}").write_text("old")
            unrelated.write_text("keep")

            reset_orders_db(database)

            self.assertFalse(database.exists())
            self.assertFalse(Path(f"{database}-wal").exists())
            self.assertFalse(Path(f"{database}-shm").exists())
            self.assertEqual(unrelated.read_text(), "keep")


if __name__ == "__main__":
    unittest.main()
