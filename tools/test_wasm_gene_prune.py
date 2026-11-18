"""Tests for `wasm_gene_prune.py`: what the browser build may compile out."""

from __future__ import annotations

import pathlib
import re
import sys
import tempfile
import textwrap
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import wasm_gene_prune as prune  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parents[1]

# Filler rows: the analysis refuses a registry it read under 100 rows of.
FILLER = 100


def crate(files: dict[str, str]) -> pathlib.Path:
    root = pathlib.Path(tempfile.mkdtemp(prefix="civvis-prune-"))
    for name, text in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(textwrap.dedent(text), encoding="utf-8")
    (root / "data").mkdir(exist_ok=True)
    (root / "mods").mkdir(exist_ok=True)
    return root


def registry(rows: list[tuple[str, str, str]], deployed: list[str]) -> str:
    lines = [
        f'    Gene {{ tag: "{tag}", field: "{field}", kind: Kind::{kind}, '
        f"enable: AdvancedAi::enable_{field}, disable: AdvancedAi::disable_{field} }},"
        for tag, field, kind in rows
    ]
    lines += [
        f'    Gene {{ tag: "filler-{n}", field: "filler_{n}", kind: Kind::Repair(Axis::War), '
        f"enable: AdvancedAi::enable_filler_{n}, disable: AdvancedAi::disable_filler_{n} }},"
        for n in range(FILLER)
    ]
    genome = ", ".join(f'"{tag}"' for tag in deployed)
    return ("pub const GENES: &[Gene] = &[\n" + "\n".join(lines) + "\n];\n"
            f"pub const DEPLOYMENT_GENOME: &[&str] = &[{genome}];\n")


SITE = 'pub const SITE_EXPERIMENT_GENES: &[&str] = &["site-gene"];\n'

ROWS = [
    ("plain-gene", "plain_gene", "OptIn"),
    ("deployed-gene", "deployed_gene", "OptIn"),
    ("site-gene", "site_gene", "OptIn"),
    ("named-gene", "named_gene", "OptIn"),
    ("runtime-gene", "runtime_gene", "OptIn"),
    ("tuple-gene", "tuple_gene", "OptIn"),
    ("called-gene", "called_gene", "OptIn"),
    ("shared-name", "shared_name", "OptIn"),
    ("borrowed-gene", "borrowed_gene", "OptIn"),
    ("literal-gene", "literal_gene", "OptIn"),
]

ADVANCED = """\
    //! The agent.
    pub struct AdvancedAi {
        pub(crate) plain_gene: bool,
        pub(crate) deployed_gene: bool,
        pub(crate) site_gene: bool,
        pub(crate) named_gene: bool,
        pub(crate) runtime_gene: bool,
        pub(crate) tuple_gene: bool,
        pub(crate) called_gene: bool,
        pub(crate) shared_name: bool,
        pub(crate) borrowed_gene: bool,
        pub(crate) literal_gene: bool,
    }
    pub struct Report {
        pub shared_name: bool,
    }
    impl AdvancedAi {
        pub fn new() -> Self {
            AdvancedAi { plain_gene: false, literal_gene: true, ..Default::default() }
        }
        pub fn enable_plain_gene(&mut self) {
            self.plain_gene = true;
        }
        fn turn(&mut self, science: bool) -> u32 {
            let mut score = 0;
            if self.plain_gene && self.deployed_gene {
                score += 1;
            }
            if self.plain_gene.then_some(()).is_some() || !self.site_gene {
                score += 2;
            }
            let note = "self.plain_gene stays in a string";
            // self.plain_gene stays in a comment
            self.runtime_gene = self.runtime_gene || science;
            let saved = (self.tuple_gene, self.plain_gene);
            (self.tuple_gene, self.borrowed_gene) = (saved.0, false);
            std::mem::swap(&mut self.borrowed_gene, &mut true.clone());
            self.enable_called_gene();
            let _ = (note, self.named_gene, self.shared_name, self.literal_gene);
            score
        }
    }
    #[cfg(test)]
    mod tests {
        fn reads(ai: &super::AdvancedAi) -> bool {
            ai.plain_gene
        }
    }
"""

OTHER = """\
    //! Something that seats a gene by name.
    pub fn roster() -> &'static [&'static str] {
        &["named-gene"]
    }
"""


class PruneRules(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = crate({
            "src/ai/advanced/genes.rs": registry(ROWS, ["deployed-gene"]),
            "src/ai/advanced/site_deployment.rs": SITE,
            "src/ai/advanced.rs": ADVANCED,
            "src/strategies.rs": OTHER,
        })
        cls.result = prune.analyse(cls.root)
        cls.pruned = set(cls.result["pruned"])
        cls.kept = cls.result["kept"]

    def test_a_gene_nothing_can_turn_on_is_pruned(self):
        self.assertEqual(self.pruned, {"plain_gene"}, self.kept)

    def test_every_other_way_on_keeps_the_gene(self):
        expected = {
            "deployed_gene": "DEPLOYMENT_GENOME",
            "site_gene": "SITE_EXPERIMENT_GENES",
            "named_gene": "named elsewhere",
            "runtime_gene": "written outside its toggle",
            "tuple_gene": "assigned through a tuple",
            "called_gene": "called outside the registry",
            "shared_name": "also a field of Report",
            "borrowed_gene": "assigned through a tuple",
            "literal_gene": "`literal_gene: true`",
        }
        for field, reason in expected.items():
            with self.subTest(field=field):
                self.assertIn(reason, self.kept.get(field, ""), self.kept.get(field))

    def test_only_shipping_reads_become_false(self):
        changed, reads = prune.rewrite(self.root, self.result["pruned"])
        text = changed[self.root / "src/ai/advanced.rs"]
        self.assertEqual(reads, 3)
        self.assertIn("if false && self.deployed_gene {", text)
        self.assertIn("if false.then_some(()).is_some() || !self.site_gene {", text)
        self.assertIn("let saved = (self.tuple_gene, false);", text)
        # The toggle still sets it; strings, comments and tests are untouched.
        self.assertIn("self.plain_gene = true;", text)
        self.assertIn('"self.plain_gene stays in a string"', text)
        self.assertIn("// self.plain_gene stays in a comment", text)
        self.assertIn("        ai.plain_gene\n", text)
        self.assertIn("plain_gene: false,", text)

    def test_an_unreadable_registry_changes_nothing(self):
        root = crate({"src/ai/advanced/genes.rs": "pub const GENES: &[Gene] = &[];\n",
                      "src/ai/advanced/site_deployment.rs": SITE,
                      "src/ai/advanced.rs": ADVANCED})
        before = (root / "src/ai/advanced.rs").read_text()
        self.assertEqual(prune.main([str(root)]), 0)
        self.assertEqual((root / "src/ai/advanced.rs").read_text(), before)


class TheRealCrate(unittest.TestCase):
    """The rules hold on the source this checkout would publish."""

    @classmethod
    def setUpClass(cls):
        cls.result = prune.analyse(ROOT)

    def test_most_opt_in_genes_are_pruned(self):
        self.assertGreater(len(self.result["pruned"]), 400)

    def test_flags_a_seat_switches_on_mid_turn_are_kept(self):
        # Both are set in `take_turn_inner` from the seat's specialization;
        # pruning them changed six-civ games on 2026-11-18.
        for field in ("science_building_first", "first_research_building_reserve"):
            with self.subTest(field=field):
                self.assertNotIn(field, self.result["pruned"])

    def test_nothing_the_browser_seats_is_pruned(self):
        genes = (ROOT / "src/ai/advanced/genes.rs").read_text(encoding="utf-8")
        site = (ROOT / "src/ai/advanced/site_deployment.rs").read_text(encoding="utf-8")
        seated = prune.string_list(genes, "DEPLOYMENT_GENOME") | prune.string_list(
            site, "SITE_EXPERIMENT_GENES")
        pruned_tags = {tag for tag, _ in self.result["pruned"].values()}
        self.assertFalse(seated & pruned_tags, sorted(seated & pruned_tags))

    def test_only_opt_in_rows_are_candidates(self):
        kinds = {tag: kind for tag, _, kind, _ in prune.registry(ROOT)}
        for tag, _ in self.result["pruned"].values():
            self.assertEqual(kinds[tag], "OptIn", tag)


class Scanner(unittest.TestCase):
    def test_literals_and_comments_blank_but_keep_offsets(self):
        text = 'let a = "x { y"; let c = \'{\'; // }\nlet r = r#"q"#; fn f<\'a>() {}\n'
        code = prune.blank(text)
        self.assertEqual(len(code), len(text))
        self.assertEqual(code.count("\n"), text.count("\n"))
        self.assertEqual(code.count("{"), 1)
        self.assertIn("fn f<'a>() {}", code)

    def test_a_test_item_is_not_shipping_code(self):
        code = prune.shipping(prune.blank(
            "fn a() { x.f }\n#[cfg(test)]\nmod tests;\n#[cfg(test)]\nmod t { fn b() { x.f } }\n"))
        self.assertEqual(len(re.findall(r"x\.f", code)), 1)


if __name__ == "__main__":
    unittest.main()
