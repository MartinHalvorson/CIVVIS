"""Computer control for Civilization VI.

Three pieces, each answering a question the previous harness could not:

``launcher``   start and stop the game with arguments and no window clicking
``install``    put the mod where this build finds it, with a run's settings in it
``watch``      read the controller's own events back out of the game's log

``mod/`` holds the two Lua contexts: one configures and hosts a game from the
main menu, the other takes a seat and plays it. See ``tools/civ6_play.py`` for
the command that puts them together into one run.

``CIVVIS_SCREEN_FREE=1`` (``civ6_hotseat.py --manual-lobby``) engages
``screen_free`` before any submodule loads: the screen modules then import as
stand-ins that refuse every use. Unset, nothing below runs.
"""

import os as _os

if _os.environ.get("CIVVIS_SCREEN_FREE") == "1":
    from civ6_control import screen_free as _screen_free
    _screen_free.engage()
