#!/usr/bin/env python3
"""Install the CIVVIS control mod with one run's settings baked into it.

The mod has no way to read a file at runtime -- Civilization VI's UI Lua has no
``io`` and no include path a mod file can be reached on -- so the run's
settings are prepended to each script at install time. That is enough: a run is
configured once, before the game starts, and the settings never change while it
plays.

Two constraints on this build shape the installer, both established by
measurement:

- The mod goes in the *install's* ``DLC`` tree. No user ``Mods`` directory is
  scanned, so a mod placed in one is never discovered and nothing logs why. The
  install is only added to, and ``uninstall`` reverts it completely.
- The modding database indexes scanned folders by path and mtime, so a newly
  written mod folder is not noticed until ``Mods.sqlite`` is dropped. Without
  that the scan reports the previous contents and the new script never runs.
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import civ6_env as env  # noqa: E402

MOD_SOURCE = Path(__file__).resolve().parent / "mod"
MOD_NAME = "CivvisControl"
SCRIPTS = (
    "CivvisControlSetup.lua",
    "CivvisControlAgent.lua",
    "CivvisControlAutoClose.lua",
)

PRELUDE_HEADER = """-- Prepended by tools/civ6_control/install.py. Do not edit the installed copy.
--
-- These are the run's settings. They are prepended rather than `include`d
-- because a file listed under <Files> in the .modinfo is not on the include
-- path unless an ImportFiles action puts it there -- so the include fails
-- silently and every setting falls back to its default, which for a difficulty
-- ladder means a run that reports Settler and plays Prince.

"""


def lua_value(value) -> str:
    if value is None:
        return "nil"
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        return repr(value)
    if isinstance(value, str):
        return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'
    if isinstance(value, dict):
        inner = ", ".join(f"[{lua_value(k)}] = {lua_value(v)}" for k, v in value.items())
        return "{ " + inner + " }"
    if isinstance(value, (list, tuple)):
        return "{ " + ", ".join(lua_value(v) for v in value) + " }"
    raise TypeError(f"cannot express {type(value).__name__} in Lua")


def prelude(config: dict) -> str:
    lines = [PRELUDE_HEADER, "CivvisControlConfig = {\n"]
    for key in sorted(config):
        lines.append(f"\t{key} = {lua_value(config[key])},\n")
    lines.append("}\n\n")
    return "".join(lines)


def install_dir() -> Path:
    return env.assets_dir() / "DLC" / MOD_NAME


def check_syntax(path: Path) -> str | None:
    """Reject a script that will not parse, before the game swallows it.

    A Lua syntax error in a mod script is the worst failure mode here: the
    context loads, the script dies at parse time, and nothing is written to any
    log -- so a broken agent is indistinguishable from a game that is thinking.
    ``luac -p`` costs milliseconds and turns that into an install-time error.
    Absent ``luac``, the check is skipped rather than faked.
    """
    if shutil.which("luac") is None:
        return None
    result = subprocess.run(["luac", "-p", str(path)], capture_output=True, text=True)
    if result.returncode == 0:
        return None
    return (result.stderr or result.stdout).strip()


def _write_mod(target: Path, config: dict) -> None:
    """Materialize a fully configured control mod into a writable directory."""
    target.mkdir(parents=True, exist_ok=True)
    text = prelude(config)
    for src in sorted(MOD_SOURCE.iterdir()):
        if src.name in SCRIPTS:
            written = target / src.name
            written.write_text(text + src.read_text())
            error = check_syntax(written)
            if error:
                raise SystemExit(f"{src.name} does not parse: {error}")
        elif src.name == "CivvisControlConfig.xml":
            # The setup DEFAULTS, applied through the one FrontEnd hook shipped
            # content actually uses (`UpdateDatabase`). ⚠ The seed must be a real
            # integer: a literal 0 reads as "unset" and silently restores random
            # maps, which is the failure this whole file exists to end.
            seed = int(config.get("MapSeed") or 0)
            text_xml = src.read_text()
            if seed > 0:
                text_xml = text_xml.replace("CIVVIS_SEED", str(seed))
            else:
                # No seed asked for: drop the two seed updates rather than write a
                # placeholder the database would reject.
                text_xml = re.sub(
                    r"\t\t<Update>\s*<Where ParameterId=\"(?:Map|Game)RandomSeed\"/>.*?</Update>\n",
                    "", text_xml, flags=re.S)
            (target / src.name).write_text(text_xml)
        else:
            shutil.copy2(src, target / src.name)
    # Kept beside the scripts so the active settings are readable in the
    # install without reading a whole script.
    # ⚠⚠ THE RUN TAG IN THIS FILE IS THE GAME LOCK'S IDENTITY. `gamelock.foreign_run`
    # reads it and, if the game is up and the tag is not the caller's, reports
    # "another run holds the game" — which `civ6_climb` counts as a spent ATTEMPT.
    # A throwaway install (a probe, a syntax check) that writes a made-up tag and
    # leaves Civ 6 running therefore locks out every subsequent run: it burned all
    # four attempts twice in a row with RunTag 'probe'. None is treated as "not
    # foreign", so an install with no real run behind it must not invent one.
    if not config.get("RunTag"):
        config = dict(config, RunTag=None)
    (target / "config.json").write_text(json.dumps(config, indent=2, sort_keys=True))


def _finder_replace(source: Path, target: Path) -> None:
    """Replace the protected DLC copy through Finder when TCC blocks shell I/O."""
    script = f'''tell application "Finder"
    set sourceFolder to POSIX file {json.dumps(str(source))} as alias
    set dlcFolder to POSIX file {json.dumps(str(target.parent))} as alias
    if exists folder {json.dumps(target.name)} of dlcFolder then
        delete folder {json.dumps(target.name)} of dlcFolder
    end if
    set targetFolder to make new folder at dlcFolder with properties {{name:{json.dumps(target.name)}}}
    repeat with sourceItem in (get every item of sourceFolder)
        duplicate sourceItem to targetFolder
    end repeat
end tell'''
    result = subprocess.run(
        ["osascript", "-e", script], capture_output=True, text=True, timeout=60,
    )
    if result.returncode:
        detail = (result.stderr or result.stdout).strip()
        raise PermissionError(f"Finder could not install {target}: {detail}")


def _finder_delete(path: Path) -> None:
    script = f'''tell application "Finder"
    set targetItem to POSIX file {json.dumps(str(path))} as alias
    delete targetItem
end tell'''
    result = subprocess.run(
        ["osascript", "-e", script], capture_output=True, text=True, timeout=30,
    )
    if result.returncode:
        detail = (result.stderr or result.stdout).strip()
        raise PermissionError(f"Finder could not remove {path}: {detail}")


def _drop_mod_index() -> None:
    mod_db = env.user_dir() / "Mods.sqlite"
    try:
        if mod_db.exists():
            mod_db.unlink()
    except PermissionError:
        _finder_delete(mod_db)


def install(config: dict) -> Path:
    target = install_dir()
    try:
        _write_mod(target, config)
    except PermissionError as direct_error:
        # The Steam application tree can be under a macOS privacy rule that
        # grants Finder access while denying a child of Terminal. Build first in
        # /tmp, then have Finder replace only this mod's directory.
        staging = Path(tempfile.mkdtemp(prefix="civvis-control-"))
        try:
            _write_mod(staging, config)
            _finder_replace(staging, target)
        except Exception as fallback_error:
            raise PermissionError(
                f"cannot install {target} directly or through Finder: {fallback_error}"
            ) from direct_error
        finally:
            shutil.rmtree(staging, ignore_errors=True)
    _drop_mod_index()
    return target


def uninstall() -> bool:
    target = install_dir()
    if not target.is_dir():
        return False
    try:
        shutil.rmtree(target)
    except PermissionError:
        _finder_delete(target)
    _drop_mod_index()
    return True


def installed_config() -> dict | None:
    path = install_dir() / "config.json"
    if not path.is_file():
        return None
    return json.loads(path.read_text())


if __name__ == "__main__":
    import argparse

    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--uninstall", action="store_true")
    ap.add_argument("--status", action="store_true")
    ap.add_argument("--config", help="JSON file of settings to bake in")
    args = ap.parse_args()

    if args.uninstall:
        print("removed" if uninstall() else "not installed")
    elif args.config:
        target = install(json.loads(Path(args.config).read_text()))
        print(f"installed -> {target}")
    if args.status:
        target = install_dir()
        print(f"install dir : {target}  ({'present' if target.is_dir() else 'absent'})")
        cfg = installed_config()
        if cfg:
            for key in sorted(cfg):
                print(f"  {key:<20} {cfg[key]!r}")
