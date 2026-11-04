"""How many turns a Civilization VI game lasts at each game speed.

Civilization VI has no turn-count column. A game's length is the sum of
``GameSpeed_Turns.TurnsPerIncrement`` for its speed
(``Base/Assets/Gameplay/Data/GameSpeeds.xml``). No expansion or ruleset
overrides that table; only the scenario DLCs do. Summed on this host's install
on 2026-11-03:

    Online 250, Quick 330, Standard 500, Epic 750, Marathon 1500

CIVVIS's engine carries the same numbers as ``GameSpeed::turn_limit``
(``src/setup.rs``). ``test_game_speed`` pins that table and this one to each
other, and to the shipped XML wherever the game is installed.

★ WHY THIS TABLE EXISTS. Every piece of the live lane used to write the turn
limit as a bare 250, which is Online's limit and nobody else's:
``civ6_civvis_climb --max-turns`` defaulted to it, ``follow.py`` started the
visible page server with ``--speed online --turns 250``, and ``civvis play
--mirror`` rebuilt every board with it. The lane played Online for months, so
all three were right by coincidence. When the operator moved the lane to
Standard on 2026-11-03, the page printed ``TURN n/250`` on a 500-turn game, and
without ``~/.civvis-turn-cap`` the climber would have scored a Standard game
at turn 250, halfway through.
"""

from __future__ import annotations

#: Turns in a game at each speed: Civilization VI's own ``GameSpeed_Turns`` sums.
TURN_LIMITS = {
    "GAMESPEED_ONLINE": 250,
    "GAMESPEED_QUICK": 330,
    "GAMESPEED_STANDARD": 500,
    "GAMESPEED_EPIC": 750,
    "GAMESPEED_MARATHON": 1500,
}

#: The speed a run that names none was played on. The lane's historical default.
DEFAULT_SPEED = "GAMESPEED_ONLINE"


def speed_type(speed) -> str | None:
    """``GAMESPEED_STANDARD`` for any spelling of a known speed, else ``None``.

    Accepts the Civilization VI type (``GAMESPEED_STANDARD``, what the ``seat``
    event and ``--speed`` carry) and the CIVVIS id (``standard``, what ``civvis
    play --speed`` takes), in any case and with stray whitespace.
    """
    if not isinstance(speed, str):
        return None
    name = speed.strip().upper()
    if not name:
        return None
    if not name.startswith("GAMESPEED_"):
        name = "GAMESPEED_" + name
    return name if name in TURN_LIMITS else None


def turn_limit(speed) -> int | None:
    """The turns a game at ``speed`` lasts, or ``None`` for an unknown speed."""
    name = speed_type(speed)
    return TURN_LIMITS[name] if name else None


def civvis_id(speed) -> str | None:
    """``standard`` for ``GAMESPEED_STANDARD``: the id ``civvis play --speed`` takes."""
    name = speed_type(speed)
    return name[len("GAMESPEED_"):].lower() if name else None
