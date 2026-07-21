"""Local HTTP server for the human-vs-AI browser GUI (zero deps).

The page speaks the same JSON action protocol as everything else:
GET /state, POST /action {"action": {...}}, POST /new {params}.
The human is always player 0; scripted AIs play the rest.
"""
import json
import os
import threading
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from .ai import make_ai
from .game import Game, IllegalAction

INDEX = os.path.join(os.path.dirname(__file__), "web", "index.html")
GAME_PARAMS = ("num_players", "width", "height", "seed", "max_turns",
               "num_city_states")


class Session:
    def __init__(self, **params):
        self.lock = threading.Lock()
        self.params = {k: v for k, v in params.items() if k in GAME_PARAMS}
        self.new_game()

    def new_game(self):
        self.game = Game(**self.params)
        self._ais = {p.id: make_ai("basic", seed=self.game.seed * 31 + p.id)
                     for p in self.game.players if p.id != 0}

    def state(self):
        from .obs import observation
        obs = observation(self.game, 0)
        obs["legal_actions"] = self.game.legal_actions(0)
        return obs

    def act(self, action):
        g = self.game
        try:
            g.apply(0, action)
        except IllegalAction as e:
            return str(e)
        if action.get("type") == "end_turn":
            guard = 0
            while (g.winner is None and g.current != 0
                   and g.players[0].alive and guard < 2 * len(g.players)):
                pid = g.current
                self._ais[pid].take_turn(g, pid)
                if g.current == pid and g.winner is None:
                    g.apply(pid, {"type": "end_turn"})
                guard += 1
        return None


class Handler(BaseHTTPRequestHandler):
    session = None

    def log_message(self, *args):
        pass

    def _json(self, obj, code=200):
        body = json.dumps(obj).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path in ("/", "/index.html"):
            with open(INDEX, "rb") as f:
                body = f.read()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == "/state":
            with self.session.lock:
                self._json(self.session.state())
        else:
            self._json({"error": "not found"}, 404)

    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0))
        try:
            body = json.loads(self.rfile.read(n) or b"{}")
        except json.JSONDecodeError:
            self._json({"error": "bad json"}, 400)
            return
        if self.path == "/action":
            with self.session.lock:
                err = self.session.act(body.get("action", {}))
                out = self.session.state()
                out["error"] = err
            self._json(out)
        elif self.path == "/new":
            with self.session.lock:
                self.session.params.update(
                    {k: int(v) for k, v in body.items() if k in GAME_PARAMS})
                self.session.new_game()
                self._json(self.session.state())
        else:
            self._json({"error": "not found"}, 404)


def make_server(port=8765, **params):
    handler = type("BoundHandler", (Handler,), {"session": Session(**params)})
    return ThreadingHTTPServer(("127.0.0.1", port), handler)


def serve(port=8765, open_browser=True, **params):
    httpd = make_server(port=port, **params)
    url = f"http://127.0.0.1:{httpd.server_address[1]}/"
    print(f"Martin Halvorson's Civilization VIS — playing at {url}")
    print("You are player 0. Ctrl+C to quit.")
    if open_browser:
        webbrowser.open(url)
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass
