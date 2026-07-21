"""Observation builder shared by the gym env and the GUI server."""
from . import hexgrid
from .game import growth_threshold


def observation(game, pid):
    g = game
    p = g.players[pid]
    vis = set()
    for u in g.player_units(pid):
        vis.update(hexgrid.disk(u.pos, 2))
    for c in g.player_cities(pid):
        vis.update(hexgrid.disk(c.pos, 2))
        vis.update(c.owned_tiles)
    tiles = []
    for pos in sorted(p.explored):
        t = g.map.get(pos)
        if t is None:
            continue
        oc = g.cities.get(t.owner_city) if t.owner_city is not None else None
        tiles.append({"pos": list(pos), "terrain": t.terrain, "feature": t.feature,
                      "hills": t.hills, "resource": t.resource,
                      "improvement": t.improvement, "district": t.district,
                      "owner": oc.owner if oc else None})
    units = [u.to_dict() for u in g.units.values()
             if u.owner == pid or u.pos in vis]
    cities = []
    empire = {k: 0.0 for k in ("food", "production", "gold", "science", "culture", "faith")}
    for c in g.cities.values():
        if c.pos not in p.explored:
            continue
        d = {"id": c.id, "name": c.name, "owner": c.owner, "pos": list(c.pos),
             "pop": c.pop, "hp": c.hp, "is_capital": c.is_capital}
        if c.owner == pid:
            ys = g.city_yields(c)
            for k in empire:
                empire[k] += ys[k]
            d.update({"food": round(c.food, 1), "production": round(c.production, 1),
                      "queue": c.queue, "buildings": list(c.buildings),
                      "districts": {k: list(v) for k, v in c.districts.items()},
                      "owned_tiles": [list(t) for t in c.owned_tiles],
                      "yields": {k: round(v, 2) for k, v in ys.items()},
                      "housing": g.city_housing(c),
                      "amenity_surplus": g.city_amenity_surplus(c),
                      "growth_need": growth_threshold(c.pop),
                      "queue_cost": g.item_cost(c.queue[0]) if c.queue else None,
                      "can_strike": g._city_can_strike(c)})
        cities.append(d)
    return {
        "turn": g.turn,
        "player": pid,
        "current": g.current,
        "map": {"width": g.map.width, "height": g.map.height, "tiles": tiles},
        "visible": sorted(list(v) for v in vis if v in g.map.tiles),
        "camps": sorted(list(cp) for cp in g.barb_camps if cp in p.explored),
        "units": units,
        "cities": cities,
        "me": {"gold": round(p.gold, 1), "faith": round(p.faith, 1),
               "techs": sorted(p.techs), "research": p.research,
               "research_progress": round(p.research_progress, 1),
               "civics": sorted(p.civics), "civic": p.civic,
               "civic_progress": round(p.civic_progress, 1),
               "government": p.government,
               "boosted_techs": sorted(p.boosted_techs),
               "boosted_civics": sorted(p.boosted_civics),
               "yields": {k: round(v, 1) for k, v in empire.items()}},
        "players": [{"id": o.id, "civ": o.civ, "alive": o.alive,
                     "is_minor": o.is_minor, "is_barbarian": o.is_barbarian,
                     "government": o.government,
                     "score": g.score(o.id),
                     "cities": len(g.player_cities(o.id)),
                     "at_war_with_me": g.is_at_war(pid, o.id)}
                    for o in g.players],
        "winner": g.winner,
        "victory_type": g.victory_type,
    }
