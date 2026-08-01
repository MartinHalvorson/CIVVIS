#!/usr/bin/env python3
"""Turn a dry run's timeline into the recording's shot list.

The camera has to be somewhere *before* a launch, not after it: a launch flight
lasts under four seconds and at Lightning pace that is a dozen turns. The dry
run says which turn each project lands on, and the server is deterministic per
seed, so every space beat is set a few turns early and the pace is slowed for
the length of it.

Usage: makeplan.py <timeline.json> <plan.json>
"""
import json
import sys

# How early to arrive, and how long to stay, for each rung of the space
# programme. The expedition is the long one: the ship then crawls for the rest
# of the game, which is the shot the whole race is watched from.
LEAD = 5


def main():
    timeline = json.load(open(sys.argv[1]))
    winner = timeline["victory"]["winner"]
    end = timeline["victory"]["turn"]
    events = timeline["events"]

    def turn_of(project, civ=winner):
        for event in events:
            if event["civ"] == civ and event["project"] == project:
                return event["turn"]
        return None

    satellite = turn_of("launch_earth_satellite")
    moon = turn_of("launch_moon_landing")
    mars = turn_of("launch_mars_colony")
    expedition = turn_of("exoplanet_expedition")
    print(f"winner {winner}: satellite {satellite} moon {moon} mars {mars} "
          f"expedition {expedition} victory {end}")

    beats = [
        # The establishing shot: a whole globe, turn 1, nothing built yet.
        {"at": 1, "label": "opening", "steps": ["hold:2500"]},
        # "Slowly spin the planet a bit early in the game" — most of a turn of
        # the world, at a steady rate, while it is still empty enough to read as
        # a planet rather than as a scoreboard.
        {"at": 2, "label": "spin the globe", "steps": ["spin:200:14000", "hold:1200"]},
        # Down onto the ground, where Grand Canals II is actually legible: a
        # shelf ring round every block and a deep channel between them.
        {"at": 26, "label": "into the canals",
         "steps": ["zoom:5:3000", "hold:3200", "zoom:-5:2600", "hold:1000"]},
        # Mid-game, with borders drawn and cities on: the same ground, now owned.
        {"at": 120, "label": "a settled world",
         "steps": ["zoom:4:2600", "hold:2800", "zoom:-4:2400", "hold:800"]},
    ]

    # Two rules the sky shots are built around, both found by driving it:
    #
    # * The sky bar is only up while home is *on* the stage with room around it,
    #   so a flight has to be set up by pulling back off the board first
    #   (`earth:90`), and every sky visit ends by flying home again.
    # * Once anybody has a satellite, any view with the world under SKY_SURFACE
    #   animates continuously — 30 fps for as long as the camera stays there.
    #   That is what makes these shots worth having and also why the camera
    #   cannot be parked in one: forty turns of voyage at ten seconds a turn
    #   would be six minutes of real-time footage in a three-minute clip. Every
    #   visit is a visit, and the waiting is done back on the board.
    if satellite:
        beats.append({"at": max(2, satellite - LEAD), "label": "first satellite",
                      "pause": False, "pace": 2000,
                      "steps": ["earth:90:3500", "hold:14000", "fly:world:earth"]})
    if moon:
        beats.append({"at": max(2, moon - 4), "label": "the Moon",
                      "pause": False, "pace": 2000,
                      "steps": ["earth:90:3000", "fly:world:moon", "hold:9000"]})
    if mars:
        beats.append({"at": max(2, mars - 4), "label": "Mars",
                      "pause": False, "pace": 2000,
                      "steps": ["fly:world:mars", "hold:9000", "fly:world:earth"]})
    if expedition:
        # The voyage: the Sun at one end, the destination at the other, and the
        # ships on the road between them. This is the view the race is watched
        # from, and it is visited twice — once as the expedition leaves, once
        # with the ships well down the road — because the trip is forty turns
        # long and a single held shot of it would be most of the clip.
        beats.append({"at": max(2, expedition - LEAD), "label": "the voyage begins",
                      "pause": False, "pace": 2500,
                      "steps": ["earth:90:3000", "fly:scale:voyage", "hold:16000",
                                "fly:world:earth"],
                      "thenPace": 900})
        beats.append({"at": max(2, end - 14), "label": "the last light-years",
                      "pause": False, "pace": 1400,
                      "steps": ["earth:90:3000", "fly:scale:voyage", "hold:9000",
                                "fly:world:exo", "hold:12000"]})
    beats.append({"onVictory": True, "at": end, "label": "the finish",
                  "pause": False, "steps": ["hold:9000"]})

    plan = {"seed": timeline["seed"], "players": 6, "maxTurns": 500,
            "maxMinutes": 75, "pace": 0, "beats": beats}
    with open(sys.argv[2], "w") as handle:
        json.dump(plan, handle, indent=2)
    print(json.dumps([{k: v for k, v in beat.items() if k != "steps"} for beat in beats],
                     indent=None))


if __name__ == "__main__":
    main()
