#!/usr/bin/env python3
"""Wizard Chess: the page, what it is made of, and a game played on it.

Operator, 2026-11-23: "below civ 6, add wizard chess. the background there
can be black and white checkerboard. in it, i want you to build wizard chess.
a laptop version and a version for a VR headset (but that can be later) ...
the chess strategy can be set to a chess elo and the chess will play at that
elo."

Three things are held here.

The rules and the command reader (`beta/chess/rules.js`, `command.js`) are
checked by node (`beta/chess/rules_test.mjs`): the published perft counts,
which is how a move generator is known to be right, a whole game read and
written back, and what is said to the pieces as a microphone hears it.

The page is held to what it ships: its address, its place in the site list,
and the two things it did not write, three.js and the Stockfish engine, each
to the bytes it had when it was put here.

And the page is opened in a browser and played: the engine wakes and says
what range of Elo it takes, a move is commanded and the piece makes it, and
the opponent answers with a legal move. A browser that will not draw in
three dimensions (no WebGL) cannot open the board, and that part is skipped
there with a word saying so.

    python3 beta/wizard_chess_test.py
"""

from __future__ import annotations

import functools
import hashlib
import http.server
import json
import pathlib
import re
import shutil
import socketserver
import subprocess
import sys
import threading
import time
import unittest
import urllib.request

BETA = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(BETA))

from verify import Devtools, find_chrome, free_port, fresh_profile, tether  # noqa: E402

PAGE = (BETA / "chess.html").read_text(encoding="utf-8")
PARTS = BETA / "chess"
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")

#: The two things the page did not write, as they were put here
#: (`beta/chess/vendor/README.md`).
VENDORED = {
    "three.module.min.js": "3e690ac7d180b0aadf0891bea39eec643e29e2d3e75c99b18689518665f69ba6",
    "stockfish-18-lite-single.js": "5243fd9b276cab7dfe3ad1d43ab9ead73568fac76468c614242977a210c4a391",
    "stockfish-18-lite-single.wasm": "a8fbc05ec6920b56d7485826dcb02c5ffd2826bcbf751cf973046f237a9096f1",
}


class Rules(unittest.TestCase):
    def test_the_rules_and_the_commands_hold(self):
        node = shutil.which("node")
        if not node:
            self.skipTest("no node on this machine to run beta/chess/rules_test.mjs")
        ran = subprocess.run([node, str(PARTS / "rules_test.mjs")], capture_output=True, text=True, timeout=300)
        self.assertEqual(ran.returncode, 0, ran.stderr[-2000:])
        self.assertIn("all hold", ran.stdout)


class Page(unittest.TestCase):
    def test_it_states_its_address_and_is_the_last_name_in_its_own_list(self):
        self.assertIn('<link rel="canonical" href="https://civvis.ai/wizard-chess/">', PAGE)
        self.assertIn("<title>Wizard Chess · CIVVIS</title>", PAGE)
        nav = PAGE[PAGE.index('<nav class="site-nav" aria-label="CIVVIS sites">'):]
        nav = nav[:nav.index("</nav>")]
        names = re.findall(r">([^<]+)</a>", nav)
        self.assertEqual(names, ["Home", "Charts/", "Tournaments", "CIVVIS", "Doctor Strange's Civ 6", "AI Debugger",
                                 "Civ 6", "Wizard Chess"])
        self.assertIn('<a class="site-link" href="./" aria-current="page">Wizard Chess</a>', nav)

    def test_it_is_a_board_a_command_and_an_opponent(self):
        for piece in (
            '<canvas id="stage"',
            '<input type="range" id="elo"',
            '<input type="text" id="typed"',
            '<button type="button" id="mic"',
            '<script type="module" src="chess/game.js"></script>',
        ):
            self.assertIn(piece, PAGE)
        # Everything it loads is beside it: no script from anywhere else.
        self.assertEqual(re.findall(r'<script[^>]*src="([^"]*)"', PAGE), ["chess/game.js"])
        game = (PARTS / "game.js").read_text(encoding="utf-8")
        self.assertEqual(
            sorted(re.findall(r'from "(\./[^"]+)"', game)),
            ["./command.js", "./engine.js", "./rules.js", "./vendor/three.module.min.js"])
        self.assertIn('new URL("./vendor/stockfish-18-lite-single.js", import.meta.url)', game)
        for name in ("game.js", "rules.js", "command.js", "engine.js"):
            self.assertTrue((PARTS / name).is_file(), name)

    def test_what_it_did_not_write_is_as_it_was_published(self):
        for name, digest in VENDORED.items():
            with self.subTest(file=name):
                self.assertEqual(hashlib.sha256((PARTS / "vendor" / name).read_bytes()).hexdigest(), digest)
        said = (PARTS / "vendor" / "README.md").read_text(encoding="utf-8")
        for word in ("three@0.160.1", "stockfish@18.0.8", "GPL-3.0", "MIT"):
            self.assertIn(word, said)
        self.assertTrue((PARTS / "vendor" / "three.LICENSE").is_file())
        # And the page says whose work the opponent and the drawing are.
        self.assertIn("https://github.com/official-stockfish/Stockfish", PAGE)
        self.assertIn("https://threejs.org/", PAGE)

    def test_the_strength_is_the_engines_own_setting(self):
        engine = (PARTS / "engine.js").read_text(encoding="utf-8")
        self.assertIn("setoption name UCI_LimitStrength value ${limited}", engine)
        self.assertIn("setoption name UCI_Elo value", engine)
        # Its range is read from what the engine says of itself.
        self.assertIn(r"option name UCI_Elo type spin default \d+ min (\d+) max (\d+)", engine)

    def test_publish_sets_its_parts_beside_its_page(self):
        self.assertIn('chess_page="$source_tree/beta/chess.html"\n', PUBLISH)
        self.assertIn('  cp "$chess_page" "$out/$chess_at/index.html"\n', PUBLISH)
        self.assertIn('  if [ -d "$chess_parts" ]; then cp -R "$chess_parts" "$out/$chess_at/chess"; fi\n', PUBLISH)
        self.assertIn('  pages+=("$chess_at")\n', PUBLISH)


class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):  # noqa: D401
        pass


class Played(unittest.TestCase):
    """The page, opened and played in a browser."""

    @classmethod
    def setUpClass(cls):
        cls.server = socketserver.TCPServer(("127.0.0.1", 0), functools.partial(Quiet, directory=str(BETA)))
        cls.server.daemon_threads = True
        threading.Thread(target=cls.server.serve_forever, daemon=True).start()
        cls.address = f"http://127.0.0.1:{cls.server.server_address[1]}/chess.html"
        cls.profile = fresh_profile("civvis-wizard-chess-profile-")
        port = free_port()
        cls.chrome = None
        chrome = find_chrome()
        if not pathlib.Path(chrome).exists() and not shutil.which(chrome):
            return
        # A headless browser has no graphics card: these ask it to draw in
        # software, which is slow and enough.
        cls.chrome = subprocess.Popen(
            [chrome, "--headless=new", f"--remote-debugging-port={port}", f"--user-data-dir={cls.profile}", "--no-first-run",
             "--enable-unsafe-swiftshader", "--use-angle=swiftshader", "--ignore-gpu-blocklist", "--window-size=1440,900",
             "about:blank"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        tether(cls.chrome)
        target, deadline = None, time.time() + 60
        while time.time() < deadline and target is None:
            try:
                with urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=2) as answer:
                    pages = [page for page in json.load(answer) if page.get("type") == "page"]
                target = next((page for page in pages if page.get("webSocketDebuggerUrl")), None)
            except OSError:
                time.sleep(0.2)
        if target is None:
            cls.chrome.terminate()
            cls.chrome = None
            return
        cls.dev = Devtools(target["webSocketDebuggerUrl"])
        cls.dev.call("Page.enable")
        cls.dev.call("Runtime.enable")
        # A page nobody is looking at is not drawn, and nothing moves on it.
        cls.dev.call("Page.startScreencast", {"format": "jpeg", "quality": 10, "everyNthFrame": 6})

    @classmethod
    def tearDownClass(cls):
        if cls.chrome:
            cls.chrome.terminate()
        cls.server.shutdown()
        shutil.rmtree(cls.profile, ignore_errors=True)

    def until(self, expression: str, what: str, seconds: float = 60):
        deadline = time.time() + seconds
        while time.time() < deadline:
            try:
                if self.dev.evaluate(expression):
                    return
            except Exception:  # noqa: BLE001 - the page may be between two documents
                pass
            time.sleep(0.2)
        self.fail(f"never happened: {what} ({self.dev.console[-5:]})")

    def test_a_move_is_commanded_and_the_opponent_answers(self):
        if not self.chrome:
            self.skipTest("no Chrome on this machine to open the page in")
        self.dev.call("Page.navigate", {"url": self.address})
        self.until("document.readyState === 'complete' && (typeof wizardChess !== 'undefined' || "
                   "!document.getElementById('loading').classList.contains('gone'))", "the page loads")
        time.sleep(1)
        if not self.dev.evaluate("typeof wizardChess !== 'undefined'"):
            said = self.dev.evaluate("document.getElementById('loading').textContent")
            if "WebGL" in said:
                self.skipTest(f"this browser will not draw the board: {said}")
            self.fail(f"the page did not start: {said!r} {self.dev.console[-5:]}")
        ev = self.dev.evaluate
        # The board is set, and the seat is White's.
        self.assertEqual(ev("[wizardChess.world.pieces.size, wizardChess.game.side, wizardChess.game.position.turn]"), [32, "w", "w"])
        # The engine wakes, and the slider is the range it gave.
        self.until("wizardChess.opponent.awake === true", "the engine wakes", seconds=120)
        low, high = ev("[wizardChess.opponent.range.min, wizardChess.opponent.range.max]")
        self.assertLess(low, high)
        self.assertEqual(ev("[+document.getElementById('elo').min, +document.getElementById('elo').max]"), [low, high])
        self.assertIn("Stockfish", ev("document.getElementById('elo-about').textContent"))
        # A press on the board. From the seat the king stands in front of
        # the pawn behind it, and at the start it cannot move: a press on it
        # is the pawn's. A press on the chosen piece lets it go, and a press
        # on an empty square it may go to is the move.
        self.assertEqual(ev("wizardChess.meant({pieces: [4, 12], floor: 20})"), {"choose": 12, "square": 12})
        self.assertEqual(ev("wizardChess.meant({pieces: [4], floor: -1})"), {"stuck": 4})
        ev("wizardChess.point({pieces: [4, 12], floor: 20})")
        self.assertEqual(ev("[wizardChess.game.chosen, wizardChess.world.dots.filter(held => held.dot.visible).length]"), [12, 2])
        self.assertEqual(ev("wizardChess.meant({pieces: [12], floor: 20})"), {"letGo": True, "square": 12})
        self.assertEqual(ev("wizardChess.meant({pieces: [], floor: 28}).square"), 28)
        ev("wizardChess.point({pieces: [12], floor: 20})")
        self.assertEqual(ev("wizardChess.game.chosen"), -1)
        # A command that is no move is refused in words, and the board stands.
        self.assertFalse(ev("wizardChess.command('knight to e5')"))
        self.assertIn("not a move that can be played", ev("document.getElementById('said').textContent"))
        # A command that is one: the pawn goes there by itself, and the
        # opponent answers with a move of its own.
        self.assertTrue(ev("wizardChess.command('pawn to e4')"))
        self.until("wizardChess.game.played.length >= 2 && !wizardChess.game.busy", "the opponent answers", seconds=120)
        played = ev("wizardChess.game.played.map(entry => [entry.san, entry.by])")
        self.assertEqual(played[0], ["e4", "w"])
        self.assertEqual(played[1][1], "b")
        self.assertEqual(ev("[wizardChess.game.position.turn, wizardChess.world.pieces.size]"), ["w", 32])
        # The piece stands where the game says it does.
        self.assertEqual(ev("wizardChess.world.pieces.get(28).userData.piece"), "P")
        self.assertEqual(ev("[...document.querySelectorAll('#moves b')].map(held => held.textContent)"), [played[0][0], played[1][0]])
        # Taken back: the player's move and the answer to it.
        ev("wizardChess.takeBack()")
        self.assertEqual(ev("[wizardChess.game.played.length, wizardChess.world.pieces.has(12), wizardChess.world.pieces.has(28)]"), [0, True, False])
        # A game set down mid-way plays on: Black's seat, and a capture
        # breaks the piece taken.
        ev("wizardChess.opponent.stop(), wizardChess.begin('b', ['e2e4', 'd7d5', 'g1f3'])")
        self.assertEqual(ev("[wizardChess.game.side, wizardChess.game.position.turn, wizardChess.game.played.length]"), ["b", "b", 3])
        self.assertTrue(ev("wizardChess.command('pawn takes e4')"))
        self.until("wizardChess.game.played.length >= 4 && wizardChess.world.pieces.size === 31", "the pawn is taken", seconds=60)
        self.assertEqual(ev("wizardChess.game.played[3].san"), "dxe4")
        self.assertEqual(self.dev.console, [], f"the page complained: {self.dev.console}")


if __name__ == "__main__":
    unittest.main()
