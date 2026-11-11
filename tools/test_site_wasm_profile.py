#!/usr/bin/env python3
"""The browser profile reader names what the ordered maps hold, and says when a module has no names."""

from __future__ import annotations

import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import site_wasm_profile as reader  # noqa: E402

#: Real symbols from the 2026-11-11 profile, shortened only in the crate hash.
SEARCH_NAME = ("_RINvMs3_NtNtNtNtCsX_5alloc11collections5btree4node"
               "INtB6_7NodeRefNtNtBa_6marker3MutNtNtCsY_6civvis4name4Name"
               "lNtB1i_14LeafOrInternalE11search_treeB1y_EB1G_")
ITERATE_UNIT = ("_RNvXs8_NtNtNtCsX_5alloc11collections5btree3map"
                "INtB5_6ValuesmNtNtCsY_6civvis4game4UnitE"
                "NtNtNtNtCsZ_4core4iter6traits8iterator8Iterator4nextB1r_")
SEARCH_STRING = ("_RINvMs3_NtNtNtNtCsX_5alloc11collections5btree4node"
                 "INtB6_7NodeRefNtNtBa_6marker5ImmutNtNtBb_6string6String"
                 "dNtB1i_14LeafOrInternalE11search_treeeECsY_6civvis")
MALLOC = "_RNvMs0_NtCsQ_8dlmalloc8dlmallocINtB5_8DlmallocNtNtB7_3sys6SystemE6malloc"
PLAIN = "_RNvMNtNtCsY_6civvis4game10visibilityNtB4_4Game16tile_has_line_to"


def a_profile(spent: dict) -> dict:
    """A `.cpuprofile` in which each function was the innermost frame for the
    microseconds given."""
    nodes = [{"id": at + 1, "callFrame": {"functionName": name}}
             for at, name in enumerate(spent)]
    return {"nodes": nodes,
            "samples": [node["id"] for node in nodes],
            "timeDeltas": list(spent.values())}


PROFILE = a_profile({SEARCH_NAME: 300, ITERATE_UNIT: 200, SEARCH_STRING: 100,
                     MALLOC: 150, PLAIN: 250})


class ASymbolIsReadByItsLengths(unittest.TestCase):
    """`4Unit` is four letters. A pattern that takes the letters after a digit
    runs on into the hash behind it and never finds the type, which is how the
    first by-type table came out as sixteen rows of `?`."""

    def test_the_identifiers_come_out_whole(self):
        self.assertEqual(reader.identifiers("6civvis4game4UnitEENt"),
                         ["civvis", "game", "Unit"])

    def test_a_type_followed_by_more_letters_is_still_found(self):
        self.assertIn("Unit", reader.identifiers(ITERATE_UNIT))
        self.assertIn("Name", reader.identifiers(SEARCH_NAME))
        self.assertIn("search_tree", reader.identifiers(SEARCH_NAME))

    def test_a_symbol_with_no_lengths_has_none(self):
        self.assertEqual(reader.identifiers("wasm-function[4321]"), [])


class TheOrderedMapsAreNamedByWhatTheyHold(unittest.TestCase):
    def test_a_search_and_a_walk_are_told_apart(self):
        self.assertEqual(reader.held_and_asked(SEARCH_NAME).split(), ["search", "Name"])
        self.assertEqual(reader.held_and_asked(ITERATE_UNIT).split(), ["iterate", "Unit"])
        self.assertEqual(reader.held_and_asked(SEARCH_STRING).split(),
                         ["search", "String"])

    def test_only_map_machinery_is_counted(self):
        kinds = reader.ordered_maps(reader.self_time(PROFILE))
        self.assertEqual(sum(kinds.values()), 600)
        self.assertEqual(kinds.most_common(1)[0][1], 300)
        self.assertFalse(any("visibility" in kind for kind in kinds))


class TheReportAddsUpWhatATableHides(unittest.TestCase):
    def text(self, profile: dict) -> str:
        out = io.StringIO()
        reader.report(reader.self_time(profile), out=out)
        return out.getvalue()

    def test_self_time_is_summed_per_function(self):
        twice = a_profile({PLAIN: 100})
        twice["samples"] += twice["samples"]
        twice["timeDeltas"] += [50]
        self.assertEqual(reader.self_time(twice)[PLAIN], 150)

    def test_the_roll_ups_are_shares_of_all_sampled_time(self):
        text = self.text(PROFILE)
        self.assertIn("  60.00%  ordered maps (BTreeMap, BTreeSet)", text)
        self.assertIn("  15.00%  allocator (dlmalloc)", text)
        self.assertIn("  30.00%  search   Name", text)
        self.assertIn("  20.00%  iterate  Unit", text)
        self.assertIn("tile_has_line_to", text)

    def test_a_module_without_names_is_said_to_be_one(self):
        """A published module has no name section, so its profile is a list of
        `wasm-function[n]`. That must read as a missing build step, not as a
        result."""
        unnamed = a_profile({"wasm-function[1]": 900, "wasm-function[2]": 100})
        self.assertIn("no name section", self.text(unnamed))
        self.assertNotIn("no name section", self.text(PROFILE))

    def test_an_empty_profile_is_an_error_not_a_table_of_zeros(self):
        with self.assertRaises(SystemExit):
            reader.report(reader.self_time({"nodes": [], "samples": []}))


class TheNewestProfileInADirectoryIsRead(unittest.TestCase):
    def test_a_directory_is_searched_and_an_empty_one_explains_itself(self):
        with tempfile.TemporaryDirectory() as scratch:
            where = Path(scratch)
            with self.assertRaises(SystemExit) as empty:
                reader.newest_profile(where)
            self.assertIn("--cpu-prof", str(empty.exception))
            written = where / "CPU.1.cpuprofile"
            written.write_text(json.dumps(PROFILE), encoding="utf-8")
            self.assertEqual(reader.newest_profile(where), written)
            self.assertEqual(reader.newest_profile(written), written)


class PublishedFunctionMapsNameTheExactModule(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.directory = Path(self.scratch.name)
        self.module = self.directory / "engine.wasm"
        # One empty function, index zero. The name is supplied by Binaryen.
        self.module.write_bytes(bytes.fromhex(
            "0061736d01000000010401600000030201000a040102000b"))
        self.mapping = self.directory / "functions.map"
        self.mapping.write_text("0:" + SEARCH_NAME + "\n", encoding="utf-8")
        reader.site_wasm_symbols.record(self.module, self.mapping, "test-revision")

    def test_indexes_are_translated_and_other_frames_keep_their_time(self):
        symbols, manifest = reader.site_wasm_symbols.load(self.directory, self.module)
        profile = a_profile({"wasm-function[0]": 300, PLAIN: 90, "paint": 10})
        before = json.dumps(profile)
        self.assertEqual(reader.self_time(profile, symbols),
                         {SEARCH_NAME: 300, PLAIN: 90, "paint": 10})
        self.assertEqual(json.dumps(profile), before)
        self.assertEqual(manifest["wasm_bytes"], self.module.stat().st_size)

    def test_same_length_different_engine_is_rejected(self):
        changed = bytearray(self.module.read_bytes())
        changed[-1] ^= 1
        self.module.write_bytes(changed)
        with self.assertRaisesRegex(ValueError, "different WebAssembly module"):
            reader.site_wasm_symbols.load(self.directory, self.module)

    def test_changed_map_is_rejected(self):
        self.mapping.write_text("0:" + SEARCH_STRING + "\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "checksum"):
            reader.site_wasm_symbols.load(self.directory, self.module)

    def test_unknown_manifest_format_is_rejected(self):
        path = self.directory / "manifest.json"
        manifest = json.loads(path.read_text())
        manifest["format"] = 2
        path.write_text(json.dumps(manifest))
        with self.assertRaisesRegex(ValueError, "manifest format"):
            reader.site_wasm_symbols.load(self.directory, self.module)

    def test_missing_function_is_an_error_instead_of_a_partial_attribution(self):
        with self.assertRaisesRegex(ValueError, "index 17 is absent"):
            reader.self_time(a_profile({"wasm-function[17]": 300}), {0: SEARCH_NAME})

    def test_malformed_empty_and_duplicate_maps_are_rejected(self):
        for text in ["", "\n", "x:name", "-1:name", "0:", "0:first\n0:second"]:
            with self.subTest(text=text), self.assertRaises(ValueError):
                reader.site_wasm_symbols.parse_function_map(text)

    def test_cli_prints_named_costs_from_the_bound_sidecar(self):
        profile = self.directory / "CPU.1.cpuprofile"
        profile.write_text(json.dumps(a_profile({"wasm-function[0]": 300, PLAIN: 100})))
        output, diagnostics = io.StringIO(), io.StringIO()
        with redirect_stdout(output), redirect_stderr(diagnostics):
            self.assertEqual(reader.main([str(profile), "--symbols", str(self.directory),
                                          "--wasm", str(self.module)]), 0)
        self.assertIn("search   Name", output.getvalue())
        self.assertNotIn("wasm-function[0]", output.getvalue())
        self.assertIn("test-revision", diagnostics.getvalue())

    def test_cli_requires_module_and_sidecar_together(self):
        with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as missing:
            reader.main(["unused.cpuprofile", "--symbols", str(self.directory)])
        self.assertEqual(missing.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
