"""Present a confirmed host victory even when its last state predates the win.

A completion inferred from the result is a lower bound, never an observed
final counter. Keep the original frame in the journal and label that distinction.
"""
import copy

LANES = {"VICTORY_TECHNOLOGY": "science", "VICTORY_SCIENCE": "science", "VICTORY_CULTURE": "culture",
         "VICTORY_DIPLOMATIC": "diplomatic", "VICTORY_CONQUEST": "domination",
         "VICTORY_RELIGIOUS": "religious", "VICTORY_SCORE": "score"}


def local_victory(event):
    return (event.get("kind") == "victory" and event.get("victory_type") in LANES
            and event.get("won") is True
            and isinstance(event.get("turn"), int)
            and event.get("local_player", -1) >= 0)


def completed_frame(frame, outcome):
    """Copy a stale frame, advancing only the confirmed winning race."""
    result = copy.deepcopy(frame)
    if not local_victory(outcome) or outcome["turn"] < frame.get("turn", 0):
        return result
    lane = LANES[outcome["victory_type"]]
    result["turn"] = outcome["turn"]
    result["confirmed_victory"] = lane
    result["victory_progress_source"] = "confirmed_result_lower_bound"
    if lane == "diplomatic":
        result["dvp"] = max(result.get("dvp") or 0, 20)
    elif lane == "science":
        need = result.get("science_victory_points_needed") or 0
        if need > 0:
            result["science_victory_points"] = max(result.get("science_victory_points") or 0, need)
    elif lane == "culture":
        rivals = [r for r in result.get("rivals", [])
                  if r.get("civ") and "CITY_STATE" not in r["civ"]]
        if rivals:
            need = max(r.get("domestic_tourists") or 0 for r in rivals) + 1
            result["foreign_tourists"] = max(result.get("foreign_tourists") or 0, need)
    return result


def apply_game_result(game, events):
    """The mirror remaps the local host seat to CIVVIS player zero.

    Result flags finish every lane, without inventing cities, conversions or
    rival ownership. Numeric race counters use only the winner's lower bound.
    """
    outcome = next((e for e in reversed(events) if e.get("kind") == "victory"), {})
    if not local_victory(outcome) or outcome["turn"] < game.get("turn", 0):
        return game
    lane = LANES[outcome["victory_type"]]
    game["turn"] = outcome["turn"]
    game["winner"], game["victory_type"] = 0, lane
    player = game["players"][0]
    if lane == "diplomatic":
        player["dvp"] = max(player.get("dvp") or 0, 20)
    stats = game.get("observed_public_empire_stats", {})
    ours = stats.get("0", {})
    if lane == "science":
        need = ours.get("science_victory_points_needed") or 0
        if need > 0:
            ours["science_victory_points"] = max(ours.get("science_victory_points") or 0, need)
    elif lane == "culture":
        need = max([s.get("domestic_tourists") or 0 for p, s in stats.items() if p != "0"] or [0]) + 1
        if ours:
            ours["foreign_tourists"] = max(ours.get("foreign_tourists") or 0, need)
    return game
