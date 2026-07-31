#!/usr/bin/env python3
"""Find identifiers used outside the scope that declares them.

⚠ WHY THIS EXISTS. `orderSettler` used `id` on four lines and never declared it,
so `id` was a nil global and `refusedSite[id] = ...` threw "table index is nil"
for every settler the engine would not path to its site. The settler then
received no order at all, the empire stayed on two cities, and the operator asked
"you never settled?". Lua reports nothing for this: reading an undeclared global
yields nil, and only the eventual table index errors.

It survived every existing check. `luac -p` parses it happily — it is valid Lua.
`check_lua51.py` only looks for constructs newer than 5.1. And it was invisible
at runtime until the roster `pcall` moved inside its loop, because before that
the throw silently ended the whole unit walk (see the controller memory note).
`luacheck` would catch it, but it is not installed and pulling in luarocks for
one rule is not worth it.

The signal that matters is a name that IS a local somewhere in the file but is
used where that local is not in scope. That is a scope slip, essentially always a
bug. A name never declared anywhere is presumably an engine global (`Map`,
`GameInfo`, `UnitManager`) and is only reported under --globals.

The scope tracker is deliberately simple — a stack of blocks, pushed on
`function`/`do`/`then`/`else`/`repeat` and popped on `end`/`until`. That is not a
full Lua parser, so treat findings as leads. It finds the bug it was written for,
and the test at the bottom of this file proves it still does.

    python3 tools/civ6_control/check_scope.py tools/civ6_control/mod/*.lua
    python3 tools/civ6_control/check_scope.py --self-test
"""
from __future__ import annotations

import argparse
import pathlib
import re
import sys

# Lua 5.1 standard library plus the Civilization VI script API. Anything here is
# expected to be global; everything else global is at least worth a look.
KNOWN_GLOBALS = {
    # Lua 5.1
    "_G", "_VERSION", "assert", "collectgarbage", "coroutine", "debug", "dofile",
    "error", "getfenv", "getmetatable", "io", "ipairs", "load", "loadfile",
    "loadstring", "math", "module", "next", "os", "package", "pairs", "pcall",
    "print", "rawequal", "rawget", "rawset", "require", "select", "setfenv",
    "setmetatable", "string", "table", "tonumber", "tostring", "type", "unpack",
    "xpcall",
    # Civilization VI
    "Automation", "CityCommandTypes", "CityManager", "CityOperationTypes",
    "Controls", "DB", "DiplomacyManager", "Events", "ExposedMembers", "Game",
    "GameConfiguration", "GameEffects", "GameInfo", "GameplayEvents", "Input",
    "Locale", "LuaEvents", "Map", "MapConfiguration", "ModUserData",
    "NotificationManager", "PlayerConfigurations", "PlayerManager", "Players",
    "PlayersVisibility",
    "RevealedState", "UI", "UILens", "UnitCommandTypes", "UnitManager",
    "UnitOperationTypes", "YieldTypes",
    # Context-scoped engine globals, present in every UI script.
    "ContextPtr", "Controls", "include", "Keys", "KeyEvents", "Mouse",
    # This mod's own install-time config global.
    "CivvisControlConfig",
    # Engine type enumerations, same family as the ones above.
    "ActionTypes", "CivilizationLevelTypes", "EndTurnBlockingTypes",
    "InterfaceModeTypes", "Modding", "Network", "PlayerOperations",
    "ServerType", "SlotStatus", "TurnLimitTypes",
}

KEYWORDS = {
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function",
    "goto", "if", "in", "local", "nil", "not", "or", "repeat", "return", "then",
    "true", "until", "while",
}

NAME = r"[A-Za-z_]\w*"
TOKEN = re.compile(r"--\[(=*)\[|--[^\n]*|\[(=*)\[|\"|'|" + NAME + r"|\S")


def strip_noise(text: str) -> str:
    """Blank out comments and string bodies, preserving line structure."""
    out, i, n = [], 0, len(text)
    while i < n:
        if text.startswith("--", i):
            m = re.match(r"--\[(=*)\[", text[i:])
            if m:
                close = "]" + "=" * len(m.group(1)) + "]"
                j = text.find(close, i)
                j = n if j < 0 else j + len(close)
            else:
                j = text.find("\n", i)
                j = n if j < 0 else j
            out.append(re.sub(r"[^\n]", " ", text[i:j]))
            i = j
            continue
        m = re.match(r"\[(=*)\[", text[i:])
        if m:
            close = "]" + "=" * len(m.group(1)) + "]"
            j = text.find(close, i)
            j = n if j < 0 else j + len(close)
            out.append(re.sub(r"[^\n]", " ", text[i:j]))
            i = j
            continue
        if text[i] in "\"'":
            quote, j = text[i], i + 1
            while j < n and text[j] != quote:
                j += 2 if text[j] == "\\" else 1
            j = min(j + 1, n)
            out.append(re.sub(r"[^\n]", " ", text[i:j]))
            i = j
            continue
        out.append(text[i])
        i += 1
    return "".join(out)


def tokenize(text: str) -> list[tuple[str, int]]:
    tokens = []
    for line_no, line in enumerate(text.splitlines(), 1):
        for m in re.finditer(NAME + r"|[=,\.\:\(\)\{\}\[\]]|\S", line):
            tokens.append((m.group(0), line_no))
    return tokens


def guarded_names(text: str) -> set[str]:
    """Names the file checks for existence before using.

    `if type(OnClose) == "function" then OnClose() end` and
    `if ms_ActiveSessionID ~= nil then ...` are deliberate probes of something
    that may legitimately not exist — a shipped screen's handler, say. Reporting
    those as suspicious globals buried the one real finding under sixteen
    intentional ones, which is how a gate becomes something you stop reading.
    An unguarded call like `try(...)` has no such excuse.
    """
    clean = strip_noise(text)
    guarded = set(re.findall(r"\btype\s*\(\s*(" + NAME + r")\s*\)", clean))
    guarded |= set(re.findall(r"\b(" + NAME + r")\s*[=~]=\s*nil\b", clean))
    return guarded


def analyse(text: str) -> tuple[list[tuple[int, str]], set[str], set[str]]:
    """Return (scope slips, all locals declared, globals read)."""
    tokens = tokenize(strip_noise(text))
    scopes: list[set[str]] = [set()]
    all_locals: set[str] = set()
    globals_read: set[str] = set()
    slips: list[tuple[int, str]] = []

    def declare(name: str) -> None:
        scopes[-1].add(name)
        all_locals.add(name)

    def in_scope(name: str) -> bool:
        return any(name in s for s in scopes)

    i = 0
    pending_params = False
    while i < len(tokens):
        tok, line = tokens[i]

        if tok == "local":
            j = i + 1
            if j < len(tokens) and tokens[j][0] == "function":
                # `local function f` — f is visible in the enclosing scope.
                if j + 1 < len(tokens):
                    declare(tokens[j + 1][0])
                i = j
                continue
            while j < len(tokens) and (
                    re.fullmatch(NAME, tokens[j][0]) or tokens[j][0] == ","):
                if tokens[j][0] != ",":
                    declare(tokens[j][0])
                j += 1
            i = j
            continue

        if tok == "for":
            # Loop variables belong to the loop body, which `do` will open.
            j, names = i + 1, []
            while j < len(tokens) and tokens[j][0] not in ("=", "in", "do"):
                if re.fullmatch(NAME, tokens[j][0]) and tokens[j][0] not in KEYWORDS:
                    names.append(tokens[j][0])
                j += 1
            scopes.append(set(names))
            all_locals.update(names)
            # Skip the header so `do` does not open a second scope.
            while i < len(tokens) and tokens[i][0] != "do":
                i += 1
            i += 1
            continue

        if tok == "function":
            scopes.append(set())
            pending_params = True
            i += 1
            continue

        if pending_params:
            if tok == "(":
                j = i + 1
                while j < len(tokens) and tokens[j][0] != ")":
                    if re.fullmatch(NAME, tokens[j][0]):
                        declare(tokens[j][0])
                    j += 1
                pending_params = False
                i = j + 1
                continue
            # A named function DEFINES that name. Without recording it,
            # `function Initialize()` reported as an undeclared read of
            # `Initialize` — the definition accusing itself.
            if re.fullmatch(NAME, tok) and tok not in KEYWORDS:
                scopes[0].add(tok)
                all_locals.add(tok)
            i += 1
            continue

        if tok in ("do", "then", "repeat"):
            scopes.append(set())
            i += 1
            continue

        if tok in ("end", "until"):
            if len(scopes) > 1:
                scopes.pop()
            i += 1
            continue

        if tok in ("else", "elseif"):
            if len(scopes) > 1:
                scopes.pop()
            scopes.append(set())
            i += 1
            continue

        if re.fullmatch(NAME, tok) and tok not in KEYWORDS:
            prev = tokens[i - 1][0] if i else ""
            nxt = tokens[i + 1][0] if i + 1 < len(tokens) else ""
            # Skip field accesses: a.b, a:b.
            if prev in (".", ":"):
                i += 1
                continue
            # Skip table-constructor keys: `{ x = cx, y = cy }` names a field,
            # it does not read a variable called x. Without this the checker
            # reported eight bogus `x` and eight bogus `y` findings in this very
            # file, which is exactly the noise that makes a linter ignorable.
            if nxt == "=" and prev in ("{", ","):
                i += 1
                continue
            if not in_scope(tok) and tok not in KNOWN_GLOBALS:
                globals_read.add(tok)
                slips.append((line, tok))
        i += 1

    return slips, all_locals, globals_read


def check(path: pathlib.Path, show_globals: bool) -> int:
    text = path.read_text()
    slips, all_locals, globals_read = analyse(text)
    findings = 0
    seen: set[tuple[int, str]] = set()
    for line, name in slips:
        # The high-signal case: a name that IS a local elsewhere in this file,
        # used where that local is not in scope.
        if name in all_locals and (line, name) not in seen:
            seen.add((line, name))
            print(f"{path}:{line}: scope slip: '{name}' is a local elsewhere "
                  f"in this file but is not in scope here")
            findings += 1
    if show_globals:
        unguarded = globals_read - all_locals - guarded_names(text)
        for name in sorted(unguarded):
            print(f"{path}: calls undeclared global '{name}' without guarding it")
            findings += 1
    return findings


def self_test() -> int:
    """The bug this file was written for, and a clean counterpart."""
    bad = """
local refused = {};
local function ok(unit)
    local id = unit:GetID();
    refused[id] = true;
end
local function bug(unit)
    refused[id] = true;
end
"""
    good = """
local refused = {};
local function ok(unit)
    local id = unit:GetID();
    refused[id] = true;
end
local function fixed(unit)
    local id = unit:GetID();
    refused[id] = true;
end
"""
    bad_slips = [n for _, n in analyse(bad)[0] if n in analyse(bad)[1]]
    good_slips = [n for _, n in analyse(good)[0] if n in analyse(good)[1]]
    failures = 0
    if "id" not in bad_slips:
        print("FAIL: did not catch the out-of-scope 'id'")
        failures += 1
    if "id" in good_slips:
        print("FAIL: false positive on the fixed version")
        failures += 1
    print("self-test: " + ("ok" if not failures else f"{failures} failure(s)"))
    return failures


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("paths", nargs="*", type=pathlib.Path)
    ap.add_argument("--globals", action="store_true",
                    help="also list globals this file reads but never declares")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return 1 if self_test() else 0

    # ⚠ With no arguments this used to check NOTHING and exit 0, printing
    # "checked 0 file(s): 0 finding(s)" — a guard that passes vacuously is worse
    # than no guard, because it is quoted as evidence. It reported green on the
    # very edit that added `chooseEnvoy`. Same family as `install.py` with no
    # args exiting 0 having installed nothing.
    paths = args.paths or sorted((pathlib.Path(__file__).parent / "mod").glob("*.lua"))
    total = sum(check(p, args.globals) for p in paths)
    if not paths:
        print("FAIL: no Lua files found to check — refusing to report success")
        return 2
    print(f"checked {len(paths)} file(s) for scope slips: {total} finding(s)")
    return 1 if total else 0


if __name__ == "__main__":
    sys.exit(main())
