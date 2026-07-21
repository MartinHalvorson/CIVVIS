"""v0.4 systems: barbarians, eurekas, promotions, fortify, housing/amenities,
governments, city strikes."""
from civvis.game import Game


def make(seed=5, **kw):
    kw.setdefault("num_players", 2)
    kw.setdefault("width", 24)
    kw.setdefault("height", 16)
    return Game(seed=seed, **kw)


def cycle(g, rounds=1):
    for _ in range(rounds):
        for _ in range(len(g.players)):
            if g.winner is not None:
                return
            g.apply(g.current, {"type": "end_turn"})


def test_barbarians_exist_and_spawn():
    g = make()
    assert g.barb_pid is not None
    barb = g.players[g.barb_pid]
    assert barb.is_barbarian and barb.is_minor
    assert len(g.barb_camps) >= 1
    assert g.is_at_war(0, g.barb_pid)
    cycle(g, 8)
    assert len(g.player_units(g.barb_pid)) > 0
    # barbarians never get eliminated or win
    assert barb.alive


def test_camp_clear_reward():
    g = make(seed=9)
    pos = sorted(g.barb_camps)[0]
    u = g._spawn_unit("warrior", 0, pos)  # place directly on camp
    g._maybe_clear_camp(u)
    assert pos not in g.barb_camps
    assert g.players[0].gold >= 50
    assert g.players[0].counters.get("camps") == 1


def test_eureka_boost():
    g = make(barbarians=False)
    p = g.players[0]
    p.counters["kills"] = 1
    g._check_boosts(p)
    assert "archery" in p.boosted_techs
    p.techs.add("animal_husbandry")
    g.apply(0, {"type": "research", "tech": "archery"})
    assert p.research_progress >= 0.4 * g.rules.techs["archery"]["cost"]


def test_promotions_and_fortify():
    g = make(barbarians=False)
    u = next(u for u in g.player_units(0) if u.type == "warrior")
    u.xp = 20
    base = g.unit_strength(u)
    g.apply(0, {"type": "fortify", "unit": u.id})
    assert u.fortified and u.moves_left == 0
    assert g.unit_strength(u, defending=True) == base + 6
    cycle(g)  # begin_turn applies promotion
    assert u.level == 2
    assert g.unit_strength(u) == base + 5


def test_government_effects():
    g = make(barbarians=False)
    p = g.players[0]
    p.civics.update(["code_of_laws", "state_workforce", "early_empire",
                     "political_philosophy"])
    g.apply(0, {"type": "government", "government": "oligarchy"})
    assert p.government == "oligarchy"
    assert g.gov_effects(0)["combat_strength"] == 4
    u = next(u for u in g.player_units(0) if u.type == "warrior")
    assert g.unit_strength(u) == 24


def test_housing_and_amenities():
    g = make(barbarians=False)
    s = next(u for u in g.player_units(0) if u.type == "settler")
    g.apply(0, {"type": "found_city", "unit": s.id})
    city = g.player_cities(0)[0]
    assert g.city_housing(city) >= 2
    city.buildings.append("granary")
    assert g.city_housing(city) >= 4
    assert isinstance(g.city_amenity_surplus(city), int)


def test_city_strike():
    g = make(seed=12, barbarians=False)
    s = next(u for u in g.player_units(0) if u.type == "settler")
    g.apply(0, {"type": "found_city", "unit": s.id})
    city = g.player_cities(0)[0]
    city.buildings.append("walls")
    enemy = g._place_new_unit("warrior", 1, city.pos)
    assert enemy is not None  # adjacent to city center
    g.at_war.add(frozenset((0, 1)))
    hp0 = enemy.hp
    g.apply(0, {"type": "city_strike", "city": city.id,
                "target": list(enemy.pos)})
    assert enemy.hp < hp0 or enemy.id not in g.units
    assert city.struck


def test_new_content_present():
    g = make()
    for u in ("pikeman", "knight", "musketman"):
        assert u in g.rules.units
    for b in ("aqueduct", "bank", "arena", "medieval_walls"):
        assert b in g.rules.buildings
    for d in ("industrial_zone", "entertainment_complex"):
        assert d in g.rules.districts
    assert "gunpowder" in g.rules.techs
    assert "guilds" in g.rules.civics
    assert "monarchy" in g.rules.governments
