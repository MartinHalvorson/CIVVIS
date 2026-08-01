"""Check the CIVVIS-drives-Civilization-VI bridge BEFORE a ladder runs on it.

    python3 tools/civ6_preflight.py            # check everything
    python3 tools/civ6_preflight.py --run TAG  # also audit one finished run

⚠ THIS EXISTS BECAUSE EVERY BRIDGE DEFECT WAS FOUND LATE AND EXPENSIVELY. The
pattern held for the whole project: a fault was introduced, thirty attempts were
queued behind it, and it surfaced hours later as "CIVVIS made no decisions" or as a
ledger row that read `stalled` when the game had actually been lost. A 30-attempt
ladder is roughly a day of wall clock. Every check below costs under a second and
each one corresponds to a defect that actually shipped:

  mod syntax        A Lua syntax error does not announce itself. The context loads,
                    the script dies, and the run looks like a game where CIVVIS
                    never decided anything. The harness re-syncs the mod at the
                    start of EVERY attempt, so one bad edit propagates through the
                    entire ladder.
  mod installed     The installed copy under Civ6.app is a COPY, not a symlink.
                    Editing the worktree and forgetting the sync means measuring
                    yesterday's mod.
  harness syntax    Same argument for the Python side.
  modinfo XML       A malformed .modinfo silently drops the whole mod.
  engine tests      The library test suite was found UNCOMPILABLE this session --
                    two Plot initializers missing a field -- so nothing had been
                    running for an unknown period.
  identity          The seat's civ, leader and difficulty crossed the bridge for
                    the whole project and nothing read them. A Sweden game showed
                    as Rome, which made the two screens impossible to compare.

Exit status is 0 only when every check passes, so this can gate a ladder.
"""

from __future__ import annotations

import argparse
import ast
import json
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MOD = ROOT / "tools" / "civ6_control" / "mod"
INSTALLED = Path.home() / (
    "Library/Application Support/Steam/steamapps/common/"
    "Sid Meier's Civilization VI/Civ6.app/Contents/Assets/DLC/CivvisControl"
)
RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"


class Report:
    """Collects results so every check runs even after one fails.

    ⚠ Deliberately does NOT stop at the first failure. Stopping would hide the
    second and third problems until the next run, which is the slow loop this tool
    exists to break.
    """

    def __init__(self) -> None:
        self.failures: list[str] = []
        self.warnings: list[str] = []

    def ok(self, what: str, detail: str = "") -> None:
        print(f"  PASS  {what}{(' — ' + detail) if detail else ''}")

    def fail(self, what: str, why: str) -> None:
        print(f"  FAIL  {what} — {why}")
        self.failures.append(f"{what}: {why}")

    def warn(self, what: str, why: str) -> None:
        print(f"  WARN  {what} — {why}")
        self.warnings.append(f"{what}: {why}")


def check_lua(report: Report) -> None:
    print("mod Lua")
    luac = None
    for candidate in ("/opt/homebrew/bin/luac", "/usr/local/bin/luac", "luac"):
        try:
            subprocess.run([candidate, "-v"], capture_output=True, timeout=10)
            luac = candidate
            break
        except (OSError, subprocess.SubprocessError):
            continue
    if luac is None:
        report.warn("luac", "not installed; mod syntax cannot be checked here")
        return
    for path in sorted(MOD.glob("*.lua")):
        done = subprocess.run([luac, "-p", str(path)], capture_output=True, timeout=60)
        if done.returncode == 0:
            report.ok(path.name)
        else:
            report.fail(path.name, done.stderr.decode(errors="replace").strip()[:200])


def check_modinfo(report: Report) -> None:
    print("modinfo")
    for path in sorted(MOD.glob("*.modinfo")):
        try:
            ET.parse(path)
        except ET.ParseError as exc:
            report.fail(path.name, f"malformed XML: {exc}")
            continue
        # A ReplaceUIScript naming a file that is not imported silently does nothing:
        # <Files> is not in the virtual file system without an ImportFiles entry.
        text = path.read_text(errors="replace")
        replaced = text.count("<LuaReplace>")
        imported = "ImportFiles" in text
        if replaced and not imported:
            report.fail(path.name, f"{replaced} LuaReplace entries but no ImportFiles")
        else:
            report.ok(path.name, f"{replaced} screen hooks")


def check_python(report: Report) -> None:
    print("harness Python")
    bad = 0
    paths = list((ROOT / "tools").glob("civ6_*.py"))
    paths.append(ROOT / "tools" / "civ6_control" / "macos_input.py")
    for path in sorted(paths):
        try:
            ast.parse(path.read_text(errors="replace"))
        except SyntaxError as exc:
            report.fail(path.name, f"line {exc.lineno}: {exc.msg}")
            bad += 1
    if not bad:
        report.ok("all civ6_*.py parse")


def installed_source_matches(live: bytes, source: bytes) -> bool:
    """Accept an installer-generated settings prelude before the source Lua.

    ``install.py`` bakes one run's settings into each installed script, then
    appends the worktree file byte-for-byte. Comparing whole files therefore
    reported a stale installed module even immediately after a correct sync.
    """
    return live == source or live.endswith(source)


def check_installed(report: Report) -> None:
    """The installed mod is a COPY. Compare it against the worktree."""
    print("installed mod")
    if not INSTALLED.exists():
        report.warn("install dir", f"not found at {INSTALLED}")
        return
    for path in sorted(MOD.glob("*.lua")):
        live = INSTALLED / path.name
        if not live.exists():
            report.fail(path.name, "not installed")
        elif not installed_source_matches(live.read_bytes(), path.read_bytes()):
            # Not a failure: the harness re-syncs at attempt start, so a difference
            # before a run is normal. It is only fatal if someone is reading the
            # installed copy expecting it to be current.
            report.warn(path.name, "installed source differs; harness syncs at attempt start")
        else:
            report.ok(path.name, "matches worktree source")


def check_host(report: Report) -> None:
    """Can a game start AT ALL on this machine right now.

    ⚠ THIS TOOL GATES A LADDER AND DID NOT CHECK THE ONE THING THAT STOPS A LADDER
    DEAD. On 2026-07-31 a login exited Steam and the climb loop burned eleven of
    twenty-four attempts in two minutes; every check here passed, because every check
    here was about our own code. The mod can be perfect and the harness can parse and
    the engine tests can be green while the game cannot be launched.

    A WARNING, not a failure. Preflight is often run to check an edit on a machine
    where nobody intends to play — failing there would train the operator to pass
    `--skip` and lose the check entirely. The climb loop enforces it; this reports it.
    """
    print("host")
    sys.path.insert(0, str(ROOT / "tools"))
    from civ6_control import launcher, macos_input, vision

    if launcher.steam_running():
        report.ok("Steam", "running")
    else:
        report.warn("Steam", "not running; a ladder started now plays no games")
    try:
        report.ok("input", macos_input.probe())
    except macos_input.InputUnavailable as error:
        report.fail("input", str(error))
    if vision.available():
        report.ok("setup vision", "Pillow")
    else:
        report.fail(
            "setup vision",
            "Pillow is required for verified lobby navigation; run "
            "python3 -m pip install --user Pillow",
        )
    # Asked of the launcher rather than rebuilt from INSTALLED: the real binary is
    # `Civ6_Exe_Child`, four directories up and under a different name, and writing
    # that path out a second time is how two checks come to disagree.
    binary = launcher.game_binary()
    if binary.is_file():
        report.ok("game binary", binary.name)
    else:
        report.warn("game binary", f"not found at {binary}")


def check_engine(report: Report) -> None:
    print("engine")
    cargo = Path.home() / ".cargo" / "bin" / "cargo"
    if not cargo.exists():
        report.warn("cargo", "not found; engine tests skipped")
        return
    done = subprocess.run(
        [str(cargo), "test", "--release", "--lib"],
        cwd=ROOT, capture_output=True, timeout=3600,
    )
    tail = done.stdout.decode(errors="replace").strip().splitlines()
    summary = next((line for line in reversed(tail) if "test result" in line), "")
    if done.returncode == 0:
        report.ok("cargo test --lib", summary.strip())
    else:
        report.fail("cargo test --lib", summary.strip() or "did not compile or failed")


def check_run(report: Report, tag: str) -> None:
    """Audit a finished run for the failures that used to be invisible."""
    print(f"run {tag}")
    events = RUN_ROOT / tag / "events.jsonl"
    if not events.exists():
        report.fail(tag, "no events.jsonl")
        return
    seat, turns, source, refusals = None, 0, {}, {}
    for line in events.read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        kind = event.get("kind")
        if kind == "seat":
            seat = event
        elif kind == "turn":
            turns += 1
            key = event.get("orders_source")
            source[key] = source.get(key, 0) + 1
        elif kind == "orders":
            for reason, count in (event.get("refusals") or {}).items():
                refusals[reason] = refusals.get(reason, 0) + count

    # ⚠ Identity is the check that would have caught the mismatch the operator spotted
    # by eye. `civ` reading "?" means the seat event fired but the getter failed.
    if seat and seat.get("civ") not in (None, "", "?"):
        report.ok("identity", f"{seat.get('civ')} / {seat.get('leader')} / {seat.get('difficulty')}")
    else:
        report.fail("identity", "seat event missing or civ unresolved")

    if turns == 0:
        report.fail("turns", "no turn records — the mod probably never ran")
    else:
        civvis = source.get("civvis", 0)
        share = 100 * civvis // turns
        # Below 100% something else decided part of the game, which makes the run a
        # measurement of the fallbacks rather than of CIVVIS.
        (report.ok if share == 100 else report.warn)(
            "CIVVIS decided", f"{civvis}/{turns} turns ({share}%)")

    if refusals:
        worst = sorted(refusals.items(), key=lambda kv: -kv[1])[:3]
        report.warn("refusals", ", ".join(f"{k}={v}" for k, v in worst))
    else:
        report.ok("refusals", "none")


def check_decider(report: Report, orders_bin: str | None) -> None:
    """The decider's --serve protocol: one line in, one ORDERS line out.

    ⚠ THIS CHECK EXISTS BECAUSE A `println` COST A WHOLE RUN. `--serve` speaks a
    strict one-line-per-request protocol and `civ6_brain.py` does exactly one
    `readline()` per turn, reading `payload["orders"]`. A diagnostic line printed to
    STDOUT at startup sits in front of the first response; it is valid JSON with no
    `orders` key, so it parses cleanly, yields an empty list, and shifts every later
    turn by one.

    Nothing raised. A live run that had been 236 turns of `orders_source: civvis`
    flipped to `fallback` the moment a binary carrying that line was swapped in --
    the hand-written ladder playing while CIVVIS decided correctly into a pipe
    nobody read. `why.log` showed it founding its capital on the very turn the brain
    recorded zero orders.

    So: probe the real binary against the newest finished run and require that the
    FIRST line of stdout is an orders response. A second of preflight against a day
    of ladder.
    """
    print("decider protocol")
    if orders_bin is None:
        report.warn("decider", "no --orders-bin given; protocol not probed")
        return
    binary = Path(orders_bin)
    if not binary.is_file():
        report.fail("decider", f"{binary} does not exist")
        return
    runs = sorted((Path.home() / "civvis-civ6-runs" / "control").glob("*/events.jsonl"),
                  key=lambda p: p.stat().st_mtime, reverse=True)
    probe = next((p.parent for p in runs if p.stat().st_size > 0), None)
    if probe is None:
        report.warn("decider", "no finished run to probe against; protocol not checked")
        return
    try:
        proc = subprocess.run(
            [str(binary), "--mirror", str(probe), "--turn", "1"],
            capture_output=True, text=True, timeout=180)
    except (subprocess.SubprocessError, OSError) as exc:
        report.fail("decider", f"could not run: {exc}")
        return
    first = next((ln for ln in proc.stdout.splitlines() if ln.startswith("{")), None)
    if first is None:
        report.fail("decider", "printed no JSON line at all")
        return
    try:
        payload = json.loads(first)
    except ValueError:
        report.fail("decider", f"first stdout line is not JSON: {first[:90]}")
        return
    if "orders" not in payload:
        # The exact regression. Name the intruder so the fix is obvious.
        report.fail("decider",
                    f"first stdout line is NOT an orders response — the brain will read "
                    f"it as 'CIVVIS chose nothing'. Got keys {sorted(payload)}; move "
                    f"that output to stderr")
        return
    report.ok("--serve first line is an orders response",
              f"{len(payload.get('orders') or [])} orders from {probe.name}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--run", default=None, help="also audit this finished run tag")
    parser.add_argument("--skip-engine", action="store_true",
                        help="skip cargo test (the slow check)")
    parser.add_argument("--orders-bin", default=None,
                        help="probe this decider's --serve protocol")
    args = parser.parse_args()

    report = Report()
    check_lua(report)
    check_modinfo(report)
    check_python(report)
    check_installed(report)
    check_host(report)
    if not args.skip_engine:
        check_engine(report)
    check_decider(report, args.orders_bin)
    if args.run:
        check_run(report, args.run)

    print()
    if report.failures:
        print(f"PREFLIGHT FAILED — {len(report.failures)} problem(s):")
        for line in report.failures:
            print(f"  - {line}")
        return 1
    print(f"preflight clear ({len(report.warnings)} warning(s))")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
