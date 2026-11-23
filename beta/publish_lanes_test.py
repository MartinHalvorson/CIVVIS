#!/usr/bin/env python3
"""A lane is one revision's site and nothing else: `publish.sh`, run for real.

The site is two whole sets of pages, the promoted revision's at `/` and
main's at `/test` (operator 2026-11-22: "1 test set of everything and 1 real
prod set of everything"). Until that day the script that builds a lane took
several things from the tree it was run from, which is main's, whatever
revision the lane named: `shim.js` and `worker.js`, the scripts that fill the
pages' figures, most of the data those read, and the whole of any page the
lane's revision did not have. So the front page moved with every merge, and
the AI Interpretability page answered there the day it was written, with
nobody having promoted it.

None of that shows in a pull request's checks unless the script is run: the
deploy that runs it (`to-test-auto-30`) runs on main only. So this builds a
small repository of two revisions, an earlier one standing for the promoted
revision and a later one for main, and publishes lanes from it with
`publish.sh` as it ships. The engine build is the one step stood in for (a
`cargo` on the path that writes an empty module), because what is asked here
is where every other file of a lane comes from.

    python3 beta/publish_lanes_test.py
"""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest

BETA = pathlib.Path(__file__).resolve().parent

#: A page's figures, written by a stand-in for the scripts that fill the real
#: pages. It says which revision's copy of the script ran and which revision's
#: data it read, which are the two things the lane rule is about. A copy
#: marked `fails` stands for a script that cannot read today's ledger.
FILLER = textwrap.dedent(
    '''\
    import pathlib, sys
    REVISION = "{revision}"
    source, page = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    if {fails}:
        sys.exit("this revision's script cannot read the ledger any more")
    data = (source / "data" / "figures.txt").read_text(encoding="utf-8").strip()
    text = page.read_text(encoding="utf-8")
    page.write_text(text + f"<!-- {{pathlib.Path(__file__).name}}: script {{REVISION}}, data {{data}} -->\\n", encoding="utf-8")
    '''
)

#: The scripts `publish.sh` fills pages with, by name.
FILLERS = (
    "home_rankings.py",
    "tournaments_page.py",
    "research_page.py",
    "performance_page.py",
    "interpretability_page.py",
)

#: The viewer, as little of it as the script's own checks ask for: one
#: `<head>`, a root-absolute script for the lane rewrite to make relative, and
#: links to other pages of the site.
VIEWER = textwrap.dedent(
    """\
    <!doctype html>
    <html><head><title>viewer of {revision}</title></head>
    <body>
    <a href="/home">Home</a> <a href="/research">Charts</a>
    <script src="/assets/app.js"></script>
    </body></html>
    """
)

SHIM = textwrap.dedent(
    """\
    // shim of {revision}
    const here = document.currentScript.src;
    const WASM_URL = new URL("civvis.wasm", here).href;
    const WORKER_URL = new URL("worker.js", here).href;
    """
)


def page(name: str, revision: str) -> str:
    return f'<!doctype html>\n<title>{name} of {revision}</title>\n<a href="../">viewer</a>\n'


class Lanes(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.scratch = pathlib.Path(tempfile.mkdtemp(prefix="civvis-lanes-"))
        cls.repo = cls.scratch / "repo"
        home = cls.scratch / "home"
        tools = home / ".cargo" / "bin"
        tools.mkdir(parents=True)
        # `publish.sh` puts `$HOME/.cargo/bin` first on its path, so these
        # are the `cargo` and `rustup` it finds. The module is the smallest
        # there is: the magic number and the version.
        (tools / "cargo").write_text(
            "#!/bin/sh\n"
            'target="${CARGO_TARGET_DIR:-$PWD/target}/wasm32-unknown-unknown/release"\n'
            'mkdir -p "$target" && printf \'\\000asm\\001\\000\\000\\000\' > "$target/civvis.wasm"\n',
            encoding="utf-8",
        )
        (tools / "rustup").write_text("#!/bin/sh\necho wasm32-unknown-unknown\n", encoding="utf-8")
        for tool in tools.iterdir():
            tool.chmod(0o755)
        cls.env = {
            # Nothing but the system's own tools and this Python: a machine's
            # npx, wasm-opt or brotli would only make the build slower, and
            # none of them decides where a lane's files come from.
            "PATH": os.pathsep.join([str(pathlib.Path(sys.executable).parent), "/usr/bin", "/bin"]),
            "HOME": str(home),
            "CARGO_TARGET_DIR": str(cls.scratch / "target"),
            # The gene prune reads the engine's source, and there is none here.
            "CIVVIS_PRUNE_GENES": "0",
            "GIT_AUTHOR_NAME": "lanes test",
            "GIT_AUTHOR_EMAIL": "lanes@example.invalid",
            "GIT_COMMITTER_NAME": "lanes test",
            "GIT_COMMITTER_EMAIL": "lanes@example.invalid",
            "GIT_CONFIG_NOSYSTEM": "1",
        }
        cls.git("init", "--quiet", str(cls.repo), cwd=cls.scratch)

        # The promoted revision: five pages, and no AI Interpretability page.
        # Its Performance script is one that fails.
        cls.write_revision("promoted", pages=("landing", "download", "tournaments", "research", "performance"),
                           failing=("performance_page.py",))
        cls.promoted = cls.commit("the promoted revision")
        # Main: every script, page and figure has moved on, and there is a
        # sixth page.
        cls.write_revision("main", pages=("landing", "download", "tournaments", "research", "performance",
                                          "interpretability"))
        cls.main = cls.commit("main")

        # The two lanes as the deploy builds them: from main's tree, the
        # stable lane pinned to the promoted revision and the test lane to
        # main's own.
        cls.stable, cls.stable_said = cls.publish(cls.promoted, "lane-stable")
        cls.test, cls.test_said = cls.publish(cls.main, "lane-test")

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.scratch, ignore_errors=True)

    @classmethod
    def git(cls, *args, cwd=None):
        return subprocess.run(
            ["git", *args], cwd=cwd or cls.repo, env=cls.env, check=True, capture_output=True, text=True
        ).stdout.strip()

    @classmethod
    def write_revision(cls, revision: str, *, pages, failing=()):
        beta, web = cls.repo / "beta", cls.repo / "web"
        for directory in (beta, web):
            shutil.rmtree(directory, ignore_errors=True)
        (web / "assets").mkdir(parents=True)
        beta.mkdir()
        (cls.repo / "data").mkdir(exist_ok=True)
        # What a lane is assembled by, which is the invoking tree's in both
        # lanes: the script itself and the pass that versions a lane's files.
        shutil.copy(BETA / "publish.sh", beta / "publish.sh")
        shutil.copy(BETA / "cache_bust.py", beta / "cache_bust.py")
        # The serving shell, one per deployment.
        (beta / "_worker.js").write_text(f"// shell of {revision}\n", encoding="utf-8")
        (beta / "404.html").write_text(f"<!-- shell of {revision} -->\n", encoding="utf-8")
        # What a lane is.
        (beta / "shim.js").write_text(SHIM.format(revision=revision), encoding="utf-8")
        (beta / "worker.js").write_text(f"// worker of {revision}\n", encoding="utf-8")
        (web / "index.html").write_text(VIEWER.format(revision=revision), encoding="utf-8")
        (web / "assets" / "app.js").write_text(f"// viewer script of {revision}\n", encoding="utf-8")
        (cls.repo / "data" / "figures.txt").write_text(f"{revision}\n", encoding="utf-8")
        for name in pages:
            (beta / f"{name}.html").write_text(page(name, revision), encoding="utf-8")
        for name in FILLERS:
            if name == "interpretability_page.py" and "interpretability" not in pages:
                continue
            (beta / name).write_text(
                FILLER.format(revision=revision, fails=name in failing), encoding="utf-8")

    @classmethod
    def commit(cls, message: str) -> str:
        cls.git("add", "--all")
        cls.git("commit", "--quiet", "-m", message)
        return cls.git("rev-parse", "HEAD")

    @classmethod
    def publish(cls, revision: str, name: str, check: bool = True):
        out = cls.scratch / name
        done = subprocess.run(
            ["bash", str(cls.repo / "beta" / "publish.sh"), "--commit", revision, "--out", str(out)],
            cwd=cls.repo, env=cls.env, capture_output=True, text=True,
        )
        said = done.stdout + done.stderr
        if check and done.returncode != 0:
            raise AssertionError(f"publish.sh --commit {revision[:9]} failed:\n{said}")
        return out, (said if check else (done.returncode, said))

    def read(self, lane: pathlib.Path, relative: str) -> str:
        return (lane / relative).read_text(encoding="utf-8")

    # ------------------------------------------------------------ the pages

    def test_a_lane_carries_the_pages_its_revision_has_and_no_others(self):
        # The page only main has is in the test lane and not in the stable
        # lane: nothing reaches the front page except by a promotion.
        self.assertTrue((self.test / "interpretability" / "index.html").is_file())
        self.assertFalse((self.stable / "interpretability").exists())
        self.assertIn("published without: interpretability", self.stable_said)
        # And each lane says which pages it carries, for the checks.
        self.assertEqual(
            json.loads(self.read(self.stable, "build.json"))["pages"],
            ["home", "tournaments", "research", "performance", "download"],
        )
        self.assertEqual(
            json.loads(self.read(self.test, "build.json"))["pages"],
            ["home", "tournaments", "research", "performance", "interpretability", "download"],
        )

    def test_every_page_of_a_lane_is_its_own_revisions(self):
        for directory, name in (("home", "landing"), ("download", "download"), ("tournaments", "tournaments"),
                                ("research", "research"), ("performance", "performance")):
            self.assertIn(f"{name} of promoted", self.read(self.stable, f"{directory}/index.html"))
            self.assertIn(f"{name} of main", self.read(self.test, f"{directory}/index.html"))

    # ----------------------------------------------------------- the figures

    def test_a_page_is_filled_by_its_own_revisions_script_from_its_own_revisions_data(self):
        for directory, script in (("home", "home_rankings.py"), ("tournaments", "tournaments_page.py"),
                                  ("research", "research_page.py"), ("research", "home_rankings.py")):
            self.assertIn(f"<!-- {script}: script promoted, data promoted -->",
                          self.read(self.stable, f"{directory}/index.html"))
            self.assertIn(f"<!-- {script}: script main, data main -->",
                          self.read(self.test, f"{directory}/index.html"))
        self.assertIn("<!-- interpretability_page.py: script main, data main -->",
                      self.read(self.test, "interpretability/index.html"))
        # Nothing of main's is in the promoted lane's pages.
        for filled in self.stable.glob("*/index.html"):
            self.assertNotIn("main", filled.read_text(encoding="utf-8"), filled)

    def test_a_pinned_revisions_failing_script_costs_its_page_the_figures_not_the_deploy(self):
        # A revision that is not the invoking tree's cannot be mended, only
        # promoted past, and its script runs on a ledger written after it.
        # The page goes out as it came, and the build says so.
        performance = self.read(self.stable, "performance/index.html")
        self.assertIn("performance of promoted", performance)
        self.assertNotIn("performance_page.py: script", performance)
        self.assertIn("::warning::", self.stable_said)
        self.assertIn("beta/performance_page.py failed on performance/index.html", self.stable_said)

    def test_the_invoking_trees_own_failing_script_stops_the_publish(self):
        # The same failing script, published from its own revision: that is
        # main's fault to mend, and a lane must not go out without figures
        # for it.
        self.git("checkout", "--quiet", "--detach", self.promoted)
        try:
            _, (code, said) = self.publish(self.promoted, "lane-own-failure", check=False)
        finally:
            self.git("checkout", "--quiet", "--detach", self.main)
        self.assertNotEqual(code, 0, said)
        self.assertIn("cannot read the ledger", said)

    # ------------------------------------------------------------ the viewer

    def test_the_viewer_and_its_scripts_are_the_lanes_own(self):
        for lane, revision, other in ((self.stable, "promoted", "main"), (self.test, "main", "promoted")):
            for relative in ("index.html", "shim.js", "worker.js", "assets/app.js"):
                text = self.read(lane, relative)
                self.assertIn(f"of {revision}", text, relative)
                self.assertNotIn(f"of {other}", text, relative)

    def test_a_lanes_links_stay_inside_it(self):
        for lane in (self.stable, self.test):
            viewer = self.read(lane, "index.html")
            self.assertIn('href="home/"', viewer)
            self.assertIn('href="research/"', viewer)
            self.assertNotIn('href="/', viewer)
            self.assertNotIn('src="/', viewer)

    # ------------------------------------------------------------- the shell

    def test_the_serving_shell_is_the_invoking_trees_in_both_lanes(self):
        # A deployment has one request handler and one 404 page, so they
        # cannot be a lane's: the deploy keeps the root copy, which is main's.
        for lane in (self.stable, self.test):
            self.assertEqual(self.read(lane, "_worker.js"), "// shell of main\n")
            self.assertEqual(self.read(lane, "404.html"), "<!-- shell of main -->\n")


class Script(unittest.TestCase):
    def test_nothing_a_lane_serves_is_read_from_the_invoking_tree(self):
        # What `publish.sh` may take from `$repo_root`, the tree it is run
        # from: itself, the tools that assemble a lane, and the serving
        # shell. Everything a visitor is served is read from `$source_tree`.
        publish = (BETA / "publish.sh").read_text(encoding="utf-8")
        taken = sorted(
            {
                line.split('"$repo_root/', 1)[1].split('"', 1)[0]
                for line in publish.splitlines()
                if '"$repo_root/' in line and not line.lstrip().startswith("#")
            }
        )
        self.assertEqual(
            taken,
            [
                "beta/404.html",
                "beta/_worker.js",
                "beta/cache_bust.py",
                "beta/dist",
                "tools/site_wasm_symbols.py",
                "tools/wasm_gene_prune.py",
            ],
        )


if __name__ == "__main__":
    unittest.main()
