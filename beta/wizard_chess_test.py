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

Set aside on 2026-11-24 (operator: "lets set aside wizard chess right now.
please remove from civvis.ai"): the game is kept in the tree and is not
published. Everything here reads and serves it from `beta/` itself, so it
stays checked while it is off the site.

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
        self.assertEqual(names, ["Home", "Charts/", "Tournaments", "CIVVIS", "Doctor Strange's CIVVIS", "CIVVIS AI Debugger",
                                 "CIVVIS: Edge of Tomorrow", "Civ 6", "Wizard Chess"])
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
        # Stockfish is GPL-3.0 whatever the repository around it says of
        # itself, so the text travels with the engine, not at the root.
        licence = (PARTS / "vendor" / "stockfish.LICENSE").read_text(encoding="utf-8")
        self.assertIn("GNU GENERAL PUBLIC LICENSE", licence)
        self.assertIn("Version 3, 29 June 2007", licence)
        self.assertIn("`stockfish.LICENSE`", said)
        # And the page says whose work the opponent and the drawing are.
        self.assertIn("https://github.com/official-stockfish/Stockfish", PAGE)
        self.assertIn("https://threejs.org/", PAGE)

    def test_nothing_in_the_game_is_written_twice(self):
        # A merge of two branches that both carry the same new code can leave
        # it in twice without a conflict (it did, #5782): the later copy of a
        # method silently wins. Each method and function is written once.
        game = (PARTS / "game.js").read_text(encoding="utf-8")
        # Each class's own methods (two classes may share a name, as World.play
        # and Sound.play do), and the functions at the top level.
        scopes = re.split(r"(?m)^class ", game)
        groups = [(f"class {scope.split()[0]}", re.findall(r"(?m)^  (?:async )?([A-Za-z]+)\([^)]*\) \{$", scope)) for scope in scopes[1:]]
        groups.append(("the top level", re.findall(r"(?m)^(?:async )?function ([A-Za-z]+)\(", game)))
        for where, names in groups:
            twice = sorted({name for name in names if names.count(name) > 1})
            self.assertEqual(twice, [], f"written twice in {where} of game.js: {twice}")

    def test_the_strength_is_the_engines_own_setting(self):
        engine = (PARTS / "engine.js").read_text(encoding="utf-8")
        self.assertIn("setoption name UCI_LimitStrength value ${limited}", engine)
        self.assertIn("setoption name UCI_Elo value", engine)
        # Its range is read from what the engine says of itself.
        self.assertIn(r"option name UCI_Elo type spin default \d+ min (\d+) max (\d+)", engine)

    def test_it_is_set_aside_and_publish_leaves_it_here(self):
        # From 2026-11-23 `publish.sh` set the page at /wizard-chess/ with
        # its parts beside it. Set aside, it copies nothing of the game into
        # a lane, and says why where the block stood. Putting the game back
        # on the site is the revert of the pull request that took it off
        # (docs/WIZARD_CHESS.md names it).
        # One line names it and copies nothing: the address an older
        # revision's viewer still links, kept inside that revision's lane.
        self.assertEqual(
            [line for line in PUBLISH.splitlines() if "chess" in line.lower() and not line.lstrip().startswith("#")],
            ['set_aside = ["wizard-chess", "chaos-monkey"]'])
        self.assertIn("# Wizard Chess is set aside and no lane carries it (operator 2026-11-24:\n", PUBLISH)


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
        # The headset seat: the camera stands in a rig, "Enter VR" is only
        # shown where a headset is offered (this browser has none), a
        # controller's ray picks what a click on the same line of sight picks,
        # and a headset's wearer stands at their side's edge of the board.
        self.assertEqual(ev("[wizardChess.seat.camera.parent === wizardChess.seat.rig, document.getElementById('enter-vr').hidden, "
                            "wizardChess.seat.pointers.length]"), [True, True, 2])
        same = ev("""(seat => {
          const box = seat.canvas.getBoundingClientRect(), clicked = seat.under({clientX: box.left + box.width / 2, clientY: box.top + box.height * 0.62});
          const hand = new (seat.rig.constructor)();
          const look = new seat.ray.ray.direction.constructor();
          seat.ray.setFromCamera({x: 0, y: -0.24}, seat.camera);
          hand.position.copy(seat.ray.ray.origin);
          hand.lookAt(look.copy(seat.ray.ray.origin).add(seat.ray.ray.direction));
          hand.rotateY(Math.PI);
          const pointed = seat.pointed(hand);
          return [JSON.stringify(clicked), JSON.stringify(pointed)];
        })(wizardChess.seat)""")
        self.assertEqual(same[0], same[1])
        self.assertIn('"floor":', same[0])
        self.assertEqual(ev("(at => [at.at.x, at.at.y, at.at.z, at.yaw])(wizardChess.seat.headsetPlace())"), [0, 1.3, 5.8, 0])
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
        # Each piece its own stone: the pawn's going was sounded as a pawn's.
        self.assertIn(["slide", "p"], ev("wizardChess.sound.heard"))
        # The hall answers a sound made in it. A capture's crash, rendered
        # offline with the hall and without it, is still sounding long after
        # the crash with it; and the four braziers are where fire is heard.
        self.assertEqual(ev("wizardChess.sound.fires.length"), 4)
        # Rendered cheaply (one channel, 11 kHz, panned left and right rather
        # than through the head), and given 25 s: a browser too slow to
        # render it in that (CI draws the page in software) says so.
        tails = ev("""(async () => {
          const S = wizardChess.sound, kept = [S.audio, S.out, S.noise, S.send, S.air, S.on, S.panning];
          const tail = async hall => {
            const rate = 11025, context = new OfflineAudioContext(1, rate * 3, rate);
            context.resume = () => Promise.resolve();
            S.on = true;
            S.panning = "equalpower";
            S.wire(context);
            S.air.gain.value = 0;
            if (!hall) S.send.gain.value = 0;
            S.play('shatter', 1, {x: 0, y: 0, z: -2}, 'q');
            const heard = (await context.startRendering()).getChannelData(0);
            // What the crash scheduled for later is let finish on this context.
            await new Promise(done => setTimeout(done, 700));
            // How much is still sounding after the crash itself: 0.6 s to 2.5 s.
            let sum = 0;
            for (let at = Math.round(0.6 * rate); at < Math.round(2.5 * rate); at++) sum += heard[at] * heard[at];
            return Math.sqrt(sum / (1.9 * rate));
          };
          // The page's own sound is given back whichever comes first, and again
          // when a render that came late is done with it.
          const restore = () => { [S.audio, S.out, S.noise, S.send, S.air, S.on, S.panning] = kept; };
          const measured = (async () => [await tail(false), await tail(true)])();
          measured.then(restore, restore);
          const result = await Promise.race([measured, new Promise(done => setTimeout(() => done(null), 25000))]);
          restore();
          return result;
        })()""")
        if tails is None:
            print("the hall's echo was not measured: this browser could not render 6 s of sound in 25 s", file=sys.stderr)
        else:
            self.assertGreater(tails[1], 2 * tails[0], tails)
        self.assertEqual(played[1][1], "b")
        self.assertEqual(ev("[wizardChess.game.position.turn, wizardChess.world.pieces.size]"), ["w", 32])
        # It deliberated: its pieces shifted their weight while it thought,
        # and the piece it chose was shown before it was played. Thinking
        # over, the board is still.
        self.assertGreaterEqual(ev("wizardChess.world.musings"), 1)
        self.assertEqual(ev("wizardChess.game.foreseen"), ev("wizardChess.game.played[1].move.from"))
        self.assertEqual(ev("[wizardChess.world.musing, [...wizardChess.world.pieces.values()].some(held => held.userData.stir)]"), [None, False])
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
        # The fallen stay fallen: the pawn taken lies in a heap beside the board.
        self.assertEqual(ev("wizardChess.world.fallen.children.map(heap => heap.userData.piece)"), ["P"])
        # A piece falling flares the fires, and the hall is a little darker
        # for a board with a piece fewer.
        self.assertGreaterEqual(ev("wizardChess.world.mood.falls"), 1)
        self.assertGreater(ev("wizardChess.world.mood.gloomWant"), 0)
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
        # The hall's light follows the game: the shaft from the dome narrows
        # onto the king in check; a mate makes the fires blaze; and a board
        # set down afresh is lit as a board is.
        self.until("wizardChess.world.moon.target.position.distanceTo(wizardChess.world.pieces.get(60).position) < 0.3 && "
                   "wizardChess.world.moon.angle < 0.07", "the light narrows onto the king in check", seconds=60)
        ev("wizardChess.opponent.stop(), wizardChess.begin('w', ['e2e4', 'e7e5', 'f1c4', 'b8c6', 'd1h5', 'g8f6', 'h5f7'])")
        self.assertEqual(ev("[wizardChess.game.over, wizardChess.world.mood.blaze]"), ["checkmate", 1])
        ev("wizardChess.opponent.stop(), wizardChess.begin('w'), wizardChess.game.turn++")
        self.assertEqual(ev("[wizardChess.world.mood.blaze, wizardChess.world.mood.spot, wizardChess.world.mood.gloomWant]"), [0, None, 0])
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
        # A headset's wearer riding is put at the rider's eye, turned the way
        # the knight faces (toward Black: no turn).
        place = ev("(at => [at.at.x, at.at.y, at.at.z, at.yaw])(wizardChess.seat.headsetPlace())")
        self.assertAlmostEqual(place[0], 3.0, delta=0.1)
        self.assertAlmostEqual(place[3], 0, delta=0.05)
        # And what is heard is heard from there: full strength only close by.
        self.assertEqual(ev("wizardChess.sound.near"), 2.2)
        # The knight ridden is alive: two ears, two eyes, and its ears turn
        # toward a piece that moves (the knight on b1, off to its left).
        self.assertEqual(ev("(made => [made.ears.length, made.eyes.length])(wizardChess.world.ridden.userData.saddle.userData)"), [2, 2])
        turned = ev("(w => { w.world.moving = w.world.pieces.get(1); for (let at = 0; at < 12; at++) w.world.attend(0.1); "
                    "const yaw = w.world.ridden.userData.saddle.userData.ears[1].rotation.y; w.world.moving = null; return yaw; })(wizardChess)")
        self.assertGreater(turned, 0.8)
        # The rider's own hands hold the reins, and grip while the knight moves.
        gripped = ev("(w => { const arms = w.world.ridden.userData.saddle.userData.hands; w.world.moving = w.world.ridden; "
                     "for (let at = 0; at < 10; at++) w.world.attend(0.1); const grip = arms.map(arm => arm.grip); "
                     "w.world.moving = null; return [arms.length, Math.min(...grip)]; })(wizardChess)")
        self.assertEqual(gripped[0], 2)
        self.assertGreater(gripped[1], 0.9)
        # In the saddle the plates fold.
        self.assertTrue(ev("document.body.classList.contains('riding')"))
        # "ride to f3" is the knight's move, and the rider goes with it.
        self.assertTrue(ev("wizardChess.command('ride to f3')"))
        # The rider commands with their hand: the right one points at f3.
        self.assertGreater(ev("(w => { for (let at = 0; at < 6; at++) w.world.attend(0.1); "
                              "return w.world.ridden.userData.saddle.userData.hands[1].point; })(wizardChess)"), 0.5)
        self.until("wizardChess.game.played.length >= 1 && wizardChess.world.ridden.userData.square === 21 && "
                   "Math.abs(wizardChess.seat.camera.position.x - 2.0) < 0.1 && wizardChess.seat.camera.position.z < 2.2",
                   "the rider is carried to f3", seconds=120)
        self.assertEqual(ev("wizardChess.game.played[0].san"), "Nf3")
        # The knight ridden came down on its hooves, and breathed out.
        heard = ev("wizardChess.sound.heard")
        self.assertIn(["set", "n"], heard)
        self.assertIn(["snort", "n"], heard)
        # Riding into the blow: the knight ridden takes, and as the blow lands
        # at the rider's side the world's time slows for a moment.
        ev("wizardChess.opponent.stop(), wizardChess.begin('w', ['g1f3', 'e7e5']), wizardChess.game.turn++; wizardChess.seat.blend = null")
        slowed = ev("wizardChess.pace.count")
        self.assertTrue(ev("wizardChess.command('ride takes e5')"))
        self.until(f"wizardChess.pace.count > {slowed}", "the moment slows as the blow lands", seconds=120)
        self.until("wizardChess.game.played.length >= 3", "the knight takes", seconds=120)
        self.assertEqual(ev("wizardChess.game.played[2].san"), "Nxe5")
        ev("wizardChess.opponent.stop(), wizardChess.game.turn++")
        # A blow a square away from the knight ridden rocks it, away from the blow.
        rocked = ev("(w => { const at = w.world.ridden.position.clone(); at.x += 1; w.world.shudder(at, 1); "
                    "const jolt = w.world.ridden.userData.jolt; return jolt && [jolt.much, jolt.away.x]; })(wizardChess)")
        self.assertTrue(rocked and rocked[0] > 0.05 and rocked[1] < 0, rocked)
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
        # Any of the player's pieces can be ridden (a double click on it): the
        # pawn on d2, its rider standing on its head, carried as it goes.
        ev("wizardChess.opponent.stop(), wizardChess.begin('w'), wizardChess.game.turn++")
        self.assertTrue(ev("wizardChess.rideSquare(11)"))
        ev("wizardChess.seat.blend = null")
        self.assertEqual(ev("[wizardChess.world.ridden.userData.kind, wizardChess.seat.mode, document.getElementById('seat-knight').textContent]"),
                         ["p", "ride", "On a pawn"])
        self.until("Math.abs(wizardChess.seat.camera.position.y - 1.34) < 0.06", "the eye stands on the pawn", seconds=60)
        self.assertTrue(ev("wizardChess.command('ride to d4')"))
        self.until("wizardChess.world.ridden && wizardChess.world.ridden.userData.square === 27 && "
                   "Math.abs(wizardChess.seat.camera.position.z - 0.45) < 0.15", "the pawn carries its rider to d4", seconds=120)
        ev("wizardChess.opponent.stop(), wizardChess.game.turn++")
        # A pawn ridden to the last rank carries its rider up onto what it
        # becomes. (The opponent is held still, so that its answer cannot take
        # the queen before it is looked at.)
        ev("wizardChess.heldMove = wizardChess.opponent.move; wizardChess.opponent.move = () => Promise.resolve(null)")
        ev("wizardChess.opponent.stop(), wizardChess.begin('w', ['a2a4', 'b7b5', 'a4b5', 'a7a6', 'b5a6', 'c8b7', 'a6b7', 'b8c6']), wizardChess.game.turn++")
        self.assertTrue(ev("wizardChess.rideSquare(49)"))
        ev("(w => w.make(w.moveFromUci(w.game.position, 'b7a8q'), 'w'))(wizardChess)")
        self.until("wizardChess.game.played.length >= 9 && !wizardChess.game.busy", "the pawn crowns", seconds=120)
        self.assertEqual(ev("[wizardChess.world.ridden.userData.kind, wizardChess.seat.mode, wizardChess.seat.mounted === wizardChess.world.ridden, wizardChess.game.mount]"),
                         ["q", "ride", True, 8])
        ev("wizardChess.opponent.move = wizardChess.heldMove; wizardChess.game.turn++")
        # Two players at one laptop: no engine; after each move the board is
        # handed to the side to move, each rides a knight of their own, and the
        # rider who has moved stays on their piece. A take back is one move,
        # and the board goes back to whoever made it.
        ev("wizardChess.versus('person')")
        self.assertEqual(ev("[wizardChess.game.versus, wizardChess.game.side, wizardChess.world.ridden.userData.square, document.body.classList.contains('two-players')]"),
                         ["person", "w", 6, True])
        self.assertTrue(ev("wizardChess.command('ride to f3')"))
        self.until("wizardChess.game.played.length === 1 && wizardChess.game.side === 'b' && !wizardChess.game.busy", "the board is handed to Black", seconds=120)
        self.assertEqual(ev("[wizardChess.seat.side, wizardChess.world.ridden.userData.square, wizardChess.game.thinking, "
                            "!!wizardChess.world.pieces.get(21).userData.saddle]"), ["b", 62, False, True])
        self.assertTrue(ev("wizardChess.command('ride to f6')"))
        self.until("wizardChess.game.played.length === 2 && wizardChess.game.side === 'w' && !wizardChess.game.busy", "and back to White", seconds=120)
        self.assertEqual(ev("[wizardChess.world.ridden.userData.square, !!wizardChess.world.pieces.get(45).userData.saddle]"), [21, True])
        ev("wizardChess.takeBack()")
        self.assertEqual(ev("[wizardChess.game.played.length, wizardChess.game.side, wizardChess.world.ridden.userData.square, "
                            "!!wizardChess.world.pieces.get(21).userData.saddle]"), [1, "b", 62, True])
        ev("wizardChess.versus('engine')")
        self.assertEqual(ev("[wizardChess.game.versus, wizardChess.game.side, document.body.classList.contains('two-players')]"), ["engine", "w", False])
        ev("wizardChess.opponent.stop(), wizardChess.game.turn++")
        # And back to the board.
        ev("wizardChess.ride(false)")
        self.assertEqual(ev("[wizardChess.seat.mode, wizardChess.world.ridden]"), ["table", None])
        self.until("wizardChess.sound.near === 14", "the ear goes back to the board's side", seconds=60)
        self.assertEqual(self.dev.console, [], f"the page complained: {self.dev.console}")


if __name__ == "__main__":
    unittest.main()
