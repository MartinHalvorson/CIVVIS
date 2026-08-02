"""The stall watchdog has to see BOTH deaths.

`stall_s` asks "has anything been emitted". That cannot see a game whose turn has
stopped advancing while the harness keeps polling it — which is now the common way
a run dies here, and which nothing could detect until `frozen_s` existed.

Measured on run `civvis-20260802T033552Z`: the game wedged on the World Congress at
turn 209 and stayed there. `events.jsonl` kept growing every poll, so `last_event`
was never stale and the silence watchdog would never have fired.
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import watch  # noqa: E402


class FakeTail:
    """Emits whatever the test scripts, one batch per poll."""

    def __init__(self, batches):
        self.batches = list(batches)

    def poll(self):
        return self.batches.pop(0) if self.batches else []


def _follow(batches, **kw):
    # `env.game_pids()` is consulted every pass and must look alive, or the loop
    # returns "game exited" before either watchdog is reached.
    original = watch.env.game_pids
    watch.env.game_pids = lambda: [1]
    try:
        return watch.follow(FakeTail(batches), timeout_s=5.0, on_event=lambda e: None,
                            poll_s=0.01, **kw)
    finally:
        watch.env.game_pids = original


def test_a_frozen_turn_is_caught_even_though_events_keep_arriving():
    # The exact shape of the congress wedge: a turn record, then the same turn
    # over and over, forever.
    batches = [[{"kind": "turn", "turn": 209}]] + [[{"kind": "state", "turn": 209}]] * 400
    reason = _follow(batches, stall_s=None, frozen_s=0.05)
    assert reason.startswith("stalled"), reason
    assert "209" in reason and "not advanced" in reason, reason


class RisingTail:
    """A game that never stops advancing, so the loop can only end on timeout.

    ⚠ A finite script cannot test this: once it runs out the turn really has
    stopped, and the watchdog fires — correctly. My first version of this test
    asserted the bug back.
    """

    def __init__(self):
        self.turn = 0

    def poll(self):
        self.turn += 1
        return [{"kind": "turn", "turn": self.turn}]


def test_a_run_whose_turn_is_advancing_is_never_called_frozen():
    original = watch.env.game_pids
    watch.env.game_pids = lambda: [1]
    try:
        reason = watch.follow(RisingTail(), timeout_s=0.5, on_event=lambda e: None,
                              poll_s=0.01, stall_s=None, frozen_s=0.05)
    finally:
        watch.env.game_pids = original
    assert reason == "timeout", reason


def test_setup_emits_no_turn_and_must_not_be_killed():
    """⚠ Before a turn is ever seen there is nothing to freeze.

    Setup emits no turn at all. Treating "no turn yet" as a frozen turn would kill
    every run before it started — the same mistake `popup_clear.py` guards with its
    own "no turn recorded yet; this is setup" refusal.
    """
    batches = [[{"kind": "seat", "civ": "CIVILIZATION_CANADA"}]] * 400
    reason = _follow(batches, stall_s=None, frozen_s=0.05)
    assert "not advanced" not in reason, reason


def test_silence_is_still_caught_by_its_own_watchdog():
    """The original death must keep working; the two are independent."""
    reason = _follow([[]], stall_s=0.05, frozen_s=None)
    assert reason.startswith("stalled: no event"), reason


if __name__ == "__main__":
    failures = 0
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            started = time.time()
            try:
                fn()
                print(f"PASS {name}  ({time.time() - started:.2f}s)")
            except AssertionError as exc:
                failures += 1
                print(f"FAIL {name}: {exc}")
    raise SystemExit(1 if failures else 0)
