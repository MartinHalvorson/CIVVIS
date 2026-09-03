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
- That tree is inside a signed application bundle, so installing INVALIDATES
  the bundle's code signature and uninstalling restores it (issue #1342).
  ``signature_report()`` is how the harness says so out loud, with the offending
  files attributed to this mod rather than left as an anonymous "sealed resource
  is missing or invalid". It is a fact to report, not a failure to refuse on: a
  host with a healthy trust record plays perfectly well with the seal broken.
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
from xml.sax.saxutils import escape as xml_escape
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


# The Configuration database's ruleset-specific leader domain is the one
# actual FrontEnd channel this macOS build loads. Keeping the mapping here
# makes a direct Create Game launch select the requested human leader without
# the unreliable in-game rehost workaround. Unknown rulesets retain Civ's
# ordinary roster rather than inventing an invalid database domain.
LEADER_DOMAINS = {
    "RULESET_STANDARD": "Players:StandardPlayers",
    "RULESET_EXPANSION_1": "Players:Expansion1_Players",
    "RULESET_EXPANSION_2": "Players:Expansion2_Players",
}

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


def _frontend_defaults_xml(source: str, config: dict) -> str:
    """Render direct Create Game defaults into the FrontEnd database update."""
    defaults = {
        "CIVVIS_DEFAULT_DIFFICULTY": config.get("Difficulty") or "DIFFICULTY_PRINCE",
        "CIVVIS_DEFAULT_SPEED": config.get("GameSpeed") or "GAMESPEED_STANDARD",
        "CIVVIS_DEFAULT_MAP": config.get("MapScript") or "Continents.lua",
        "CIVVIS_DEFAULT_MAP_SIZE": config.get("MapSize") or "MAPSIZE_SMALL",
        "CIVVIS_DEFAULT_RULESET": config.get("RuleSet") or "RULESET_EXPANSION_2",
    }
    try:
        max_turns = int(config.get("MaxTurns") or 500)
    except (TypeError, ValueError) as error:
        raise ValueError("MaxTurns must be an integer") from error
    if max_turns < 1:
        raise ValueError("MaxTurns must be positive")
    defaults["CIVVIS_DEFAULT_MAX_TURNS"] = str(max_turns)

    rendered = source
    # MAP_SIZE contains the MAP token, so render longest placeholders first.
    # A normal dict order would turn ``CIVVIS_DEFAULT_MAP_SIZE`` into
    # ``Continents.lua_SIZE`` before its own replacement had a chance to run.
    for token in sorted(defaults, key=len, reverse=True):
        value = defaults[token]
        if not isinstance(value, str) or not value:
            raise ValueError(f"{token} must render to a non-empty string")
        rendered = rendered.replace(token, xml_escape(value))

    leader = config.get("Leader")
    if leader:
        ruleset = defaults["CIVVIS_DEFAULT_RULESET"]
        domain = LEADER_DOMAINS.get(ruleset)
        if domain is None:
            raise ValueError(
                f"cannot constrain a requested leader for unsupported ruleset {ruleset!r}"
            )
        rendered = rendered.replace("CIVVIS_DEFAULT_LEADER_DOMAIN", xml_escape(domain))
        rendered = rendered.replace("CIVVIS_DEFAULT_LEADER", xml_escape(str(leader)))
    else:
        rendered = re.sub(
            r"\n\t<!-- install\.py retains this one-row whitelist.*?</RulesetSupportedValues>",
            "",
            rendered,
            flags=re.S,
        )

    unresolved = re.findall(r"CIVVIS_DEFAULT_[A-Z_]+", rendered)
    if unresolved:
        raise ValueError(f"unresolved FrontEnd defaults: {sorted(set(unresolved))}")
    return rendered


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
            # The setup defaults are applied through the one FrontEnd hook shipped
            # content actually uses (`UpdateDatabase`). This seed branch is only
            # for an explicit diagnostic probe: a literal 0 reads as unset, so
            # omit both rows unless the probe supplied a positive request.
            seed = int(config.get("MapSeed") or 0)
            text_xml = _frontend_defaults_xml(src.read_text(), config)
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
    # "another run holds the game" — which the climb loop counts as a spent ATTEMPT.
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


def _finder_put_file(source: Path, target: Path) -> None:
    """Land one staged file at `target` through Finder when direct writes 403.

    The duplicate keeps the SOURCE's name, so the caller stages the file under
    exactly `target.name`. Delete-then-duplicate rather than a replacing copy:
    Finder's `duplicate ... with replacing` raises its own confirmation sheet
    on some macOS builds, and a sheet nobody answers is this project's oldest
    failure mode.
    """
    script = f'''tell application "Finder"
    if exists (POSIX file {json.dumps(str(target))}) then
        delete (POSIX file {json.dumps(str(target))} as alias)
    end if
    duplicate (POSIX file {json.dumps(str(source))} as alias) to (POSIX file {json.dumps(str(target.parent))} as alias)
end tell'''
    result = subprocess.run(
        ["osascript", "-e", script], capture_output=True, text=True, timeout=30,
    )
    if result.returncode:
        detail = (result.stderr or result.stdout).strip()
        raise PermissionError(f"Finder could not write {target}: {detail}")


def clear_run_tag() -> bool:
    """Blank the installed config's RunTag; True if a tag was actually cleared.

    A tag with no game behind it describes nothing, and `gamelock.foreign_run`
    refuses the NEXT attempt over it — so the climb clears it at teardown. The
    climb used to rewrite config.json in place, which fails on hosts where the
    game's TCC rule lets Finder into the bundle and refuses Terminal
    ("Operation not permitted", measured at the end of both 2026-08-07 games).
    Owned here, beside install(), so the write path and its Finder fallback
    cannot drift apart from the module that writes this file everywhere else.
    """
    config = install_dir() / "config.json"
    if not config.is_file():
        return False
    data = json.loads(config.read_text())
    if data.get("RunTag") is None:
        return False
    data["RunTag"] = None
    text = json.dumps(data, indent=2, sort_keys=True)
    try:
        config.write_text(text)
    except PermissionError:
        staging = Path(tempfile.mkdtemp(prefix="civvis-tagclear-"))
        try:
            staged = staging / config.name
            staged.write_text(text)
            _finder_put_file(staged, config)
        finally:
            shutil.rmtree(staging, ignore_errors=True)
    return True


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


# ------------------------------------------------------- the seal we break
#
# Installing this mod invalidates the game bundle's sealed resource manifest,
# and until now nothing in the harness said so. Measured on macOS 26.5.1 with
# Civ 6 1.0.12.54 (issue #1342): after `install()`, `codesign -v` reports "a
# sealed resource is missing or invalid" and names one `file added:` line per
# file written; after `uninstall()` it reports "valid on disk" and "satisfies
# its Designated Requirement" again.
#
# ⚠ THE BROKEN SEAL IS NOT, BY ITSELF, WHAT REFUSES THE GAME, and conflating
# the two sends the operator to the wrong repair. A host with a fresh trust
# record launches and plays with the seal broken; a host with a poisoned trust
# record is refused by Gatekeeper even with the signature VALID and the mod
# removed (measured both ways, `docs/CIV6_COMPUTER_CONTROL.md`). So this
# reports the seal as a fact with an author attached, rather than as a health
# bit -- "we did this, and `--uninstall` undoes it" is a different situation
# from "something else did this", and only the second needs a human.
#
# ⚠⚠ AND RELOCATING THE MOD IS NOT THE FIX, however obvious it looks. Installing
# into the user `Mods` directory instead would touch no signed bundle and would
# also never run: no user `Mods` directory is scanned on this build, so the mod
# is never discovered and NOTHING LOGS WHY -- the run reports "CIVVIS decided
# nothing", four layers from the cause.
#
# The game's own scan index says so directly, which is worth keeping here
# because prose has not stopped anyone proposing the move. `Mods.sqlite` in the
# live user directory holds 74 `ScannedFiles` rows and every one of them is
# relative to the install tree; `select Path from ScannedFiles where Path not
# like '../../../%'` returns nothing at all, while `../../../DLC/CivvisControl`
# is present and indexed. A third-party mod sitting in
# `~/Library/Application Support/Sid Meier's Civilization VI/Mods` since July
# appears nowhere in it -- and note that is the LEGACY user directory anyway;
# `civ6_env.user_dir()` resolves the live one to the nested path, which has no
# `Mods` directory at all.
#
# Re-signing is equally unavailable: `_CodeSignature/` is not writable without
# App Management permission, and `codesign --force --deep --sign -` fails with
# an internal error on `Civ6_Exe_Child`. All of this is in
# `docs/CIV6_COMPUTER_CONTROL.md` and issue #1342; do not spend another cycle
# rediscovering it.


def bundle_dir() -> Path | None:
    """The signed application bundle the mod is installed inside, if any.

    Walked up from ``install_dir()`` rather than rebuilt from ``civ6_env``, so
    the bundle this reports and the bundle ``install()`` actually breaks cannot
    drift apart -- the failure mode where two checks quietly disagree about a
    path is the one this project has already paid for twice. ``None`` means the
    mod is not inside a bundle at all, which is also the answer to "does
    installing still break a signature?".
    """
    for parent in install_dir().parents:
        if parent.suffix == ".app":
            return parent
    return None


def seal_breakers(output: str, mod_dir: Path) -> tuple[list[str], list[str]]:
    """Split ``codesign``'s complaint into this mod's files and everything else.

    ``codesign -v --verbose=2`` names each offending path on its own line::

        …/Civ6.app: a sealed resource is missing or invalid
        file added: …/Civ6.app/Contents/Assets/DLC/CivvisControl/config.json
        file modified: …/Civ6.app/Contents/Resources/something-else

    A line names a file when the text after its first ``": "`` is an absolute
    path; the leading summary line is the other way round (a path, then a
    sentence), which is what distinguishes them without matching on the exact
    verdict words -- ``added``, ``modified`` and ``missing`` all appear, and a
    future macOS is free to add another.

    Returns ``(ours, foreign)``. ``ours`` is reversible by ``--uninstall``;
    anything in ``foreign`` is not, and is the only half worth waking a human
    over.
    """
    ours: list[str] = []
    foreign: list[str] = []
    root = str(mod_dir)
    for line in output.splitlines():
        _, sep, rest = line.partition(": ")
        if not sep or not rest.startswith("/"):
            continue
        (ours if rest == root or rest.startswith(root + "/") else foreign).append(rest)
    return ours, foreign


def signature_report() -> dict:
    """Whether the bundle is still sealed, and whose files broke it.

    Keys: ``bundle`` (path or None), ``state`` (``valid``/``broken``/
    ``unknown``/``no-bundle``), ``ours``, ``foreign``, ``detail``.
    """
    bundle = bundle_dir()
    if bundle is None:
        return {"bundle": None, "state": "no-bundle", "ours": [], "foreign": [],
                "detail": "the mod does not install inside an application bundle"}
    try:
        done = subprocess.run(
            ["codesign", "-v", "--verbose=2", str(bundle)],
            capture_output=True, text=True, timeout=120,
        )
    except (OSError, subprocess.SubprocessError) as error:
        return {"bundle": str(bundle), "state": "unknown", "ours": [], "foreign": [],
                "detail": f"could not run codesign: {error}"}
    # codesign writes its verdict AND its file list to stderr.
    output = (done.stderr or "") + (done.stdout or "")
    if done.returncode == 0:
        return {"bundle": str(bundle), "state": "valid", "ours": [], "foreign": [],
                "detail": "valid on disk"}
    ours, foreign = seal_breakers(output, install_dir())
    detail = next(
        (ln.split(": ", 1)[1] for ln in output.splitlines()
         if ": " in ln and not ln.split(": ", 1)[1].startswith("/")),
        f"codesign exited {done.returncode}",
    )
    return {"bundle": str(bundle), "state": "broken", "ours": ours,
            "foreign": foreign, "detail": detail}


if __name__ == "__main__":
    import argparse

    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--uninstall", action="store_true")
    ap.add_argument("--status", action="store_true")
    ap.add_argument("--config", help="JSON file of settings to bake in")
    args = ap.parse_args()

    def print_signature() -> None:
        """Say what the install or teardown just did to the bundle's seal."""
        seal = signature_report()
        print(f"signature   : {seal['state']} — {seal['detail']}")
        if seal["ours"]:
            print(f"              {len(seal['ours'])} file(s) added by this mod; "
                  f"--uninstall restores the seal")
        for path in seal["foreign"]:
            # Not ours, so not ours to fix — and teardown will not touch it.
            print(f"              NOT this mod: {path}")

    if args.uninstall:
        print("removed" if uninstall() else "not installed")
        print_signature()
    elif args.config:
        target = install(json.loads(Path(args.config).read_text()))
        print(f"installed -> {target}")
        print_signature()
    if args.status:
        target = install_dir()
        print(f"install dir : {target}  ({'present' if target.is_dir() else 'absent'})")
        print_signature()
        cfg = installed_config()
        if cfg:
            for key in sorted(cfg):
                print(f"  {key:<20} {cfg[key]!r}")
