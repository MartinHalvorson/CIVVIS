#!/usr/bin/env python3
"""What `civ6_thread_time.py` makes of a game's threads: the processor time
between two moments from the kernel's running totals, the play it is over,
and how a thread waits from a `sample` report."""

from __future__ import annotations

import json
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import civ6_thread_time as thread_time  # noqa: E402


def snap(at: float, **threads: tuple[str, float]) -> dict:
    return {"at": at, "threads": {handle: {"name": name, "seconds": seconds} for handle, (name, seconds) in threads.items()}}


#: Ten seconds of a game with a window thread flat out, a game thread at work
#: three seconds in ten, and two workers.
SNAPSHOTS = [
    snap(100.0, a=("WinMain (WinID 2)", 50.0), b=("Game Core (WinID 7)", 20.0), c=("TBB (WinID 3)", 8.0), d=("TBB (WinID 4)", 8.0), e=("", 1.0)),
    snap(105.0, a=("WinMain (WinID 2)", 55.0), b=("Game Core (WinID 7)", 21.5), c=("TBB (WinID 3)", 10.0), d=("TBB (WinID 4)", 10.5), e=("", 1.0)),
    snap(110.0, a=("WinMain (WinID 2)", 60.0), b=("Game Core (WinID 7)", 23.0), c=("TBB (WinID 3)", 12.0), d=("TBB (WinID 4)", 13.0), e=("", 1.0), f=("TBB (WinID 5)", 0.5)),
]

REPORT = """Analysis of sampling Civ6_Exe_Child (pid 1) every 1 millisecond
Call graph:
    1000 Thread_1   DispatchQueue_1: com.apple.main-thread  (serial)
    + 1000 start  (in dyld) + 1  [0x1]
    +   1000 mach_msg2_trap  (in libsystem_kernel.dylib) + 8  [0x2]
    1000 Thread_2: WinMain (WinID 2)
    + 1000 thread_start  (in libsystem_pthread.dylib) + 8  [0x3]
    +   600 Platform::JobManager::Wait(Platform::JobSet*, Platform::JobID)  (in Civ6_Exe_Child) + 56  [0x4]
    +   ! 590 cthread_yield  (in libsystem_pthread.dylib) + 36  [0x5]
    +   ! : 590 swtch_pri  (in libsystem_kernel.dylib) + 8  [0x6]
    +   ! 10 ???  (in libtbb.dylib)  load address 0x1 + 0x2  [0x7]
    +   400 ???  (in Civ6_Exe_Child)  load address 0x1 + 0x3  [0x8]
    1000 Thread_3: Game Core (WinID 7)
    + 1000 thread_start  (in libsystem_pthread.dylib) + 8  [0x3]
    +   700 _pthread_cond_wait  (in libsystem_pthread.dylib) + 980  [0x9]
    +   ! 700 __psynch_cvwait  (in libsystem_kernel.dylib) + 8  [0xa]
    +   300 ???  (in GameCore_XP2.dll)  load address 0x1 + 0x4  [0xb]
    +     50 swtch_pri  (in libsystem_kernel.dylib) + 8  [0x6]
    1000 Thread_4: TBB (WinID 3)
    + 500 __psynch_cvwait  (in libsystem_kernel.dylib) + 8  [0xa]
    + 500 ???  (in Civ6_Exe_Child)  load address 0x1 + 0x5  [0xc]
    1000 Thread_5: TBB (WinID 4)
    + 400 __psynch_cvwait  (in libsystem_kernel.dylib) + 8  [0xa]
    + 600 ???  (in Civ6_Exe_Child)  load address 0x1 + 0x5  [0xc]

Total number in stack (recursive counted multiple, when >=5):
        5       __psynch_cvwait  (in libsystem_kernel.dylib) + 0  [0xa]
"""


class Spent(unittest.TestCase):
    def test_a_thread_s_name_loses_the_number_the_game_gives_it(self):
        self.assertEqual(thread_time.named("TBB (WinID 4)"), "TBB")
        self.assertEqual(thread_time.named("Game Core (WinID 7)"), "Game Core")
        self.assertEqual(thread_time.named("com.apple.audio.IOThread.client"), "com.apple.audio.IOThread.client")
        self.assertEqual(thread_time.named(""), "(unnamed)")

    def test_the_time_between_two_moments_is_the_totals_at_the_second_less_those_at_the_first(self):
        used = thread_time.spent(SNAPSHOTS, 100.0, 110.0)
        self.assertEqual(used["seconds"], 10.0)
        # A name's threads are added together, and a thread that began after
        # the first snapshot counts from nothing.
        self.assertEqual(used["threads"], {"WinMain": 10.0, "TBB": 9.5, "Game Core": 3.0})
        self.assertEqual(used["cpu_seconds"], 22.5)
        # A thread that used nothing is left out, and the sum is still whole.
        self.assertNotIn("(unnamed)", used["threads"])

    def test_the_moments_are_the_snapshots_nearest_them(self):
        self.assertIsNone(thread_time.spent(SNAPSHOTS, 101.9, 102.4))
        used = thread_time.spent(SNAPSHOTS, 103.0, 111.0)
        self.assertEqual((used["seconds"], used["threads"]["Game Core"]), (5.0, 1.5))
        self.assertIsNone(thread_time.spent(SNAPSHOTS[:1], 100.0, 110.0))

    def test_a_reading_is_the_play_the_snapshots_cover(self):
        # Turns 1 and 2 began before the recording did: the reading is over
        # the turns it saw and says which.
        played = [(1, 90.0), (2, 95.0), (3, 100.0), (4, 104.9), (5, 110.0), (6, 115.0)]
        row = thread_time.reading(SNAPSHOTS, played)
        self.assertEqual(row, {"from_turn": 3, "to_turn": 5, "play_seconds": 10.0, "cpu_seconds": 22.5,
                               "game_seconds": 3.0, "window_seconds": 10.0, "workers_seconds": 9.5})
        self.assertIsNone(thread_time.reading(SNAPSHOTS, [(1, 90.0), (2, 95.0)]))
        self.assertIsNone(thread_time.reading([], played))

    def test_the_turns_are_read_from_the_run_s_events(self):
        with tempfile.TemporaryDirectory() as scratch:
            run = pathlib.Path(scratch)
            (run / "events.jsonl").write_text("\n".join([
                json.dumps({"kind": "seat", "utc": "2026-11-25T03:49:00.000Z"}),
                json.dumps({"kind": "turn", "turn": 1, "utc": "2026-11-25T03:49:08.500Z"}),
                "not json",
                json.dumps({"kind": "turn", "turn": 2, "utc": "2026-11-25T03:49:10.000Z"}),
                json.dumps({"kind": "victory", "turn": 2, "utc": "2026-11-25T03:49:12.000Z"}),
            ]), encoding="utf-8")
            played = thread_time.turns(run)
        self.assertEqual([turn for turn, _ in played], [1, 2])
        self.assertAlmostEqual(played[1][1] - played[0][1], 1.5)


class Stacks(unittest.TestCase):
    def test_a_thread_is_parked_or_passing_its_turn_where_its_branch_ends_so(self):
        found = thread_time.stacks(REPORT)
        # The window's thread never sleeps: it passes its turn while it
        # waits on a job. The game's thread sleeps until it is woken.
        self.assertEqual(found["WinMain"], {"samples": 1000, "parked": 0, "yielding": 590})
        self.assertEqual(found["Game Core"], {"samples": 1000, "parked": 700, "yielding": 50})
        # A name's threads are added together.
        self.assertEqual(found["TBB"], {"samples": 2000, "parked": 900, "yielding": 0})
        # The table under the call graph counts the same frames again and is
        # not read.
        self.assertEqual(sum(row["parked"] for row in found.values()), 700 + 900 + 1000)


if __name__ == "__main__":
    unittest.main()
