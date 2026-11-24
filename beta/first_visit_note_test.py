#!/usr/bin/env python3
"""The note a person is shown the first time they come to the site.

Operator, 2026-11-24: "please add a note to the site when a person first
visits: Everything is a work in progress. Trying to move fast so things are
sloppy but I'll try to fix what I can when I can. Currently using Fable 5.2
xhigh and Opus 5.5 xhigh from CLI to build everything. I don't write the
code, I don't review the code. Just kinda throwing down features and charts
and apps. I don't accept code from others because of the increased security
risks. Everything is just from building with Claude (and a bit of Gemini,
Codex, others) but all should be publicly available, open source,
inspectable, free. with an Ack button."

A first visit can be to any page: the simulator at the lane's own address,
Home, a page of Charts/, Tournaments, the downloads. So every page carries
the note, as one block of script that is the same in each of them (the site
has no script its pages share), and the browser keeps the Ack for all of
them at once. This holds:

  * every page a lane publishes, and the simulator, to carrying the block
    once, at the end of its body, the same in all of them;
  * the note to the operator's words and a button that says Ack;
  * the block to the site alone, by the test the simulator already uses to
    tell the site from a desktop build, so no page under test and no game
    being filmed shows it;
  * what the block does, run as it ships against a stand-in for the browser:
    shown on a first visit, put away for good by Ack or Escape, never shown
    again, never shown inside a frame, and harmless in a browser that keeps
    nothing or has no dialog.

    python3 beta/first_visit_note_test.py
"""

from __future__ import annotations

import json
import pathlib
import re
import shutil
import subprocess
import unittest

BETA = pathlib.Path(__file__).resolve().parent
VIEWER = BETA.parent / "web" / "index.html"
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")

#: The pages `publish.sh` publishes, found in the script rather than listed
#: here, and the simulator: a page added to a lane carries the note the day
#: it is added, or this fails.
SOURCES = sorted(set(re.findall(r'_page="\$source_tree/beta/([a-z_]+\.html)"', PUBLISH)))
PAGES = [BETA / source for source in SOURCES] + [VIEWER]

BLOCK = re.compile(r"[ \t]*<script data-first-visit>\n(.*?)\n[ \t]*</script>\n", re.S)

#: What the note says, as the operator wrote it, start to end.
WORDS = (
    "Everything is a work in progress. Trying to move fast so things are "
    "sloppy but I\u2019ll try to fix what I can when I can. Currently using "
    "Fable 5.2 xhigh and Opus 5.5 xhigh from CLI to build everything. I "
    "don\u2019t write the code, I don\u2019t review the code. Just kinda "
    "throwing down features and charts and apps. I don\u2019t accept code from "
    "others because of the increased security risks. Everything is just from "
    "building with Claude (and a bit of Gemini, Codex, others) but all should "
    "be publicly available, open source, inspectable, free."
)
#: The paragraphs the page sets them in. Where one ends is the page's choice;
#: read one after another they are the operator's words and nothing else.
PARAGRAPHS = [
    "Everything is a work in progress. Trying to move fast so things are "
    "sloppy but I\u2019ll try to fix what I can when I can.",
    "Currently using Fable 5.2 xhigh and Opus 5.5 xhigh from CLI to build "
    "everything. I don\u2019t write the code, I don\u2019t review the code. Just "
    "kinda throwing down features and charts and apps.",
    "I don\u2019t accept code from others because of the increased security "
    "risks. Everything is just from building with Claude (and a bit of Gemini, "
    "Codex, others) but all should be publicly available, open source, "
    "inspectable, free.",
]
KEY = "civvis-first-visit-ack"


def blocks(page: pathlib.Path) -> list:
    return BLOCK.findall(page.read_text(encoding="utf-8"))


def name(page: pathlib.Path) -> str:
    return str(page.relative_to(BETA.parent))


class EveryPageCarriesTheNote(unittest.TestCase):
    def test_the_pages_are_found(self):
        # The lane's pages by publish.sh's own variables: an empty or short
        # list here would be this check holding nothing.
        self.assertGreaterEqual(len(SOURCES), 8, SOURCES)
        for page in PAGES:
            self.assertTrue(page.is_file(), name(page))

    def test_each_page_has_it_once_as_the_last_thing_in_its_body(self):
        for page in PAGES:
            with self.subTest(page=name(page)):
                text = page.read_text(encoding="utf-8")
                self.assertEqual(len(blocks(page)), 1)
                self.assertEqual(text.count("data-first-visit"), 1)
                self.assertTrue(text.endswith(BLOCK.search(text).group(0) + "</body>\n</html>\n"))

    def test_it_is_the_same_block_in_every_page(self):
        first = blocks(PAGES[0])[0]
        for page in PAGES[1:]:
            with self.subTest(page=name(page)):
                self.assertEqual(blocks(page)[0], first)

    def test_it_says_the_operators_words_and_has_an_ack_button(self):
        block = blocks(VIEWER)[0]
        self.assertEqual(" ".join(PARAGRAPHS), WORDS)
        listed = "".join(f"      {json.dumps(paragraph)},\n" for paragraph in PARAGRAPHS)
        self.assertIn(f"    const words = [\n{listed}    ];\n", block)
        self.assertIn('ack.textContent = "Ack";', block)
        # Nothing else is said: those paragraphs and one button, and nothing
        # written into the page as markup (the pages' own rule).
        self.assertEqual(block.count('createElement("p")'), 1)  # once, for each of the words
        self.assertEqual(block.count('createElement("button")'), 1)
        self.assertEqual(block.count(".textContent = "), 3)  # a paragraph, Ack, the styles
        for unsafe in ("innerHTML", "insertAdjacentHTML", "outerHTML", "document.write"):
            self.assertNotIn(unsafe, block)

    def test_it_shows_on_the_site_alone_by_the_simulators_own_test(self):
        # The simulator tells the site from a desktop build by its host name
        # (`CIVVIS_PUBLISHED_SITE`). The note uses the same test, so the two
        # cannot come to disagree about what the site is.
        viewer = VIEWER.read_text(encoding="utf-8")
        site = re.search(r"window\.CIVVIS_PUBLISHED_SITE = (/.+?/)\.test\(location\.hostname\);", viewer)
        self.assertIsNotNone(site)
        self.assertIn(f"if (!{site.group(1)}.test(location.hostname)) return;", blocks(VIEWER)[0])


# The block as it ships, run against a stand-in for the browser: only what
# the block touches, each piece recording what was done to it.
STAND_IN = r"""
const KEY = %(key)s;
const block = %(block)s;

function visit({ hostname, kept = null, framed = false, keepsNothing = false, noDialog = false }) {
  const store = new Map(kept === null ? [] : [[KEY, kept]]);
  const localStorage = {
    getItem(key) { if (keepsNothing) throw new Error("denied"); return store.has(key) ? store.get(key) : null; },
    setItem(key, value) { if (keepsNothing) throw new Error("denied"); store.set(key, String(value)); },
  };
  const body = { children: [], append(node) { this.children.push(node); } };
  const made = {};
  const document = {
    body,
    createElement(tag) {
      const node = { tag, children: [], attributes: {}, listeners: {} };
      node.addEventListener = (kind, fn) => { (node.listeners[kind] ||= []).push(fn); };
      node.fire = (kind) => { for (const fn of node.listeners[kind] || []) fn({ stopPropagation() {} }); };
      node.append = (...nodes) => { node.children.push(...nodes); };
      node.setAttribute = (name, value) => { node.attributes[name] = value; };
      node.remove = () => { body.children = body.children.filter((child) => child !== node); };
      node.attachShadow = () => (node.tree = { children: [], append(...nodes) { this.children.push(...nodes); } });
      if (tag === "dialog" && !noDialog) {
        node.open = false;
        node.showModal = () => { node.open = true; };
        node.close = () => { node.open = false; node.fire("close"); };
      }
      made[tag] = node;
      return node;
    },
  };
  const window = {};
  window.top = framed ? {} : window;
  new Function("window", "location", "localStorage", "document", block)(
    window, { hostname }, localStorage, document);
  const host = body.children[0] || null;
  const dialog = host ? host.tree.children.find((node) => node.tag === "dialog") : null;
  return {
    shown: () => !!dialog && dialog.open,
    onThePage: () => body.children.length,
    // What the note holds, in order: each part and what it says.
    says: dialog ? dialog.children.map((node) => [node.tag, node.textContent]) : [],
    id: host ? host.id : null,
    kept: () => store.get(KEY) ?? null,
    ack: () => made.button.fire("click"),
    escape: () => dialog.close(),
    quiet: host ? Object.keys(host.listeners) : [],
  };
}

const out = {};

// Off the site there is nothing: a desktop build, a page under test, and
// names that only look like the site's.
out.offSite = ["127.0.0.1", "localhost", "", "civvis.ai.example.com", "notcivvis.ai", "pages.dev"]
  .map((hostname) => visit({ hostname }).onThePage());

// On it, a first visit shows the note.
out.onSite = ["civvis.ai", "www.civvis.ai", "civvis.pages.dev", "abc123.civvis.pages.dev"]
  .map((hostname) => visit({ hostname }).shown());

const first = visit({ hostname: "civvis.ai" });
out.first = {
  shown: first.shown(), onThePage: first.onThePage(), id: first.id, says: first.says,
  keptBefore: first.kept(), quiet: first.quiet,
};
first.ack();
out.acked = { shown: first.shown(), onThePage: first.onThePage(), kept: first.kept() };

// Escape closes a dialog without its button, and counts.
const escaped = visit({ hostname: "civvis.ai" });
escaped.escape();
out.escaped = { onThePage: escaped.onThePage(), kept: escaped.kept() };

// Kept: no note.
out.returning = visit({ hostname: "civvis.ai", kept: "2026-11-24T00:00:00.000Z" }).onThePage();

// Inside a frame (Doctor Strange's two simulators): nothing.
out.framed = visit({ hostname: "civvis.ai", framed: true }).onThePage();

// A browser that keeps nothing still shows the note and still lets it go.
const forgetful = visit({ hostname: "civvis.ai", keepsNothing: true });
out.forgetful = { shown: forgetful.shown() };
forgetful.ack();
out.forgetful.afterAck = forgetful.onThePage();

// A browser with no dialog shows nothing and breaks nothing.
out.noDialog = visit({ hostname: "civvis.ai", noDialog: true }).onThePage();

console.log(JSON.stringify(out));
"""


@unittest.skipUnless(shutil.which("node"), "no node on this host; CI runners have one")
class TheNoteAsItRuns(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        script = STAND_IN % {"key": json.dumps(KEY), "block": json.dumps(blocks(VIEWER)[0])}
        ran = subprocess.run(
            [shutil.which("node"), "-e", script], capture_output=True, text=True, timeout=60
        )
        if ran.returncode != 0:
            raise AssertionError(ran.stderr)
        cls.out = json.loads(ran.stdout)

    def test_nothing_shows_off_the_site(self):
        self.assertEqual(self.out["offSite"], [0, 0, 0, 0, 0, 0])

    def test_a_first_visit_to_the_site_shows_it(self):
        self.assertEqual(self.out["onSite"], [True, True, True, True])
        first = self.out["first"]
        self.assertTrue(first["shown"])
        self.assertEqual(first["onThePage"], 1)
        self.assertEqual(first["id"], "first-visit-note")
        self.assertIsNone(first["keptBefore"])
        self.assertEqual(
            first["says"], [*(["p", paragraph] for paragraph in PARAGRAPHS), ["button", "Ack"]]
        )
        self.assertEqual(" ".join(said for part, said in first["says"] if part == "p"), WORDS)

    def test_ack_puts_it_away_and_the_browser_keeps_that(self):
        self.assertFalse(self.out["acked"]["shown"])
        self.assertEqual(self.out["acked"]["onThePage"], 0)
        self.assertTrue(self.out["acked"]["kept"])
        self.assertEqual(self.out["returning"], 0)

    def test_escape_counts_as_an_ack(self):
        self.assertEqual(self.out["escaped"]["onThePage"], 0)
        self.assertTrue(self.out["escaped"]["kept"])

    def test_a_framed_simulator_shows_none_of_its_own(self):
        self.assertEqual(self.out["framed"], 0)

    def test_a_browser_that_keeps_nothing_is_still_shown_it(self):
        self.assertTrue(self.out["forgetful"]["shown"])
        self.assertEqual(self.out["forgetful"]["afterAck"], 0)

    def test_a_browser_with_no_dialog_is_left_alone(self):
        self.assertEqual(self.out["noDialog"], 0)

    def test_what_is_done_to_the_note_stays_with_the_note(self):
        # The simulator hears keys and the pointer on the whole window; a
        # Space or a click meant for the note must not reach the game.
        for kind in ("keydown", "keyup", "pointerdown", "click", "wheel"):
            self.assertIn(kind, self.out["first"]["quiet"])


if __name__ == "__main__":
    unittest.main()
