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
from verify import Devtools, find_chrome, free_port, fresh_profile, tether  # noqa: E402

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

// Each lane states its own build, as the published lanes do, so a check can
// tell which lane's build.json something read.
const laneBuilds = { "/build.json": "stable01", "/test/build.json": "head0002" };
const laneBuilt = { "/build.json": "2026-11-26T17:48:29Z", "/test/build.json": "2026-11-26T18:12:03Z" };

const env = {
  ASSETS: {
    fetch: (request) => {
      const path = new URL(request.url).pathname;
      if (path in laneBuilds) {
        return new Response(JSON.stringify({ short: laneBuilds[path], built_at: laneBuilt[path] }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      }
      return new Response("asset:" + path, {
        status: 200,
        headers: { "Content-Type": "text/html" },
      });
    },
  },
};

// The report endpoint's outbound call to api.github.com, intercepted: the
// test records what would have been filed and answers with a scripted reply,
// so no real issue is ever opened by a test run.
window.githubCalls = [];
window.githubReply = {
  status: 201,
  body: { html_url: "https://github.com/MartinHalvorson/CIVVIS/issues/999", number: 999 },
};
const realFetch = globalThis.fetch.bind(globalThis);
globalThis.fetch = async (resource, init) => {
  const target = typeof resource === "string" ? resource : resource.url;
  if (target.startsWith("https://api.github.com/")) {
    const headers = (init && init.headers) || {};
    window.githubCalls.push({
      url: target,
      method: (init && init.method) || "GET",
      auth: headers.Authorization || "",
      agent: headers["User-Agent"] || "",
      payload: JSON.parse((init && init.body) || "{}"),
    });
    return new Response(JSON.stringify(window.githubReply.body), { status: window.githubReply.status });
  }
  return realFetch(resource, init);
};

// A deployment that holds `.br` copies: those are real files, anything else is
// the same stand-in as above.
const compressedEnv = {
  ASSETS: {
    fetch: (request) => {
      const path = new URL(request.url).pathname;
      return new Response("asset:" + path, {
        status: 200,
        headers: { "Content-Type": path.endsWith(".br") ? "application/octet-stream" : "text/html" },
      });
    },
  },
};

// A deployment whose lanes say which pages they carry, as published lanes do
// (`pages` in build.json): `listed` is the list per lane, "" and "/test".
// Every other address is the same stand-in as above, which is the point: the
// stand-in answers anything, as the asset layer answers a page it kept from
// an earlier deployment.
// `simulator` is, per lane, whether its build says it has the simulator's own
// pages at addresses of their own (`simulator_pages`).
const listingEnv = (listed, simulator = {}) => ({
  ASSETS: {
    fetch: (request) => {
      const path = new URL(request.url).pathname;
      if (path in laneBuilds) {
        const lane = path.slice(0, -"/build.json".length);
        const build = { short: laneBuilds[path], pages: listed[lane] };
        if (simulator[lane]) build.simulator_pages = ["debugger", "doctor-strange", "edge-of-tomorrow"];
        return new Response(JSON.stringify(build), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      }
      return new Response("asset:" + path, { status: 200, headers: { "Content-Type": "text/html" } });
    },
  },
});

window.hit = async ({ path, method = "GET", body, json, cookie, env: overrides, accepts, compressed, listed, simulator, ask }) => {
  const init = { method, redirect: "manual" };
  if (body) init.body = new URLSearchParams(body);
  if (json !== undefined) init.body = JSON.stringify(json);
  if (cookie || ask) init.headers = { ...(cookie ? { Cookie: cookie } : {}), ...(ask || {}) };
  const request = new Request("https://civvis.ai" + path, init);
  if (accepts) Object.defineProperty(request, "cf", { value: { clientAcceptEncoding: accepts } });
  const response = await worker.fetch(
    request,
    { ...env, ...(compressed ? compressedEnv : {}), ...(listed ? listingEnv(listed, simulator) : {}), ...(overrides || {}) },
  );
  return {
    status: response.status,
    location: response.headers.get("Location"),
    setCookie: response.headers.get("Set-Cookie") || "",
    contentType: response.headers.get("Content-Type"),
    robots: response.headers.get("X-Robots-Tag"),
    cacheControl: response.headers.get("Cache-Control"),
    contentEncoding: response.headers.get("Content-Encoding"),
    vary: response.headers.get("Vary"),
    etag: response.headers.get("ETag"),
    lastModified: response.headers.get("Last-Modified"),
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


def format_b36(number: int) -> str:
    digits = "0123456789abcdefghijklmnopqrstuvwxyz"
    out = ""
    while number:
        number, rest = divmod(number, 36)
        out = digits[rest] + out
    return out or "0"


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
    profile = fresh_profile("civvis-worker-profile-")
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
    tether(chrome)

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

        print("==> a page asked about again is answered 304 while its lane's build stands")
        # The edge drops the asset layer's ETag from text/html, so the worker
        # gives each page its lane's build as a validator, and a browser that
        # names it is told its copy stands, with no page sent.
        tag, since = root["etag"], root["lastModified"]
        head_page = hit(path="/test/")
        check(problems, "a page carries its lane's build as a validator",
              tag == 'W/"stable-stable01-' + format_b36(1795715309) + '"' and since == "Thu, 26 Nov 2026 17:48:29 GMT",
              f"got {tag!r} {since!r}")
        check(problems, "each lane's pages have their own validator",
              head_page["etag"] not in (None, tag) and head_page["lastModified"] == "Thu, 26 Nov 2026 18:12:03 GMT",
              f"got {head_page['etag']!r} {head_page['lastModified']!r}")
        again = hit(path="/", ask={"If-None-Match": tag})
        check(problems, "the page asked about with its validator is a 304 with no body",
              again["status"] == 304 and again["body"] == "" and again["etag"] == tag
              and "must-revalidate" in (again["cacheControl"] or ""),
              f"got {again['status']} {again['body'][:40]!r} {again['etag']!r} {again['cacheControl']!r}")
        check(problems, "a strong copy of the same validator counts too",
              hit(path="/", ask={"If-None-Match": tag.removeprefix("W/")})["status"] == 304)
        check(problems, "the stable lane's validator does not hold for the test lane",
              hit(path="/test/", ask={"If-None-Match": tag})["status"] == 200)
        check(problems, "another build's validator gets the page",
              hit(path="/", ask={"If-None-Match": 'W/"stable-stable00-0"'})["body"].strip() == "asset:/")
        check(problems, "the build's own date is a 304",
              hit(path="/", ask={"If-Modified-Since": since})["status"] == 304)
        # A lane can be promoted back to an older build: a date later than the
        # build's is no proof the copy held is the one served.
        later = hit(path="/", ask={"If-Modified-Since": "Fri, 27 Nov 2026 00:00:00 GMT"})
        check(problems, "only the build's own date counts", later["status"] == 200, f"got {later['status']}")
        check(problems, "If-None-Match outranks If-Modified-Since",
              hit(path="/", ask={"If-None-Match": 'W/"x"', "If-Modified-Since": since})["status"] == 200)
        nothing = hit(path="/charts/nope/", listed={"": ["home"], "/test": ["home"]})
        check(problems, "the 404 page has no validator", nothing["status"] == 404 and nothing["etag"] is None,
              f"got {nothing['status']} {nothing['etag']!r}")

        print("==> the game page is served at maximum compression")
        engine = hit(path="/test/civvis.wasm?v=abc", accepts="gzip, br", compressed=True)
        check(problems, "the engine is served from its .br copy",
              engine["body"] == "asset:/test/civvis.wasm.br" and engine["contentEncoding"] == "br",
              f"got {engine['body']!r} {engine['contentEncoding']!r}")
        check(problems, "the compressed engine is still application/wasm",
              engine["contentType"] == "application/wasm", f"got {engine['contentType']!r}")
        check(problems, "the compressed engine stays immutable",
              "immutable" in (engine["cacheControl"] or ""), f"got {engine['cacheControl']!r}")
        check(problems, "the copy says it varies by encoding",
              engine["vary"] == "Accept-Encoding", f"got {engine['vary']!r}")
        lane = hit(path="/test/assets/app.js?v=abc", accepts="br", compressed=True)
        check(problems, "the test lane has its own copy",
              lane["body"] == "asset:/test/assets/app.js.br" and lane["contentEncoding"] == "br"
              and "javascript" in (lane["contentType"] or ""),
              f"got {lane['body']!r} {lane['contentEncoding']!r} {lane['contentType']!r}")
        plain = hit(path="/test/civvis.wasm?v=abc", accepts="gzip", compressed=True)
        check(problems, "a browser without brotli gets the plain file",
              plain["body"] == "asset:/test/civvis.wasm" and plain["contentEncoding"] is None,
              f"got {plain['body']!r} {plain['contentEncoding']!r}")
        missing = hit(path="/test/civvis.wasm?v=abc", accepts="br")
        check(problems, "a build with no copies falls back to the plain file",
              missing["body"] == "asset:/test/civvis.wasm" and missing["contentEncoding"] is None,
              f"got {missing['body']!r} {missing['contentEncoding']!r}")
        setup = hit(path="/test/assets/app_setup.js?v=abc", accepts="br", compressed=True)
        check(problems, "every viewer script has its copy",
              setup["body"] == "asset:/test/assets/app_setup.js.br"
              and "javascript" in (setup["contentType"] or ""),
              f"got {setup['body']!r} {setup['contentType']!r}")
        shim = hit(path="/shim.js?v=abc", accepts="br", compressed=True)
        check(problems, "and the shim",
              shim["body"] == "asset:/shim.js.br" and "javascript" in (shim["contentType"] or ""),
              f"got {shim['body']!r} {shim['contentType']!r}")
        # The edge re-encodes every HTML response whatever the Worker sends,
        # so the page is left to it (publish.sh prices it at brotli 4).
        for door in ("/", "/test/", "/test/index.html"):
            page = hit(path=door + "?mode=play", accepts="br", compressed=True)
            check(problems, f"{door} looks for no copy",
                  page["body"] == "asset:" + door and page["contentEncoding"] is None,
                  f"got {page['body']!r} {page['contentEncoding']!r}")
            check(problems, f"{door} is still revalidated on every visit",
                  "must-revalidate" in (page["cacheControl"] or ""), f"got {page['cacheControl']!r}")
        art = hit(path="/test/assets/unit-glyphs.webp?v=abc", accepts="br", compressed=True)
        check(problems, "images, already compressed, look for no copy",
              art["body"] == "asset:/test/assets/unit-glyphs.webp", f"got {art['body']!r}")
        landing = hit(path="/home/", accepts="br", compressed=True)
        check(problems, "nor does a page outside the game",
              landing["body"] == "asset:/home/", f"got {landing['body']!r}")
        root = hit(path="/civvis.wasm?v=abc", accepts="br", compressed=True)
        check(problems, "the front page serves its own copy",
              root["body"] == "asset:/civvis.wasm.br" and root["contentEncoding"] == "br"
              and root["contentType"] == "application/wasm",
              f"got {root['body']!r} {root['contentEncoding']!r} {root['contentType']!r}")
        renderer = hit(path="/assets/app.js?v=abc", accepts="br", compressed=True)
        check(problems, "and its renderer",
              renderer["body"] == "asset:/assets/app.js.br" and renderer["contentEncoding"] == "br",
              f"got {renderer['body']!r} {renderer['contentEncoding']!r}")
        nested = hit(path="/home/civvis.wasm", accepts="br", compressed=True)
        check(problems, "a like-named file elsewhere looks for no copy",
              nested["body"] == "asset:/home/civvis.wasm" and nested["contentEncoding"] is None,
              f"got {nested['body']!r}")

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
        # So are the pages of Charts/, in their folder, and Tournaments. An
        # address follows the name in the site list (operator 2026-11-23:
        # "clean up the URL please. standardized. please remove research from
        # the url"), so a page of Charts/ is two names deep.
        for address, name in (
            ("/charts/win-analytics/", "Win Analytics"),
            ("/charts/elo-rankings/", "ELO Rankings"),
            ("/charts/civ-analytics/", "Civ Analytics"),
            ("/charts/civvis-performance/", "CIVVIS Performance"),
            ("/charts/civ-ai-interpretability/", "Civ AI Interpretability"),
            ("/charts/ai-insights/", "AI Insights"),
            ("/charts/unreviewed-gen-ai-charts/", "Unreviewed Gen AI Charts"),
            ("/tournaments/", "Tournaments"),
        ):
            passed = hit(path=address)
            check(
                problems,
                f"{address} passes through to the {name} page",
                passed["status"] == 200 and passed["body"].strip() == f"asset:{address}",
                f"got {passed['status']} {passed['body'][:60]!r}",
            )
        # A lane is one commit's site, and its build.json lists the pages it
        # has. The router holds an address to that list, because the files in
        # the deployment are not the last word: Pages goes on serving a file
        # it served before for up to a week after a deployment removes it.
        # The AI Interpretability page answered on the front page that way on
        # 2026-11-23, from a deployment that no longer had it. The stand-in
        # asset layer here answers every address, as that cache did.
        print("==> a page a lane does not carry is a 404, whatever the asset layer kept of it")
        # The two lanes as they stood on 2026-11-23: the front page pinned to
        # a revision from before the addresses moved, five pages at the
        # addresses they had then, and main with all six at their names
        # (Tournaments back at its first, as it is since 2026-11-24; the
        # page now named Civ AI Interpretability at the address it had as AI
        # Interpretability, until later that day).
        before = ["home", "tournaments", "research", "performance", "download"]
        after = ["home", "tournaments", "charts/win-analytics", "charts/civvis-performance",
                 "charts/ai-interpretability", "download"]
        # And a lane pinned to a revision from 2026-11-23 to 2026-11-24, when
        # the page was Auto-Tourneys, at /auto-tourneys/.
        between = ["home", "auto-tourneys", "charts/win-analytics", "charts/civvis-performance",
                   "charts/ai-interpretability", "download"]
        lanes = {"": before, "/test": after}
        for address in ("/interpretability/", "/interpretability", "/interpretability/index.html"):
            kept = hit(path=address, listed=lanes)
            check(problems, f"{address} is a 404 on a front page whose commit has no such page",
                  kept["status"] == 404 and "asset:/interpretability" not in kept["body"],
                  f"got {kept['status']} {kept['body'][:60]!r}")
        check(problems, "it answers with the 404 page, from an address that never had a file",
              kept["body"].strip() == "asset:/__no-such-page__", f"got {kept['body'][:60]!r}")
        check(problems, "and is revalidated like any page",
              "max-age=0" in (kept["cacheControl"] or ""), f"got {kept['cacheControl']!r}")
        carried = hit(path="/test/charts/ai-interpretability/", listed=lanes)
        check(problems, "the test lane, which has the page, serves it",
              carried["status"] == 200 and carried["body"].strip() == "asset:/test/charts/ai-interpretability/"
              and carried["robots"] == "noindex",
              f"got {carried['status']} {carried['body'][:60]!r}")
        for address in ("/home", "/research/", "/download/index.html", "/test/home/",
                        "/test/charts/win-analytics", "/test/charts/win-analytics/index.html",
                        "/test/tournaments/"):
            page = hit(path=address, listed=lanes)
            check(problems, f"{address} is a page its lane carries",
                  page["status"] == 200 and page["body"].strip() == f"asset:{address}",
                  f"got {page['status']} {page['body'][:60]!r}")
        # The lanes' own roots and files are no page's address.
        for address in ("/", "/test", "/test/", "/test/index.html", "/build.json", "/test/build.json",
                        "/shim.js", "/test/assets/app.js", "/civvis.wasm"):
            passed = hit(path=address, listed=lanes)
            check(problems, f"{address} is not held to the list of pages",
                  passed["status"] == 200, f"got {passed['status']} {passed['body'][:60]!r}")
        # The other way round: a page only the front page still has.
        only_stable = hit(path="/test/home/", listed={"": ["home"], "/test": []})
        check(problems, "a page the test lane does not carry is a 404 there",
              only_stable["status"] == 404 and only_stable["robots"] == "noindex",
              f"got {only_stable['status']} {only_stable['robots']!r}")
        check(problems, "while the front page keeps its own",
              hit(path="/home/", listed={"": ["home"], "/test": []})["status"] == 200)
        # Wizard Chess was a page at /wizard-chess from 2026-11-23 and was set
        # aside on 2026-11-24 (operator: "lets set aside wizard chess right
        # now. please remove from civvis.ai"). No lane lists it any more, so
        # its address is a 404 in both, whatever Pages kept of the page.
        for address in ("/wizard-chess/", "/wizard-chess", "/wizard-chess/index.html", "/test/wizard-chess/"):
            gone = hit(path=address, listed=lanes)
            check(problems, f"{address} is a 404: Wizard Chess is set aside and no lane lists it",
                  gone["status"] == 404 and gone["body"].strip() == "asset:/__no-such-page__",
                  f"got {gone['status']} {gone['body'][:60]!r}")
        # Operator, 2026-11-26: "maybe change the url for this to be of the
        # form: civvis.ai/edge-of-tomorrow. the others can be
        # civvis.ai/debugger and civvis.ai/doctor-strange". Each is its
        # lane's viewer, served at its own address, in a lane whose build
        # says it has them.
        print("==> the simulator's own pages are the viewer at addresses of their own")
        both = {"": True, "/test": True}
        for address, viewer in (("/edge-of-tomorrow", "/"), ("/debugger", "/"), ("/doctor-strange", "/"),
                                ("/test/edge-of-tomorrow", "/test/"), ("/test/debugger", "/test/"),
                                ("/test/doctor-strange", "/test/"), ("/test/edge-of-tomorrow?seed=4", "/test/")):
            page = hit(path=address, listed=lanes, simulator=both)
            check(problems, f"{address} is the viewer at {viewer}",
                  page["status"] == 200 and page["body"].strip() == f"asset:{viewer}"
                  and "max-age=0" in (page["cacheControl"] or ""),
                  f"got {page['status']} {page['body'][:60]!r} {page['cacheControl']!r}")
        check(problems, "and the test lane's are not for search engines",
              hit(path="/test/debugger", listed=lanes, simulator=both)["robots"] == "noindex")
        # No slash ends the address: the viewer's paths are lane-relative, and
        # are the lane's only from the address without one.
        for asked, to in (("/edge-of-tomorrow/", "/edge-of-tomorrow"),
                          ("/test/doctor-strange/index.html", "/test/doctor-strange"),
                          ("/test/debugger/?seed=4", "/test/debugger?seed=4")):
            moved = hit(path=asked, listed=lanes, simulator=both)
            check(problems, f"{asked} forwards to {to}", moved["status"] == 301 and moved["location"] == to,
                  f"got {moved['status']} {moved['location']!r}")
        # The old addresses are in links this site does not control: each
        # forwards to the new one, with whatever else was asked.
        for asked, to in (("/?mode=tomorrow", "/edge-of-tomorrow"), ("/test/?mode=strange", "/test/doctor-strange"),
                          ("/test/?mode=play&seed=4", "/test/debugger?seed=4"),
                          ("/test/index.html?seed=4&mode=tomorrow", "/test/edge-of-tomorrow?seed=4")):
            moved = hit(path=asked, listed=lanes, simulator=both)
            check(problems, f"{asked} forwards to {to}", moved["status"] == 301 and moved["location"] == to,
                  f"got {moved['status']} {moved['location']!r}")
        for asked, viewer in (("/test/?mode=watch", "/test/"), ("/?players=6", "/")):
            page = hit(path=asked, listed=lanes, simulator=both)
            check(problems, f"{asked} is the viewer, as it was",
                  page["status"] == 200 and page["body"].strip() == f"asset:{viewer}",
                  f"got {page['status']} {page['body'][:60]!r}")
        # A lane built before its viewer read them: the new address forwards
        # to the old, for now, and the old is served as it was.
        older = {"": False, "/test": True}
        moved = hit(path="/edge-of-tomorrow?seed=4", listed=lanes, simulator=older)
        check(problems, "on a lane from before, /edge-of-tomorrow forwards to its old address for now",
              moved["status"] == 302 and moved["location"] == "/?seed=4&mode=tomorrow",
              f"got {moved['status']} {moved['location']!r}")
        page = hit(path="/?mode=tomorrow", listed=lanes, simulator=older)
        check(problems, "and its old address is served there",
              page["status"] == 200 and page["body"].strip() == "asset:/", f"got {page['status']} {page['body'][:60]!r}")
        # An address a page has left forwards to the page, in a lane that has
        # the page at its new address. The old ones are in links this site
        # does not control, and nothing a visitor was sent should go dead.
        print("==> an address a page has left forwards to where the lane has the page")
        for old, new in (
            ("/test/research/", "/test/charts/win-analytics/"),
            ("/test/research", "/test/charts/win-analytics/"),
            ("/test/research/index.html", "/test/charts/win-analytics/"),
            ("/test/performance/", "/test/charts/civvis-performance/"),
            ("/test/interpretability/", "/test/charts/ai-interpretability/"),
            ("/test/auto-tourneys/", "/test/tournaments/"),
            # With whatever was asked of the page.
            ("/test/auto-tourneys/?series=7", "/test/tournaments/?series=7"),
            # A folder of the site list opens its first page, as Charts/
            # does in the list: the folder has no page of its own.
            ("/test/charts/", "/test/charts/win-analytics/"),
            ("/test/charts", "/test/charts/win-analytics/"),
        ):
            moved = hit(path=old, listed=lanes)
            check(problems, f"{old} forwards to {new}",
                  moved["status"] == 301 and moved["location"] == new and moved["body"] == "",
                  f"got {moved['status']} {moved['location']!r} {moved['body'][:60]!r}")
        check(problems, "a forward out of the test lane stays in it and is not indexed",
              moved["robots"] == "noindex", f"got {moved['robots']!r}")
        # Since 2026-11-24 AI Insights leads Charts/ (operator: "move ai
        # interpretablity to the top of the charts/ then civ analytics, win
        # analytics. elo rankings.. civvis performanc", and later that day
        # "we want a new ai insights", listed over them), and a lane built
        # from main lists the pages in that order, so the folder opens it.
        # The page that led the folder until then was named Civ AI
        # Interpretability the same day (operator: "please change AI
        # interpretability to "Civ AI Interpretability""), and its address
        # followed the name.
        now = ["home", "tournaments", "charts/ai-insights", "charts/civ-ai-interpretability", "charts/civ-analytics",
               "charts/win-analytics", "charts/elo-rankings", "charts/civvis-performance", "download"]
        named = {"": after, "/test": now}
        for old in ("/test/charts/", "/test/charts"):
            opened = hit(path=old, listed=named)
            check(problems, f"{old} opens AI Insights, the first page of Charts/ now",
                  opened["status"] == 301 and opened["location"] == "/test/charts/ai-insights/",
                  f"got {opened['status']} {opened['location']!r}")
        # A lane from before AI Insights was written, the same list without
        # it, still opens the page that led the folder then.
        for old in ("/test/charts/", "/test/charts"):
            opened = hit(path=old, listed={"": after, "/test": now[:2] + now[3:]})
            check(problems, f"{old} opens Civ AI Interpretability in a lane that has no AI Insights",
                  opened["status"] == 301 and opened["location"] == "/test/charts/civ-ai-interpretability/",
                  f"got {opened['status']} {opened['location']!r}")
        # So the page has moved twice, and an address it has left is followed
        # move by move to wherever the lane has it. In the lane with the new
        # name both of its old addresses forward to the new one.
        for old, new in (
            ("/test/charts/ai-interpretability/", "/test/charts/civ-ai-interpretability/"),
            ("/test/charts/ai-interpretability", "/test/charts/civ-ai-interpretability/"),
            ("/test/charts/ai-interpretability/?gene=x", "/test/charts/civ-ai-interpretability/?gene=x"),
            ("/test/interpretability/", "/test/charts/civ-ai-interpretability/"),
        ):
            moved_twice = hit(path=old, listed=named)
            check(problems, f"{old} forwards to {new} in a lane with the page's new name",
                  moved_twice["status"] == 301 and moved_twice["location"] == new and moved_twice["body"] == "",
                  f"got {moved_twice['status']} {moved_twice['location']!r} {moved_twice['body'][:60]!r}")
        served = hit(path="/test/charts/civ-ai-interpretability/", listed=named)
        check(problems, "and the page answers at its new address there",
              served["status"] == 200 and served["body"].strip() == "asset:/test/charts/civ-ai-interpretability/",
              f"got {served['status']} {served['body'][:60]!r}")
        # The front page there is pinned to a revision from before the name:
        # the page is still at the address it had, /interpretability/ still
        # forwards to that one, and the new address is a page it does not have.
        still = hit(path="/charts/ai-interpretability/", listed=named)
        check(problems, "/charts/ai-interpretability/ is still the page on a front page from before the name",
              still["status"] == 200 and still["body"].strip() == "asset:/charts/ai-interpretability/",
              f"got {still['status']} {still['location']!r} {still['body'][:60]!r}")
        first_move = hit(path="/interpretability/", listed=named)
        check(problems, "/interpretability/ forwards to /charts/ai-interpretability/ there, as it did",
              first_move["status"] == 301 and first_move["location"] == "/charts/ai-interpretability/",
              f"got {first_move['status']} {first_move['location']!r}")
        ahead = hit(path="/charts/civ-ai-interpretability/", listed=named)
        check(problems, "/charts/civ-ai-interpretability/ is a 404 on a front page from before the name",
              ahead["status"] == 404 and not ahead["location"],
              f"got {ahead['status']} {ahead['location']!r}")
        # And in a lane that has the page at none of them, each is a 404.
        for address in ("/interpretability/", "/charts/ai-interpretability/", "/charts/civ-ai-interpretability/"):
            none = hit(path=address, listed={"": ["home"], "/test": now})
            check(problems, f"{address} is a 404 in a lane that has no such page",
                  none["status"] == 404 and not none["location"],
                  f"got {none['status']} {none['location']!r}")
        # Which address a lane serves a page at follows the revision promoted
        # to it, and a promotion can go back: a browser must not remember the
        # forward as it would an ordinary permanent one.
        check(problems, "and is asked about again each time",
              "max-age=0" in (moved["cacheControl"] or ""), f"got {moved['cacheControl']!r}")
        # The front page here is pinned before the move, so there the old
        # address IS the page, and the new ones are pages it does not have.
        for address in ("/research/", "/tournaments/", "/performance/"):
            still = hit(path=address, listed=lanes)
            check(problems, f"{address} is still the page on a front page from before the move",
                  still["status"] == 200 and still["body"].strip() == f"asset:{address}",
                  f"got {still['status']} {still['location']!r} {still['body'][:60]!r}")
        for address in ("/charts/", "/charts/win-analytics/"):
            ahead = hit(path=address, listed=lanes)
            check(problems, f"{address} is a 404 on a front page from before the move",
                  ahead["status"] == 404 and not ahead["location"],
                  f"got {ahead['status']} {ahead['location']!r}")
        # /auto-tourneys/ is an address the page has left, and the page is
        # at /tournaments/ on that front page too.
        back = hit(path="/auto-tourneys/", listed=lanes)
        check(problems, "/auto-tourneys/ forwards to /tournaments/ on a front page from before the move",
              back["status"] == 301 and back["location"] == "/tournaments/",
              f"got {back['status']} {back['location']!r}")
        # A page back at an address it once left (Tournaments, at
        # /auto-tourneys/ from 2026-11-23 to 2026-11-24): a lane pinned to a
        # revision from in between keeps the page there, and the address the
        # page is back at forwards to it, as it did then.
        pinned = {"": between, "/test": after}
        kept = hit(path="/auto-tourneys/", listed=pinned)
        check(problems, "/auto-tourneys/ is the page on a front page pinned while it was there",
              kept["status"] == 200 and kept["body"].strip() == "asset:/auto-tourneys/",
              f"got {kept['status']} {kept['location']!r} {kept['body'][:60]!r}")
        for old, new in (("/tournaments/", "/auto-tourneys/"), ("/tournaments/?series=7", "/auto-tourneys/?series=7")):
            moved = hit(path=old, listed=pinned)
            check(problems, f"{old} forwards to {new} on that front page",
                  moved["status"] == 301 and moved["location"] == new,
                  f"got {moved['status']} {moved['location']!r}")
        moved = hit(path="/test/auto-tourneys/", listed=pinned)
        check(problems, "while the test lane forwards /test/auto-tourneys/ to /test/tournaments/",
              moved["status"] == 301 and moved["location"] == "/test/tournaments/",
              f"got {moved['status']} {moved['location']!r}")
        # On 2026-11-25 the page was named Experiments (operator: "rename
        # tournaments to experiments") and its address followed, and on
        # 2026-11-26 it took both back (operator: "change experiments back
        # to tournaments"). So it has had five: /tournaments/,
        # /auto-tourneys/, /tournaments/ again, /experiments/ and
        # /tournaments/ once more. A lane built from main has it at the last,
        # and the others forward there; the front page, pinned to a revision
        # from while it was Experiments until a promotion, still has it at
        # /experiments/ and serves it there, and every other address it had
        # forwards to that.
        back = ["home", "tournaments", "charts/ai-insights", "charts/civ-ai-interpretability", "charts/civ-analytics",
                "charts/win-analytics", "charts/elo-rankings", "charts/civvis-performance", "download"]
        named = ["home", "experiments", "charts/ai-insights", "charts/civ-ai-interpretability",
                 "charts/civ-analytics", "charts/win-analytics", "charts/elo-rankings",
                 "charts/civvis-performance", "download"]
        renamed = {"": named, "/test": back}
        page = hit(path="/test/tournaments/", listed=renamed)
        check(problems, "/test/tournaments/ is the page in a lane that has the name back",
              page["status"] == 200 and page["body"].strip() == "asset:/test/tournaments/",
              f"got {page['status']} {page['location']!r} {page['body'][:60]!r}")
        for old, new in (
            ("/test/experiments/", "/test/tournaments/"),
            ("/test/experiments", "/test/tournaments/"),
            ("/test/experiments/?series=7", "/test/tournaments/?series=7"),
            ("/test/auto-tourneys/", "/test/tournaments/"),
        ):
            moved = hit(path=old, listed=renamed)
            check(problems, f"{old} forwards to {new}",
                  moved["status"] == 301 and moved["location"] == new and moved["body"] == "",
                  f"got {moved['status']} {moved['location']!r} {moved['body'][:60]!r}")
        still = hit(path="/experiments/", listed=renamed)
        check(problems, "/experiments/ is still the page on a front page from while it was Experiments",
              still["status"] == 200 and still["body"].strip() == "asset:/experiments/",
              f"got {still['status']} {still['location']!r} {still['body'][:60]!r}")
        for old, new in (
            ("/tournaments/", "/experiments/"),
            ("/tournaments/?series=7", "/experiments/?series=7"),
            # Its moves end at /tournaments/, which that lane has not got,
            # and the page was at /experiments/ in between.
            ("/auto-tourneys/", "/experiments/"),
        ):
            moved = hit(path=old, listed=renamed)
            check(problems, f"{old} forwards to {new} on that front page",
                  moved["status"] == 301 and moved["location"] == new and moved["robots"] is None,
                  f"got {moved['status']} {moved['location']!r} {moved['robots']!r}")
        promoted = hit(path="/experiments/", listed={"": back, "/test": back})
        check(problems, "/experiments/ forwards to /tournaments/ once the name is promoted",
              promoted["status"] == 301 and promoted["location"] == "/tournaments/" and promoted["robots"] is None,
              f"got {promoted['status']} {promoted['location']!r} {promoted['robots']!r}")
        # And a lane pinned to the days it was Auto-Tourneys still has the
        # page at /auto-tourneys/, where /tournaments/ forwards to it as it
        # did then, and so does /experiments/, an address it had later.
        for old in ("/tournaments/", "/experiments/"):
            moved = hit(path=old, listed={"": between, "/test": back})
            check(problems, f"{old} forwards to /auto-tourneys/ on a front page pinned while the page was there",
                  moved["status"] == 301 and moved["location"] == "/auto-tourneys/",
                  f"got {moved['status']} {moved['location']!r}")
        # A name that is no address any page has had is still a 404.
        nowhere = hit(path="/test/experiment/", listed=renamed)
        check(problems, "/test/experiment/ is a 404",
              nowhere["status"] == 404 and not nowhere["location"],
              f"got {nowhere['status']} {nowhere['location']!r}")
        # Once that revision is promoted the front page forwards too, and
        # there the forward is one a search engine should follow.
        promoted = hit(path="/research/", listed={"": after, "/test": after})
        check(problems, "/research/ forwards to /charts/win-analytics/ once the move is promoted",
              promoted["status"] == 301 and promoted["location"] == "/charts/win-analytics/"
              and promoted["robots"] is None,
              f"got {promoted['status']} {promoted['location']!r} {promoted['robots']!r}")
        # Only reading is forwarded: a form posted to an old address is
        # answered as it always was.
        posted_old = hit(path="/test/research/", method="POST", body={"x": "1"}, listed=lanes)
        check(problems, "a POST to an address a page has left is not forwarded",
              posted_old["status"] == 200 and not posted_old["location"],
              f"got {posted_old['status']} {posted_old['location']!r}")
        # A name in the folder that is no page is a 404, not a forward.
        stray = hit(path="/test/charts/no-such-chart/", listed=lanes)
        check(problems, "a page the folder does not hold is a 404",
              stray["status"] == 404 and not stray["location"],
              f"got {stray['status']} {stray['location']!r}")
        # A lane that does not say what it carries is passed through: a
        # bundle from before lanes listed their pages still serves them.
        unlisted = hit(path="/interpretability/")
        check(problems, "a lane with no list is passed through",
              unlisted["status"] == 200 and unlisted["body"].strip() == "asset:/interpretability/",
              f"got {unlisted['status']} {unlisted['body'][:60]!r}")
        # Only reading is held to it; the test lane's password form posts to
        # whatever address it was shown at.
        posted = hit(path="/interpretability/", method="POST", body={"x": "1"}, listed=lanes)
        check(problems, "a POST is not answered from the list",
              posted["status"] == 200, f"got {posted['status']}")

        download = hit(path="/download/")
        check(
            problems,
            "/download/ is not gated",
            download["status"] == 200 and "asset:/download/" in download["body"],
            f"got {download['status']}",
        )

        # The old /rust and /wasm build channels are gone; they take no
        # special routing and fall through to the asset layer like any other
        # retired URL, where the deployed 404.html answers for them.
        for retired in ("/rust", "/wasm"):
            gone = hit(path=retired)
            check(problems, f"{retired} is no longer a redirect",
                  gone["status"] != 302 and not gone["location"],
                  f"got {gone['status']} {gone['location']!r}")

        print("==> so is the test lane, which is the point of publishing it")
        lane = hit(path="/test/")
        check(problems, "/test/ serves the viewer to anyone", lane["body"].strip() == "asset:/test/",
              f"got {lane['status']} {lane['body'][:60]!r}")
        check(problems, "/test/ asks for no password", "Test build" not in lane["body"])
        # Open to anyone following a link is not the same as wanting an
        # unfinished build to be the first search result for the project.
        check(problems, "the test lane is not indexed", lane["robots"] == "noindex", f"got {lane['robots']!r}")

        # Every page exists in both lanes now, and the /test prefix is the
        # whole of the difference: the worker passes /test/home and
        # /test/download through like any lane file, with the lane's noindex.
        test_home = hit(path="/test/home")
        check(problems, "/test/home passes through to the test landing page",
              test_home["status"] == 200 and test_home["body"].strip() == "asset:/test/home",
              f"got {test_home['status']} {test_home['body'][:60]!r}")
        check(problems, "the test landing page is not indexed",
              test_home["robots"] == "noindex", f"got {test_home['robots']!r}")
        for address, name in (
            ("/test/tournaments/", "Tournaments"),
            ("/test/charts/win-analytics/", "Win Analytics"),
            ("/test/charts/civvis-performance/", "CIVVIS Performance"),
            ("/test/charts/civ-ai-interpretability/", "Civ AI Interpretability"),
            ("/test/charts/ai-insights/", "AI Insights"),
            ("/test/charts/unreviewed-gen-ai-charts/", "Unreviewed Gen AI Charts"),
        ):
            passed = hit(path=address)
            check(problems, f"{address} passes through to the test {name} page",
                  passed["status"] == 200 and passed["body"].strip() == f"asset:{address}"
                  and passed["robots"] == "noindex",
                  f"got {passed['status']} {passed['body'][:60]!r} {passed['robots']!r}")
        test_download = hit(path="/test/download/")
        check(problems, "/test/download/ passes through to the test downloads page",
              test_download["status"] == 200 and "asset:/test/download/" in test_download["body"],
              f"got {test_download['status']}")

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
            atlas = hit(path=f"{prefix}/assets/unit-plates.webp")
            check(problems, f"{which} old atlas URLs are revalidated",
                  "must-revalidate" in (atlas["cacheControl"] or ""),
                  f"got {atlas['cacheControl']!r}")
            for filename in ("shim.js", "worker.js", "civvis.wasm", "assets/unit-plates.webp"):
                versioned = hit(path=f"{prefix}/{filename}?v=content-hash")
                check(problems, f"{which} versioned {filename} is immutable",
                      "max-age=31536000" in (versioned["cacheControl"] or "")
                      and "immutable" in (versioned["cacheControl"] or ""),
                      f"got {versioned['cacheControl']!r}")

        # The home page's report dialog files GitHub issues through POST
        # /report. The GitHub call itself is stubbed in the harness, so these
        # checks see exactly what would have been filed — and no test run can
        # open a real issue.
        print("==> the report dialog can file a GitHub issue")
        tokened = {"GITHUB_ISSUE_TOKEN": "a-fine-grained-token"}
        report = {
            "kind": "feature",
            "title": "A sharper minimap",
            "details": "The coastlines smear at small sizes.",
            "reporter": "swift-settler-47",
        }
        unconfigured = hit(path="/report", method="POST", json=report)
        check(problems, "no token means an honest 503",
              unconfigured["status"] == 503 and "not configured" in unconfigured["body"],
              f"got {unconfigured['status']} {unconfigured['body'][:60]!r}")
        check(problems, "an unconfigured endpoint calls nobody",
              not dev.evaluate("githubCalls.length"))

        wrong_verb = hit(path="/report", env=tokened)
        check(problems, "/report takes POST only", wrong_verb["status"] == 405,
              f"got {wrong_verb['status']}")

        filed = hit(path="/report", method="POST", json=report, env=tokened)
        check(problems, "a report is filed as an issue",
              filed["status"] == 201 and "issues/999" in filed["body"],
              f"got {filed['status']} {filed['body'][:80]!r}")
        call = dev.evaluate("githubCalls[0]") or {}
        payload = call.get("payload") or {}
        check(problems, "the issue lands on the CIVVIS repository",
              call.get("url") == "https://api.github.com/repos/MartinHalvorson/CIVVIS/issues"
              and call.get("method") == "POST",
              f"got {call.get('method')!r} {call.get('url')!r}")
        check(problems, "the token rides in the Authorization header",
              call.get("auth") == "Bearer a-fine-grained-token" and call.get("agent"),
              f"got {call.get('auth')!r} agent {call.get('agent')!r}")
        check(problems, "a feature request is labelled for triage",
              payload.get("labels") == ["from-site", "enhancement"],
              f"got {payload.get('labels')!r}")
        check(problems, "the issue signs the chosen pen name",
              payload.get("title") == "A sharper minimap"
              and "swift-settler-47" in (payload.get("body") or ""),
              f"got {payload.get('body')!r}")
        # Each lane's home page posts to its own lane, so the build an issue
        # names is the one its reporter was looking at: the promoted build
        # from the front page, never the test lane's.
        check(problems, "a report from the front page names the stable build",
              "Build: `stable01`" in (payload.get("body") or "")
              and "head0002" not in (payload.get("body") or ""),
              f"got {payload.get('body')!r}")

        bugged = hit(path="/report", method="POST",
                     json={**report, "kind": "issue", "reporter": ""}, env=tokened)
        labels = (dev.evaluate("githubCalls[1]") or {}).get("payload", {}).get("labels")
        check(problems, "an issue report is labelled a bug",
              bugged["status"] == 201 and labels == ["from-site", "bug"],
              f"got {bugged['status']} {labels!r}")
        check(problems, "no reporter signs as anonymous",
              "anonymous" in (dev.evaluate("githubCalls[1]") or {}).get("payload", {}).get("body", ""))

        # The honeypot: the dialog ships an off-screen field no person fills.
        # A filled one is a bot, thanked and given nothing.
        honeyed = hit(path="/report", method="POST",
                      json={**report, "website": "https://spam.example"}, env=tokened)
        check(problems, "a filled honeypot files nothing",
              honeyed["status"] == 201 and dev.evaluate("githubCalls.length") == 2,
              f"got {honeyed['status']} after {dev.evaluate('githubCalls.length')} calls")

        untitled = hit(path="/report", method="POST",
                       json={**report, "title": "x"}, env=tokened)
        check(problems, "a one-letter title is refused", untitled["status"] == 400,
              f"got {untitled['status']}")
        unparsed = hit(path="/report", method="POST", body={"not": "json"}, env=tokened)
        check(problems, "a non-JSON body is refused", unparsed["status"] == 400,
              f"got {unparsed['status']}")

        # The browser line is consent-gated: only a report carrying `device`
        # — the dialog's ticked checkbox — quotes it on the issue, and a
        # report without one records no browser line at all, whatever
        # User-Agent the request itself carried.
        consented = hit(path="/report", method="POST",
                        json={**report, "kind": "issue",
                              "device": "TestBrowser/9.9 (macOS)"},
                        env=tokened)
        quoted = (dev.evaluate("githubCalls[2]") or {}).get("payload", {}).get("body", "")
        check(problems, "a ticked box quotes the browser on the issue",
              consented["status"] == 201
              and "Browser: TestBrowser/9.9 (macOS)" in quoted,
              f"got {consented['status']} {quoted[-120:]!r}")
        check(problems, "an unticked box records no browser line",
              "Browser:" not in (dev.evaluate("githubCalls[0]") or {}).get("payload", {}).get("body", ""))

        dev.evaluate("githubReply.status = 401")
        refused = hit(path="/report", method="POST", json=report, env=tokened)
        check(problems, "GitHub refusing the token surfaces as 502",
              refused["status"] == 502, f"got {refused['status']}")
        dev.evaluate("githubReply.status = 201")

        # The test lane's home page files through /test/report, the same
        # endpoint under its own lane, and its issue names the test build.
        calls_before = dev.evaluate("githubCalls.length")
        lane_filed = hit(path="/test/report", method="POST", json=report, env=tokened)
        lane_body = (dev.evaluate(f"githubCalls[{calls_before}]") or {}).get("payload", {}).get("body", "")
        check(problems, "a report from the test lane is filed",
              lane_filed["status"] == 201 and "issues/999" in lane_filed["body"],
              f"got {lane_filed['status']} {lane_filed['body'][:80]!r}")
        check(problems, "a report from the test lane names the test build",
              "Build: `head0002`" in lane_body and "stable01" not in lane_body
              and "civvis.ai/test" in lane_body,
              f"got {lane_body!r}")
        check(problems, "/test/report takes POST only",
              hit(path="/test/report", env=tokened)["status"] == 405)
        check(problems, "the test lane's report endpoint is no page of the lane",
              "asset:" not in hit(path="/test/report", method="POST", json=report)["body"])

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
        check(problems, "a shut door covers every test page",
              "asset:" not in hit(path="/test/home", env=gated)["body"]
              and "asset:" not in hit(path="/test/download/", env=gated)["body"])

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
