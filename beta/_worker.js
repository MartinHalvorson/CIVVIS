// The whole site's request handler.
//
// The site is the same set of pages twice — one complete lane per build:
//
//   /              the STABLE lane — the promoted build, deliberately chosen:
//                  the viewer at /, the landing page at /home, the native
//                  binaries at /download
//   /test/...      the HEAD lane — the same pages, republished automatically
//                  from the latest main: /test, /test/home, /test/download
//
// Both lanes are relative-pathed, so each works at its mount point without
// rewrites, and every link inside a lane stays inside the lane — reviewing
// /test means clicking around /test and never landing in prod. The lanes
// differ only in which commit they were built from, and each states its
// commit in its own build.json.
//
// **This is a `_worker.js`, deliberately, and it must stay one.** Cloudflare
// Pages also supports a `functions/` directory, and that is the obvious way to
// write this — but `functions/` is resolved relative to the *working
// directory* wrangler is invoked from, not the directory being deployed. Run
// the deploy from anywhere but the project root and the gate is silently left
// behind: the upload succeeds, the site works, and the test lane is wide open with
// nothing to show that anything went wrong. It happened once here already,
// which is why `verify.py` now proves the routing answers rather than assuming
// it is there. A `_worker.js` sits *inside* the deployed directory and cannot
// be separated from it.

const COOKIE = "civvis_test";
const WEEK = 60 * 60 * 24 * 7;

/// What the cookie carries: proof of the password rather than the password.
async function token(password) {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(`civvis.ai/test:${password}`),
  );
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/// Compare without leaking where two strings first differ.
function sameToken(a, b) {
  if (typeof a !== "string" || typeof b !== "string" || a.length !== b.length) return false;
  let differences = 0;
  for (let i = 0; i < a.length; i++) differences |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return differences === 0;
}

function cookieValue(request, name) {
  const header = request.headers.get("Cookie") || "";
  for (const part of header.split(";")) {
    const [key, ...rest] = part.trim().split("=");
    if (key === name) return rest.join("=");
  }
  return null;
}

function askForIt(wrong) {
  const page = `<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>CIVVIS — test build</title>
<style>
  :root { color-scheme: dark; }
  * { box-sizing: border-box; }
  body {
    margin: 0; min-height: 100vh; display: flex; align-items: center; justify-content: center;
    background: radial-gradient(ellipse at 50% 0%, #10231d 0%, #07110f 60%);
    color: #e8e2d2; font-family: Georgia, "Times New Roman", serif; padding: 24px;
  }
  form { width: 100%; max-width: 340px; text-align: center; }
  h1 { margin: 0; font-size: 34px; font-weight: 400; letter-spacing: 0.34em; text-indent: 0.34em; color: #d7b66a; }
  p { margin: 10px 0 30px; font-size: 12px; letter-spacing: 0.2em; text-transform: uppercase; color: #6f8279; }
  input {
    width: 100%; padding: 13px 16px; font-size: 17px; font-family: inherit; text-align: center;
    letter-spacing: 0.3em; color: #e8e2d2; background: #0d1a17;
    border: 1px solid #24413a; border-radius: 7px; outline: none;
  }
  input:focus { border-color: #d7b66a; }
  button {
    width: 100%; margin-top: 12px; padding: 12px; font-size: 13px; font-family: inherit;
    letter-spacing: 0.24em; text-transform: uppercase; cursor: pointer;
    color: #15221c; background: #d7b66a; border: 0; border-radius: 7px;
  }
  button:hover { background: #f6e4ac; }
  .wrong { margin-top: 16px; font-size: 12px; letter-spacing: 0.14em; color: #c98b7a; }
</style></head>
<body>
  <form method="POST">
    <h1>CIVVIS</h1>
    <p>Test build</p>
    <input name="password" type="password" autofocus autocomplete="current-password"
           aria-label="Password" placeholder="password">
    <button type="submit">Enter</button>
    ${wrong ? '<div class="wrong">That is not the password.</div>' : ""}
  </form>
</body></html>`;
  return new Response(page, {
    status: wrong ? 401 : 200,
    headers: {
      "Content-Type": "text/html; charset=utf-8",
      "Cache-Control": "no-store",
      "X-Robots-Tag": "noindex",
    },
  });
}

/// File a visitor's report as a GitHub issue.
///
/// The home page's "Report Issue / Suggest Feature" dialog posts here for the
/// visitors who chose *not* to use a GitHub account — the site itself opens
/// the issue on their behalf, under a token the Pages environment names as
/// `GITHUB_ISSUE_TOKEN`. Without that token the answer is an honest 503 and
/// the dialog falls back to offering the prefilled GitHub form, so an
/// unconfigured deployment degrades to "bring your own account" rather than
/// swallowing reports.
///
/// The issue carries a `from-site` label (plus `bug` or `enhancement`), so
/// triage is one filter, and the body states the reporter's chosen name, the
/// deployed build and the browser — the context a bug report from a stranger
/// otherwise never has.
async function fileReport(request, env, url) {
  const token = env.GITHUB_ISSUE_TOKEN;
  const answer = (status, payload) =>
    new Response(JSON.stringify(payload), {
      status,
      headers: { "Content-Type": "application/json", "Cache-Control": "no-store" },
    });
  if (!token) return answer(503, { error: "reporting is not configured" });

  // A report is a couple of paragraphs. Refuse anything that is not, before
  // parsing it, and cap every field again after: limits here are the whole
  // spam defence besides the honeypot, so they are deliberately tight.
  const raw = await request.text();
  if (raw.length > 20_000) return answer(413, { error: "too long" });
  let report;
  try {
    report = JSON.parse(raw);
  } catch {
    return answer(400, { error: "not JSON" });
  }
  // The dialog ships an off-screen "website" field no person ever fills.
  // A filled one is a bot, told "thanks" and given nothing.
  if (report.website) return answer(201, { url: "" });
  const kind = report.kind === "feature" ? "feature" : "issue";
  const title = String(report.title || "").trim();
  const details = String(report.details || "").trim();
  // The name is rendered as an identity claim in the issue body, so it gets
  // word characters only — a "pen name" must not smuggle markdown, links or
  // an @-mention into bold type.
  const reporter =
    String(report.reporter || "").replace(/[^\w. -]/g, "").trim().slice(0, 60) || "anonymous";
  if (title.length < 3 || title.length > 140) return answer(400, { error: "title must be 3–140 characters" });
  if (details.length > 5000) return answer(400, { error: "details must stay under 5000 characters" });

  // One report per address per minute, kept in the edge cache. Best-effort —
  // the cache is per-colo and may evict early — but it prices out the only
  // cheap abuse: a loop hammering this endpoint from one machine.
  const ip = request.headers.get("CF-Connecting-IP") || "unknown";
  const cooldown = new Request(`https://civvis.ai/__report-cooldown/${ip}`);
  const cache = typeof caches !== "undefined" ? caches.default : null;
  if (cache) {
    if (await cache.match(cooldown)) return answer(429, { error: "one report a minute, please" });
    await cache.put(cooldown, new Response("held", { headers: { "Cache-Control": "max-age=60" } }));
  }

  // Stamp the report with the build it was filed against — the bundle's own
  // build.json says which commit is live, and a bug without a commit is a
  // guessing game. Absence (old bundle, test harness) is fine: unstamped.
  let build = "";
  try {
    const shipped = await env.ASSETS.fetch(new Request(new URL("/build.json", url)));
    if (shipped.ok) build = (await shipped.json()).short || "";
  } catch {}

  const body =
    `${details || "(no details given)"}\n\n---\n` +
    `Reported from civvis.ai by **${reporter}** via the site's report dialog (no GitHub account).\n` +
    (build ? `Build: \`${build}\`\n` : "") +
    `Browser: ${request.headers.get("User-Agent") || "unknown"}`;
  const filed = await fetch("https://api.github.com/repos/MartinHalvorson/CIVVIS/issues", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: "application/vnd.github+json",
      "Content-Type": "application/json",
      // GitHub refuses any request that does not say who is asking.
      "User-Agent": "civvis.ai report dialog",
    },
    body: JSON.stringify({
      title,
      body,
      labels: ["from-site", kind === "feature" ? "enhancement" : "bug"],
    }),
  });
  if (!filed.ok) return answer(502, { error: "GitHub would not take the issue" });
  const issue = await filed.json();
  return answer(201, { url: issue.html_url || "", number: issue.number });
}

/// Serve a file, with the headers a `_headers` file would have carried — that
/// file is ignored once a `_worker.js` exists, so the rules live here instead.
async function asset(request, env, gated) {
  const response = await env.ASSETS.fetch(request);
  const headers = new Headers(response.headers);
  const url = new URL(request.url);
  const path = url.pathname;

  headers.set("X-Content-Type-Options", "nosniff");
  headers.set("Referrer-Policy", "strict-origin-when-cross-origin");
  if (gated) headers.set("X-Robots-Tag", "noindex");

  if (path.endsWith(".wasm")) {
    // `WebAssembly.instantiateStreaming` refuses anything else.
    headers.set("Content-Type", "application/wasm");
  }
  const versionedBuildFile = url.searchParams.has("v") && (
    path.endsWith(".js") || path.endsWith(".wasm") || path.includes("/assets/")
  );
  if (versionedBuildFile) {
    // publish.sh derives `v` from these exact bytes. They may therefore live
    // in a browser cache indefinitely: changed content always has a new URL.
    headers.set("Cache-Control", "public, max-age=31536000, immutable");
  } else {
    // HTML and legacy/unversioned URLs are moving pointers. Revalidate them on
    // every visit so an old page or hand-written URL cannot pin a past build.
    headers.set("Cache-Control", "public, max-age=0, must-revalidate");
  }
  return new Response(response.body, { status: response.status, headers });
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    // `/` is the stable simulator. `ROOT_REDIRECT` in the Pages environment
    // sends the root somewhere else without a deploy — a **302**, never a 301,
    // because browsers cache a permanent redirect effectively for ever and
    // this pointer exists to be movable. (It once defaulted to the YouTube
    // channel, before the root became the product; the landing page and the
    // viewer's own header still link the channel.)
    if (url.pathname === "/" || url.pathname === "/index.html") {
      const destination = env.ROOT_REDIRECT;
      if (destination && destination !== "off") {
        return new Response(null, {
          status: 302,
          headers: { Location: destination, "Cache-Control": "no-store" },
        });
      }
    }
    // `/home`, `/download` and the whole `/test` lane need no cases here:
    // every page is a real file in the deployed tree, and Pages resolves the
    // directory indexes on its own. (`/rust` and `/wasm`, the old build
    // channels, were dropped in favour of those real addresses; they fall
    // through to the 404 like any other retired URL.)

    // The home page's report dialog files GitHub issues through here.
    if (url.pathname === "/report") {
      if (request.method !== "POST") {
        return new Response("POST only", {
          status: 405,
          headers: { Allow: "POST", "Cache-Control": "no-store" },
        });
      }
      return fileReport(request, env, url);
    }

    // /beta — the lane's name for its first day — is scrapped. Answered with
    // an explicit 404 rather than left to fall through, because falling
    // through does not do what it looks like it does: for an unknown HTML
    // path Pages serves the root index.html (its single-page-app fallback),
    // which would quietly resurrect the old name as a second copy of the
    // front page. The CI routing gate caught exactly that.
    if (url.pathname === "/beta" || url.pathname.startsWith("/beta/")) {
      return new Response("Not found", {
        status: 404,
        headers: { "Cache-Control": "no-store", "X-Robots-Tag": "noindex" },
      });
    }

    if (!url.pathname.startsWith("/test")) return asset(request, env, false);

    // The test lane is open.
    //
    // It was behind a shared password while it was a thing not meant to be
    // found. It is now where anyone watches the next build being judged, so
    // the door is only there if somebody puts it there: set `TEST_PASSWORD`
    // in the Pages environment and it closes again, with no deploy.
    //
    // There is deliberately **no fallback password**. The old one was a
    // literal in this file, in a public repository, which is not a password —
    // it is a speed bump with a published height. Either the environment
    // names a secret or there is no gate, and both of those are honest.
    //
    // What stays either way is `X-Robots-Tag: noindex` on everything under
    // /test: open to anyone following a link is not the same as wanting an
    // unfinished build to be the first search result for the project's name.
    const password = env.TEST_PASSWORD;
    if (!password) return asset(request, env, true);

    const expected = await token(password);

    if (sameToken(cookieValue(request, COOKIE), expected)) {
      return asset(request, env, true);
    }

    if (request.method === "POST") {
      const form = await request.formData().catch(() => null);
      if (form && form.get("password") === password) {
        return new Response(null, {
          status: 303,
          headers: {
            Location: url.pathname,
            "Set-Cookie":
              `${COOKIE}=${expected}; Path=/test; Max-Age=${WEEK}; ` +
              "HttpOnly; Secure; SameSite=Lax",
            "Cache-Control": "no-store",
          },
        });
      }
      return askForIt(true);
    }

    return askForIt(false);
  },
};
