#!/usr/bin/env python3
"""Exercise `beta/_worker.js` — the site's routing — without deploying it.

Everything civvis.ai does *before* a file is served lives in that one module:
the forward from the apex to the YouTube channel, the landing page's move to
`/home`, and the password on `/test`. None of it is exercised by opening the
bundle in a browser, because a static server never runs it, so the whole file
is invisible to `verify.py`'s browser check. That is the exact shape of failure
the gate has already had once — written as a Pages `functions/` directory it was
silently left out of a deploy and the test lane was wide open with nothing to say so.

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
import re
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

# The page that loads the worker and hands it fabricated requests.
#
# `env.ASSETS` answers every request with the path it was asked for, so a test
# can tell "served the landing page" from "served the test lane" without any of the
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


def engine_route_drift(here: pathlib.Path) -> list[str]:
    """Routes the wasm engine answers that the shim would send to the network.

    `src/wasm.rs` and `ENGINE_ROUTES` in `shim.js` describe the same boundary
    from opposite sides, and nothing at build time ties them together. A route
    added engine-side only is not a broken button: the published page's request
    escapes to the real civvis.ai, 404s, and whatever polls it retries for
    ever — the machine-metrics poll shipped exactly that way. The shim is
    allowed routes the engine lacks (it answers `/saves` from browser
    storage), so only the engine-side surplus counts as drift.
    """
    wasm = (here.parent / "src" / "wasm.rs").read_text(encoding="utf-8")
    shim = (here / "shim.js").read_text(encoding="utf-8")
    engine = set(re.findall(r'\("(?:GET|POST)",\s*"(/[^"]+)"\)', wasm))
    listed = re.search(r"ENGINE_ROUTES = new Set\(\[(.*?)\]\)", shim, re.DOTALL)
    shimmed = set(re.findall(r'"(/[^"]+)"', listed.group(1))) if listed else set()
    return sorted(engine - shimmed)


def main(argv: list[str] | None = None) -> int:
    here = pathlib.Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--chrome", default=find_chrome())
    # There is no password in `_worker.js` any more; this is one made up here
    # purely to prove that setting TEST_PASSWORD still closes the door.
    parser.add_argument("--password", default="not-the-real-one")
    args = parser.parse_args(argv)

    print("==> the shim intercepts every route the engine answers")
    drift = engine_route_drift(here)
    if drift:
        print(
            f"    FAIL shim.js ENGINE_ROUTES is missing {', '.join(drift)}",
            file=sys.stderr,
        )
        return 1
    print("    ok   ENGINE_ROUTES covers src/wasm.rs")

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

        print("==> the apex is the stable simulator")
        root = hit(path="/")
        check(
            problems,
            "/ serves the stable lane",
            root["status"] == 200 and root["body"].strip() == "asset:/",
            f"got {root['status']} {root['body'][:60]!r}",
        )
        check(problems, "/ is indexable", root["robots"] is None, f"got {root['robots']!r}")

        # The root once forwarded to the channel by default; now it only
        # forwards if somebody sets the pointer. A stray ROOT_REDIRECT in the
        # Pages environment is therefore the failure mode to keep visible.
        elsewhere = hit(path="/", env={"ROOT_REDIRECT": "https://example.com/"})
        check(problems, "ROOT_REDIRECT turns the root into a forward",
              elsewhere["status"] == 302 and elsewhere["location"] == "https://example.com/",
              f"got {elsewhere['status']} {elsewhere['location']!r}")
        check(problems, "that forward is temporary, not permanent", elsewhere["status"] != 301)
        off = hit(path="/", env={"ROOT_REDIRECT": "off"})
        check(problems, "ROOT_REDIRECT=off still serves the site",
              off["status"] == 200 and off["body"].strip() == "asset:/")

        print("==> the pages that are meant to be public are public")
        # /home is a real directory now; the worker just passes it through and
        # Pages resolves home/index.html on its own.
        home = hit(path="/home")
        check(
            problems,
            "/home passes through to the landing page",
            home["status"] == 200 and home["body"].strip() == "asset:/home",
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
              wasm["status"] == 302 and wasm["location"] == "/test/?game=7311",
              f"got {wasm['status']} {wasm['location']!r}")
        check(problems, "/wasm is temporary and uncached",
              wasm["status"] != 301 and wasm["cacheControl"] == "no-store",
              f"got {wasm['status']} {wasm['cacheControl']!r}")
        wasm_slash = hit(path="/wasm/")
        check(problems, "/wasm/ is the same browser channel",
              wasm_slash["location"] == "/test/",
              f"got {wasm_slash['location']!r}")

        print("==> so is the test lane, which is the point of publishing it")
        lane = hit(path="/test/")
        check(problems, "/test/ serves the viewer to anyone", lane["body"].strip() == "asset:/test/",
              f"got {lane['status']} {lane['body'][:60]!r}")
        check(problems, "/test/ asks for no password", "Test build" not in lane["body"])
        # Open to anyone following a link is not the same as wanting an
        # unfinished build to be the first search result for the project.
        check(problems, "the test lane is not indexed", lane["robots"] == "noindex", f"got {lane['robots']!r}")

        # /beta — the lane's day-one name — is scrapped: an explicit 404 from
        # the worker itself. It cannot merely fall through, because for an
        # unknown HTML path the asset layer serves the front page (Pages'
        # single-page-app fallback) — the CI routing gate caught the old name
        # answering with a working copy of the site.
        for scrapped_path in ("/beta", "/beta/", "/beta/civvis.wasm"):
            scrapped = hit(path=scrapped_path)
            check(problems, f"{scrapped_path} is scrapped",
                  scrapped["status"] == 404 and not scrapped["location"],
                  f"got {scrapped['status']} {scrapped['location']!r}")

        # The same cache rules hold in both lanes. Moving entry points and old
        # unversioned dependency URLs revalidate; generated content-addressed
        # URLs are immutable because changed bytes always get a different URL.
        for prefix, which in (("", "the stable lane's"), ("/test", "the test lane's")):
            module = hit(path=f"{prefix}/civvis.wasm")
            check(problems, f"{which} module is served as application/wasm",
                  module["contentType"] == "application/wasm", f"got {module['contentType']!r}")
            check(problems, f"{which} module is revalidated rather than trusted",
                  "must-revalidate" in (module["cacheControl"] or ""), f"got {module['cacheControl']!r}")
            atlas = hit(path=f"{prefix}/assets/feature-atlas.webp")
            check(problems, f"{which} old atlas URLs are revalidated",
                  "must-revalidate" in (atlas["cacheControl"] or ""),
                  f"got {atlas['cacheControl']!r}")
            for filename in ("shim.js", "worker.js", "civvis.wasm", "assets/feature-atlas.webp"):
                versioned = hit(path=f"{prefix}/{filename}?v=content-hash")
                check(problems, f"{which} versioned {filename} is immutable",
                      "max-age=31536000" in (versioned["cacheControl"] or "")
                      and "immutable" in (versioned["cacheControl"] or ""),
                      f"got {versioned['cacheControl']!r}")

        # The door still exists; it is just not shut unless somebody shuts it.
        # This is checked because an unused capability is one that has quietly
        # stopped working, and the day it is wanted is not the day to find out.
        print("==> TEST_PASSWORD closes it again")
        gated = {"TEST_PASSWORD": args.password}
        closed = hit(path="/test/", env=gated)
        check(problems, "a set password shuts the door", "Test build" in closed["body"],
              f"got {closed['body'][:60]!r}")
        check(problems, "a shut door serves no viewer", "asset:" not in closed["body"])
        check(problems, "a shut door hides the engine too",
              "asset:" not in hit(path="/test/civvis.wasm", env=gated)["body"])

        wrong = hit(path="/test/", method="POST", body={"password": "0000"}, env=gated)
        check(problems, "a wrong password is refused", wrong["status"] == 401, f"got {wrong['status']}")
        check(problems, "a wrong password sets no cookie", not wrong["setCookie"])

        right = hit(path="/test/", method="POST", body={"password": args.password}, env=gated)
        check(problems, "the password is accepted", right["status"] == 303, f"got {right['status']}")
        cookie = (right["setCookie"] or "").split(";")[0]
        check(problems, "the cookie is HttpOnly and Secure",
              "HttpOnly" in (right["setCookie"] or "") and "Secure" in (right["setCookie"] or ""))
        # What the cookie carries has to be proof of the password, never the
        # password: it is readable by anything that can read the response.
        check(problems, "the cookie is not the password itself", args.password not in cookie, f"got {cookie!r}")
        check(problems, "the cookie opens it",
              hit(path="/test/", cookie=cookie, env=gated)["body"].strip() == "asset:/test/")
        check(problems, "a forged cookie does not",
              "asset:" not in hit(path="/test/", cookie="civvis_test=" + "0" * 64, env=gated)["body"])
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
