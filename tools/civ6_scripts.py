#!/usr/bin/env python3
"""Grep the shipped Civilization VI scripts and rules on this machine.

Every rule CIVVIS drives in the live game is written down somewhere under
the install's ``Civ6.app/Contents/Assets``: the UI scripts (``.lua``) the
game actually runs, the localisation and gameplay XML, and the expansions'
replacements of each. A PR that models one of those interactions quotes the
line it is modelled on (see AGENTS.md, "A claim is not a check"); this is the
tool that finds the line.

Usage::

    python tools/civ6_scripts.py locate
    python tools/civ6_scripts.py grep 'MakeDeal_ApplyStatement'
    python tools/civ6_scripts.py grep 'LOC_DIPLO_MODIFIER' --where text
    python tools/civ6_scripts.py grep 'BarbarianAttackForces' --where gameplay --expansion base

``locate`` prints the install roots it found; ``grep`` prints
``file:line: text`` with the file relative to the assets root, so a match
can be pasted into a PR body as it stands. ``--where`` picks the scripts
(``ui``), the text tables (``text``), the gameplay XML (``gameplay``) or all
three; ``--expansion`` picks the base game, one expansion (``1`` or ``2``),
or every content directory. ripgrep is used when it is on the PATH and a
Python walk otherwise; both print the same lines.

Exit status is 0 with at least one match, 1 with none, 2 with no install.
This needs the game on the machine, so it is a fleet-Mac tool, not a gate.
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import civ6_env  # noqa: E402

# Where each kind of source lives, relative to the assets root, for the base
# game and for a DLC directory. The expansions ship their UI as
# ``UI/Replacements`` and ``UI/Additions`` under ``DLC/ExpansionN/UI``.
WHERE = {
    "ui": ("Base/Assets/UI", "UI"),
    "text": ("Base/Assets/Text", "Text"),
    "gameplay": ("Base/Assets/Gameplay/Data", "Data"),
}
EXTENSIONS = (".lua", ".xml", ".sql")
EXPANSIONS = {"1": "Expansion1", "2": "Expansion2"}


def assets_root(explicit: str | None = None) -> Path | None:
    try:
        root = civ6_env.assets_dir(explicit)
    except Exception:  # noqa: BLE001 - any failure to resolve is "no install"
        return None
    return root if (root / "Base/Assets").is_dir() else None


def roots(assets: Path, where: str, expansion: str) -> list[Path]:
    """The directories a search covers, in load order: base first, then DLC."""
    kinds = list(WHERE) if where == "all" else [where]
    found: list[Path] = []
    for kind in kinds:
        base, dlc = WHERE[kind]
        if expansion in ("base", "all"):
            found.append(assets / base)
        if expansion in EXPANSIONS:
            found.append(assets / "DLC" / EXPANSIONS[expansion] / dlc)
        elif expansion == "all" and (assets / "DLC").is_dir():
            for pack in sorted(assets / "DLC" / name for name in os.listdir(assets / "DLC")):
                # Our own control mod is installed into this tree; reading it
                # back as the shipped game is how an invented name once looked
                # legitimate (`data/civ6_type_names.json`).
                if pack.name == "CivvisControl":
                    continue
                found.append(pack / dlc)
    return [path for path in found if path.is_dir()]


def grep_python(pattern: re.Pattern, assets: Path, directories: list[Path]) -> list[str]:
    lines = []
    for directory in directories:
        for path in sorted(directory.rglob("*")):
            if path.suffix.lower() not in EXTENSIONS or not path.is_file():
                continue
            relative = path.relative_to(assets)
            with path.open(encoding="utf-8", errors="replace") as handle:
                for number, text in enumerate(handle, start=1):
                    if pattern.search(text):
                        lines.append(f"{relative}:{number}: {text.rstrip()}")
    return lines


def grep_ripgrep(regex: str, assets: Path, directories: list[Path], ignore_case: bool) -> list[str] | None:
    """ripgrep's lines in the same ``file:line: text`` form, or None without it."""
    rg = shutil.which("rg")
    if rg is None:
        return None
    command = [rg, "--no-heading", "--line-number", "--with-filename", "--color", "never",
               "--sort", "path", "-e", regex]
    if ignore_case:
        command.append("-i")
    for extension in EXTENSIONS:
        command += ["-g", f"*{extension}"]
    command += [str(path) for path in directories]
    result = subprocess.run(command, capture_output=True, text=True, errors="replace", cwd=str(assets))
    if result.returncode not in (0, 1):
        return None
    # ripgrep prints ``path:line:text``; the Python walk prints a space after
    # the line number, so both are rebuilt into the one quotable form.
    lines = []
    for line in result.stdout.splitlines():
        path, _, rest = line.partition(":")
        number, _, text = rest.partition(":")
        where = Path(path)
        relative = where.resolve().relative_to(assets.resolve()) if where.is_absolute() else where
        lines.append(f"{relative}:{number}: {text.rstrip()}")
    return lines


def grep(regex: str, assets: Path, where: str, expansion: str, *, ignore_case: bool = False,
         use_ripgrep: bool = True) -> list[str]:
    directories = roots(assets, where, expansion)
    if use_ripgrep:
        found = grep_ripgrep(regex, assets, directories, ignore_case)
        if found is not None:
            return found
    pattern = re.compile(regex, re.IGNORECASE if ignore_case else 0)
    return grep_python(pattern, assets, directories)


def locate(assets: Path | None) -> list[str]:
    if assets is None:
        return ["no Civilization VI install found (set CIV6_INSTALL or pass --civ6)"]
    lines = [f"assets: {assets}"]
    for kind in WHERE:
        for path in roots(assets, kind, "all"):
            lines.append(f"{kind:9}{path.relative_to(assets)}")
    cache = civ6_env.user_dir() / "Cache" / "DebugGameplay.sqlite"
    lines.append(f"database: {cache}{'' if cache.is_file() else ' (absent)'}")
    return lines


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--civ6", help="install root or assets directory")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("locate", help="print the install roots this tool searches")
    search = commands.add_parser("grep", help="search the shipped scripts and XML")
    search.add_argument("regex")
    search.add_argument("--where", choices=("ui", "text", "gameplay", "all"), default="all")
    search.add_argument("--expansion", choices=("base", "1", "2", "all"), default="all")
    search.add_argument("-i", "--ignore-case", action="store_true")
    search.add_argument("--no-rg", action="store_true", help="use the Python walk even when ripgrep is present")
    args = parser.parse_args(argv)

    assets = assets_root(args.civ6)
    if args.command == "locate":
        print("\n".join(locate(assets)))
        return 0 if assets else 2
    if assets is None:
        print("no Civilization VI install found (set CIV6_INSTALL or pass --civ6)", file=sys.stderr)
        return 2
    lines = grep(args.regex, assets, args.where, args.expansion,
                 ignore_case=args.ignore_case, use_ripgrep=not args.no_rg)
    for line in lines:
        print(line)
    return 0 if lines else 1


if __name__ == "__main__":
    raise SystemExit(main())
