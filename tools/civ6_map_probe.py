#!/usr/bin/env python3
"""Photograph the OPEN map-type dropdown so its order can be READ, not guessed.

# Why this matters more than it sounds

**49% of runs past turn 60 never meet another civilization** — 21 of 43 measured. On
those runs `findWarTarget` has nothing to find, so there is no war, no captured capital,
and no domination victory; and score victory is out of reach at roughly 5:1 behind. Half
of all runs are therefore unwinnable from the map draw alone, however good the agent gets.

The cause is known: `MapScript` in the baked config is ignored because the FrontEnd/setup
context never loads on this install (`setup: "(absent)"` in every `seat` event), so every
game is **Continents** — a script that can strand a seat on its own landmass. Pangaea
would guarantee a shared continent and therefore contact.

The Create Game dropdown is the only route that works, and `civ6_play.OPTIONS["map_type"]`
is currently a **hypothesis**: the shipped `Maps` table sorted by SortIndex, assuming
fixed-size maps are filtered out at Tiny. ⚠ An index guess against that hypothesis broke
setup four consecutive times earlier and was reverted, so it is not to be retried blind.

# Why a screenshot rather than OCR

`vision.py` reads row POSITIONS by brightness, not text, which is why the code comment
says the name "cannot be matched on screen". That is true of the harness — but a
screenshot can simply be looked at, and one photograph settles the real order for good.
This writes the picture and changes nothing.

    python3 tools/civ6_map_probe.py                  # navigate, open, photograph
    python3 tools/civ6_map_probe.py --which map_size # sanity-check on a KNOWN list first
"""
from __future__ import annotations

import argparse
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent / "civ6_control"))
import civ6_env as env  # noqa: E402
from civ6_control import gamelock, launcher, vision  # noqa: E402
import civ6_play as play  # noqa: E402

OUT = Path.home() / "civvis-civ6-runs" / "map-probe"


def quit_game() -> None:
    """⚠ Quit Civ 6 first: killing a harness leaves the game holding the run lock."""
    subprocess.run(["osascript", "-e", 'tell application "Civ6" to quit'],
                   capture_output=True)
    for _ in range(6):
        if not env.game_pids():
            break
        time.sleep(2)
    subprocess.run(["pkill", "-f", "Civ6_Exe"], capture_output=True)
    time.sleep(2)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--which", default="map_type",
                    choices=sorted(play.DROPDOWN),
                    help="which dropdown to open (map_size is a known-good control)")
    ap.add_argument("--scroll", type=int, default=6,
                    help="how many scroll steps to photograph through "
                         "the map grid (Pangaea is below the fold)")
    ap.add_argument("--keep-running", action="store_true",
                    help="leave the game up afterwards instead of quitting it")
    args = ap.parse_args()

    OUT.mkdir(parents=True, exist_ok=True)
    if not vision.available():
        print("vision unavailable: cannot locate the Create Game row", file=sys.stderr)
        return 2

    quit_game()
    # ⚠ `gamelock.break_stale()` does not exist — that is a CLI flag. The library
    # call is `release(force=True)`, and skipping it means every later attempt dies
    # with "another run holds the game".
    gamelock.release(force=True)
    print("launching Civ 6…")
    launcher.launch()
    # The mod scan lands in Modding.log minutes before the menu can be clicked, so
    # use the launcher's own wait rather than guessing a settle time.
    if not launcher.wait_for_main_menu():
        print("main menu never appeared", file=sys.stderr)
        return 2

    bounds = None
    for attempt in range(1, play.BOOTSTRAP_ATTEMPTS + 1):
        play.focus_game("right", 0.5)
        time.sleep(2.0)
        bounds = play.game_window()
        if bounds is None:
            print(f"attempt {attempt}: no window yet", file=sys.stderr)
            time.sleep(20.0)
            continue
        x, y, w, h = bounds
        # First click activates the window and is consumed; spend it on empty art.
        play.click_at(int(x + w * 0.15), int(y + h * 0.85))
        time.sleep(1.5)
        play.click_menu("single_player", bounds)
        time.sleep(2.5)
        shot = OUT / f"submenu-{attempt}.png"
        play.screenshot(shot)
        rows = vision.submenu_rows(shot, bounds)
        if len(rows) < 3:
            print(f"attempt {attempt}: submenu not up ({len(rows)} rows)",
                  file=sys.stderr)
            if not env.game_pids():
                print("game exited during bootstrap", file=sys.stderr)
                return 2
            time.sleep(15.0)
            continue
        # Create Game is the third row from the END: what varies is the START of
        # the list (Resume/Load appear only when a save exists).
        row = vision.create_game_row(shot, bounds)
        if row is None:
            print(f"attempt {attempt}: could not locate Create Game", file=sys.stderr)
            time.sleep(10.0)
            continue
        play.click_at(int(x + w * play.SUBMENU_X), int(y + h * row))
        time.sleep(4.0)
        break
    else:
        print("never reached Create Game", file=sys.stderr)
        return 2

    x, y, w, h = bounds
    closed = OUT / f"create-{args.which}-closed.png"
    play.screenshot(closed)
    print(f"wrote {closed}")

    # Open the dropdown and photograph the list. Nothing is selected: the probe
    # only looks, so a wrong assumption here cannot mis-start a game.
    box = play.DROPDOWN[args.which]
    play.click_at(int(x + w * play.SETUP_X), int(y + h * box))
    time.sleep(1.5)
    opened = OUT / f"create-{args.which}-open.png"
    play.screenshot(opened)
    print(f"wrote {opened}")

    # ⚠⚠ THE MAP CONTROL IS NOT A DROPDOWN. The photograph shows a full-screen
    # "SELECT MAP" picker: a two-column ALPHABETICAL grid of map tiles with a
    # scrollbar, filter tabs (Official / World Builder / All Maps) and a
    # "Select Map" confirm button at the bottom. Visible on the first screen:
    # 4-Leaf Clover, 6-Armed Snowflake, Archipelago, Continents (selected),
    # Continents and Islands, Earth, Earth Huge, East Asia, Europe, Fractal.
    #
    # So `OPTIONS["map_type"]` was wrong in KIND, not merely in order, and
    # `set_dropdown`'s model — click the box, then click row N beneath it — cannot
    # work here: it lands on arbitrary tiles and never presses Select Map. That is
    # exactly how an index guess broke setup four consecutive times.
    #
    # Pangaea is alphabetically below the fold, so scroll and photograph until it
    # appears rather than computing a position for it.
    for step in range(args.scroll + 1):
        shot = OUT / f"picker-scroll{step}.png"
        play.screenshot(shot)
        print(f"wrote {shot}")
        if step == args.scroll:
            break
        # Scroll inside the grid: point at the middle of the list, not the edge.
        subprocess.run([
            "osascript", "-e",
            f'tell application "System Events" to scroll {{0, -5}} at '
            f'{{{int(x + w * 0.45)}, {int(y + h * 0.55)}}}'
        ], capture_output=True)
        time.sleep(1.0)

    # Close it again with Escape so nothing is left half-selected.
    play.press_escape(1)
    if not args.keep_running:
        quit_game()
        print("\ngame quit; nothing was selected and no game was started")
    return 0


if __name__ == "__main__":
    sys.exit(main())
