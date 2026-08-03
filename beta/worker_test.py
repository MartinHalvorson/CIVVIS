#!/usr/bin/env python3
"""Exercise `beta/_worker.js` — the site's routing — without deploying it.

Everything civvis.ai does *before* a file is served lives in that one module:
the forward from the apex to the YouTube channel, the landing page's move to
`/home`, and the password on `/beta`. None of it is exercised by opening the
bundle in a browser, because a static server never runs it, so the whole file
is invisible to `verify.py`'s browser check. That is the exact shape of failure
the gate has already had once — written as a Pages `functions/` directory it was
silently left out of a deploy and the beta was wide open with nothing to say so.

`verify.py --no-gate` skips this class of check entirely, and the full check
needs `npx wrangler`, which means Node — so on a machine without it the routing
is never checked at all. This needs only Chrome and the standard library: the
module is a Workers module, and a Workers module is ES2020 plus `Request`,
`Response`, `Headers` and `crypto.subtle`, all of which a browser has. So it is
imported into a page and *called*, with `env.ASSETS` stubbed to report which
file would have been served.

    ./beta/worker_test.py
    ./beta/worker_test.py --chrome <path>
"""

from __future__ import annotations

import argparse
import functools
import http.server
import json
import pathlib
import shutil
import socketserver
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from verify import Devtools, find_chrome, free_port  # noqa: E402

CHANNEL = "https://www.youtube.com/@civvis"

# The page that loads the worker and hands it fabricated requests.
#
# `env.ASSETS` answers every request with the path it was asked for, so a test
# can tell "served the landing page" from "served the beta" without any of the
# bundle being present.
HARNESS = """<!doctype html><meta charset="utf-8"><title>worker</title>
<script type="module">
// A browser is a stand-in for the Workers runtime, and it differs in exactly
// one place that matters here: `Cookie` and `Set-Cookie` are *forbidden header
// names* on the web, so a browser drops them from any `Request` or `Response`
// built with them. Workers keeps both — cookies are the whole mechanism there.
// Left alone, that difference makes a perfectly working password gate report
// that it never sets a cookie and never sees one, which is a false alarm and,
// worse, a false all-clear if it ever went the other way.
//
// So both classes are subclassed to carry the headers the browser discards,
// and the module is imported *after* that is in place. This is the harness
// bending to match the runtime, not the module being changed to suit the
// harness — but it does mean the authority on the door is still `verify.py`'s
// `check_gate`, which runs the real thing under wrangler.
const RealRequest = Request;
const RealResponse = Response;

function collect(headers) {
  const out = new Headers();
  if (!headers) return out;
  if (typeof headers.forEach === "function") headers.forEach((v, k) => out.set(k, v));
  else for (const [k, v] of Object.entries(headers)) out.set(k, v);
  return out;
}

globalThis.Request = class extends RealRequest {
  constructor(input, init) {
    super(input, init);
    const kept = collect(super.headers);
    if (init && init.headers) collect(init.headers).forEach((v, k) => kept.set(k, v));
    else if (input instanceof RealRequest && input.__headers) {
      input.__headers.forEach((v, k) => kept.set(k, v));
    }
    this.__headers = kept;
  }
  get headers() { return this.__headers; }
};

globalThis.Response = class extends RealResponse {
  constructor(body, init) {
    super(body, init);
    this.__headers = collect(init && init.headers);
  }
  get headers() { return this.__headers; }
};

const worker = (await import("./_worker.js")).default;

const env = {
  ASSETS: {
    fetch: (request) =>
      new Response("asset:" + new URL(request.url).pathname, {
        status: 200,
        headers: { "Content-Type": "text/html" },
      }),
  },
};

window.hit = async ({ path, method = "GET", body, cookie, env: overrides }) => {
  const init = { method, redirect: "manual" };
  if (body) init.body = new URLSearchParams(body);
  if (cookie) init.headers = { Cookie: cookie };
  const response = await worker.fetch(
    new Request("https://civvis.ai" + path, init),
    { ...env, ...(overrides || {}) },
  );
  return {
    status: response.status,
    location: response.headers.get("Location"),
    setCookie: response.headers.get("Set-Cookie") || "",
    contentType: response.headers.get("Content-Type"),
    robots: response.headers.get("X-Robots-Tag"),
    cacheControl: response.headers.get("Cache-Control"),
    body: await response.text(),
  };
};
window.ready = true;
</script>
"""


class Quiet(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".js": "text/javascript",
    }

    def log_message(self, fmt, *args):
        pass


def serve(directory: pathlib.Path, port: int) -> socketserver.TCPServer:
    httpd = socketserver.TCPServer(
        ("127.0.0.1", port), functools.partial(Quiet, directory=str(directory))
    )
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    return httpd


def check(problems: list[str], name: str, condition: bool, detail: str = "") -> None:
    if condition:
        print(f"    ok   {name}")
    else:
        print(f"    FAIL {name} {detail}", file=sys.stderr)
        problems.append(name)


def main(argv: list[str] | None = None) -> int:
    here = pathlib.Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--chrome", default=find_chrome())
    # There is no password in `_worker.js` any more; this is one made up here
    # purely to prove that setting BETA_PASSWORD still closes the door.
    parser.add_argument("--password", default="not-the-real-one")
    args = parser.parse_args(argv)

    if not pathlib.Path(args.chrome).exists():
        print(f"no Chrome at {args.chrome}", file=sys.stderr)
        return 1

    stage = pathlib.Path(tempfile.mkdtemp(prefix="civvis-worker-"))
    shutil.copy(here / "_worker.js", stage / "_worker.js")
    (stage / "index.html").write_text(HARNESS, encoding="utf-8")

    port = free_port()
    httpd = serve(stage, port)
    profile = tempfile.mkdtemp(prefix="civvis-worker-profile-")
    debug_port = free_port()
    chrome = subprocess.Popen(
        [
            args.chrome,
            "--headless=new",
            f"--remote-debugging-port={debug_port}",
            f"--user-data-dir={profile}",
            "--no-first-run",
            "--disable-gpu",
            f"http://127.0.0.1:{port}/",
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    problems: list[str] = []
    try:
        target = None
        deadline = time.time() + 40
        while time.time() < deadline and not target:
            try:
                pages = json.load(urllib.request.urlopen(f"http://127.0.0.1:{debug_port}/json", timeout=2))
                target = next((p for p in pages if p.get("type") == "page" and p.get("webSocketDebuggerUrl")), None)
            except Exception:
                time.sleep(0.5)
        if not target:
            print("Chrome never offered a debuggable page", file=sys.stderr)
            return 1

        dev = Devtools(target["webSocketDebuggerUrl"])
        dev.call("Runtime.enable")
        dev.call("Page.enable")

        deadline = time.time() + 30
        while time.time() < deadline and not dev.evaluate("window.ready === true"):
            time.sleep(0.3)
        if not dev.evaluate("window.ready === true"):
            detail = "; ".join(dev.console[-3:]) or "no error reported"
            print(f"_worker.js did not load: {detail}", file=sys.stderr)
            return 1
        print("==> _worker.js parses and exports a fetch handler")

        def hit(**kwargs):
            return dev.evaluate(f"hit({json.dumps(kwargs)})")

        print("==> the apex forwards to the channel")
        root = hit(path="/")
        check(problems, "/ redirects", root["status"] in (301, 302, 303, 307, 308), f"got {root['status']}")
        check(problems, "/ points at the channel", root["location"] == CHANNEL, f"got {root['location']!r}")
        # A 301 is cached by browsers effectively for ever; the day this becomes
        # a real front page, every past visitor would still land on YouTube.
        check(problems, "/ forwards temporarily, not permanently", root["status"] != 301)

        override = hit(path="/", env={"ROOT_REDIRECT": "off"})
        check(
            problems,
            "ROOT_REDIRECT=off serves the landing page instead",
            override["status"] == 200 and "asset:/" in override["body"],
            f"got {override['status']} {override['body'][:60]!r}",
        )
        elsewhere = hit(path="/", env={"ROOT_REDIRECT": "https://example.com/"})
        check(problems, "ROOT_REDIRECT retargets the forward", elsewhere["location"] == "https://example.com/")

        print("==> the pages that are meant to be public are public")
        home = hit(path="/home")
        check(
            problems,
            "/home serves the landing page",
            home["status"] == 200 and home["body"].strip() == "asset:/",
            f"got {home['status']} {home['body'][:60]!r}",
        )
        download = hit(path="/download/")
        check(
            problems,
            "/download/ is not gated",
            download["status"] == 200 and "asset:/download/" in download["body"],
            f"got {download['status']}",
        )

        print("==> stable build channels follow the latest published artifacts")
        rust = hit(path="/rust")
        check(problems, "/rust points at the latest native release",
              rust["status"] == 302 and rust["location"] == "/download/",
              f"got {rust['status']} {rust['location']!r}")
        check(problems, "/rust is temporary and uncached",
              rust["status"] != 301 and rust["cacheControl"] == "no-store",
              f"got {rust['status']} {rust['cacheControl']!r}")
        rust_slash = hit(path="/rust/")
        check(problems, "/rust/ is the same native channel",
              rust_slash["location"] == "/download/",
              f"got {rust_slash['location']!r}")

        wasm = hit(path="/wasm?game=7311")
        check(problems, "/wasm points at the latest WASM build",
              wasm["status"] == 302 and wasm["location"] == "/beta/?game=7311",
              f"got {wasm['status']} {wasm['location']!r}")
        check(problems, "/wasm is temporary and uncached",
              wasm["status"] != 301 and wasm["cacheControl"] == "no-store",
              f"got {wasm['status']} {wasm['cacheControl']!r}")
        wasm_slash = hit(path="/wasm/")
        check(problems, "/wasm/ is the same browser channel",
              wasm_slash["location"] == "/beta/",
              f"got {wasm_slash['location']!r}")

        print("==> so is the beta, which is the point of publishing it")
        beta = hit(path="/beta/")
        check(problems, "/beta/ serves the viewer to anyone", beta["body"].strip() == "asset:/beta/",
              f"got {beta['status']} {beta['body'][:60]!r}")
        check(problems, "/beta/ asks for no password", "Beta build" not in beta["body"])
        # Open to anyone following a link is not the same as wanting an
        # unfinished build to be the first search result for the project.
        check(problems, "the beta is not indexed", beta["robots"] == "noindex", f"got {beta['robots']!r}")

        module = hit(path="/beta/civvis.wasm")
        # instantiateStreaming refuses anything else.
        check(problems, "the module is served as application/wasm",
              module["contentType"] == "application/wasm", f"got {module['contentType']!r}")
        check(problems, "the module is revalidated rather than trusted",
              "must-revalidate" in (module["cacheControl"] or ""), f"got {module['cacheControl']!r}")
        atlas = hit(path="/beta/assets/feature-atlas.webp")
        check(problems, "atlases are cached", "max-age=86400" in (atlas["cacheControl"] or ""),
              f"got {atlas['cacheControl']!r}")

        # The door still exists; it is just not shut unless somebody shuts it.
        # This is checked because an unused capability is one that has quietly
        # stopped working, and the day it is wanted is not the day to find out.
        print("==> BETA_PASSWORD closes it again")
        gated = {"BETA_PASSWORD": args.password}
        closed = hit(path="/beta/", env=gated)
        check(problems, "a set password shuts the door", "Beta build" in closed["body"],
              f"got {closed['body'][:60]!r}")
        check(problems, "a shut door serves no viewer", "asset:" not in closed["body"])
        check(problems, "a shut door hides the engine too",
              "asset:" not in hit(path="/beta/civvis.wasm", env=gated)["body"])

        wrong = hit(path="/beta/", method="POST", body={"password": "0000"}, env=gated)
        check(problems, "a wrong password is refused", wrong["status"] == 401, f"got {wrong['status']}")
        check(problems, "a wrong password sets no cookie", not wrong["setCookie"])

        right = hit(path="/beta/", method="POST", body={"password": args.password}, env=gated)
        check(problems, "the password is accepted", right["status"] == 303, f"got {right['status']}")
        cookie = (right["setCookie"] or "").split(";")[0]
        check(problems, "the cookie is HttpOnly and Secure",
              "HttpOnly" in (right["setCookie"] or "") and "Secure" in (right["setCookie"] or ""))
        # What the cookie carries has to be proof of the password, never the
        # password: it is readable by anything that can read the response.
        check(problems, "the cookie is not the password itself", args.password not in cookie, f"got {cookie!r}")
        check(problems, "the cookie opens it",
              hit(path="/beta/", cookie=cookie, env=gated)["body"].strip() == "asset:/beta/")
        check(problems, "a forged cookie does not",
              "asset:" not in hit(path="/beta/", cookie="civvis_beta=" + "0" * 64, env=gated)["body"])
    finally:
        chrome.terminate()
        try:
            chrome.wait(timeout=15)
        except subprocess.TimeoutExpired:
            chrome.kill()
        httpd.shutdown()

    print()
    if problems:
        print(f"FAILED: {len(problems)} routing checks — {', '.join(problems)}", file=sys.stderr)
        return 1
    print("the site routes correctly.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
