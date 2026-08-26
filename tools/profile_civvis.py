#!/usr/bin/env python3
"""Sample the simulator and name every symbol, including the folded ones.

⚠⚠ **ELEVEN PERCENT OF THE MAIN THREAD HAD NO NAME.** A profile taken on
2026-08-26 at `4b26eafb`, the screen's own shape, put `<deduplicated_symbol>`
at the top of the self-time table with 11.2% — larger than any named leaf in
the run and larger than the last four shipped optimizations put together. It is
not a mystery function. It is the linker: identical-code folding merges
functions with identical machine code into one address, `sample` resolves that
address to the *first* name it finds, and every other caller of that body is
reported under a placeholder. The fix is one link flag, and this file exists so
that nobody has to remember it:

    RUSTFLAGS='-C link-arg=-Wl,-no_deduplicate'

`docs/SIMULATOR_PERFORMANCE.md` is a long argument that a measurement taken
with the wrong instrument is worse than no measurement, because it is acted on.
Its own history has four entries where the profile pointed at the wrong thing —
a 2.7% leaf that shipped as -0.67%, an `enemy_attack_envelopes` "504 samples,
do not reach for it again" beside the same subsystem holding 32.7% inclusive, a
hotspot that reads 1.42% on one map and 13.19% on another. An unnamed leaf is
the same failure with the label filed off, and it is the largest one on the
board today.

## What this does that an ad-hoc `sample` invocation does not

1. **Names folded symbols.** Builds into its own target directory with
   `-Wl,-no_deduplicate` so identical bodies keep separate addresses. The
   default build is left alone; nothing shipped changes.
2. **Samples the right process.** `/usr/bin/time ./civvis …` makes `time` the
   parent and `civvis` the child, and sampling the parent yields 20,000
   samples of `__sigsuspend`. This launches the binary directly and samples its
   own pid, and it *refuses a profile whose busiest thread never enters the
   crate* rather than printing a confident table of nothing.
3. **Reads the shape from the screen.** Players, map, size and city-state
   count come out of `src/bin/gene_screen.rs`'s `SCREEN_*` constants, so the
   profile is taken where the compute is actually spent. The clock defaults to
   the screen's 250 turns, not the CI gate's 120: the gate's own ledger entry
   says it is blind after turn 120, and the late game is where a per-unit pass
   hurts most.
4. **Demangles.** Rust v0 symbols arrive as `_RNvMsU_NtCs…4Game9flow_past`.
   `rustfilt` is used when installed; otherwise the length-prefixed components
   are recovered directly, which is enough to read a path.
5. **Attributes.** `--parents <regex>` prints the callers of a symbol summed
   over every stack it appears in — the question a flat profile cannot answer
   and the one that located the minor-seat evacuation cost.
6. **Rolls up the allocator.** `memmove`/`memcmp`/`free`/`malloc`/`bzero` are
   individually small and collectively 17.5%. A per-symbol table hides that;
   this prints it as one line.

## Usage

    tools/profile_civvis.py                      # build, run, sample, report
    tools/profile_civvis.py --seed 7311002 --turns 250
    tools/profile_civvis.py --attach 12345       # a running gene_screen batch
    tools/profile_civvis.py --read out.sample    # re-report a saved sample
    tools/profile_civvis.py --parents 'healing_step'

⚠ **Sampling a peer's already-running batch costs the host nothing and is the
workload that burns the core-hours.** `--attach` exists for that. A purpose-built
`simulate` run and a live `gene_screen` profile *differently* — varied genomes
drive far more speculative tactical applies — so profile the workload you are
actually paying for, and say which one a number came from.

⚠ This needs `/usr/bin/sample`, which is macOS-only, so it is listed in
`tools/test_ci_wiring.py`'s `CANNOT_RUN_IN_CI`. The parser is not: it is pure
text and `tools/test_profile_civvis.py` exercises it against a checked-in
fixture on every pull request, because the parsing is where this file can be
silently wrong.
"""

from __future__ import annotations

import argparse
import collections
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

REPO = Path(__file__).resolve().parent.parent

#: The link flag that gives every folded body its own address again. Identical
#: code folding is a size optimization; it costs nothing to disable in a build
#: whose only job is to be measured, and disabling it is the difference between
#: an 11% unnamed leaf and a name.
NO_FOLD_RUSTFLAGS = "-C link-arg=-Wl,-no_deduplicate"

#: A separate target directory, so a profiling build never becomes the binary
#: someone then times. `speed_ab.py` compares two binaries and would happily
#: compare a folded one against an unfolded one; keeping them apart on disk is
#: what stops that.
PROFILE_TARGET = "target/profile"

#: `sample`'s call graph is drawn with box characters, not indentation, so the
#: depth of a row is the column its count starts in. Splitting on whitespace
#: instead silently flattens the tree.
CALL_LINE = re.compile(r"^([ +!:|]*)(\d+) (.*)$")

#: The leaf table at the end of the report, which is where self time is exact.
TOP_OF_STACK = re.compile(r"^\s+(?P<symbol>.*?)\s{2,}(?P<count>\d+)$")

#: Allocator and libc memory primitives. Individually small, collectively the
#: largest single block in the profile; the roll-up is the reading.
ALLOCATOR = re.compile(
    r"memmove|memcpy|memcmp|memset|bzero|malloc|free|realloc|"
    r"xzm_|nanov2|szone|_platform_")

#: Runtime scaffolding every stack begins with. Reporting `main` at 99.9% is
#: true and useless.
SCAFFOLD = re.compile(
    r"^(start|main|civvis::main|std::rt::|core::ops::function::FnOnce::call_once|"
    r"std::sys::backtrace|_?_rust_begin_short_backtrace|\?\?\?|dyld)")

#: What a profile must contain to be a profile of this program. A run that
#: samples the wrong pid produces a perfectly well-formed call graph of
#: `__sigsuspend`, and that is the failure this catches.
EXPECTED_CRATE = "civvis"


# --------------------------------------------------------------------------
# Symbol names
# --------------------------------------------------------------------------

#: Components of a Rust v0 symbol are length-prefixed: `4Game9flow_past`. The
#: encoding has more in it than this — generics, backrefs, disambiguators — but
#: the *path* is what a profile reader needs and the path is recoverable
#: without implementing the grammar.
IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*$")

#: Crate disambiguator hashes (`Cs7URXWk9eTIJ_`) and the `$LT$`-style legacy
#: escapes carry no information for a reader.
NOISE = {"core", "alloc", "std", "civvis"}

#: ⚠ The crate root is `C` followed by a disambiguator, `s<base-62>_`, and a
#: base-62 digit can be a decimal digit — so `Cs7URXWk9eTIJ_6civvis` reads as
#: "a 7-character component named `URXWk9e`" to anything scanning for lengths.
#: That produced `URXWk9e::game::Game::flow_past` on every civvis symbol until
#: a test said so. Stripped before scanning rather than filtered afterwards,
#: because the false component also consumes the characters that follow it.
#:
#: ⚠⚠ Anchored on the `C`, and the first attempt was not. `(?<=[A-Za-z])s[0-9]
#: [0-9A-Za-z]*_` also matches inside `…6civvis4gameNtB5_`, because `civvis`
#: ends in `s` and `4gameNtB5` is a legal base-62 run — it ate the real path
#: and returned `civvi4::flow_past`. The other disambiguator positions
#: (`Msd_`, `Xs0_`) are harmless: a length that resolves to a component
#: starting with a digit is rejected by `IDENT` and the scan resumes.
CRATE_HASH = re.compile(r"Cs[0-9A-Za-z]*_")


def v0_components(symbol: str) -> List[str]:
    """The length-prefixed identifiers in a Rust v0 mangled symbol, in order.

    Deliberately tolerant: anything that does not parse is skipped rather than
    raising, because a profile contains C symbols, stubs, and truncated names
    alongside Rust ones and a demangler that throws on the first oddity is a
    demangler nobody runs.
    """
    symbol = CRATE_HASH.sub("", symbol)
    out: List[str] = []
    index = 0
    end = len(symbol)
    while index < end:
        if not symbol[index].isdigit():
            index += 1
            continue
        digits = index
        while digits < end and symbol[digits].isdigit():
            digits += 1
        length = int(symbol[index:digits])
        start = digits
        # A `_` directly after the length marks a punycode/escaped identifier.
        if start < end and symbol[start] == "_":
            start += 1
        component = symbol[start:start + length]
        if len(component) == length and IDENT.match(component):
            out.append(component)
            index = start + length
        else:
            index = digits
    return out


def demangle(symbol: str, rustfilt: Optional[str] = None) -> str:
    """A readable path for a mangled symbol, or the symbol unchanged.

    `rustfilt` is exact and is used when present. The fallback keeps the
    length-prefixed components, which is what makes
    `_RNvMsU_NtCs7URXWk9eTIJ_6civvis4gameNtB5_4Game9flow_past` legible as
    `game::Game::flow_past` on a machine that has no cargo tooling installed.
    """
    if not symbol.startswith("_R"):
        return symbol
    if rustfilt:
        try:
            done = subprocess.run([rustfilt], input=symbol, capture_output=True,
                                  text=True, timeout=10)
            if done.returncode == 0 and done.stdout.strip():
                return done.stdout.strip()
        except (OSError, subprocess.SubprocessError):
            pass
    parts = [part for part in v0_components(symbol) if part not in NOISE]
    return "::".join(parts) if parts else symbol


def clean(raw: str) -> str:
    """Strip `sample`'s per-row decoration down to a symbol name.

    The address list and `+ 2436,2456,…` offset run identify *which instruction*
    a sample landed on, which matters when reading one leaf and is pure noise
    when aggregating a table.
    """
    text = re.sub(r"\s+\(in [^)]*\)", "", raw)
    text = re.sub(r"\s*\[[0-9a-fx,.]+\]", "", text)
    text = re.sub(r"\s*\+\s*[\d,.]+(?=\s|$)", "", text)
    text = re.sub(r"\s+load address.*$", "", text)
    return text.strip()


def shorten(name: str, width: int = 74) -> str:
    """A long generic path, kept readable at both ends.

    Truncating from the right throws away the function and keeps `alloc::vec`,
    which is the half a reader does not need.
    """
    if len(name) <= width:
        return name
    parts = name.split("::")
    for keep_head in (2, 1):
        if len(parts) > keep_head + 2:
            squeezed = "::".join(parts[:keep_head] + ["…"] + parts[-2:])
            if len(squeezed) <= width:
                return squeezed
    tail = parts[-1]
    if len(tail) + 1 <= width:
        return "…" + tail[-(width - 1):]
    return "…" + tail[-(width - 1):]


# --------------------------------------------------------------------------
# Parsing
# --------------------------------------------------------------------------

class Profile:
    """One `sample` report, parsed into per-thread call trees.

    `nodes` is the busiest thread's rows as `(depth, count, symbol)` in file
    order, which is a pre-order walk of the tree; every share below is derived
    from it rather than from a second pass over the text.
    """

    def __init__(self, text: str, rustfilt: Optional[str] = None) -> None:
        self.text = text
        self.rustfilt = rustfilt
        self._names: Dict[str, str] = {}
        self.threads = self._threads()
        if not self.threads:
            raise SystemExit(
                "no call graph in this sample. `sample` writes one under a "
                "'Call graph:' line; a report without one is usually a process "
                "that exited before sampling began.")
        self.thread = max(self.threads, key=lambda one: one["count"])
        self.total = self.thread["count"]
        self.nodes = self.thread["nodes"]

    def name(self, symbol: str) -> str:
        if symbol not in self._names:
            self._names[symbol] = demangle(symbol, self.rustfilt)
        return self._names[symbol]

    def _threads(self) -> List[dict]:
        lines = self.text.splitlines()
        try:
            start = lines.index("Call graph:") + 1
        except ValueError:
            return []
        stop = next((i for i in range(start, len(lines))
                     if lines[i].startswith("Total number in stack")), len(lines))
        threads: List[dict] = []
        current: Optional[dict] = None
        root_depth: Optional[int] = None
        for line in lines[start:stop]:
            found = CALL_LINE.match(line)
            if not found:
                continue
            depth = len(found.group(1))
            count = int(found.group(2))
            symbol = clean(found.group(3))
            if root_depth is None:
                root_depth = depth
            if depth == root_depth:
                current = {"name": symbol, "count": count, "nodes": []}
                threads.append(current)
            elif current is not None:
                current["nodes"].append((depth, count, symbol))
        return threads

    def check_is_this_program(self) -> None:
        """Refuse a profile of the wrong process.

        This is not defensive padding. Sampling `/usr/bin/time` instead of its
        child produced a complete, plausible, entirely `__sigsuspend` call
        graph, and the only thing that gave it away was reading it.
        """
        blob = "\n".join(symbol for _, _, symbol in self.nodes[:400])
        if EXPECTED_CRATE not in blob:
            raise SystemExit(
                "this sample's busiest thread never enters `%s`. The usual "
                "cause is sampling a wrapper process — `/usr/bin/time ./civvis` "
                "makes `time` the parent, and its stack is one `__sigsuspend`. "
                "Sample the binary's own pid.\n  busiest thread: %s (%d samples)"
                % (EXPECTED_CRATE, self.thread["name"][:60], self.total))

    def inclusive(self) -> collections.Counter:
        """Samples with each symbol anywhere on the stack, recursion counted once.

        A symbol that calls itself would otherwise be credited once per frame
        and can exceed the thread total, which reads as a share above 100%.
        """
        totals: collections.Counter = collections.Counter()
        stack: List[Tuple[int, str]] = []
        for depth, count, symbol in self.nodes:
            while stack and stack[-1][0] >= depth:
                stack.pop()
            if symbol not in {name for _, name in stack}:
                totals[symbol] += count
            stack.append((depth, symbol))
        return totals

    def self_time(self) -> collections.Counter:
        """Samples whose innermost frame is each symbol.

        Read from `sample`'s own "Sort by top of stack" table, which is exact,
        rather than derived from the tree by subtracting children — that
        derivation has to guess which rows are children of which and gets it
        wrong on the recursive frames.
        """
        totals: collections.Counter = collections.Counter()
        lines = self.text.splitlines()
        try:
            start = next(i for i, line in enumerate(lines)
                         if line.startswith("Sort by top of stack"))
        except StopIteration:
            return totals
        for line in lines[start + 1:]:
            if line.startswith("Binary Images"):
                break
            found = TOP_OF_STACK.match(line)
            if found:
                totals[clean(found.group("symbol"))] += int(found.group("count"))
        return totals

    def parents(self, pattern: str, depth: int = 3,
                floor: int = 0) -> collections.Counter:
        """Callers of every symbol matching `pattern`, summed over the profile.

        The flat tables say a symbol is expensive; this says *who made it
        expensive*, which is the question that decides whether a cost belongs
        to the major seats an evaluation reads or to the city-states and
        barbarians no evaluation varies.
        """
        wanted = re.compile(pattern)
        found: collections.Counter = collections.Counter()
        stack: List[Tuple[int, str]] = []
        for node_depth, count, symbol in self.nodes:
            while stack and stack[-1][0] >= node_depth:
                stack.pop()
            if count >= floor and wanted.search(self.name(symbol)):
                chain = [shorten(self.name(one), 44) for _, one in stack[-depth:]]
                found[" ← ".join(reversed(chain))] += count
            stack.append((node_depth, symbol))
        return found

    def folded(self) -> int:
        """Self samples the linker could not attribute to a name."""
        return sum(count for symbol, count in self.self_time().items()
                   if "deduplicated_symbol" in symbol)

    def allocator(self) -> int:
        return sum(count for symbol, count in self.self_time().items()
                   if ALLOCATOR.search(symbol))


# --------------------------------------------------------------------------
# Report
# --------------------------------------------------------------------------

def report(profile: Profile, top: int = 40, parents: Optional[str] = None,
           out=sys.stdout) -> None:
    total = profile.total
    share = lambda count: 100.0 * count / total if total else 0.0

    print("thread %s — %d samples on the busiest of %d"
          % (profile.thread["name"][:52], total, len(profile.threads)), file=out)

    folded = profile.folded()
    if folded:
        print("\n⚠ %.2f%% of self time is `<deduplicated_symbol>`: this binary was "
              "linked WITH identical-code folding, so those samples have no name. "
              "Rebuild with RUSTFLAGS='%s' — or run this tool without --read, "
              "which does it for you." % (share(folded), NO_FOLD_RUSTFLAGS),
              file=out)
    else:
        print("\nno folded symbols: every leaf in this profile has a name.",
              file=out)

    print("\n== INCLUSIVE  (%% of thread; a symbol is never counted inside itself)",
          file=out)
    shown = 0
    for symbol, count in profile.inclusive().most_common():
        name = profile.name(symbol)
        if SCAFFOLD.match(name) or "deduplicated_symbol" in name:
            continue
        print("%7.2f%%  %s" % (share(count), shorten(name)), file=out)
        shown += 1
        if shown >= top:
            break

    print("\n== SELF  (innermost frame; from sample's own leaf table)", file=out)
    for symbol, count in profile.self_time().most_common(min(top, 25)):
        print("%7.2f%%  %s" % (share(count), shorten(profile.name(symbol))), file=out)

    print("\n== ROLL-UPS", file=out)
    print("%7.2f%%  allocator and libc memory primitives (malloc/free/memmove/"
          "memcmp/memset)" % share(profile.allocator()), file=out)
    print("%7.2f%%  unnamed, folded by the linker" % share(folded), file=out)

    if parents:
        print("\n== CALLERS OF /%s/  (nearest three frames, summed)" % parents,
              file=out)
        found = profile.parents(parents, floor=max(1, total // 400))
        if not found:
            print("        (no stack matched)", file=out)
        for chain, count in found.most_common(12):
            print("%7.2f%%  %s" % (share(count), chain), file=out)


# --------------------------------------------------------------------------
# Running the thing
# --------------------------------------------------------------------------

def screen_shape(repo: Path = REPO) -> Dict[str, str]:
    """The map row `gene_screen` actually plays, read from its source.

    ⚠ Hard-coding this is how `speed_ab.py` spent every reading it ever took on
    `tennis_ball` while the screen played Continents — and a sibling then
    measured one hotspot at 1.42% on the first map and 13.19% on the second.
    The map does not change how much code runs, it changes *which code is hot*,
    so a profiler that guesses the shape is measuring a different program.
    """
    source = (repo / "src" / "bin" / "gene_screen.rs").read_text(encoding="utf-8")
    found = dict(re.findall(
        r"const SCREEN_(\w+):\s*\w+\s*=\s*([A-Za-z:]+|\d+)\s*;", source))
    missing = {"PLAYERS", "WIDTH", "HEIGHT", "CITY_STATES", "MAP"} - set(found)
    if missing:
        raise SystemExit(
            "src/bin/gene_screen.rs no longer defines %s. This tool reads the "
            "screen's shape from there on purpose; update the pattern rather "
            "than pinning a copy here." % ", ".join(sorted(missing)))
    return {
        "players": found["PLAYERS"],
        "width": found["WIDTH"],
        "height": found["HEIGHT"],
        "city-states": found["CITY_STATES"],
        "map": found["MAP"].rsplit("::", 1)[-1].lower(),
    }


def build(repo: Path, quiet: bool = False) -> Path:
    """Build `civvis` into the profiling target directory, folding disabled."""
    env = dict(os.environ)
    existing = env.get("RUSTFLAGS", "").strip()
    env["RUSTFLAGS"] = (existing + " " + NO_FOLD_RUSTFLAGS).strip()
    command = ["cargo", "build", "--profile", "ci", "--bin", "civvis",
               "--target-dir", PROFILE_TARGET]
    if not quiet:
        print("$ RUSTFLAGS='%s' %s" % (env["RUSTFLAGS"], " ".join(command)),
              file=sys.stderr)
    done = subprocess.run(command, cwd=str(repo), env=env,
                          capture_output=quiet, text=True)
    if done.returncode != 0:
        if quiet and done.stderr:
            sys.stderr.write(done.stderr)
        raise SystemExit("the profiling build failed")
    binary = repo / PROFILE_TARGET / "ci" / "civvis"
    if not binary.exists():
        raise SystemExit("built, but %s is missing" % binary)
    return binary


def simulate_command(binary: Path, seed: int, turns: int,
                     shape: Dict[str, str]) -> List[str]:
    command = [str(binary), "simulate", "--seed", str(seed), "--jobs", "1",
               "--turns", str(turns), "--speed", "online"]
    for flag, value in shape.items():
        command += ["--" + flag, str(value)]
    return command


def sample_process(pid: int, seconds: int, out_file: Path) -> None:
    done = subprocess.run(
        ["/usr/bin/sample", str(pid), str(seconds), "1", "-mayDie",
         "-f", str(out_file)],
        capture_output=True, text=True)
    if done.returncode != 0 and not out_file.exists():
        raise SystemExit("sample failed: %s" % (done.stderr.strip() or done.stdout))


def load_average() -> float:
    try:
        return os.getloadavg()[0]
    except OSError:
        return float("nan")


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__.splitlines()[0],
        formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--seed", type=int, default=7311001)
    parser.add_argument("--turns", type=int, default=250,
                        help="the screen's clock, not the CI gate's 120; the "
                             "gate is documented blind after turn 120")
    parser.add_argument("--seconds", type=int, default=300,
                        help="sampling duration ceiling; sampling stops when "
                             "the process exits")
    parser.add_argument("--top", type=int, default=40)
    parser.add_argument("--parents", metavar="REGEX",
                        help="also print the callers of symbols matching REGEX")
    parser.add_argument("--attach", type=int, metavar="PID",
                        help="sample a process that is already running (a live "
                             "gene_screen batch is the workload that costs)")
    parser.add_argument("--read", metavar="FILE", type=Path,
                        help="re-report a saved sample instead of taking one")
    parser.add_argument("--out", type=Path, default=None,
                        help="where to write the raw sample (default: a "
                             "temporary file beside the report)")
    parser.add_argument("--no-build", action="store_true",
                        help="use the existing %s binary" % PROFILE_TARGET)
    parser.add_argument("--quiet-build", action="store_true")
    args = parser.parse_args(argv)

    rustfilt = shutil.which("rustfilt")

    if args.read:
        profile = Profile(args.read.read_text(errors="replace"), rustfilt)
        profile.check_is_this_program()
        report(profile, args.top, args.parents)
        return 0

    out_file = args.out or Path(
        os.environ.get("TMPDIR", "/tmp")) / ("civvis-%d.sample" % os.getpid())

    if args.attach:
        print("sampling pid %d for up to %ds (load %.2f)"
              % (args.attach, args.seconds, load_average()), file=sys.stderr)
        sample_process(args.attach, args.seconds, out_file)
    else:
        binary = (REPO / PROFILE_TARGET / "ci" / "civvis") if args.no_build \
            else build(REPO, args.quiet_build)
        if not binary.exists():
            raise SystemExit("%s does not exist; drop --no-build" % binary)
        shape = screen_shape()
        command = simulate_command(binary, args.seed, args.turns, shape)
        print("$ %s\n  load %.2f at start" % (" ".join(command), load_average()),
              file=sys.stderr)
        started = time.time()
        running = subprocess.Popen(command, stdout=subprocess.DEVNULL,
                                   stderr=subprocess.DEVNULL)
        # `sample` needs the process to be up; it attaches by pid and a race
        # here costs the first samples, not the run.
        time.sleep(1)
        sample_process(running.pid, args.seconds, out_file)
        running.wait()
        print("  ran %.1fs, exit %d, load %.2f at end"
              % (time.time() - started, running.returncode, load_average()),
              file=sys.stderr)

    profile = Profile(out_file.read_text(errors="replace"), rustfilt)
    profile.check_is_this_program()
    report(profile, args.top, args.parents)
    print("\nraw sample: %s" % out_file, file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
