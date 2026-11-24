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

    def open(self):
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

    def test_a_move_is_commanded_and_the_opponent_answers(self):
        self.open()
        ev = self.dev.evaluate
        # The hall stands round the board: twelve columns, each part of them
        # (a shaft, a base and a capital) drawn twelve at once.
        self.assertEqual(ev("wizardChess.world.scene.children.filter(part => part.isInstancedMesh && part.count === 12).length"), 3)
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
        # The pieces are alive. They watch what moves: the pawn on d2 leans
        # toward the pawn on e2 while it moves, and stands straight after.
        ev("wizardChess.opponent.stop(), wizardChess.begin('w'), wizardChess.game.turn++")
        ev("(w => { w.world.moving = w.world.pieces.get(12); for (let at = 0; at < 12; at++) w.world.regard(0.1); })(wizardChess)")
        self.assertLess(ev("wizardChess.world.pieces.get(11).quaternion.w"), 0.9999)
        ev("(w => { w.world.moving = null; for (let at = 0; at < 60; at++) w.world.regard(0.1); })(wizardChess)")
        self.assertEqual(ev("wizardChess.world.pieces.get(11).quaternion.w"), 1)
        # A piece about to be taken trembles.
        self.assertLess(ev("(w => { w.world.doomed = w.world.pieces.get(52); w.world.regard(0.1); const q = w.world.doomed.quaternion.w; "
                           "w.world.doomed.quaternion.identity(); w.world.doomed = null; return q; })(wizardChess)"), 0.9999)
        # A king in check flinches away from what checks it (the queen on h5).
        ev("wizardChess.opponent.stop(), wizardChess.begin('w', ['e2e4', 'f7f6', 'd1h5']), wizardChess.game.turn++, wizardChess.opponent.stop()")
        away = ev("(king => king.userData.check && king.userData.check.toArray())(wizardChess.world.pieces.get(60))")
        self.assertTrue(away and away[0] < 0, away)
        self.until("wizardChess.world.pieces.get(60).quaternion.w < 0.9995", "the king flinches", seconds=60)
        # The board hears a command as it is given. Typed "knight", the two
        # knights stir; "knight takes e5", the pawn on e5 turns to face what is
        # coming and braces; cleared, the board is still again.
        ev("wizardChess.opponent.stop(), wizardChess.begin('w', ['g1f3', 'e7e5']), wizardChess.game.turn++")
        typing = "(t => { t.value = %s; t.dispatchEvent(new Event('input')); })(document.getElementById('typed'))"
        stirred = "[...wizardChess.world.pieces.values()].filter(held => held.userData.stir).map(held => held.userData.square).sort((a, b) => a - b)"
        ev(typing % "'knight'")
        self.assertEqual(ev(stirred), [1, 21])
        ev(typing % "'knight takes e5'")
        self.assertEqual(ev(stirred), [21])
        self.assertTrue(ev("!!wizardChess.world.pieces.get(36).userData.brace"))
        self.until("wizardChess.world.pieces.get(36).quaternion.w < 0.9997", "the pawn braces", seconds=60)
        ev(typing % "''")
        self.assertEqual(ev("[...wizardChess.world.pieces.values()].filter(held => held.userData.stir || held.userData.brace).length"), 0)
        # A new game begun while a move is still being played out: the move
        # was the old game's, and the new board stays as it was set down,
        # every piece in the world where the game says it is and on the board.
        ev("wizardChess.opponent.stop(), wizardChess.begin('w'), wizardChess.command('pawn to e4')")
        self.assertTrue(ev("wizardChess.game.busy"))
        ev("wizardChess.opponent.stop(), wizardChess.begin('w', ['d2d4']), wizardChess.game.turn++, wizardChess.opponent.stop()")
        self.until("wizardChess.running.size === 0", "the old move is played out", seconds=90)
        self.assertEqual(ev("wizardChess.game.played.map(entry => entry.san)"), ["d4"])
        self.assertEqual(ev("[wizardChess.world.pieces.size, wizardChess.world.pieces.has(28), wizardChess.world.pieces.get(27).userData.piece]"),
                         [32, False, "P"])
        self.assertTrue(ev("[...wizardChess.world.pieces].every(([sq, held]) => held.userData.square === sq && !!held.parent && "
                           "wizardChess.game.position.board[sq] === held.userData.piece)"))
        self.assertEqual(self.dev.console, [], f"the page complained: {self.dev.console}")

    def test_a_rider_is_carried_by_their_knight_and_thrown_when_it_is_taken(self):
        self.open()
        ev = self.dev.evaluate
        ev("wizardChess.opponent.stop(), wizardChess.begin('w')")
        # On a knight: the king's knight, unless another is asked for.
        ev("wizardChess.ride(true)")
        self.assertEqual(ev("[wizardChess.seat.mode, wizardChess.world.ridden.userData.square, wizardChess.game.mount]"), ["ride", 6, 6])
        # Time on the page is the world's: a frame moves it on 50 ms at most,
        # so where frames are slow (a browser drawing in software, as on CI)
        # an animation is too. The view's travel to the saddle is cut, and
        # each wait below is long enough for a slow frame.
        ev("wizardChess.seat.blend = null")
        self.until("Math.abs(wizardChess.seat.camera.position.y - 1.24) < 0.1", "the eye is in the saddle", seconds=60)
        # The eye is on the knight: on g1, low on its neck and to its right
        # (half a square toward the h-file), a pawn's height up.
        x, y, z = ev("wizardChess.seat.camera.position.toArray()")
        self.assertAlmostEqual(x, 3.0, delta=0.1)
        self.assertTrue(1.1 < y < 1.4 and 3.6 < z < 4.2, (x, y, z))
        # And what is heard is heard from there: full strength only close by.
        self.assertEqual(ev("wizardChess.sound.near"), 2.2)
        # The knight ridden is alive: two ears, two eyes, and its ears turn
        # toward a piece that moves (the knight on b1, off to its left).
        self.assertEqual(ev("(made => [made.ears.length, made.eyes.length])(wizardChess.world.ridden.userData.saddle.userData)"), [2, 2])
        turned = ev("(w => { w.world.moving = w.world.pieces.get(1); for (let at = 0; at < 12; at++) w.world.attend(0.1); "
                    "const yaw = w.world.ridden.userData.saddle.userData.ears[1].rotation.y; w.world.moving = null; return yaw; })(wizardChess)")
        self.assertGreater(turned, 0.8)
        # In the saddle the plates fold.
        self.assertTrue(ev("document.body.classList.contains('riding')"))
        # "ride to f3" is the knight's move, and the rider goes with it.
        self.assertTrue(ev("wizardChess.command('ride to f3')"))
        self.until("wizardChess.game.played.length >= 1 && wizardChess.world.ridden.userData.square === 21 && "
                   "Math.abs(wizardChess.seat.camera.position.x - 2.0) < 0.1 && wizardChess.seat.camera.position.z < 2.2",
                   "the rider is carried to f3", seconds=120)
        self.assertEqual(ev("wizardChess.game.played[0].san"), "Nf3")
        # A knight on e5 that a pawn can take: the pawn's light turns red.
        ev("wizardChess.opponent.stop(), wizardChess.begin('w', ['g1f3', 'd7d6', 'f3e5']), wizardChess.game.turn++, wizardChess.opponent.stop()")
        self.assertEqual(ev("[wizardChess.seat.mode, wizardChess.world.ridden.userData.square]"), ["ride", 36])
        self.assertEqual(ev("[...wizardChess.world.pieces.values()].filter(held => held.userData.menace).map(held => held.userData.square)"), [43])
        # It leans toward the knight, as if about to come.
        self.until("wizardChess.world.pieces.get(43).quaternion.w < 0.9995", "the pawn leans in", seconds=60)
        # A threat that is new turns the rider's head to it.
        ev("wizardChess.game.threats = ''; wizardChess.peril()")
        self.assertIsNotNone(ev("wizardChess.seat.alarm"))
        # Turned toward their knight's own head, the rider rises in the saddle
        # to see over it; turned back, they sit down again.
        ev("wizardChess.seat.glance(null); Object.assign(wizardChess.seat.gaze, {yaw: 1.4, at: wizardChess.seat.clock})")
        self.until("wizardChess.seat.camera.position.y > 1.45", "the rider rises to see over the head", seconds=60)
        ev("wizardChess.seat.straighten()")
        self.until("wizardChess.seat.camera.position.y < 1.3", "the rider sits down again", seconds=60)
        # What a frame costs is measured, and says what drew it.
        cost = ev("wizardChess.frameCost(5)")
        self.assertTrue(cost["ms"] > 0 and cost["drawnBy"] and cost["seat"] == "ride", cost)
        # Taken: the rider is thrown, and stands at the side of the board.
        ev("(w => w.make(w.moveFromUci(w.game.position, 'd6e5'), 'b'))(wizardChess)")
        self.until("wizardChess.seat.mode === 'aside'", "the rider is thrown and gets up", seconds=120)
        self.assertIn("thrown clear", ev("document.getElementById('said').textContent"))
        x, y, z = ev("wizardChess.seat.camera.position.toArray()")
        self.assertTrue(max(abs(x), abs(z)) > 5.5 and y < 0.6, (x, y, z))
        self.assertEqual(ev("document.getElementById('seat-knight').textContent"), "Ride again")
        # Taken back to before the knight went to e5: the rider is on it again, on f3.
        ev("wizardChess.takeBack()")
        self.assertEqual(ev("[wizardChess.seat.mode, wizardChess.world.ridden.userData.square, wizardChess.game.played.length]"), ["ride", 21, 2])
        # A move that gives the knight up is asked about first.
        ev("wizardChess.opponent.stop(), wizardChess.begin('w', ['g1f3', 'd7d6']), wizardChess.game.turn++, wizardChess.opponent.stop()")
        self.assertTrue(ev("wizardChess.command('ride to e5')"))
        self.assertEqual(ev("[document.getElementById('risk').hidden, wizardChess.game.played.length]"), [False, 2])
        self.assertIn("Ride on?", ev("document.getElementById('risk-said').textContent"))
        ev("wizardChess.settle(false)")
        self.assertEqual(ev("[document.getElementById('risk').hidden, wizardChess.game.played.length]"), [True, 2])
        # Said again, it is the rider's choice, and the knight goes.
        self.assertTrue(ev("wizardChess.command('ride to e5')"))
        self.until("wizardChess.game.played.length >= 3", "the knight rides on", seconds=60)
        self.assertEqual(ev("wizardChess.game.played[2].san"), "Ne5")
        ev("wizardChess.opponent.stop(), wizardChess.game.turn++")
        # And back to the board.
        ev("wizardChess.ride(false)")
        self.assertEqual(ev("[wizardChess.seat.mode, wizardChess.world.ridden]"), ["table", None])
        self.until("wizardChess.sound.near === 14", "the ear goes back to the board's side", seconds=60)
        self.assertEqual(self.dev.console, [], f"the page complained: {self.dev.console}")


if __name__ == "__main__":
    unittest.main()
