#!/usr/bin/env python3
"""The arithmetic of what a tab may hold, run as it ships.

Operator, a standing rule since 2026-11-23: "please be careful about a memory
blowup on civvis. considering the typical amount of memory people computres
have". `web/assets/app_debugger.js` gives a tab a ceiling from the memory the
browser says the computer has, and holds Doctor Strange's worlds, their kept
turns and the futures played beside them under it (its section "What a tab
may hold"; measured in PR #5939). That section is cut out of the script as it
ships and run in node on stand-ins for the browser, so what is checked here
is the page's own arithmetic, not a copy of it:

  * the computer is taken as the 8 GB budget unless the browser says more
    (Chrome says 16 on a 16 GB computer) or a link does (`?memory=`), and the
    ceiling is 3/16 of it: 1.5 GB on 8 GB, 3 GB on 16;
  * the worlds that fit are as many as fit with each box, each world's
    engine at its least and one future beside them, by the end of a game,
    never past the dozen there are letters for; the most that may be open
    is that, but never fewer than the two the page opens on: one fits on
    the 8 GB budget, and two may be open;
  * the line each world's engine is told shares what the boxes leave between
    the worlds and the futures played at once, never under the engine's
    least nor over its most, and every world open under the most fits;
  * the AI Debugger's page and Edge of Tomorrow are told what the page and a
    split's boxes leave, and a small device keeps its own line;
  * the words a full page says.

`beta/debugger_test.py` (`check_doctor_strange_memory`) drives the same in a
browser: the cap holding against every way of making a world.
"""

from __future__ import annotations

import json
import pathlib
import shutil
import subprocess
import unittest

SCRIPT = pathlib.Path(__file__).resolve().parent.parent / "web" / "assets" / "app_debugger.js"


def section() -> str:
    """The script's "What a tab may hold", and the one constant it reads from before it."""
    text = SCRIPT.read_text(encoding="utf-8")
    begin = text.index("// ---- What a tab may hold")
    end = text.index("let strangeRoomTold = null;", begin)
    letters = next(line for line in text.splitlines() if line.startswith("const STRANGE_LETTERS = "))
    return letters + "\n" + text[begin:end]


def run(expression: str, memory=None, search: str = "", small: bool = False, boxes: int = 0):
    """`expression` evaluated after the section, on a browser that says `memory` GB."""
    stand_in = (
        f"const navigator = {{deviceMemory: {json.dumps(memory)}}};\n"
        f"const location = {{search: {json.dumps(search)}}};\n"
        f"const DEBUG_SMALL_DEVICE = {json.dumps(small)};\n"
        f"let debugSplit = {('{panes: ' + json.dumps({chr(97 + at): {} for at in range(boxes)}) + '}') if boxes else 'null'};\n"
        "let debugRoomAskedAt = 0;\n"
        "function strangeOpen() { return debugSplit ? Object.keys(debugSplit.panes) : []; }\n"
    )
    script = stand_in + section() + f"\nconsole.log(JSON.stringify({expression}));\n"
    out = subprocess.run(["node", "-e", script], check=True, capture_output=True, text=True).stdout
    return json.loads(out)


@unittest.skipUnless(shutil.which("node"), "needs node to run the page's arithmetic")
class TabMemory(unittest.TestCase):
    def test_the_computer_is_the_8_gb_budget_unless_told_more(self):
        # Chrome says 8 on an 8 GB laptop; Safari and Firefox say nothing; a small device
        # says less, and is put on the budget too (it opens on one timeline).
        self.assertEqual(run("[tabMemoryGB(), tabCeilingMB()]", memory=8), [8, 1536])
        self.assertEqual(run("[tabMemoryGB(), tabCeilingMB()]", memory=None), [8, 1536])
        self.assertEqual(run("[tabMemoryGB(), tabCeilingMB()]", memory=4), [8, 1536])
        self.assertEqual(run("[tabMemoryGB(), tabCeilingMB()]", memory=16), [16, 3072])
        self.assertEqual(run("tabMemoryGB()", memory=1024), 64)
        # A link says what the computer has, either way, within reason.
        self.assertEqual(run("[tabMemoryGB(), tabCeilingMB()]", memory=8, search="?mode=strange&memory=32"), [32, 6144])
        self.assertEqual(run("tabMemoryGB()", memory=8, search="?memory=4"), 4)
        self.assertEqual(run("tabMemoryGB()", memory=8, search="?memory=0.5"), 1)
        self.assertEqual(run("tabMemoryGB()", memory=8, search="?memory=lots"), 8)
        self.assertEqual(run("tabMemoryGB()", memory=8, search="?memory="), 8)
        self.assertEqual(run("tabMemoryGB()", memory=8, search="?memory=9999"), 64)

    def test_two_worlds_may_be_open_on_the_8_gb_budget(self):
        # 1536 - 920 for the page leaves 616: one box (200) and one engine at
        # its least with one future beside it (256) fit in it, two boxes and
        # two such (400 + 384) do not. The page's own two may still be open,
        # each world's engine at its least, and futures played one at a time.
        self.assertEqual(run("strangeBudgetOf(1536, 2)"), {"ceilingMB": 1536, "fits": 1, "most": 2, "futures": 1, "roomMB": 128})
        self.assertEqual(run("strangeBudgetOf(1536, 1)"), {"ceilingMB": 1536, "fits": 1, "most": 2, "futures": 1, "roomMB": 208})
        # A box out of sight is a page still: one world in two boxes.
        self.assertEqual(run("strangeBudgetOf(1536, 1, 2)"), {"ceilingMB": 1536, "fits": 1, "most": 2, "futures": 1, "roomMB": 128})

    def test_more_memory_holds_more_worlds_up_to_the_letters(self):
        self.assertEqual(run("[8, 16, 24, 32, 64].map(gb => strangeBudgetOf(tabCeilingMB(gb), 1).most)"), [2, 6, 10, 12, 12])
        # And plays every world's future at once when each world and its
        # copy fit at the least.
        self.assertEqual(run("strangeBudgetOf(3072, 6)"), {"ceilingMB": 3072, "fits": 6, "most": 6, "futures": 1, "roomMB": 136})
        self.assertEqual(run("strangeBudgetOf(3072, 4)"), {"ceilingMB": 3072, "fits": 6, "most": 6, "futures": 4, "roomMB": 169})
        self.assertEqual(run("strangeBudgetOf(12288, 12)"), {"ceilingMB": 12288, "fits": 12, "most": 12, "futures": 12, "roomMB": 373})

    def test_every_world_that_fits_fits_under_the_ceiling(self):
        # The page, every box, every world's engine at the line it is told
        # and as many copies as futures are played at once: never over the
        # ceiling, for every computer and every count of worlds that fit.
        over = run("""(() => {
          const over = [];
          for (const gb of [8, 12, 16, 20, 24, 32, 48, 64]) {
            const ceiling = tabCeilingMB(gb), fits = strangeBudgetOf(ceiling, 1).fits;
            for (let worlds = 1; worlds <= fits; worlds++) for (let boxes = worlds; boxes <= Math.min(fits, worlds + 1); boxes++) {
              const {roomMB, futures} = strangeBudgetOf(ceiling, worlds, boxes);
              const total = TAB_MEMORY.pageMB + boxes * TAB_MEMORY.boxMB + (worlds + futures) * roomMB;
              if (total > ceiling && roomMB < TAB_MEMORY.engineMostMB) over.push([gb, worlds, boxes, total, ceiling]);
            }
          }
          return over;
        })()""")
        self.assertEqual(over, [])

    def test_a_line_is_one_the_engine_keeps(self):
        # The engine keeps a line of 128 to 3072 MB (`room`): a line it would
        # change would be told again and again.
        self.assertEqual(run("[tabEngineMB(5), tabEngineMB(128), tabEngineMB(586.9), tabEngineMB(9000)]"), [128, 128, 586, 3072])
        self.assertEqual(run("strangeBudgetOf(12288, 1).roomMB"), 3072)
        self.assertEqual(run("strangeBudgetOf(1536, 12, 12).roomMB"), 128)

    def test_the_ai_debugger_and_edge_of_tomorrow_keep_their_turns_under_the_ceiling(self):
        # The page alone leaves 616 MB; a split's two boxes leave 216, the
        # one line of the engine both halves are worlds on.
        self.assertEqual(run("debugRoomMB()", memory=8), 616)
        self.assertEqual(run("debugRoomMB()", memory=8, boxes=2), 216)
        self.assertEqual(run("debugRoomMB()", memory=16), 2152)
        # A small device keeps the line it always had.
        self.assertEqual(run("debugRoomMB()", memory=2, small=True), 384)

    def test_a_full_page_says_why(self):
        self.assertEqual(run("strangeFullWords(strangeBudgetOf(1536, 2))"),
                         "2 worlds are open, which is the most this computer's memory holds (a tab is kept under 1.5 GB)")
        self.assertEqual(run("strangeFullWords(strangeBudgetOf(1536, 2), false)"),
                         "2 worlds is the most this computer's memory holds (a tab is kept under 1.5 GB)")
        self.assertEqual(run("strangeFullWords(strangeBudgetOf(3072, 6))"),
                         "6 worlds are open, which is the most this computer's memory holds (a tab is kept under 3.0 GB)")
        # Where the letters run out first, the words are what they were.
        self.assertEqual(run("strangeFullWords(strangeBudgetOf(12288, 12))"), "12 worlds are open, which is the most")
        self.assertEqual(run("strangeFullWords(strangeBudgetOf(12288, 12), false)"), "12 worlds is the most")


if __name__ == "__main__":
    unittest.main()
