#!/usr/bin/env python3
"""Serve the local WASM channel and persist its finished games."""

import functools
import http.server
import json
import pathlib
import socketserver
import subprocess
import sys
import urllib.parse


MAX_RESULT_BYTES = 64 * 1024


def channel_path(path):
    """Map the local /wasm channel onto the published viewer.

    The viewer sits at the lane root now — publish.sh emits one complete
    lane per revision, viewer at its root with /home and /download beside it
    — so the channel maps straight onto the root rather than a test/
    subdirectory.
    """
    parsed = urllib.parse.urlsplit(path)
    if parsed.path == "/wasm":
        mapped = "/"
    elif parsed.path.startswith("/wasm/"):
        mapped = "/" + parsed.path.removeprefix("/wasm/")
    else:
        return path
    return urllib.parse.urlunsplit(("", "", mapped, parsed.query, parsed.fragment))


class RatingHost:
    """Thin trusted adapter around the native league implementation."""

    def __init__(self, binary, league):
        self.binary = pathlib.Path(binary).resolve()
        self.league = pathlib.Path(league).resolve()

    def _run(self, command, *, report=None):
        result = subprocess.run(
            (str(self.binary), command, "--league", str(self.league)),
            input=report,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
            check=False,
        )
        if result.returncode != 0:
            detail = result.stderr.strip() or result.stdout.strip() or "native helper failed"
            raise RuntimeError(detail[-2000:])
        try:
            answer = json.loads(result.stdout)
        except json.JSONDecodeError as error:
            raise RuntimeError("native helper returned invalid JSON") from error
        if not isinstance(answer, dict):
            raise RuntimeError("native helper returned a non-object")
        return answer

    def initialize(self):
        answer = self._run("league-init")
        if answer.get("status") != "ready":
            raise RuntimeError("native helper did not initialize the league")
        return answer

    def record(self, report):
        answer = self._run("rate-game", report=report)
        if answer.get("status") not in {"recorded", "duplicate"}:
            raise RuntimeError("native helper did not accept the result")
        return answer

    def roster(self):
        return (self.league / "league.json").read_bytes()


class Handler(http.server.SimpleHTTPRequestHandler):
    # Python's table has no entry for wasm, and `instantiateStreaming` refuses
    # anything that is not application/wasm.
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".json": "application/json",
        ".webp": "image/webp",
    }

    def __init__(self, *args, rating_host, **kwargs):
        self.rating_host = rating_host
        super().__init__(*args, **kwargs)

    def translate_path(self, path):
        return super().translate_path(channel_path(path))

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def _json(self, status, value):
        body = json.dumps(value, separators=(",", ":")).encode("utf-8") + b"\n"
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if urllib.parse.urlsplit(self.path).path != "/wasm/league.json":
            return super().do_GET()
        try:
            body = self.rating_host.roster()
        except OSError as error:
            return self._json(503, {"error": str(error)})
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        if urllib.parse.urlsplit(self.path).path != "/wasm/league-result":
            return self._json(404, {"error": "unknown local host route"})
        # A different website must not be able to spend localhost's authority
        # merely because a CIVVIS desktop server happens to be open.
        origin = self.headers.get("Origin")
        own_origin = "http://" + self.headers.get("Host", "")
        if origin is not None and origin != own_origin:
            return self._json(403, {"error": "foreign origin"})
        try:
            length = int(self.headers.get("Content-Length", ""))
        except ValueError:
            return self._json(400, {"error": "invalid content length"})
        if not 0 < length <= MAX_RESULT_BYTES:
            return self._json(413, {"error": "result body is empty or too large"})
        raw = self.rfile.read(length)
        try:
            report = raw.decode("utf-8")
            parsed = json.loads(report)
        except (UnicodeDecodeError, json.JSONDecodeError):
            return self._json(400, {"error": "result body is not JSON"})
        if not isinstance(parsed, dict):
            return self._json(400, {"error": "result body is not an object"})
        try:
            answer = self.rating_host.record(report)
        except (OSError, RuntimeError, subprocess.TimeoutExpired) as error:
            return self._json(503, {"error": str(error)})
        self._json(200, answer)

    def log_message(self, fmt, *args):
        sys.stderr.write("  %s\n" % (fmt % args))


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


def main():
    if len(sys.argv) != 5:
        raise SystemExit(
            "usage: serve.py SITE_ROOT PORT RATING_BINARY LEAGUE_DIRECTORY"
        )
    root, port, binary, league = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]
    rating_host = RatingHost(binary, league)
    try:
        rating_host.initialize()
    except (OSError, RuntimeError, subprocess.TimeoutExpired) as error:
        raise SystemExit(f"cannot initialize the live league: {error}") from error
    handler = functools.partial(Handler, directory=root, rating_host=rating_host)
    with Server(("127.0.0.1", port), handler) as server:
        server.serve_forever()


if __name__ == "__main__":
    main()
