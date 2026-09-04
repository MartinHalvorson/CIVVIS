#!/usr/bin/env python3
"""Configure a Civilization VI installation for grounding runs.

The game ships every channel this project needs, all of them off by default:
its Lua console (FireTuner) listens only when ``EnableTuner`` is set, and its
per-turn history, effect-application and game-core event logs only write when
their levels are raised. This turns them on, in the user directory the game
actually reads (see ``civ6_env.user_dir`` -- there are two, and only one
counts).

The game rewrites both options files on launch and on exit, so it has to be
closed while this runs. ``--restart`` handles that: quit, configure, relaunch.

Usage::

    python tools/civ6_setup.py                # report current settings
    python tools/civ6_setup.py --apply        # turn the channels on
    python tools/civ6_setup.py --apply --restart
    python tools/civ6_setup.py --revert       # back to shipped defaults
    python tools/civ6_setup.py --verification # the cosmetic cuts a live game makes
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import civ6_env as env  # noqa: E402

# Options that open a channel, and why each one is wanted.
APP_OPTIONS = {
    # The Lua console listener. Bidirectional: reads any gameplay value and
    # issues any order, which is the difference between observing the game and
    # driving it.
    "EnableTuner": 1,
    # Every game-core event, stamped per turn. The spine of a replay trace.
    "EnableGameCoreEventLog": 1,
    # Debug menu and WorldBuilder construct the exact states a micro-scenario
    # diff needs. Both already default on in this install, kept here so the
    # configuration is complete rather than incidental.
    "EnableDebugMenu": 1,
    "EnableWorldBuilder": 1,
    # Surfaces database problems a mod introduces instead of failing silently.
    "EnableDataErrorCollection": 1,
}

USER_OPTIONS = {
    # Per-turn, per-player history: score, yields, cities, tech pace. This is
    # the series CIVVIS' own trajectories get compared against.
    "GameHistoryLogLevel": 1,
    "GameHistorySequentialLogLevel": 1,
    # Every modifier as it applies. The finest-grained rules evidence the game
    # emits without a mod, and the one that catches a yield that is right in
    # the database but wrong in the engine.
    "GameEffectsLogLevel": 2,
    "AI_MasterLogging": 1,
    "GameEraMomentsLog": 1,
}

# What a VERIFICATION game turns off or acknowledges, by file. None changes a
# rule, an order, a turn or anything the ledger reads. The cosmetic cuts save
# wall clock; the two acknowledgements are exactly the choices Civ VI writes
# after its own one-time front-end dialogs are accepted. Those dialogs sit in
# front of the main menu, where a capture-free launcher cannot safely discover
# them before aiming at a fixed menu row. Measured 2026-08-24 on the ladder's
# own artefacts (run civvis-20260819T102855Z): the first bootstrap attempt of
# EVERY game fires into a black window -- the intro video -- fails "top menu
# not readable (0 rows)" and sleeps twenty seconds; the historic-moment
# animation plays over the board on every era-first; and two shadow passes
# render terrain no reader ever looks at while the game core takes five rival
# AI turns on the same cores. `civ6_play.py` applies these before every
# launch; `--verification` applies them by hand.
VERIFICATION_OPTIONS = {
    "AppOptions.txt": {
        # "Set to 1 play the intro video on startup." The game's own comment.
        "PlayIntroVideo": 0,
        # These are set by the stock unknown-device OK and outdated-driver
        # "do not remind me" actions.  They prevent known native dialogs from
        # masking the main menu during unattended capture-free starts.
        "AcceptedUnknownDevice": 1,
        "AcceptedOutdatedDriver": 1,
    },
    "UserOptions.txt": {
        "PlayHistoricMomentAnimation": 0,
    },
    "GraphicsOptions.txt": {
        "EnableShadows": 0,
        "EnableCloudShadows": 0,
    },
}

# The game's own values for the same keys, for --revert. ⚠ Keep in step with
# VERIFICATION_OPTIONS; a test holds the two key sets equal.
VERIFICATION_DEFAULTS = {
    "AppOptions.txt": {
        "PlayIntroVideo": 1,
        "AcceptedUnknownDevice": 0,
        "AcceptedOutdatedDriver": 0,
    },
    "UserOptions.txt": {"PlayHistoricMomentAnimation": 1},
    "GraphicsOptions.txt": {"EnableShadows": 1, "EnableCloudShadows": 1},
}

# Shipped defaults, for --revert.
DEFAULTS = {
    "EnableTuner": 0,
    "EnableGameCoreEventLog": 0,
    "EnableDebugMenu": 1,
    "EnableWorldBuilder": 1,
    "EnableDataErrorCollection": 0,
    "GameHistoryLogLevel": 0,
    "GameHistorySequentialLogLevel": 0,
    "GameEffectsLogLevel": 0,
    "AI_MasterLogging": 1,
    "GameEraMomentsLog": 0,
}


def report(user: Path) -> None:
    print(f"user dir : {user}")
    for name, keys in (("AppOptions.txt", APP_OPTIONS), ("UserOptions.txt", USER_OPTIONS)):
        path = user / name
        print(f"\n{name}  ({'present' if path.is_file() else 'MISSING'})")
        for key, want in keys.items():
            have = env.read_option(path, key)
            flag = "ok " if have == str(want) else "-> "
            print(f"  {flag}{key:<32} {have!r:<8} want {want!r}")
    print("\nverification (startup cuts and front-end acknowledgements)")
    for name, keys in VERIFICATION_OPTIONS.items():
        path = user / name
        for key, want in keys.items():
            have = env.read_option(path, key)
            flag = "ok " if have == str(want) else "-> "
            print(f"  {flag}{name}: {key:<28} {have!r:<8} want {want!r}")


def apply_verification(user: Path, wanted: dict[str, dict[str, object]] | None = None
                       ) -> dict[str, dict[str, tuple]]:
    """Rewrite the verification options in place; {file: {key: (old, new)}}.

    Only what actually changed is returned, so a second call on a configured
    install returns nothing and a caller can print exactly what moved. A file
    the game has not written yet is skipped, not created: these are the game's
    own files and `env.set_options` only rewrites keys already present, so a
    key this version does not define is reported with an ``old`` of None and
    left alone (see that docstring). ⚠ The game rewrites every options file on
    launch and on exit; call this while it is closed or the change is lost.
    """
    applied: dict[str, dict[str, tuple]] = {}
    for name, keys in (wanted or VERIFICATION_OPTIONS).items():
        path = user / name
        if not path.is_file():
            continue
        changes = env.set_options(path, keys)
        if changes:
            applied[name] = changes
    return applied


def apply(user: Path, values: dict) -> None:
    for name, keys in (("AppOptions.txt", APP_OPTIONS), ("UserOptions.txt", USER_OPTIONS)):
        path = user / name
        changes = {k: values[k] for k in keys if k in values}
        applied = env.set_options(path, changes)
        for key, (old, new) in applied.items():
            if old is None:
                print(f"  !! {name}: {key} not defined by this version, skipped")
            else:
                print(f"  {name}: {key} {old} -> {new}")
        if not applied:
            print(f"  {name}: already as wanted")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--apply", action="store_true", help="turn the channels on")
    ap.add_argument("--revert", action="store_true", help="restore shipped defaults")
    ap.add_argument("--verification", action="store_true",
                    help="apply verification startup cuts and front-end acknowledgements")
    ap.add_argument("--restart", action="store_true", help="quit and relaunch around the change")
    args = ap.parse_args(argv)

    user = env.user_dir()
    if not (args.apply or args.revert or args.verification):
        report(user)
        return 0

    if env.game_pids():
        if not args.restart:
            print(
                "Civilization VI is running; it rewrites its options on exit and would\n"
                "discard this change. Re-run with --restart, or quit the game first.",
                file=sys.stderr,
            )
            return 2
        print("quitting the game so the options survive...")
        if not env.quit_game():
            print("could not stop the game", file=sys.stderr)
            return 2

    if args.apply or args.revert:
        wanted = DEFAULTS if args.revert else {**APP_OPTIONS, **USER_OPTIONS}
        apply(user, wanted)
    if args.verification or args.revert:
        verification = VERIFICATION_DEFAULTS if args.revert else VERIFICATION_OPTIONS
        for name, changes in apply_verification(user, verification).items():
            for key, (old, new) in changes.items():
                if old is None:
                    print(f"  !! {name}: {key} not defined by this version, skipped")
                else:
                    print(f"  {name}: {key} {old} -> {new}")

    # The game only scans a Mods directory that exists.
    mods = env.mods_dir()
    if not mods.is_dir():
        mods.mkdir(parents=True, exist_ok=True)
        print(f"  created {mods}")

    print()
    report(user)

    if args.restart:
        print("\nrelaunching...")
        env.launch_game()
    return 0


if __name__ == "__main__":
    sys.exit(main())
