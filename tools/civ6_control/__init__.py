"""Computer control for Civilization VI.

Three pieces, each answering a question the previous harness could not:

``launcher``   start and stop the game with arguments and no window clicking
``install``    put the mod where this build finds it, with a run's settings in it
``watch``      read the controller's own events back out of the game's log

``mod/`` holds the two Lua contexts: one configures and hosts a game from the
main menu, the other takes a seat and plays it. See ``tools/civ6_play.py`` for
the command that puts them together into one run.
"""
