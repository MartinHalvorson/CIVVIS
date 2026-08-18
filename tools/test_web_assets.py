#!/usr/bin/env python3
"""The browser client's first automated check.

civvis.ai is one of two shipped products and its 30,738-line renderer had no
verification of any kind — no lint, no syntax check, no test. That absence is
why nobody splits `app.js`: the roadmap has listed it as a conflict hotspot for
weeks while the two Rust hotspots were dealt with, because a careless carve
breaks a live site and nothing would catch it.

The failure is not hypothetical and the supervisor's own header records it: "one
bad top-level lookup blanks the whole map — the sidebar, buttons and title still
paint, so it reads as 'CIVVIS is up but the game is not showing'. Cost most of
an afternoon on 2026-08-10."

## The invariant a split actually trips

A script the page loads has to be named in FOUR places, and missing any one of
them serves a page that 404s a script and paints half a client:

1. `<script src="/assets/NAME.js">` in `web/index.html`
2. `include_str!("../web/assets/NAME.js")` in `src/server.rs`, so the binary
   carries it
3. a `("GET", "/assets/NAME.js")` route arm, so the binary hands it over
4. the file itself on disk

These check all four agree, in both directions, so adding a script and
forgetting to serve it fails here instead of in a browser.

## And the hazard the split itself creates

Every one of these scripts is a classic script sharing one global scope. Two
`const` declarations of the same name across two files is a `SyntaxError` that
kills the whole page — not a shadowed variable, not a warning. Splitting a file
by moving declarations is exactly the operation that can produce one.
"""

from __future__ import annotations

import re
import shutil
import subprocess
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
INDEX = REPO / "web" / "index.html"
ASSETS = REPO / "web" / "assets"
SERVER = REPO / "src" / "server.rs"

# A top-level declaration: `const X`, `let X`, `var X`, `function X`, `class X`
# written at column zero. Anything indented is inside a scope and cannot collide.
TOP_LEVEL = re.compile(r"^(?:const|let|var|function|class)\s+([A-Za-z_$][\w$]*)")
# `const` and `let` collide across scripts; `var` and `function` redeclare
# harmlessly, which is why the old inline controls could ever have worked.
FATAL_KINDS = re.compile(r"^(?:const|let|class)\s+([A-Za-z_$][\w$]*)")


def page_scripts() -> list[str]:
    """`/assets/*.js` sources the page loads, in load order."""
    html = INDEX.read_text(encoding="utf-8")
    return re.findall(r'<script src="/assets/([A-Za-z0-9_.-]+\.js)"', html)


def embedded_scripts() -> set[str]:
    text = SERVER.read_text(encoding="utf-8")
    return set(re.findall(r'include_str!\("\.\./web/assets/([A-Za-z0-9_.-]+\.js)"\)', text))


def served_scripts() -> set[str]:
    text = SERVER.read_text(encoding="utf-8")
    return set(re.findall(r'\("GET", "/assets/([A-Za-z0-9_.-]+\.js)"\)', text))


def declarations(path: Path, pattern: re.Pattern) -> dict[str, int]:
    names: dict[str, int] = {}
    for number, line in enumerate(path.read_text(encoding="utf-8").split("\n"), 1):
        match = pattern.match(line)
        if match:
            names.setdefault(match.group(1), number)
    return names


class TheScriptChainAgrees(unittest.TestCase):
    def test_the_page_loads_at_least_the_renderer(self):
        scripts = page_scripts()
        self.assertIn("app.js", scripts, "the page stopped loading the renderer")

    def test_every_script_the_page_loads_exists_on_disk(self):
        for name in page_scripts():
            self.assertTrue(
                (ASSETS / name).is_file(),
                f"index.html loads /assets/{name}, which is not in web/assets/",
            )

    def test_every_script_the_page_loads_is_embedded_in_the_binary(self):
        missing = [n for n in page_scripts() if n not in embedded_scripts()]
        self.assertEqual(
            missing,
            [],
            f"index.html loads {missing} but server.rs does not include_str! them, "
            f"so a published build ships a page that cannot find its own script.",
        )

    def test_every_script_the_page_loads_has_a_route(self):
        missing = [n for n in page_scripts() if n not in served_scripts()]
        self.assertEqual(
            missing,
            [],
            f"index.html loads {missing} but server.rs has no "
            f'("GET", "/assets/NAME") arm, so the browser gets a 404 and the page '
            f"paints without them.",
        )

    def test_nothing_is_embedded_that_the_page_never_loads(self):
        # Dead weight in the binary, and a sign a script was removed from the
        # page without being removed from the server.
        orphans = sorted(embedded_scripts() - set(page_scripts()))
        self.assertEqual(
            orphans,
            [],
            f"server.rs embeds {orphans}, which index.html never loads",
        )


class OneGlobalScope(unittest.TestCase):
    """Every served script shares one scope, so a repeated name is fatal."""

    def test_no_const_or_class_name_is_declared_in_two_scripts(self):
        seen: dict[str, tuple[str, int]] = {}
        clashes = []
        for name in page_scripts():
            for symbol, line in declarations(ASSETS / name, FATAL_KINDS).items():
                if symbol in seen:
                    first_file, first_line = seen[symbol]
                    clashes.append(
                        f"{symbol}: {first_file}:{first_line} and {name}:{line}"
                    )
                else:
                    seen[symbol] = (name, line)
        self.assertEqual(
            clashes,
            [],
            "these names are declared at the top level of two scripts that share "
            "one global scope. `const`/`let`/`class` redeclaration is a "
            "SyntaxError that kills the whole page, not a shadowed variable:\n  "
            + "\n  ".join(clashes),
        )

    def test_no_script_declares_the_same_name_twice_itself(self):
        """Redeclaration inside one file is the same SyntaxError.

        Missed by the cross-file check on the first attempt, and found by
        reintroducing the bug: a name moved OUT of app.js and then re-added to
        the file it moved to is a duplicate within one script, which the
        cross-file comparison cannot see.
        """
        for name in page_scripts():
            counts: dict[str, list[int]] = {}
            path = ASSETS / name
            for number, line in enumerate(path.read_text().split("\n"), 1):
                match = FATAL_KINDS.match(line)
                if match:
                    counts.setdefault(match.group(1), []).append(number)
            repeats = {k: v for k, v in counts.items() if len(v) > 1}
            self.assertEqual(
                repeats,
                {},
                f"{name} declares these names twice at the top level, which is a "
                f"SyntaxError that kills the page: {repeats}",
            )

    def test_the_split_is_measured_so_it_can_be_continued(self):
        # Not a threshold to satisfy — a number in the record, so the next carve
        # can see whether it moved.
        sizes = {n: len((ASSETS / n).read_text().split("\n")) for n in page_scripts()}
        biggest = max(sizes.values())
        self.assertGreater(
            len(sizes), 1, "the client is a single script again; the carve was reverted"
        )
        self.assertLess(
            biggest,
            40_000,
            f"a client script reached {biggest} lines: {sizes}",
        )


class ThePublishPipelineVisitsEveryScript(unittest.TestCase):
    """A carve must not strand a root-absolute asset reference in a lane.

    `web/index.html` and the scripts ask for assets from the site root, which
    is where a desktop build serves them. Published, they sit beside the page
    instead, so `beta/publish.sh` rewrites `"/assets/` to `"assets/`.

    That rewrite named ONE script. The day the renderer was carved further,
    `app_palette.js` carried `"/assets/feature-atlas.png"` and two more like
    it, and a script the rewrite never visited would have kept them
    root-absolute: published at /test they resolve against the site root, 404,
    and the terrain atlases silently do not load. `beta/verify.py` would not
    have caught it either — it checks `index.html` for surviving `"/assets/`
    and nothing else.

    Found before it shipped, and gated here so the next carve cannot repeat it.
    """

    PUBLISH = REPO / "beta" / "publish.sh"
    CACHE_BUST = REPO / "beta" / "cache_bust.py"

    def test_the_rewrite_reads_every_script_not_a_named_one(self):
        source = self.PUBLISH.read_text(encoding="utf-8")
        self.assertIn(
            'assets.glob("*.js")',
            source,
            "beta/publish.sh rewrites a hardcoded script instead of every one "
            "the lane ships; a carved-out script keeps its root-absolute asset "
            "references and 404s in a published lane",
        )
        self.assertNotIn(
            'app_js = assets / "app.js"',
            source,
            "beta/publish.sh is back to naming one script",
        )

    def test_cache_busting_versions_every_script(self):
        source = self.CACHE_BUST.read_text(encoding="utf-8")
        self.assertIn(
            'glob("*.js")',
            source,
            "beta/cache_bust.py versions the atlas references of one named "
            "script, so a carved-out script's atlases keep a stale cache",
        )

    def test_a_script_with_root_absolute_assets_is_actually_rewritten(self):
        # The reason the two checks above matter: these files really do carry
        # root-absolute references, so a pipeline that skips one ships a 404.
        carrying = [
            name
            for name in page_scripts()
            if '"/assets/' in (ASSETS / name).read_text(encoding="utf-8")
        ]
        self.assertTrue(
            carrying,
            "no served script references /assets/ any more; if that is real, "
            "the two checks above are guarding something that no longer exists",
        )


NODE = shutil.which("node")


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class EveryScriptParses(unittest.TestCase):
    """`node --check` is the cheapest real check this client has ever had."""

    def test_each_script_the_page_loads_parses(self):
        for name in page_scripts():
            done = subprocess.run(
                [NODE, "--check", str(ASSETS / name)],
                capture_output=True,
                text=True,
                timeout=120,
            )
            self.assertEqual(
                done.returncode,
                0,
                f"web/assets/{name} does not parse:\n{done.stderr.strip()}",
            )


if __name__ == "__main__":
    unittest.main()
