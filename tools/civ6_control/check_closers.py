#!/usr/bin/env python3
"""Check that the autoclose shim can detect every screen it can close.

⚠ WHY THIS EXISTS, twice over.

`haveScreen()` decides "did the shipped script load into this context", and it
answers by looking for a close handler. `endScreen()` decides how to close it.
Those two lists must agree, and they have now failed in both directions:

1. They drifted. The ladder learned `OnHideScreen` and `OnButton1` for the relic
   screens while the detector still tested only the original four names, so
   `haveScreen` said "no screen here" about screens that had loaded perfectly
   well and never armed their timer — `autoclose_unarmed` for GreatWorkShowcase
   and ChooseArtifact.

2. Unifying them broke everything. Sharing one table and testing it with
   `type(_G[name]) == "function"` looked like the fix. It is not: each
   Civilization VI UI context runs in its own environment, so `_G["OnClose"]`
   does not resolve the same name a bare `OnClose` does. Every one of the 21
   registered contexts reported `autoclose_unarmed`, including five that had been
   arming correctly for hours.

So the detector must use bare global references, which means the list cannot be
shared at runtime — it can only be kept in step. That is what this checks.

    python3 tools/civ6_control/check_closers.py
    python3 tools/civ6_control/check_closers.py --self-test
"""
from __future__ import annotations

import argparse
import pathlib
import re
import sys

SHIM = pathlib.Path(__file__).resolve().parent / "mod" / "CivvisControlAutoClose.lua"

# Handlers the ladder may call without the detector needing to know them: these
# are reached only after some other handler has already proved the script loaded.
DETECTOR_EXEMPT: set[str] = set()


def strip_comments(text: str) -> str:
    return "\n".join(re.sub(r"--.*$", "", line) for line in text.splitlines())


def detector_names(src: str) -> set[str]:
    """Names tested inside haveScreen()."""
    body = src[src.index("local function haveScreen()"):]
    body = body[: body.index("\nend")]
    return set(re.findall(r"type\((\w+)\)\s*==\s*\"function\"", body))


def ladder_names(src: str) -> set[str]:
    """Names the close ladder calls, guarded by a type() check."""
    body = src[src.index("local function endScreen("):]
    body = body[: body.index("\nend\n")]
    return set(re.findall(r"type\((\w+)\)\s*==\s*\"function\"", body))


def check(src: str) -> list[str]:
    problems = []
    if "_G[" in strip_comments(src):
        problems.append(
            "haveScreen must use bare global references, not _G[...]: a UI "
            "context has its own environment and _G lookups miss it"
        )
    detector, ladder = detector_names(src), ladder_names(src)
    for name in sorted(ladder - detector - DETECTOR_EXEMPT):
        problems.append(
            f"endScreen can call '{name}' but haveScreen does not test it, so a "
            f"screen whose only closer is '{name}' never arms"
        )
    return problems


def self_test() -> int:
    """Both real failure modes, and the fixed shape."""
    good = """
local function haveScreen()
	return type(OnClose) == "function" or type(OnHideScreen) == "function";
end
local function endScreen(attempt)
	if type(OnHideScreen) == "function" then OnHideScreen(); return true; end
	if type(OnClose) == "function" then OnClose(); return true; end
	return false;
end
"""
    drifted = good.replace(' or type(OnHideScreen) == "function"', "", 1)
    via_g = good.replace(
        'type(OnClose) == "function" or type(OnHideScreen) == "function"',
        'type(_G["OnClose"]) == "function" or type(_G["OnHideScreen"]) == "function"',
        1,
    )
    failures = 0
    if check(good):
        print(f"FAIL: false positive on the fixed shape: {check(good)}")
        failures += 1
    if not any("OnHideScreen" in p for p in check(drifted)):
        print("FAIL: did not catch the drifted detector")
        failures += 1
    if not any("_G" in p for p in check(via_g)):
        print("FAIL: did not catch the _G lookup")
        failures += 1
    print("self-test: " + ("ok" if not failures else f"{failures} failure(s)"))
    return failures


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return 1 if self_test() else 0

    problems = check(SHIM.read_text())
    for problem in problems:
        print(f"{SHIM.name}: {problem}")
    print(f"checked the autoclose shim: {len(problems)} finding(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
