#!/usr/bin/env python3
"""Concurrent treatment and gene pull requests have to merge, so this merges them.

Adding a treatment appends to several shared anchors — the flag field on
`AdvancedAi`, the `enable_*`/`disable_*` pair, a row in one of the tables in
`advanced/treatments.rs`. Its `fn configured` initialiser used to be one more,
until `AdvancedAi` derived `Default` and that anchor went. Every one of
those appends used to land on the anchor's LAST line, so any two treatment pull
requests conflicted in every file they shared. Measured with
`tools/conflict_hotspots.py --modes` over the 200 merges ending at `2c570f4f`
(2026-08-23): 10 of 10 of `treatments.rs`'s replayed collisions sit on its two
tables, and 8 of 16 of `advanced.rs`'s sit on the struct and on `configured`.

`docs/ROADMAP.md` objective 5 separates the two reasons a file is contended and
this is the second one. It is NOT answered by moving the list to another file —
`advanced/treatments.rs` is exactly that move (#2022, #2029) and five days
later it was the most anchored file in the repository. It is answered by giving
the anchor more than one append point, so that two treatments do not append to
the same line.

Each anchor therefore carries a run of markers, one per range of first letters:

    // ---- append: a-b ------------------------------------------------

⚠⚠ AND THAT IS A CONVENTION, WHICH THIS REPOSITORY HAS LEARNED IS NOT A CHECK.
So this suite does not assert that two treatment or gene pull requests merge.
It builds them with git plumbing and merges them, and it builds two that break
the rule and requires those to conflict — a test that cannot fail is the
defect, not the reassurance.

The anchors are DISCOVERED by globbing the tracked sources for the marker, never
listed: a hand-written list of anchors is complete on the day it is written and
silently shrinks afterwards, which is how this repository lost twenty-five
ungated suites and a whole language from the hotspot ranking.
"""

from __future__ import annotations

import os
import re
import subprocess
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

MARKER = re.compile(r"^(\s*)// ---- append: ([a-z])-([a-z]) -+\s*$")

#: Every anchor carries the same ranges, in the same order, so that one rule
#: covers all of them and a reader who has learned it once is done.
RANGES = [("a", "b"), ("c", "d"), ("e", "f"), ("g", "k"),
          ("l", "o"), ("p", "r"), ("s", "s"), ("t", "z")]

#: Below this the glob has broken or the markers have been deleted. Pinned as a
#: floor rather than an exact count so that adding an anchor is not a failure.
MIN_ANCHORS = 2

GENE_PATH = "src/ai/advanced/genes.rs"
FLAGS_PATH = "src/ai/advanced/treatment_flags.rs"
#: Ceilings, not targets: what `main` held on 2026-11-06 plus what the pull
#: requests open that day add. They only ever come down.
GENE_ROWS_ABOVE_THE_MARKERS = 880
TOGGLES_OUTSIDE_THE_MARKERS = 638
GENE_TAG = re.compile(r'Gene\s*\{\s*tag:\s*"([^"]+)"')


#: ⚠ A CI RUNNER HAS NO GIT IDENTITY AND THIS SUITE WRITES COMMITS.
#:
#: `git commit-tree` refuses without a committer, and `actions/checkout` does
#: not set one. Every local run passed because a developer machine has a
#: `user.email`; the first CI run died on `exit status 128` in `setUpClass`,
#: which is the whole "it works on my machine" failure in one line. Supplied
#: through the environment rather than `git config`, so the suite never writes
#: to the repository it is measuring.
PROBE = {
    "GIT_AUTHOR_NAME": "append-point probe",
    "GIT_AUTHOR_EMAIL": "probe@civvis.invalid",
    "GIT_COMMITTER_NAME": "append-point probe",
    "GIT_COMMITTER_EMAIL": "probe@civvis.invalid",
}


def git(*args: str, env: dict | None = None, **kwargs) -> str:
    return subprocess.run(["git", "-C", str(REPO), *args], capture_output=True,
                          text=True, check=True,
                          env={**os.environ, **PROBE, **(env or {})},
                          **kwargs).stdout


def tracked_sources() -> list[str]:
    return [line for line in git("ls-files", "--", "*.rs").splitlines() if line]


def marks(lines: list[str]) -> list[tuple[int, tuple[str, str]]]:
    """(0-based line of each marker, its range) in one file's lines."""
    return [(number, (match.group(2), match.group(3)))
            for number, line in enumerate(lines)
            if (match := MARKER.match(line))]


def anchors() -> dict[str, list[tuple[int, tuple[str, str]]]]:
    """{path: markers}, discovered from the WORKING TREE.

    The working tree rather than `HEAD`, so that a change is checked before it
    is committed as well as after. The merge test below builds its base commit
    out of the same content for the same reason.
    """
    found = {}
    for path in tracked_sources():
        found[path] = marks((REPO / path).read_text(
            encoding="utf-8").splitlines())
    return {path: found[path] for path in found if found[path]}


def bucket(name: str) -> tuple[str, str]:
    for low, high in RANGES:
        if low <= name[0] <= high:
            return low, high
    raise AssertionError(f"no range holds {name!r}")


def _blob(rev: str, path: str) -> str:
    return git("show", f"{rev}:{path}")


def _treatment_pr(rev: str, name: str, under: tuple[str, str] | None = None
                  ) -> dict[str, str]:
    """One pull request's version of every anchored file.

    It files `name` under its own range at every marker in the repository,
    which is what a treatment pull request does: the same treatment reaches
    every anchor. `under` overrides the range, to build the pull request that
    breaks the rule.
    """
    want = under or bucket(name)
    out = {}
    for path in anchors():
        lines = _blob(rev, path).splitlines(True)
        for number, span in reversed(marks([line.rstrip("\n")
                                            for line in lines])):
            if span == want:
                indent = MARKER.match(lines[number]).group(1)
                lines.insert(number + 1, f"{indent}// {name}\n")
        out[path] = "".join(lines)
    return out


def _gene_tags(text: str) -> list[str]:
    """The registry tags in source order, without mistaking verdict rows for genes."""
    return GENE_TAG.findall(text)


def _gene_with_entry(text: str, name: str,
                     under: tuple[str, str] | None = None) -> str:
    """`text` with one realistic registry row under its range's tail marker."""
    want = under or bucket(name)
    lines = text.splitlines(True)
    found = [(number, span) for number, span in marks(
        [line.rstrip("\n") for line in lines]) if span == want]
    if len(found) != 1:
        raise AssertionError(f"expected one {want[0]}-{want[1]} gene marker, found {found}")
    number, _ = found[0]
    indent = MARKER.match(lines[number].rstrip("\n")).group(1)
    field = name.replace("-", "_")
    lines.insert(
        number + 1,
        f'{indent}Gene {{ tag: "{name}", field: "{field}", kind: Kind::OptIn, '
        f'enable: AdvancedAi::enable_{field}, disable: AdvancedAi::disable_{field} }},\n',
    )
    return "".join(lines)


def _gene_pr(rev: str, name: str, under: tuple[str, str] | None = None
             ) -> dict[str, str]:
    """One branch adding a real-looking row to the positional gene registry."""
    return {GENE_PATH: _gene_with_entry(_blob(rev, GENE_PATH), name, under)}


def _commit(rev: str, files: dict[str, str], message: str) -> str:
    """A commit on `rev` carrying those file contents, built with plumbing."""
    index = Path(git("rev-parse", "--absolute-git-dir").strip()) / f"ix-{message}"
    env = {"GIT_INDEX_FILE": str(index)}
    try:
        git("read-tree", rev, env=env)
        for path, text in files.items():
            blob = subprocess.run(["git", "-C", str(REPO), "hash-object", "-w",
                                   "--stdin"], input=text,
                                  capture_output=True, text=True,
                                  check=True).stdout.strip()
            git("update-index", "--cacheinfo", f"100644,{blob},{path}", env=env)
        tree = git("write-tree", env=env).strip()
    finally:
        index.unlink(missing_ok=True)
    return git("commit-tree", tree, "-p", rev, "-m", message).strip()


# Git's empty tree: reading attributes from it switches `.gitattributes` off.
NO_ATTRIBUTES = "4b825dc642cb6eb9a060e54bf8d69288fbee4904"


def _merge(one: str, other: str, *, union: bool = True) -> str:
    """'' when they merge, else git's report of what conflicted.

    `union=False` merges as if `.gitattributes` were absent, which is the only
    way to see what the per-letter markers do on their own.
    """
    source = [] if union else [f"--attr-source={NO_ATTRIBUTES}"]
    done = subprocess.run(["git", *source, "-C", str(REPO), "merge-tree", "--write-tree",
                           "--name-only", one, other],
                          capture_output=True, text=True, check=False,
                          env={**os.environ, **PROBE})
    return "" if done.returncode == 0 else done.stdout


class TheAnchorsCarryTheMarkers(unittest.TestCase):
    def test_the_glob_finds_them(self):
        found = anchors()
        self.assertGreaterEqual(
            len(found), MIN_ANCHORS,
            f"only {sorted(found)} carry append markers; a marker run was "
            f"deleted, or the glob stopped matching")

    def test_every_anchor_carries_every_range_in_order(self):
        """One rule for all of them, or it is several rules and no rule."""
        for path, found in anchors().items():
            with self.subTest(path=path):
                for start in range(0, len(found), len(RANGES)):
                    run = [span for _, span in found[start:start + len(RANGES)]]
                    self.assertEqual(run, RANGES)

    def test_no_two_markers_share_a_line(self):
        """The whole point is that two treatments append in different places."""
        for path, found in anchors().items():
            numbers = [number for number, _ in found]
            self.assertEqual(len(numbers), len(set(numbers)), path)


class AFlagIsFiledUnderItsOwnRange(unittest.TestCase):
    """The convention, checked. A flag under the wrong marker puts two pull
    requests back on one line without anybody noticing."""

    def test_every_entry_matches_the_marker_above_it(self):
        for path, found in anchors().items():
            if path == GENE_PATH:
                continue  # Gene rows have their own positional parser below.
            lines = (REPO / path).read_text(encoding="utf-8").splitlines()
            bounds = [number for number, _ in found] + [len(lines)]
            for (start, span), stop in zip(found, bounds[1:]):
                for line in lines[start + 1:stop]:
                    body = line.strip()
                    if body.startswith(("}", "]", ")")):
                        break        # the anchor's own block ended here
                    if not body or body.startswith(("//", "/*", "*", "#[")):
                        continue
                    entry = re.match(r'\(?"?([A-Za-z_]\w*)', body)
                    if not entry:
                        continue
                    name = entry.group(1).lower()
                    with self.subTest(path=path, entry=name):
                        self.assertEqual(
                            bucket(name), span,
                            f"{name} is filed under {span[0]}-{span[1]} in "
                            f"{path}; it belongs under "
                            f"{'-'.join(bucket(name))}")


class TwoTreatmentPullRequestsMerge(unittest.TestCase):
    """★★★ THE CLAIM, MERGED RATHER THAN ASSERTED."""

    @classmethod
    def setUpClass(cls):
        # The base is the WORKING TREE's version of every anchored file, so a
        # marker run being edited right now is what gets merged.
        cls.base = _commit(
            git("rev-parse", "HEAD").strip(),
            {path: (REPO / path).read_text(encoding="utf-8")
             for path in anchors()},
            "append-base")

    def test_it_still_builds_a_commit_with_no_git_identity_configured(self):
        """The first CI run died here and every local run had passed.

        `actions/checkout` sets no `user.email`, `git commit-tree` refuses
        without one, and a developer machine always has one — so the failure
        was invisible until it ran somewhere real. Strip the identity and
        require a commit anyway, rather than trusting that it is supplied.
        """
        bare = {"GIT_CONFIG_GLOBAL": os.devnull,
                "GIT_CONFIG_SYSTEM": os.devnull}
        previous = {key: os.environ.get(key) for key in bare}
        os.environ.update(bare)
        try:
            self.assertRegex(
                _commit(self.base, {}, "no-identity"), r"^[0-9a-f]{40}$")
        finally:
            for key, was in previous.items():
                if was is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = was

    def test_two_treatments_in_different_ranges_merge(self):
        one = _commit(self.base, _treatment_pr(self.base, "alpha_probe"),
                      "treatment-a")
        other = _commit(self.base, _treatment_pr(self.base, "zephyr_probe"),
                        "treatment-z")
        self.assertEqual(_merge(one, other), "")

    def test_two_ADJACENT_ranges_merge_which_is_the_tight_case(self):
        """Markers two lines apart is the closest two append points ever get,
        and git's merge conflicts only when two insertions land on the SAME
        line — so this is the case worth pinning, not the far-apart one."""
        for first, second in (("alpha_probe", "company_probe"),
                              ("company_probe", "eureka_probe"),
                              ("gamma_probe", "lodge_probe"),
                              ("parade_probe", "siege_probe"),
                              ("siege_probe", "terrain_probe")):
            with self.subTest(first=first, second=second):
                one = _commit(self.base, _treatment_pr(self.base, first), "one")
                other = _commit(self.base, _treatment_pr(self.base, second),
                                "two")
                self.assertEqual(_merge(one, other), "")

    def test_two_in_the_same_range_still_conflict(self):
        """⚠ THE CONTROL. Without it this suite would pass just as happily if
        the two synthetic pull requests never touched the same file at all,
        and would be reassurance rather than a check. The remedy divides the
        collision rate by the number of ranges; it does not claim to remove it,
        and this is where that limit is written down."""
        one = _commit(self.base, _treatment_pr(self.base, "alpha_probe"),
                      "same-a")
        other = _commit(self.base, _treatment_pr(self.base, "beacon_probe"),
                        "same-b")
        self.assertNotEqual(
            _merge(one, other), "",
            "two treatments in one range merged, so this suite is not "
            "measuring the append point at all")


class GeneRegistryAppendPoints(unittest.TestCase):
    """The registry's markers must append rows without renumbering old genes."""

    SAMPLES = (
        "alpha-probe", "copper-probe", "ember-probe", "gale-probe",
        "lilac-probe", "prism-probe", "sage-probe", "zephyr-probe",
    )

    def test_the_registry_carries_one_complete_marker_run(self):
        found = marks((REPO / GENE_PATH).read_text(encoding="utf-8").splitlines())
        self.assertEqual([span for _, span in found], RANGES)

    def test_a_new_row_keeps_the_existing_genes_in_order(self):
        """The order holds, not the index. A row under an earlier marker shifts
        the rows filed below it by one, and a run reads its bits against the
        gene list in its own header, so that moves nothing a run wrote. This
        used to demand an exact prefix, which meant any row under a later
        marker failed the next row under an earlier one, so every gene was
        filed above the markers and they went unused."""
        source = (REPO / GENE_PATH).read_text(encoding="utf-8")
        before = _gene_tags(source)
        self.assertGreater(len(before), 90, "the registry scrape found almost nothing")
        for tag in self.SAMPLES:
            with self.subTest(tag=tag):
                after = _gene_tags(_gene_with_entry(source, tag))
                self.assertIn(tag, after)
                self.assertEqual([row for row in after if row != tag], before)
        # The case the old prefix rule failed: a row already under a late
        # marker, then one under an early one.
        after = _gene_tags(_gene_with_entry(
            _gene_with_entry(source, "zephyr-probe"), "alpha-probe"))
        self.assertLess(after.index("alpha-probe"), after.index("zephyr-probe"))
        self.assertEqual(
            [row for row in after if not row.endswith("-probe")], before)

    def test_new_rows_go_under_the_markers(self):
        """Above the markers every gene PR lands on one line. Measured over
        the 200 merges before 2026-11-06: 40 of 58 consecutive pairs collided,
        all on that line. The ceiling leaves room for the pull requests open
        that day, and only ever comes down."""
        lines = (REPO / GENE_PATH).read_text(encoding="utf-8").splitlines(True)
        first = marks([line.rstrip("\n") for line in lines])[0][0]
        above = len(_gene_tags("".join(lines[:first])))
        self.assertLessEqual(
            above, GENE_ROWS_ABOVE_THE_MARKERS,
            "file a new gene row under the marker for its tag's first letter")

    def test_new_toggles_go_under_the_markers(self):
        """The same, for `toggles!` entries: 16 of 57 pairs collided there."""
        text = (REPO / FLAGS_PATH).read_text(encoding="utf-8")
        blocks = re.findall(r"^    toggles! \{\n(.*?)^    \}$", text, re.M | re.S)
        self.assertEqual(len(blocks), 2, "expected the marked block and the old one")
        marked, old = blocks
        self.assertTrue(marks(marked.splitlines()), "the first block lost its markers")
        self.assertLessEqual(
            len(re.findall(r" => enable_", old)), TOGGLES_OUTSIDE_THE_MARKERS,
            "file a new toggle under the marker for its flag's first letter")

    def test_every_gene_row_below_a_marker_matches_that_marker(self):
        lines = (REPO / GENE_PATH).read_text(encoding="utf-8").splitlines(True)
        found = marks([line.rstrip("\n") for line in lines])
        bounds = [number for number, _ in found] + [len(lines)]
        for (start, span), stop in zip(found, bounds[1:]):
            for tag in _gene_tags("".join(lines[start + 1:stop])):
                with self.subTest(tag=tag, span=span):
                    self.assertEqual(bucket(tag), span)


class TwoGenePullRequestsMerge(unittest.TestCase):
    """The positional registry's claim, merged with actual Gene rows."""

    @classmethod
    def setUpClass(cls):
        cls.base = _commit(
            git("rev-parse", "HEAD").strip(),
            {GENE_PATH: (REPO / GENE_PATH).read_text(encoding="utf-8")},
            "gene-append-base")

    def test_two_genes_in_different_ranges_merge(self):
        one = _commit(self.base, _gene_pr(self.base, "alpha-probe"), "gene-a")
        other = _commit(self.base, _gene_pr(self.base, "zephyr-probe"), "gene-z")
        self.assertEqual(_merge(one, other), "")

    def test_two_adjacent_gene_ranges_merge(self):
        for first, second in (("alpha-probe", "copper-probe"),
                              ("copper-probe", "ember-probe"),
                              ("gale-probe", "lilac-probe"),
                              ("prism-probe", "sage-probe"),
                              ("sage-probe", "terrain-probe")):
            with self.subTest(first=first, second=second):
                one = _commit(self.base, _gene_pr(self.base, first), "gene-one")
                other = _commit(self.base, _gene_pr(self.base, second), "gene-two")
                self.assertEqual(_merge(one, other), "")

    def test_two_genes_in_the_same_range_conflict_without_the_union_merge(self):
        one = _commit(self.base, _gene_pr(self.base, "alpha-probe"), "gene-same-a")
        other = _commit(self.base, _gene_pr(self.base, "beacon-probe"), "gene-same-b")
        self.assertNotEqual(
            _merge(one, other, union=False), "",
            "two genes in one range merged, so this suite is not measuring the marker")

    def test_two_genes_in_the_same_range_merge_by_union(self):
        one = _commit(self.base, _gene_pr(self.base, "alpha-probe"), "gene-union-a")
        other = _commit(self.base, _gene_pr(self.base, "beacon-probe"), "gene-union-b")
        self.assertEqual(_merge(one, other), "", "see .gitattributes")


if __name__ == "__main__":
    unittest.main()
