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
import shutil
import subprocess
import sys
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


def install(config: dict) -> Path:
    target = install_dir()
    target.mkdir(parents=True, exist_ok=True)
    text = prelude(config)
    for src in sorted(MOD_SOURCE.iterdir()):
        if src.name in SCRIPTS:
            written = target / src.name
            written.write_text(text + src.read_text())
            error = check_syntax(written)
            if error:
                raise SystemExit(f"{src.name} does not parse: {error}")
        else:
            shutil.copy2(src, target / src.name)
    # Kept beside the scripts so the active settings are readable in the
    # install without reading a whole script.
    (target / "config.json").write_text(json.dumps(config, indent=2, sort_keys=True))

    mod_db = env.user_dir() / "Mods.sqlite"
    if mod_db.exists():
        mod_db.unlink()
    return target


def uninstall() -> bool:
    target = install_dir()
    if not target.is_dir():
        return False
    shutil.rmtree(target)
    mod_db = env.user_dir() / "Mods.sqlite"
    if mod_db.exists():
        mod_db.unlink()
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
