#!/usr/bin/env python3
"""The parsing half of `profile_civvis.py`, which is where it can be silently wrong.

The sampling half needs `/usr/bin/sample` and cannot run on a Linux runner, so
`tools/test_ci_wiring.py` carries that exemption. Everything below is pure text
and runs on every pull request through `unittest discover -s tools`.

Each case here is a mistake that was actually made while reading a real profile
on 2026-08-26, not a hypothetical:

* the tree is drawn with `+ ! : |`, so splitting on whitespace flattens it;
* `/usr/bin/time ./civvis` makes `time` the parent, and sampling the parent
  yields a well-formed call graph of nothing;
* deriving self time by subtracting children from a pre-order walk mis-assigns
  the recursive frames, while `sample` prints an exact leaf table;
* and a recursive symbol counted once per frame reports a share above 100%.
"""

from __future__ import annotations

import io
import unittest
from pathlib import Path

import profile_civvis as profiler

REPO = Path(__file__).resolve().parent.parent


#: A miniature of the real thing, in `sample`'s exact format: two threads, a
#: recursive frame, one folded leaf, one allocator leaf, and the leaf table it
#: prints at the end.
SAMPLE = """\
Analysis of sampling time (pid 4242) every 1 millisecond
Process:         civvis [4242]
Path:            /Users/x/CIVVIS-repo/target/profile/ci/civvis

Call graph:
    1000 Thread_1   DispatchQueue_1: com.apple.main-thread  (serial)
      1000 start  (in dyld) + 6992  [0x1843]
        1000 _RNvCsezDmCg98MfZ_6civvis4main  (in civvis) + 10  [0x1001]
          600 _RNvMsd_NtCs7URXWk9eTIJ_6civvis2aiNtB5_7BasicAi12healing_step  (in civvis) + 4  [0x1002]
            400 _RNvMsU_NtCs7URXWk9eTIJ_6civvis4gameNtB5_4Game9flow_past  (in civvis) + 8  [0x1003]
              200 _RNvMsU_NtCs7URXWk9eTIJ_6civvis4gameNtB5_4Game9flow_past  (in civvis) + 12  [0x1004]
              120 <deduplicated_symbol>  (in civvis) + 108,56  [0x1005,0x1006]
            100 _platform_memmove  (in libsystem_platform.dylib) + 20  [0x1007]
          300 _RNvMsb_NtNtCs7URXWk9eTIJ_6civvis2ai8advancedNtB5_10AdvancedAi27forcing_reply_penalty_owned  (in civvis) + 6  [0x1008]
            300 _RNvMsU_NtCs7URXWk9eTIJ_6civvis4gameNtB5_4Game5apply  (in civvis) + 2  [0x1009]
              150 _RNvMsU_NtCs7URXWk9eTIJ_6civvis4gameNtB5_4Game9flow_past  (in civvis) + 8  [0x1003]
    40 Thread_2
      40 _pthread_cond_wait  (in libsystem_pthread.dylib) + 12  [0x2001]

Total number in stack (recursive counted multiple, when >=5):
        1000 start
        600 healing_step

Sort by top of stack, same collapsed (when >= 5):
        <deduplicated_symbol>        120
        _platform_memmove        100
        _RNvMsU_NtCs7URXWk9eTIJ_6civvis4gameNtB5_4Game9flow_past        250
        _RNvMsU_NtCs7URXWk9eTIJ_6civvis4gameNtB5_4Game5apply        150

Binary Images:
       0x1000 - 0x9000  civvis
"""

#: What sampling `/usr/bin/time` instead of its child produces. It is a
#: complete, well-formed, entirely useless profile, and the only signal is that
#: the crate never appears.
WRONG_PROCESS = """\
Analysis of sampling time (pid 77596) every 1 millisecond
Process:         time [77596]

Call graph:
    19998 Thread_9   DispatchQueue_1: com.apple.main-thread  (serial)
      19998 start  (in dyld) + 6992  [0x1843]
        19998 ???  (in time)  load address 0x1006 + 0x990  [0x1006]
          19998 __sigsuspend  (in libsystem_kernel.dylib) + 8  [0x1846]

Total number in stack (recursive counted multiple, when >=5):

Sort by top of stack, same collapsed (when >= 5):
        __sigsuspend  (in libsystem_kernel.dylib)        19998

Binary Images:
"""


def parsed(text: str = SAMPLE) -> profiler.Profile:
    # rustfilt is deliberately withheld: the fallback demangler is the one that
    # has to work on a machine with no cargo tooling, so it is what is tested.
    return profiler.Profile(text, rustfilt=None)


class TheTreeIsDrawnNotIndented(unittest.TestCase):
    """Depth is the column the count starts in, because the rows use `+ ! : |`."""

    def test_the_busiest_thread_is_the_one_reported(self):
        profile = parsed()
        self.assertEqual(profile.total, 1000)
        self.assertEqual(len(profile.threads), 2)
        self.assertIn("Thread_1", profile.thread["name"])

    def test_nesting_survives_the_box_characters(self):
        profile = parsed()
        depths = {symbol: depth for depth, _, symbol in profile.nodes}
        outer = next(d for s, d in depths.items() if "healing_step" in s)
        inner = next(d for s, d in depths.items() if "flow_past" in s)
        self.assertLess(outer, inner)

    def test_a_row_the_pattern_cannot_read_is_skipped_not_guessed(self):
        profile = profiler.Profile(
            SAMPLE.replace("      40 _pthread_cond_wait", "      xx _pthread"),
            rustfilt=None)
        self.assertEqual(profile.total, 1000)


class InclusiveCountsRecursionOnce(unittest.TestCase):
    """`flow_past` appears inside itself; counted per frame it exceeds 100%."""

    def test_a_recursive_symbol_never_exceeds_the_thread(self):
        profile = parsed()
        inclusive = profile.inclusive()
        flow = next(count for symbol, count in inclusive.items()
                    if "flow_past" in symbol)
        # 400 under healing_step (its own 200-sample recursion NOT re-counted)
        # plus 150 under apply.
        self.assertEqual(flow, 550)
        self.assertLessEqual(flow, profile.total)

    def test_a_symbol_on_two_branches_sums_over_both(self):
        profile = parsed()
        inclusive = profile.inclusive()
        apply_count = next(count for symbol, count in inclusive.items()
                           if symbol.endswith("Game5apply"))
        self.assertEqual(apply_count, 300)


class SelfTimeComesFromTheLeafTable(unittest.TestCase):
    """Exact, rather than derived by subtracting children from the tree."""

    def test_leaves_are_read_from_sort_by_top_of_stack(self):
        totals = parsed().self_time()
        self.assertEqual(totals["_platform_memmove"], 100)
        self.assertEqual(totals["<deduplicated_symbol>"], 120)

    def test_the_allocator_is_rolled_up_into_one_number(self):
        self.assertEqual(parsed().allocator(), 100)

    def test_folded_samples_are_counted_so_they_can_be_reported(self):
        self.assertEqual(parsed().folded(), 120)


class AProfileOfTheWrongProcessIsRefused(unittest.TestCase):
    """The `/usr/bin/time` trap: a perfect call graph of `__sigsuspend`."""

    def test_a_sample_that_never_enters_the_crate_raises(self):
        profile = profiler.Profile(WRONG_PROCESS, rustfilt=None)
        with self.assertRaises(SystemExit) as raised:
            profile.check_is_this_program()
        self.assertIn("wrapper process", str(raised.exception))

    def test_a_real_profile_passes_the_same_check(self):
        parsed().check_is_this_program()  # must not raise

    def test_a_report_with_no_call_graph_says_so(self):
        with self.assertRaises(SystemExit):
            profiler.Profile("Analysis of sampling time\n", rustfilt=None)


class TheDemanglerRecoversThePath(unittest.TestCase):
    """No rustfilt on this fleet; the length-prefixed components are enough."""

    def test_a_v0_symbol_becomes_a_readable_path(self):
        name = profiler.demangle(
            "_RNvMsU_NtCs7URXWk9eTIJ_6civvis4gameNtB5_4Game9flow_past")
        self.assertEqual(name, "game::Game::flow_past")

    def test_a_nested_module_keeps_its_components(self):
        name = profiler.demangle(
            "_RNvMsb_NtNtCs7URXWk9eTIJ_6civvis2ai8advancedNtB5_"
            "10AdvancedAi27forcing_reply_penalty_owned")
        self.assertEqual(name, "ai::advanced::AdvancedAi::forcing_reply_penalty_owned")

    def test_a_c_symbol_is_left_exactly_alone(self):
        self.assertEqual(profiler.demangle("_platform_memmove"), "_platform_memmove")
        self.assertEqual(profiler.demangle("<deduplicated_symbol>"),
                         "<deduplicated_symbol>")

    def test_a_truncated_symbol_does_not_raise(self):
        profiler.demangle("_RNvMsU_NtCs7URXWk9eTIJ_6civvis4gameNtB5_4Game23att")

    def test_the_decoration_sample_adds_is_stripped(self):
        self.assertEqual(
            profiler.clean("_platform_memmove  (in libsystem_platform.dylib) "
                           "+ 108,56,...  [0x1005,0x1006]"),
            "_platform_memmove")

    def test_a_long_path_keeps_both_ends(self):
        """Truncating from the right keeps `alloc::vec` and throws away the
        function, which is the half a reader needs. The tail is never lost."""
        long_name = "::".join(["alloc", "vec", "spec_from_iter", "middle",
                               "ai", "advanced", "UnitIntervention"])
        for width in (40, 52, 30):
            short = profiler.shorten(long_name, width=width)
            self.assertTrue(short.endswith("UnitIntervention"), short)
            self.assertLessEqual(len(short), width, short)
        self.assertTrue(profiler.shorten(long_name, 40).startswith("alloc"))

    def test_a_single_component_too_long_to_fit_still_keeps_its_tail(self):
        short = profiler.shorten("a" * 50 + "the_function", width=20)
        self.assertTrue(short.endswith("the_function"), short)
        self.assertLessEqual(len(short), 20)


class ParentsAnswerWhoMadeItExpensive(unittest.TestCase):
    """A flat profile says a symbol is hot; this says which caller made it hot."""

    def test_callers_are_summed_over_every_stack(self):
        found = parsed().parents("flow_past", depth=1, floor=1)
        by_caller = {chain: count for chain, count in found.items()}
        self.assertEqual(by_caller.get("ai::BasicAi::healing_step"), 400)
        self.assertEqual(by_caller.get("game::Game::apply"), 150)

    def test_a_pattern_that_matches_nothing_returns_nothing(self):
        self.assertEqual(len(parsed().parents("no_such_symbol")), 0)


class TheShapeComesFromTheScreen(unittest.TestCase):
    """Hard-coding it is how every speed reading was taken on the wrong map."""

    def test_the_screen_constants_are_read_from_their_source(self):
        shape = profiler.screen_shape(REPO)
        source = (REPO / "src" / "bin" / "gene_screen.rs").read_text(encoding="utf-8")
        for key, flag in (("PLAYERS", "players"), ("WIDTH", "width"),
                          ("HEIGHT", "height"), ("CITY_STATES", "city-states")):
            self.assertIn("const SCREEN_%s" % key, source)
            self.assertTrue(shape[flag].isdigit(), shape)
        self.assertNotIn("::", shape["map"])
        self.assertIn("const SCREEN_MAP", source)

    def test_the_command_carries_every_leg_of_the_shape(self):
        command = profiler.simulate_command(
            Path("/bin/civvis"), 7311001, 250, profiler.screen_shape(REPO))
        for flag in ("--players", "--width", "--height", "--city-states",
                     "--map", "--speed", "--turns", "--jobs"):
            self.assertIn(flag, command)
        self.assertEqual(command[command.index("--turns") + 1], "250")
        self.assertEqual(command[command.index("--jobs") + 1], "1")

    def test_a_renamed_constant_fails_loudly(self):
        class Fake:
            def __truediv__(self, _other):
                return self

            def read_text(self, **_kwargs):
                return "const SCREEN_PLAYERS: usize = 6;"

        with self.assertRaises(SystemExit) as raised:
            profiler.screen_shape(Fake())
        self.assertIn("CITY_STATES", str(raised.exception))


class TheReportSaysWhetherItCanBeTrusted(unittest.TestCase):
    """An unnamed leaf is a measurement failure and has to read as one."""

    def test_a_folded_profile_says_no_flag_removes_it(self):
        """The claim has to match the measurement. Five builds were tried and
        the folded share stayed between 30.35% and 32.59%; a warning that sends
        the next reader after a link flag would cost them the afternoon it
        cost this one."""
        out = io.StringIO()
        profiler.report(parsed(), out=out)
        text = out.getvalue()
        self.assertIn("deduplicated_symbol", text)
        self.assertIn("No build flag tried removes it", text)
        self.assertIn("attributed by caller", text)

    def test_the_folded_samples_are_credited_to_a_named_caller(self):
        out = io.StringIO()
        profiler.report(parsed(), out=out)
        section = out.getvalue().split("== UNNAMED, BY CALLER")[1]
        self.assertIn("flow_past", section)
        self.assertIn("healing_step", section)
        self.assertIn("total attributed", section)

    def test_every_folded_sample_reaches_the_attribution(self):
        """A third of the profile is at stake, so none of it may be dropped on
        the way to the section that accounts for it."""
        profile = parsed()
        self.assertEqual(sum(profile.folded_by_caller().values()), 120)

    def test_a_placeholder_under_a_placeholder_credits_the_nearest_real_name(self):
        nested = SAMPLE.replace(
            "              120 <deduplicated_symbol>  (in civvis) + 108,56  [0x1005,0x1006]",
            "              120 <deduplicated_symbol>  (in civvis) + 108  [0x1005]\n"
            "                120 <deduplicated_symbol>  (in civvis) + 56  [0x1006]")
        found = profiler.Profile(nested, rustfilt=None).folded_by_caller(depth=1)
        self.assertNotIn("deduplicated", " ".join(found))
        self.assertEqual(sum(found.values()), 240)
        self.assertEqual(found["game::Game::flow_past"], 240)

    def test_an_unfolded_profile_says_so_instead(self):
        clean_sample = SAMPLE.replace("<deduplicated_symbol>", "game::Game::nbrs")
        out = io.StringIO()
        profiler.report(profiler.Profile(clean_sample, rustfilt=None), out=out)
        text = out.getvalue()
        self.assertIn("every leaf in this profile has a name", text)
        self.assertNotIn("== UNNAMED, BY CALLER", text)

    def test_a_parked_thread_is_never_the_largest_leaf(self):
        """`sample` counts a blocked thread at full rate and prints ONE leaf
        table for the whole process, so dividing it by the busiest thread put
        `semaphore_wait_trap` at 100.00% on the first real report this tool
        produced, and inflated every other row with it."""
        idle = SAMPLE.replace("40 _pthread_cond_wait", "40 semaphore_wait_trap") \
                     .replace("        <deduplicated_symbol>        120",
                              "        semaphore_wait_trap        900\n"
                              "        <deduplicated_symbol>        120")
        profile = profiler.Profile(idle, rustfilt=None)
        self.assertNotIn("semaphore_wait_trap", profile.working_self_time())
        self.assertEqual(profile.working_total(), 620)
        out = io.StringIO()
        profiler.report(profile, out=out)
        self.assertNotIn("semaphore_wait_trap", out.getvalue())

    def test_runtime_scaffolding_is_not_reported_as_a_hotspot(self):
        out = io.StringIO()
        profiler.report(parsed(), out=out)
        inclusive = out.getvalue().split("== SELF")[0]
        self.assertNotIn("\n  99", inclusive)
        self.assertIn("healing_step", inclusive)

    def test_the_ineffective_flag_is_documented_as_ineffective(self):
        """It is still passed — it is free and the linker accepts it — but the
        file must not read as though it were the answer."""
        source = (REPO / "tools" / "profile_civvis.py").read_text(encoding="utf-8")
        self.assertIn("-Wl,-no_deduplicate", profiler.NO_FOLD_RUSTFLAGS)
        self.assertIn("MEASURED INEFFECTIVE", source)
        self.assertIn("32.54%", source, "the five-build table is the evidence "
                                        "for that claim and has to stay with it")

    def test_the_profiling_build_never_overwrites_the_measured_binary(self):
        """`speed_ab.py` compares two binaries by path and would happily
        compare a folded arm against an unfolded one."""
        self.assertNotIn(profiler.PROFILE_TARGET, ("target/ci", "target/release"))
        self.assertTrue(profiler.PROFILE_TARGET.startswith("target/"))


if __name__ == "__main__":
    unittest.main()


class TheEntryPointIsNotAHotspot(unittest.TestCase):
    """A frame on every stack is the program, not a finding."""

    def test_a_frame_on_essentially_every_stack_is_dropped(self):
        out = io.StringIO()
        profiler.report(parsed(), out=out)
        inclusive = out.getvalue().split("== SELF")[0]
        self.assertNotIn("::main", inclusive)
        self.assertIn("healing_step", inclusive)

    def test_the_rule_is_a_share_not_a_list_of_names(self):
        """A renamed entry point must not reinstate the noise."""
        renamed = SAMPLE.replace("6civvis4main", "6civvis11brand_new_e")
        out = io.StringIO()
        profiler.report(profiler.Profile(renamed, rustfilt=None), out=out)
        self.assertNotIn("brand_new_e", out.getvalue().split("== SELF")[0])

    def test_scaffolding_is_matched_after_demangling_not_before(self):
        self.assertTrue(profiler.SCAFFOLD.match("rt::lang_start_internal"))
        self.assertTrue(profiler.SCAFFOLD.match("sys::backtrace::x"))
        self.assertFalse(profiler.SCAFFOLD.match("game::Game::flow_past"))
