//! JSON observation builder (fog-of-war view for a player) — feeds the GUI
//! and any external agent speaking the JSON protocol.
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::game::{
    City, Game, Item, RememberedCity, DIPLOMATIC_VICTORY_POINTS, EXOPLANET_DESTINATION,
};
use crate::world::Tile;
use crate::Pos;

pub fn observation(g: &Game, pid: usize) -> Value {
    obs_impl(g, pid, false, true)
}

/// The technology that teaches a people to read their bearing off the sky.
///
/// Sailing put them out of sight of the coast and astrology gave them the
/// figures to steer by; together they are the point at which a civilization can
/// say which way is north rather than only which way it came from. Until then a
/// map faces the direction it was first seen from, which the viewer keeps in
/// `found_north` below — a presentation rule, so it is reported rather than
/// enforced, but it is reported from here so the client is not left deciding
/// which discovery it was.
pub const NORTH_TECH: &str = "celestial_navigation";

/// The discoveries at which a people can say their world is a ball.
///
/// Two civilizations reach it two different ways and both are here: the sailors
/// who came home from the west having set out to the east, and the scholars who
/// read the round shadow the Earth throws on the Moon. Either is proof, so
/// either opens it — a naval empire and a scholarly one both get there, by
/// their own road. Hypatia edited the tables the second argument is made from,
/// and brings it forward for whoever recruits her.
///
/// Until one of them lands, a Planet world is drawn as a surface with no edge a
/// viewer can find: the camera stops while the horizon is still off-frame, so
/// there is no silhouette, no space around it, and nothing in the sky. Like
/// `NORTH_TECH` this is a presentation rule — nothing in the simulation turns
/// on it — but it is decided here rather than in the viewer so the client is
/// not left inventing which discovery it was.
pub const GLOBE_TECHS: [&str; 2] = ["cartography", "astronomy"];
pub const GLOBE_GREAT_PERSON: &str = "hypatia";

/// The project that puts an eye above the air.
///
/// The Moon and Mars are naked-eye objects: a people who know their own world
/// is a ball can place them, and they stand in the sky from that moment. A
/// planet around another star is not visible from the ground at all — it takes
/// an instrument above the atmosphere to know there is one — so the far end of
/// the system opens only once a civilization has actually put something up
/// there.
pub const EXOPLANET_EYE: &str = "launch_earth_satellite";

/// Whether this civilization has the round world, by discovery, by recruit, or
/// by having sailed round it: a people who set out west and came home from the
/// east have proved the thing on the water, and do not also need to read it in
/// a book.
pub fn knows_globe(p: &crate::game::Player) -> bool {
    p.went_around
        || GLOBE_TECHS.iter().any(|tech| p.techs.contains(*tech))
        || p.great_people.iter().any(|id| id.as_str() == GLOBE_GREAT_PERSON)
}

/// The shape of a Planet world, for a client that has to draw it.
///
/// A globe cannot be drawn from tile coordinates the way a flat map can: the
/// storage rectangle is not a picture of anything. This hands over the real
/// thing — where each tile sits on the sphere and what its outline is — once,
/// keyed by the subdivision frequency it belongs to, so a client asks for it
/// when it first sees a Planet game and never again. It is left out of the
/// ordinary observation, which is already close to a megabyte and is polled
/// every turn.
///
/// Corners are shared by the three tiles that meet at them and are sent once
/// each, as thousandths, with every tile naming its own by index. The order of
/// a tile's corners is the order of its neighbours, counter-clockwise seen
/// from outside the globe.
pub fn planet_geometry(g: &Game) -> Option<Value> {
    const SCALE: f64 = 10_000.0;
    let sphere = g.map.sphere()?;
    let mut corners: Vec<i64> = Vec::new();
    let mut seen: BTreeMap<[i64; 3], usize> = BTreeMap::new();
    let mut cells: Vec<Vec<i64>> = Vec::with_capacity(sphere.len());
    for cell in sphere.cells() {
        let mut entry = vec![cell.pos.0 as i64, cell.pos.1 as i64];
        for corner in &cell.corners {
            let key = [
                (corner[0] * SCALE).round() as i64,
                (corner[1] * SCALE).round() as i64,
                (corner[2] * SCALE).round() as i64,
            ];
            let next = seen.len();
            let index = *seen.entry(key).or_insert_with(|| {
                corners.extend_from_slice(&key);
                next
            });
            entry.push(index as i64);
        }
        cells.push(entry);
    }
    Some(json!({
        "frequency": sphere.frequency(),
        "scale": SCALE,
        "corners": corners,
        "cells": cells,
    }))
}

/// Fog-free view of the whole world from `pid`'s empire perspective —
/// feeds the spectator (watch-the-AIs) GUI mode.
pub fn observation_spectator(g: &Game, pid: usize) -> Value {
    obs_impl(g, pid, true, false)
}

/// Currently visible and ever-explored tile sets for `pid`, including
/// Level-2+ military-alliance shared vision. Every fog-honest observation
/// surface (the JSON protocol and the tensor builder) derives from this one
/// contract.
pub fn visibility(g: &Game, pid: usize) -> (BTreeSet<Pos>, BTreeSet<Pos>) {
    let p = &g.players[pid];
    let vis = g.player_visibility(pid);
    let mut explored = p.explored.clone();
    for (partner, alliance) in &p.alliances {
        if alliance.ends > g.turn && alliance.kind == "military" && alliance.level >= 2 {
            explored.extend(g.players[*partner].explored.iter().copied());
        }
    }
    (vis, explored)
}

/// Read-only, fog-of-war view used when a spectator chooses a civilization's
/// perspective. It intentionally omits expensive interactive affordances such
/// as per-unit reachability because the AI remains in control of the seat.
pub fn observation_player_view(g: &Game, pid: usize) -> Value {
    obs_impl(g, pid, false, false)
}

fn obs_impl(g: &Game, pid: usize, omniscient: bool, interactive: bool) -> Value {
    let p = &g.players[pid];
    let viewers: Vec<usize> = if omniscient {
        vec![pid]
    } else {
        g.visibility_viewers(pid).into_iter().collect()
    };
    let vis: BTreeSet<Pos> = if omniscient {
        g.map.tiles.keys().copied().collect()
    } else {
        g.player_visibility(pid)
    };
    let mut explored = if omniscient {
        vis.clone()
    } else {
        p.explored.clone()
    };
    if !omniscient {
        for viewer in &viewers {
            explored.extend(g.players[*viewer].explored.iter().copied());
        }
    }
    // Where this player's cities are currently building districts. Adjacency
    // matters most before the district exists, so a site under construction
    // reports what it will be worth.
    let planned: BTreeMap<Pos, &str> = g
        .cities
        .values()
        // Build queues are private, and this is read off one: the same rule
        // `live_city_json` applies to `queue` applies here.
        .filter(|city| omniscient || city.owner == pid)
        .flat_map(|city| city.queue.iter())
        .filter_map(|item| match item {
            Item::District { district, pos } => Some((*pos, district.as_str())),
            _ => None,
        })
        .collect();
    let tiles: Vec<Value> = explored
        .iter()
        .filter_map(|pos| {
            let live = omniscient || vis.contains(pos);
            let (tile, owner) = if live {
                let tile = g.map.get(*pos)?;
                let owner = tile
                    .owner_city
                    .and_then(|city| g.cities.get(&city))
                    .map(|city| city.owner);
                (tile, owner)
            } else {
                // Whose memory of this hex is the freshest. The stamp sits
                // beside the remembered map rather than inside it.
                let (memory, _) = viewers
                    .iter()
                    .filter_map(|viewer| {
                        let seat = &g.players[*viewer];
                        seat.remembered_tiles
                            .get(pos)
                            .map(|memory| (memory, seat.remembered_tiles.seen_turn(pos)))
                    })
                    .max_by_key(|(_, seen)| *seen)?;
                (&memory.tile, memory.owner)
            };
            Some(tile_json(
                g,
                pid,
                tile,
                owner,
                omniscient,
                live,
                planned.get(pos).copied().filter(|_| live),
            ))
        })
        .collect();
    let units: Vec<Value> = g
        .units
        .values()
        .filter(|u| {
            let observed_pos = u.air_patrol_pos.unwrap_or(u.pos);
            omniscient
                || u.owner == pid
                || (vis.contains(&observed_pos)
                    && viewers
                        .iter()
                        .any(|viewer| g.unit_visible_to(u.id, *viewer)))
        })
        .map(|u| {
            let mut v = serde_json::to_value(u).unwrap();
            v["embarked"] = json!(g.is_embarked(u));
            // Reachability is an interactive-player affordance. Computing it
            // for every unit of the currently observed AI can dominate late-
            // game spectator responses even though spectate mode has no legal
            // movement actions.
            if u.owner == pid && interactive {
                v["reachable"] = json!(g
                    .reachable(u.id)
                    .iter()
                    .map(|p| json!([p.0, p.1]))
                    .collect::<Vec<_>>());
                if let Some((target, gold, _)) = g.unit_gold_upgrade_offer(pid, u.id) {
                    v["upgrade"] = json!({ "to": target, "gold": gold });
                }
            }
            // Whether the unit has been left behind by the ruleset is worth
            // showing even when the upgrade itself is out of reach this turn.
            if u.owner == pid || omniscient {
                v["obsolete"] = json!(g.unit_is_obsolete(u.owner, &u.kind));
            }
            v
        })
        .collect();
    let spies: Vec<Value> = g
        .spies
        .values()
        .filter(|spy| omniscient || spy.owner == pid || spy.captured_by == Some(pid))
        .map(|spy| serde_json::to_value(spy).unwrap())
        .collect();
    let mut empire = [0.0f64; 6]; // food, prod, gold, sci, cul, faith
    for city in g.cities.values().filter(|city| city.owner == pid) {
        let yields = g.city_yields(city.id);
        empire[0] += yields.food;
        empire[1] += yields.production;
        empire[2] += yields.gold;
        empire[3] += yields.science;
        empire[4] += yields.culture;
        empire[5] += yields.faith;
    }

    enum KnownCity<'a> {
        Live(&'a City),
        Remembered(&'a RememberedCity),
    }
    let mut known_cities: BTreeMap<u32, KnownCity<'_>> = BTreeMap::new();
    if !omniscient {
        for viewer in &viewers {
            for memory in g.players[*viewer].remembered_cities.values() {
                if explored.contains(&memory.pos) && !vis.contains(&memory.pos) {
                    known_cities
                        .entry(memory.id)
                        .and_modify(|known| {
                            if matches!(known, KnownCity::Remembered(old) if memory.seen_turn > old.seen_turn)
                            {
                                *known = KnownCity::Remembered(memory);
                            }
                        })
                        .or_insert(KnownCity::Remembered(memory));
                }
            }
        }
    }
    for city in g.cities.values() {
        if omniscient || city.owner == pid || vis.contains(&city.pos) {
            known_cities.insert(city.id, KnownCity::Live(city));
        }
    }
    let cities: Vec<Value> = known_cities
        .into_values()
        .map(|known| match known {
            KnownCity::Remembered(city) => remembered_city_json(city),
            KnownCity::Live(city) => live_city_json(g, pid, city, omniscient),
        })
        .collect();
    let camps: Vec<Value> = tiles
        .iter()
        .filter(|tile| tile["improvement"] == "barbarian_camp")
        .map(|tile| tile["pos"].clone())
        .collect();
    let leading_score = g
        .players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .map(|player| g.team_score_rank_key(player.id).0)
        .max()
        .unwrap_or(0);
    json!({
        "turn": g.turn,
        "max_turns": g.max_turns,
        "seed": g.seed,
        "game_speed": g.game_speed.id(),
        "world_era": g.world_era,
        "climate_phase": g.climate_phase,
        "climate_points": g.climate_points(),
        "disaster_intensity": g.disaster_intensity(),
        // Weather is only reported where the player can actually see it, and
        // a drought is trimmed to the tiles they can see it on.
        "storms": g
            .storms
            .iter()
            .filter(|storm| omniscient || vis.contains(&storm.pos))
            .map(|storm| {
                json!({
                    "kind": storm.kind,
                    "pos": [storm.pos.0, storm.pos.1],
                    "severity": storm.severity,
                    "ends": storm.ends,
                })
            })
            .collect::<Vec<_>>(),
        "droughts": g
            .droughts
            .iter()
            .filter_map(|drought| {
                let tiles: Vec<[i32; 2]> = drought
                    .tiles
                    .iter()
                    .filter(|pos| omniscient || vis.contains(*pos))
                    .map(|pos| [pos.0, pos.1])
                    .collect();
                (!tiles.is_empty()).then(|| {
                    json!({
                        "tiles": tiles,
                        "severity": drought.severity,
                        "ends": drought.ends,
                    })
                })
            })
            .collect::<Vec<_>>(),
        "player": pid,
        "current": g.current,
        "map": {
            "size": g.map_size().id,
            "size_name": g.map_size().name,
            "script": g.map_script.id(),
            // The world's shape, which the browser needs in order to know
            // whether it is drawing a rectangle or a globe. It is read off the
            // map rather than off the setup, so a loaded save answers too.
            "shape": if g.map.sphere().is_some() { "planet" } else { "flat" },
            "poles": g.map_poles.id(),
            "width": g.map.width,
            "height": g.map.height,
            "default_players": g.map_size().default_players,
            "max_players": g.map_size().max_players,
            "default_city_states": g.map_size().default_city_states,
            "max_city_states": g.map_size().max_city_states,
            "max_religions": g.max_religions(),
            "natural_wonders": g.map_size().natural_wonders,
            "continents": g.map_size().continents,
            "tiles": tiles,
        },
        "visible": vis.iter().filter(|v| g.map.tiles.contains_key(v))
            .map(|v| json!([v.0, v.1])).collect::<Vec<_>>(),
        "camps": camps,
        "units": units,
        "spies": spies,
        "cities": cities,
        "me": {
            "team": p.team,
            "gold": round1(p.gold), "faith": round1(p.faith),
            "gold_per_turn": round1(p.gold_per_turn),
            "bankruptcy_amenity_penalty": p.bankruptcy_amenity_penalty,
            "techs": p.techs, "research": p.research,
            "research_progress": round1(p.research_progress),
            // A spectator watches from above the world rather than inside it,
            // so it is never the party that has to find north.
            "found_north": omniscient || p.techs.contains(NORTH_TECH),
            "north_tech": NORTH_TECH,
            // Whether this people's knowledge has ever run the whole way round
            // the world, which is how a civilization finds out that it does
            // come back on itself and how far round it is. Until then their
            // chart is an open sheet that stops where they have stopped, and
            // the world does not helpfully repeat past the edge of it. A
            // spectator is not living in the world and is told at once.
            "went_around": omniscient || p.went_around,
            // The same arrangement one step further out: a spectator is above
            // the world rather than on it, so it was never in any doubt about
            // the shape of the thing or about what else is out there.
            "knows_globe": omniscient || knows_globe(p),
            "globe_techs": GLOBE_TECHS,
            "globe_great_person": GLOBE_GREAT_PERSON,
            "sees_exoplanet": omniscient || p.science_projects.contains(EXOPLANET_EYE),
            "exoplanet_eye": EXOPLANET_EYE,
            "civics": p.civics, "civic": p.civic,
            "civic_progress": round1(p.civic_progress),
            "government": p.government,
            "anarchy_turns": p.anarchy_turns,
            "pending_government": p.pending_government,
            "past_governments": p.past_governments,
            "influence": round1(p.influence),
            "envoys_free": p.envoys_free,
            "envoys": p.envoys,
            "diplomatic_favor": round1(p.diplomatic_favor),
            "power_fuel_consumed": p.power_fuel_consumed,
            "co2_emissions": round1(p.co2_emissions),
            "global_co2": round1(g.global_co2_emissions()),
            "trade_capacity": g.trade_capacity(pid),
            "gpp": p.gpp,
            "gp_claimed": p.gp_claimed,
            // Which named person each kind is currently offering, and what it
            // costs. This is a world fact — it depends on who every other
            // civilization has retired — so a client cannot derive it, and
            // without it a Great People screen can only say "a Great
            // Merchant" where Civ 6 says "Marco Polo, 60 Faith".
            "great_person_offers": p.gpp.keys()
                .filter_map(|kind| {
                    let (id, spec) = g.current_great_person(kind)?;
                    Some((kind.clone(), json!({
                        "id": id,
                        "name": spec.name,
                        "era": spec.era,
                        "points": round1(g.gp_cost(pid, kind)),
                        "gold": g.great_person_patronage_price(pid, kind, "gold").map(round1),
                        "faith": g.great_person_patronage_price(pid, kind, "faith").map(round1),
                        "effects": spec.effects.keys().collect::<Vec<_>>(),
                        // Enough points is not always enough: a Great
                        // Scientist wants a Campus, a Great Writer wants an
                        // open Great Work slot. Say which, or the card reads
                        // as broken.
                        "blocked": g.great_person_blocker(pid, kind),
                    })))
                })
                .collect::<serde_json::Map<_, _>>(),
            "great_people": p.great_people,
            "era_score": p.era_score,
            "normal_age_threshold": p.normal_age_threshold,
            "golden_age_threshold": p.golden_age_threshold,
            "dedications": p.dedications,
            "dedication_choices": p.dedication_choices,
            "available_dedications": g.available_dedications(pid),
            "governors": p.governors,
            "governor_roster": p.governor_roster,
            "governor_titles": g.governor_titles(pid),
            "governor_titles_available": g.governor_titles_available(pid),
            "dvp": p.dvp,
            "grievances": p.grievances,
            "denounced_until": p.denounced_until,
            "friends_until": p.friends_until,
            "open_borders_until": p.open_borders_until,
            "alliances": p.alliances,
            "age": p.age,
            "tourism": round1(p.tourism_lifetime),
            "religious_tourism": round1(p.religious_tourism_lifetime),
            "tourism_pressure": g.players.iter()
                .filter(|target| target.id != pid && !target.is_minor && !target.is_barbarian)
                .map(|target| (target.id.to_string(), round1(g.tourism_pressure_against(pid, target.id))))
                .collect::<BTreeMap<_, _>>(),
            "monopoly_gold_per_turn": round1(g.monopoly_bonuses(pid).0),
            "monopoly_tourism_pct": round1(g.monopoly_bonuses(pid).1),
            "secret_society": p.secret_society,
            "domestic_tourists": g.domestic_tourists(pid),
            "foreign_tourists": g.foreign_tourists(pid),
            "science_projects": p.science_projects,
            "exoplanet_distance": round1(p.exoplanet_distance),
            "exoplanet_speed": round1(g.exoplanet_speed(pid)),
            "pantheon": p.pantheon,
            "religion": p.religion,
            "religion_beliefs": p.religion_beliefs,
            "prophet_pending": p.prophet_pending,
            "routes": g.routes.iter().filter(|r| r.owner == pid)
                .map(|r| json!({"origin": r.origin, "dest": r.dest, "ends": r.ends}))
                .collect::<Vec<_>>(),
            "resources": g.rules.resources.iter()
                .filter(|(_, spec)| matches!(spec.class.as_str(), "luxury" | "strategic"))
                .filter(|(resource, _)| g.resource_visible_to(pid, resource))
                .map(|(resource, spec)| json!({
                    "id": resource,
                    "class": spec.class,
                    "native": g.connected_resource_count(pid, resource),
                    "available": g.resource_access_count(pid, resource),
                    "controlled": (spec.class == "luxury")
                        .then(|| g.controlled_resource_count(pid, resource)),
                    "stockpile": (spec.class == "strategic")
                        .then(|| round1(g.strategic_stockpile(pid, resource))),
                    "capacity": (spec.class == "strategic")
                        .then(|| round1(g.strategic_stockpile_capacity(pid))),
                    "per_turn": (spec.class == "strategic")
                        .then(|| round1(g.strategic_resource_rate(pid, resource))),
                    "shortage": (spec.class == "strategic").then(|| {
                        p.strategic_resource_shortages
                            .get(resource)
                            .copied()
                            .unwrap_or(0)
                    }),
                }))
                .collect::<Vec<_>>(),
            "policies": p.policies,
            "policy_slots": g.gov_slots(pid),
            "available_policies": g.available_policies(pid),
            "boosted_techs": p.boosted_techs,
            "boosted_civics": p.boosted_civics,
            "yields": {
                "food": round1(empire[0]), "production": round1(empire[1]),
                "gold": round1(empire[2]), "science": round1(empire[3]),
                "culture": round1(empire[4]), "faith": round1(empire[5]),
            },
        },
        "players": g.players.iter().map(|o| {
            // An empire nobody has met is not on anybody's ledger. Civ VI
            // keeps an unmet civilization off the diplomacy ribbon, the
            // victory tracker and the score list alike, so the whole
            // dashboard below is withheld until contact rather than merely
            // hidden by the client — an agent reading this protocol is owed
            // the same fog a browser is. What survives is the seat's
            // identity: its id and civ decide the jersey its cities fly, and
            // whether it is still standing is already told by `wars`, which
            // is deliberately reported whole. An omniscient spectator sees
            // everyone, as it always has.
            if !omniscient && !g.has_met(pid, o.id) {
                return json!({
                    "id": o.id,
                    "civ": o.civ,
                    "met": false,
                    "alive": o.alive,
                    "is_minor": o.is_minor,
                    "is_barbarian": o.is_barbarian,
                    "is_free_city": o.is_free_city,
                    "team": o.team,
                    "teammate": false,
                });
            }
            // Civ VI's diplomacy ribbon keeps every major's broad empire
            // output visible.  These are aggregate public indicators rather
            // than hidden city details, and make spectator comparisons useful.
            let mut output = crate::rules::Yields::default();
            for cid in g.player_city_ids(o.id) {
                output.add(g.city_yields(cid));
            }
            let military = g.military_power(o.id).round() as i64;
            json!({
                "id": o.id, "civ": o.civ,
                "met": true,
                "leader": g.rules.civs.get(&o.civ).map(|c| c.leader.clone()),
                // A leader's agenda is public knowledge in Civ VI once you
                // have met them, and so is roughly how they feel about you.
                "agenda": g.agenda_of(o.id).map(|agenda| json!({
                    "name": agenda.name,
                    "description": agenda.description,
                })),
                "opinion_of_me": round1(g.agenda_opinion(o.id, pid)),
                "alive": o.alive,
                "is_minor": o.is_minor,
                "is_barbarian": o.is_barbarian,
                "is_free_city": o.is_free_city,
                "cs_type": if o.is_minor && !o.is_barbarian {
                    Some(Game::cs_type(&o.civ))
                } else {
                    None
                },
                "suzerain": if o.is_minor && !o.is_barbarian {
                    g.suzerain_of(o.id)
                } else {
                    None
                },
                "my_envoys": g.envoys_at(pid, o.id),
                "dvp": o.dvp,
                "domestic_tourists": g.domestic_tourists(o.id),
                "foreign_tourists": g.foreign_tourists(o.id),
                "science_projects": o.science_projects,
                "exoplanet_distance": round1(o.exoplanet_distance),
                "government": o.government,
                "anarchy_turns": o.anarchy_turns,
                "score": g.score(o.id),
                "cities": g.player_city_ids(o.id).len(),
                "suzerain_count": g.players.iter()
                    .filter(|minor| minor.alive && minor.is_minor && !minor.is_barbarian)
                    .filter(|minor| g.suzerain_of(minor.id) == Some(o.id))
                    .count(),
                "wonder_count": g.player_city_ids(o.id).iter()
                    .map(|city| g.cities[city].wonders.len())
                    .sum::<usize>(),
                "victories": if !o.is_minor && !o.is_barbarian {
                    Some(victory_progress_json(g, o.id, leading_score))
                } else {
                    None
                },
                "gold": round1(o.gold),
                "gold_per_turn": round1(o.gold_per_turn),
                "bankruptcy_amenity_penalty": o.bankruptcy_amenity_penalty,
                "faith": round1(o.faith),
                "yields": yields_json(&output),
                "military": military,
                "team": o.team,
                "teammate": g.same_team(pid, o.id),
                "at_war_with_me": g.is_at_war(pid, o.id),
                "grievances_against_me": o.grievances.get(&pid).copied().unwrap_or(0.0),
                "my_grievances": p.grievances.get(&o.id).copied().unwrap_or(0.0),
                "friend": g.are_friends(pid, o.id),
                "allied": g.are_allied(pid, o.id),
                "alliance": g.alliance_with(pid, o.id),
                "open_borders_to_me": g.has_open_borders(pid, o.id),
                "my_open_borders_to_them": g.has_open_borders(o.id, pid),
            })
        }).collect::<Vec<_>>(),
        "quick_deals": if omniscient { Vec::new() } else { g.quick_deals(pid) },
        "active_trade_deals": g.active_trade_deals.iter()
            .filter(|deal| deal.from == pid || deal.to == pid)
            .collect::<Vec<_>>(),
        "pending_deals": g.pending_deals.iter()
            .filter(|deal| deal.from == pid || deal.to == pid)
            .collect::<Vec<_>>(),
        "congress": g.congress,
        "active_congress_effects": g.active_congress_effects,
        "pending_emergencies": g.pending_emergencies,
        "active_emergencies": g.active_emergencies,
        "barbarian_alerts": g.barb_alerted_until.iter()
            .filter(|(camp, _)| vis.contains(camp))
            .map(|(camp, until)| json!({
                "camp": [camp.0, camp.1],
                "target": g.barb_camp_targets.get(camp).map(|target| [target.0, target.1]),
                "until": until,
            }))
            .collect::<Vec<_>>(),
        // Who is fighting whom, since when, and at what cost. War is the one
        // part of the world every civilization can see from the outside, and
        // the diplomacy panel above already names every player, so this is
        // shown whole rather than through the viewer's fog.
        "wars": wars_json(g),
        // Every detonation, newest last. Shown whole for the same reason wars
        // are: a mushroom cloud is not a thing one civilization keeps to
        // itself, and a client needs the account to place the blast on the map
        // rather than inferring one from a ring of fallout.
        "nuclear_strikes": nuclear_strikes_json(g),
        "winner": g.winner,
        "winners": g.winning_players(),
        "victory_type": g.victory_type,
        // What has happened to this civilization lately, newest last. An
        // omniscient viewer watches whichever seat it is observing, so the
        // spectator log follows the same seat as the rest of the frame.
        "events": recent_events(g, pid, omniscient),
    })
}

/// Every conflict in progress, longest-running first, followed by the most
/// recently concluded conflicts. A declaration can open several bilateral
/// fronts through teams and defensive alliances; `WarRecord::conflict` folds
/// those fronts into one durable, Wikipedia-style account here rather than
/// asking each browser to guess which records belong together.
fn wars_json(g: &Game) -> Vec<Value> {
    const RECENT_CONFLICTS: usize = 12;
    let mut grouped: BTreeMap<u32, Vec<&crate::game::WarRecord>> = BTreeMap::new();
    for war in g.wars.values().chain(g.concluded_wars.iter()) {
        grouped.entry(war.conflict).or_default().push(war);
    }
    let mut conflicts: Vec<Vec<&crate::game::WarRecord>> = grouped.into_values().collect();
    conflicts.sort_by_key(|records| {
        let ongoing = records.iter().any(|war| war.ended.is_none());
        let started = records.iter().map(|war| war.started).min().unwrap_or(0);
        let ended = records.iter().filter_map(|war| war.ended).max().unwrap_or(0);
        (!ongoing, if ongoing { started } else { u32::MAX - ended })
    });

    let mut concluded_seen = 0;
    conflicts
        .into_iter()
        .filter(|records| {
            if records.iter().any(|war| war.ended.is_none()) {
                true
            } else if concluded_seen < RECENT_CONFLICTS {
                concluded_seen += 1;
                true
            } else {
                false
            }
        })
        .map(|records| {
            let anchor = records
                .iter()
                .copied()
                .find(|war| war.aggressor == war.declarer && war.defender == war.target)
                .unwrap_or(records[0]);
            let started = records.iter().map(|war| war.started).min().unwrap_or(0);
            let ongoing = records.iter().any(|war| war.ended.is_none());
            let ended = (!ongoing)
                .then(|| records.iter().filter_map(|war| war.ended).max())
                .flatten();

            let mut losses: BTreeMap<usize, crate::game::WarLosses> = BTreeMap::new();
            for war in &records {
                for (player, toll) in &war.losses {
                    let total = losses.entry(*player).or_default();
                    total.units += toll.units;
                    total.cities += toll.cities;
                    total.city_names.extend(toll.city_names.iter().cloned());
                    for (kind, count) in &toll.unit_kinds {
                        *total.unit_kinds.entry(kind.clone()).or_insert(0) += count;
                    }
                }
            }

            // A participant may appear in several fronts at once. Merge those
            // copies into one interval, but keep a genuine later re-entry as a
            // separate row by including its entry turn in the key.
            let mut participation: BTreeMap<
                (usize, bool, Option<usize>, u32),
                (Option<u32>, i64, Option<i64>, BTreeMap<u32, i64>),
            > = BTreeMap::new();
            for participant in records.iter().flat_map(|war| &war.participants) {
                participation
                    .entry((
                        participant.player,
                        participant.declarer_side,
                        participant.suzerain,
                        participant.entered,
                    ))
                    .and_modify(|(exited, strength, peak, saw_action_units)| {
                        *exited = match (*exited, participant.exited) {
                            (Some(first), Some(second)) => Some(first.max(second)),
                            _ => None,
                        };
                        *strength = (*strength).max(participant.strength);
                        *peak = match (*peak, participant.peak_strength) {
                            (Some(first), Some(second)) => Some(first.max(second)),
                            (known, None) | (None, known) => known,
                        };
                        for (unit, value) in &participant.saw_action_units {
                            saw_action_units
                                .entry(*unit)
                                .and_modify(|known| *known = (*known).max(*value))
                                .or_insert(*value);
                        }
                    })
                    .or_insert((
                        participant.exited,
                        participant.strength,
                        participant.peak_strength,
                        participant.saw_action_units.clone(),
                    ));
            }
            let mut side_losses = [
                crate::game::WarLosses::default(),
                crate::game::WarLosses::default(),
            ];
            for (player, toll) in &losses {
                let declarer_side = participation
                    .keys()
                    .find(|(participant, _, _, _)| participant == player)
                    .map(|(_, side, _, _)| *side)
                    .unwrap_or(*player == anchor.declarer);
                let total = &mut side_losses[if declarer_side { 0 } else { 1 }];
                total.units += toll.units;
                total.cities += toll.cities;
            }
            let parties = participation
                .into_iter()
                .map(|(
                    (player, declarer_side, suzerain, entered),
                    (exited, strength, peak_strength, saw_action_units),
                )| {
                    let toll = losses.get(&player).cloned().unwrap_or_default();
                    let saw_action_strength = saw_action_units.values().sum::<i64>();
                    json!({
                        "player": player,
                        "declarer_side": declarer_side,
                        "suzerain": suzerain,
                        "entered": entered,
                        "exited": exited,
                        "strength": strength,
                        "strength_start": strength,
                        "strength_peak": peak_strength,
                        "strength_saw_action": saw_action_strength,
                        "units_lost": toll.units,
                        "cities_lost": toll.cities,
                        "unit_kinds": toll.unit_kinds,
                        "city_names": toll.city_names,
                    })
                })
                .collect::<Vec<_>>();

            let mut highlights = records
                .iter()
                .flat_map(|war| war.highlights.iter())
                .collect::<Vec<_>>();
            highlights.sort_by_key(|moment| {
                (moment.turn, moment.kind.as_str(), moment.actor, moment.subject)
            });
            highlights.dedup_by(|a, b| {
                a.turn == b.turn
                    && a.kind == b.kind
                    && a.actor == b.actor
                    && a.subject == b.subject
                    && a.city == b.city
            });
            let victor = highlights
                .iter()
                .rev()
                .find(|moment| moment.kind == "conquest")
                .map(|moment| moment.actor);
            let outcome = ended.and_then(|_| {
                if victor.is_some() {
                    Some("conquest".to_string())
                } else {
                    highlights
                        .iter()
                        .rev()
                        .find(|moment| matches!(moment.kind.as_str(), "peace" | "coalition"))
                        .map(|moment| moment.kind.clone())
                }
            });

            let mut settlements = BTreeMap::new();
            for peace in records.iter().flat_map(|war| &war.peace_terms) {
                settlements
                    .entry((peace.turn, peace.first, peace.second, peace.terms.join("\u{1f}")))
                    .or_insert(peace);
            }
            json!({
                "conflict": anchor.conflict,
                "aggressor": anchor.declarer,
                "defender": anchor.target,
                "started": started,
                "ended": ended,
                "turns": ended.unwrap_or(g.turn).saturating_sub(started),
                "outcome": outcome,
                "victor": victor,
                "sides": [
                    {"player": anchor.declarer, "units_lost": side_losses[0].units,
                     "cities_lost": side_losses[0].cities},
                    {"player": anchor.target, "units_lost": side_losses[1].units,
                     "cities_lost": side_losses[1].cities},
                ],
                "parties": parties,
                "highlights": highlights.into_iter().map(|highlight| json!({
                    "turn": highlight.turn,
                    "kind": highlight.kind,
                    "actor": highlight.actor,
                    "subject": highlight.subject,
                    "city": highlight.city,
                })).collect::<Vec<_>>(),
                "peace_terms": settlements.into_values().map(|peace| json!({
                    "turn": peace.turn,
                    "first": peace.first,
                    "second": peace.second,
                    "terms": peace.terms,
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

/// The tail of a civilization's event stream. Bounded because an observation
/// is sent every frame and a long game accumulates thousands.
///
/// The omniscient feed rotates through every seat, so per-item "researched"
/// and "adopted" entries would multiply by the number of civilizations and
/// drown the combined chronicle; its science and culture story is told by the
/// era-first world events instead. A civilization's own perspective keeps its
/// full personal log.
fn recent_events(g: &Game, pid: usize, omniscient: bool) -> Vec<Value> {
    const RECENT: usize = 60;
    let events: Vec<_> = g
        .events_for(pid)
        .into_iter()
        .filter(|event| !omniscient || !matches!(event.category.as_str(), "Science" | "Culture"))
        .collect();
    events[events.len().saturating_sub(RECENT)..]
        .iter()
        .map(|event| {
            json!({
                "turn": event.turn,
                "category": event.category,
                "text": event.text,
                "pos": event.pos.map(|pos| [pos.0, pos.1]),
                "important": event.important,
            })
        })
        .collect()
}

/// The detonations this game has seen, oldest first. `id` is what lets a client
/// tell a fresh blast from one it has already shown without comparing fields —
/// two devices can land on the same tile on the same turn.
fn nuclear_strikes_json(g: &Game) -> Vec<Value> {
    g.nuclear_strikes
        .iter()
        .map(|strike| {
            json!({
                "id": strike.id,
                "turn": strike.turn,
                "attacker": strike.attacker,
                "target": [strike.target.0, strike.target.1],
                "thermonuclear": strike.thermonuclear,
                "platform": strike.platform,
                "launched_from": [strike.launched_from.0, strike.launched_from.1],
                "blast_radius": strike.blast_radius,
                "fallout_until": strike.fallout_until,
                "victims": strike.victims,
                "cities": strike.cities,
                "units_destroyed": strike.units_destroyed,
            })
        })
        .collect()
}

/// Public victory-screen metrics. Each progress value is normalized to
/// 0..100 for sorting and meter width, while the underlying counts let the UI
/// describe the actual victory requirement instead of showing a vague percent.
fn victory_progress_json(g: &Game, pid: usize, leading_score: i64) -> Value {
    let player = &g.players[pid];
    let all_majors: Vec<usize> = g
        .players
        .iter()
        .filter(|candidate| !candidate.is_minor && !candidate.is_barbarian)
        .map(|candidate| candidate.id)
        .collect();
    let living_majors: Vec<usize> = all_majors
        .iter()
        .copied()
        .filter(|candidate| g.players[*candidate].alive)
        .collect();

    let science_projects = [
        "launch_earth_satellite",
        "launch_moon_landing",
        "launch_mars_colony",
        "exoplanet_expedition",
    ];
    let completed_projects = science_projects
        .iter()
        .filter(|project| player.science_projects.contains(**project))
        .count();
    let science_progress = if player.science_projects.contains("exoplanet_expedition") {
        75.0 + 25.0 * player.exoplanet_distance / EXOPLANET_DESTINATION
    } else {
        match completed_projects {
            0 => 0.0,
            1 => 25.0,
            2 => 45.0,
            _ => 65.0,
        }
    }
    .clamp(0.0, 100.0);
    // The space race is the last stretch of a road that starts at the first
    // technology, and for most of a game the tree is the only part of it a
    // watcher can see moving.
    let techs_known = player.techs.len();
    let tech_total = g.rules.techs.len();
    let civics_known = player.civics.len();
    let civic_total = g.rules.civics.len();

    let rival_domestic = living_majors
        .iter()
        .filter(|candidate| **candidate != pid && !g.same_team(pid, **candidate))
        .map(|candidate| g.domestic_tourists(*candidate))
        .max()
        .unwrap_or(0);
    let culture_target = rival_domestic + 1;
    let domestic_tourists = g.domestic_tourists(pid);
    // The culture a civilization keeps at home is the other half of this
    // race: it is what every rival's tourism has to out-run.
    let leading_domestic = all_majors
        .iter()
        .map(|candidate| g.domestic_tourists(*candidate))
        .max()
        .unwrap_or(0);
    let foreign_tourists = g.foreign_tourists(pid);
    let culture_progress = if culture_target > 0 {
        100.0 * foreign_tourists as f64 / culture_target as f64
    } else {
        0.0
    }
    .clamp(0.0, 100.0);

    let team_religions = g
        .team_members(pid)
        .into_iter()
        .filter_map(|member| g.players[member].religion.as_deref())
        .collect::<Vec<_>>();
    let religious_rivals = living_majors
        .iter()
        .copied()
        .filter(|candidate| player.team.is_none() || !g.same_team(pid, *candidate))
        .collect::<Vec<_>>();
    let converted_civs = religious_rivals
        .iter()
        .filter(|candidate| {
            let cities = g.player_city_ids(**candidate);
            !cities.is_empty()
                && team_religions.iter().any(|religion| {
                    cities
                        .iter()
                        .filter(|city| g.city_religion(&g.cities[city]) == Some(*religion))
                        .count()
                        * 2
                        > cities.len()
                })
        })
        .count();
    let religious_target = religious_rivals.len();
    let religious_progress = if religious_target > 0 {
        100.0 * converted_civs as f64 / religious_target as f64
    } else {
        0.0
    };

    // Domination counts every original capital in the world, a civilization's
    // own included — it starts the race holding exactly that one, and losing
    // it costs a step here just as it costs the victory in
    // `check_domination`.
    let capital_target = all_majors.len();
    let controlled_capitals = if player.team.is_some() {
        all_majors
            .iter()
            .filter(|original_owner| {
                let capital = g
                    .cities
                    .values()
                    .find(|city| city.is_capital && city.original_owner == **original_owner);
                if **original_owner == pid || g.same_team(pid, **original_owner) {
                    capital.is_some_and(|capital| capital.owner == **original_owner)
                } else {
                    capital.is_none_or(|capital| capital.owner != **original_owner)
                }
            })
            .count()
    } else {
        all_majors
            .iter()
            .filter(|original_owner| {
                g.cities
                    .values()
                    .find(|city| city.is_capital && city.original_owner == **original_owner)
                    .map_or(
                        **original_owner == pid || !g.players[**original_owner].alive,
                        |capital| capital.owner == pid,
                    )
            })
            .count()
    };
    let domination_progress = if capital_target > 0 {
        100.0 * controlled_capitals as f64 / capital_target as f64
    } else {
        0.0
    };

    let diplomatic_points = player.dvp.max(0);
    let diplomatic_progress =
        100.0 * diplomatic_points as f64 / DIPLOMATIC_VICTORY_POINTS.max(1) as f64;
    let score = g.team_score_rank_key(pid).0;
    // The score race is run against the clock, not against a threshold: it is
    // decided the turn the limit runs out, so the leader's meter is simply how
    // much of the game has been played, and everyone else sits at their share
    // of the leader's score along that same clock. On the final turn the
    // leader is full and a civilization on half their score is at half. A game
    // with no turn limit has no clock to fill, so its meter stays purely
    // relative to the leader.
    let clock = if g.max_turns > 0 && g.max_turns < 100_000 {
        (g.turn as f64 / g.max_turns as f64).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let score_progress = if leading_score > 0 {
        100.0 * clock * score.max(0) as f64 / leading_score as f64
    } else {
        0.0
    };

    json!({
        "science": {
            "progress": round1(science_progress),
            "projects": completed_projects,
            "project_target": science_projects.len(),
            "distance": round1(player.exoplanet_distance),
            "distance_target": EXOPLANET_DESTINATION,
            "techs": techs_known,
            "tech_total": tech_total,
        },
        "culture": {
            "progress": round1(culture_progress),
            "tourists": foreign_tourists,
            "target": culture_target,
            "civics": civics_known,
            "civic_total": civic_total,
            "domestic": domestic_tourists,
            "rival_domestic": rival_domestic,
            "leading_domestic": leading_domestic,
        },
        "religious": {
            "progress": round1(religious_progress),
            "converted": converted_civs,
            "target": religious_target,
        },
        "diplomatic": {
            "progress": round1(diplomatic_progress.clamp(0.0, 100.0)),
            "points": diplomatic_points,
            "target": DIPLOMATIC_VICTORY_POINTS,
        },
        "domination": {
            "progress": round1(domination_progress),
            "capitals": controlled_capitals,
            "target": capital_target,
        },
        "score": {
            "progress": round1(score_progress.clamp(0.0, 100.0)),
            "points": score,
            "leader": leading_score,
        },
    })
}

fn tile_json(
    g: &Game,
    pid: usize,
    tile: &Tile,
    owner: Option<usize>,
    omniscient: bool,
    live: bool,
    planned: Option<&str>,
) -> Value {
    let resource = tile
        .resource
        .as_ref()
        .filter(|resource| omniscient || g.resource_visible_to(pid, resource));
    // Adjacency is read off the *current* neighbors, so it may only be sent
    // for a tile being looked at right now. A remembered district would
    // otherwise report yields from tiles the player cannot see.
    let district_yields = tile
        .district
        .as_deref()
        .filter(|district| live && g.rules.districts.contains_key(*district))
        .map(|district| {
            (
                g.district_yields(district, tile.pos),
                g.district_adjacency_sources(district, tile.pos),
            )
        });
    let (district_yields, adjacency) = match district_yields {
        Some((yields, sources)) => (json!(yields), json!(sources)),
        None => (Value::Null, Value::Null),
    };
    // A district still in the production queue reports what its site would pay
    // today — the figure a player is actually deciding on.
    let planned = planned
        .filter(|district| tile.district.is_none() && g.rules.districts.contains_key(*district))
        .map(|district| {
            json!({
                "district": district,
                "yields": g.district_yields(district, tile.pos),
                "adjacency": g.district_adjacency_sources(district, tile.pos),
            })
        })
        .unwrap_or(Value::Null);
    // Appeal is read off the *current* neighbours, exactly like adjacency
    // above, so a remembered tile does not report a figure its owner has since
    // changed — and the walk is paid for the tiles in sight, not for the whole
    // explored map.
    let appeal = if live {
        json!(g.tile_appeal(tile.pos))
    } else {
        Value::Null
    };
    json!({
        "pos": [tile.pos.0, tile.pos.1],
        "terrain": tile.terrain,
        "appeal": appeal,
        "feature": tile.feature,
        "hills": tile.hills,
        "resource": resource,
        "improvement": tile.improvement,
        "pillaged": tile.pillaged,
        "district": tile.district,
        "district_yields": district_yields,
        "adjacency": adjacency,
        "planned_district": planned,
        "wonder": tile.wonder,
        "owner": owner,
        "river": tile.has_river(),
        "river_edges": tile.river_edges,
        "road": tile.road,
        "cliff_edges": tile.cliff_edges,
        "continent": tile.continent,
        "coastal_lowland": tile.coastal_lowland,
        "flooded": tile.flooded,
        "submerged": tile.submerged,
        "drought": tile.drought,
        "storm": tile.storm,
        "fallout_until": tile.fallout_until,
        "disaster_yields": {
            "food": tile.disaster_food,
            "production": tile.disaster_production,
            "faith": tile.disaster_faith,
        },
    })
}

struct PublicCity<'a> {
    id: u32,
    name: &'a str,
    owner: usize,
    pos: Pos,
    pop: i32,
    hp: i32,
    is_capital: bool,
    original_owner: usize,
    captured_from: Option<usize>,
    occupied_from: Option<usize>,
    wall_hp: i32,
    wall_max: i32,
    encampment_hp: i32,
    encampment_wall_hp: i32,
    encampment_pillaged: bool,
    religion: Option<&'a str>,
}

fn public_city_json(city: PublicCity<'_>) -> Value {
    json!({
        "id": city.id,
        "name": city.name,
        "owner": city.owner,
        "pos": [city.pos.0, city.pos.1],
        "pop": city.pop,
        "hp": city.hp,
        "is_capital": city.is_capital,
        "original_owner": city.original_owner,
        "captured_from": city.captured_from,
        "occupied_from": city.occupied_from,
        "wall_hp": city.wall_hp,
        "wall_max": city.wall_max,
        "encampment_hp": city.encampment_hp,
        "encampment_wall_hp": city.encampment_wall_hp,
        "encampment_pillaged": city.encampment_pillaged,
        "religion": city.religion,
    })
}

fn remembered_city_json(city: &RememberedCity) -> Value {
    public_city_json(PublicCity {
        id: city.id,
        name: &city.name,
        owner: city.owner,
        pos: city.pos,
        pop: city.pop,
        hp: city.hp,
        is_capital: city.is_capital,
        original_owner: city.original_owner,
        captured_from: city.captured_from,
        occupied_from: city.occupied_from,
        wall_hp: city.wall_hp,
        wall_max: city.wall_max,
        encampment_hp: city.encampment_hp,
        encampment_wall_hp: city.encampment_wall_hp,
        encampment_pillaged: city.encampment_pillaged,
        religion: city.religion.as_deref(),
    })
}

fn live_city_json(g: &Game, pid: usize, city: &City, omniscient: bool) -> Value {
    let mut value = public_city_json(PublicCity {
        id: city.id,
        name: &city.name,
        owner: city.owner,
        pos: city.pos,
        pop: city.pop,
        hp: city.hp,
        is_capital: city.is_capital,
        original_owner: city.original_owner,
        captured_from: city.captured_from,
        occupied_from: city.occupied_from,
        wall_hp: city.wall_hp,
        wall_max: g.city_max_wall_hp(city),
        encampment_hp: city.encampment_hp,
        encampment_wall_hp: city.encampment_wall_hp,
        encampment_pillaged: city.encampment_pillaged,
        religion: g.city_religion(city),
    });
    if city.owner != pid && !omniscient {
        return value;
    }

    let citizens = g.city_citizen_plan(city.id);
    let yields = g.city_yields(city.id);
    let private = json!({
        "food": round1(city.food),
        "production": round1(city.production),
        "queue": city.queue,
        "buildings": city.buildings,
        "products": city.products,
        "product_capacity": g.product_capacity(city),
        "districts": city.districts,
        "wonders": city.wonders,
        "owned_tiles": city.owned_tiles.iter()
            .map(|tile| json!([tile.0, tile.1])).collect::<Vec<_>>(),
        "yields": yields_json(&yields),
        "housing": g.city_housing(city),
        "amenities": g.city_amenities(city),
        "amenities_required": Game::city_amenities_required(city),
        "amenity_surplus": g.city_amenity_surplus(city),
        "happiness": g.city_happiness(city),
        "power_demand": g.city_power_demand(city),
        "power_supply": g.city_power_supply(city),
        "powered": g.city_is_powered(city),
        "reactor_age": city.reactor_age,
        "reactor_accident_risk": round1(100.0 * g.reactor_accident_risk(city.id)),
        "growth_need": g.growth_cost(city.pop),
        "queue_cost": city.queue.first()
            .map(|item| g.item_cost_for_city(city.owner, city.id, item)),
        "can_strike": g.city_can_strike(city),
        "loyalty": round1(city.loyalty),
        "loyalty_per_turn": round1(g.city_loyalty_per_turn(city)),
        "loyalty_state": Game::loyalty_state(city.loyalty),
        "free_city": g.players[city.owner].is_free_city,
        "governor": g.players[city.owner].governors.contains(&city.id),
        "citizens": {
            "focus": citizens.strategy.focus,
            "weights": yields_json(&citizens.strategy.weights),
            "food_target": round1(citizens.strategy.food_target),
            "worked_tiles": citizens.worked_tiles.iter()
                .map(|tile| json!([tile.0, tile.1])).collect::<Vec<_>>(),
            "specialists": citizens.specialists,
        },
    });
    merge(&mut value, private);
    value
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn yields_json(ys: &crate::rules::Yields) -> Value {
    json!({
        "food": round1(ys.food), "production": round1(ys.production),
        "gold": round1(ys.gold), "science": round1(ys.science),
        "culture": round1(ys.culture), "faith": round1(ys.faith),
    })
}

fn merge(base: &mut Value, ext: Value) {
    if let (Some(b), Some(e)) = (base.as_object_mut(), ext.as_object()) {
        for (k, v) in e {
            b.insert(k.clone(), v.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The player HUD and the victory tracker are drawn from `players`, so an
    /// empire nobody has met has to be missing from it rather than merely
    /// skipped by the browser. What is left is the seat's identity — the id
    /// and civ its cities fly on the map — and nothing anyone could rank it by.
    #[test]
    fn an_unmet_civilization_reaches_the_wire_as_identity_and_nothing_else() {
        let mut game = Game::new(2, 18, 12, 74_115, 25, 0);
        for pid in 0..2 {
            game.players[pid].met.clear();
        }
        let hidden = &observation(&game, 0)["players"][1];
        assert_eq!(hidden["met"], json!(false));
        assert_eq!(hidden["civ"], json!(game.players[1].civ));
        for withheld in [
            "score",
            "victories",
            "yields",
            "military",
            "cities",
            "gold",
            "leader",
            "agenda",
            "government",
            "wonder_count",
            "opinion_of_me",
        ] {
            assert!(
                hidden[withheld].is_null(),
                "an unmet civilization must not report {withheld}"
            );
        }
        // The viewer is always on its own ledger, and so is the omniscient
        // spectator's whole table.
        assert_eq!(observation(&game, 0)["players"][0]["met"], json!(true));
        let spectated = observation_spectator(&game, 0);
        assert_eq!(spectated["players"][1]["met"], json!(true));
        assert!(spectated["players"][1]["score"].is_number());

        game.record_contact(0, 1);
        let found = &observation(&game, 0)["players"][1];
        assert_eq!(found["met"], json!(true));
        assert!(found["score"].is_number(), "the dashboard arrives on contact");
        assert!(!found["victories"].is_null());
        assert_eq!(
            observation(&game, 1)["players"][0]["met"],
            json!(true),
            "and the meeting was mutual"
        );
    }

    /// The viewer starts a world facing wherever it was found and only puts
    /// north at the top once the civilization it is watching can find north, so
    /// the observation has to say. A spectator is above the world rather than in
    /// it and always can; a civilization has to research it.
    #[test]
    fn finding_north_is_a_technology_for_a_civilization_and_free_for_a_spectator() {
        let mut game = Game::new(2, 18, 12, 11, 25, 0);
        assert!(
            game.rules.techs.get(NORTH_TECH).is_some(),
            "{NORTH_TECH} must name a technology in the shipped tree",
        );
        game.players[0].techs.remove(NORTH_TECH);

        let own = observation(&game, 0);
        assert_eq!(own["me"]["found_north"], json!(false));
        assert_eq!(own["me"]["north_tech"], json!(NORTH_TECH));
        // Watching one civilization's own view is that civilization's knowledge.
        assert_eq!(
            observation_player_view(&game, 0)["me"]["found_north"],
            json!(false),
        );
        assert_eq!(
            observation_spectator(&game, 0)["me"]["found_north"],
            json!(true),
        );

        game.players[0].techs.insert(NORTH_TECH.to_string());
        assert_eq!(observation(&game, 0)["me"]["found_north"], json!(true));
        assert_eq!(
            observation_player_view(&game, 0)["me"]["found_north"],
            json!(true),
        );
    }

    /// The other half of a world's bearing, and all of its extent: a people
    /// know their world comes back on itself once their own knowledge has run
    /// the whole way round it. A spectator is above the world and is told at
    /// once; a civilization has to go.
    #[test]
    fn going_the_whole_way_round_is_earned_by_a_civilization_and_free_for_a_spectator() {
        let mut game = Game::new(2, 18, 12, 11, 25, 0);

        let own = observation(&game, 0);
        assert_eq!(own["me"]["went_around"], json!(false));
        assert_eq!(
            observation_player_view(&game, 0)["me"]["went_around"],
            json!(false),
        );
        assert_eq!(
            observation_spectator(&game, 0)["me"]["went_around"],
            json!(true),
        );

        game.players[0].went_around = true;
        assert_eq!(observation(&game, 0)["me"]["went_around"], json!(true));
        assert_eq!(
            observation_player_view(&game, 0)["me"]["went_around"],
            json!(true),
        );
    }

    /// Sailing round the world is a proof that it is round. A people who set
    /// out west and came home from the east have settled the question on the
    /// water, without anyone having to read it in a book first.
    #[test]
    fn a_lap_of_the_world_proves_it_round_without_the_technology() {
        let mut game = Game::new(2, 18, 12, 11, 25, 0);
        for tech in GLOBE_TECHS {
            game.players[0].techs.remove(tech);
        }
        game.players[0].great_people.clear();
        game.players[0].went_around = false;
        assert_eq!(observation(&game, 0)["me"]["knows_globe"], json!(false));

        game.players[0].went_around = true;
        assert_eq!(observation(&game, 0)["me"]["knows_globe"], json!(true));
        // Still only the world they have been round: the far end of the system
        // waits for an instrument above the air either way.
        assert_eq!(observation(&game, 0)["me"]["sees_exoplanet"], json!(false));
    }

    /// The shape of the world is the same kind of fact as which way is north:
    /// the viewer has to be told, because it draws a globe a people who have not
    /// worked it out yet are not supposed to be looking at. Either proof opens
    /// it, the great person opens it early, and the far end of the system waits
    /// for an eye above the air.
    #[test]
    fn a_round_world_is_a_discovery_and_the_far_planet_waits_for_a_satellite() {
        let mut game = Game::new(2, 18, 12, 11, 25, 0);
        for tech in GLOBE_TECHS {
            assert!(
                game.rules.techs.get(tech).is_some(),
                "{tech} must name a technology in the shipped tree",
            );
            game.players[0].techs.remove(tech);
        }
        assert!(
            game.rules.great_people.get(GLOBE_GREAT_PERSON).is_some(),
            "{GLOBE_GREAT_PERSON} must name a great person in the shipped roster",
        );
        game.players[0].great_people.clear();
        game.players[0].science_projects.remove(EXOPLANET_EYE);

        let own = observation(&game, 0);
        assert_eq!(own["me"]["knows_globe"], json!(false));
        assert_eq!(own["me"]["sees_exoplanet"], json!(false));
        assert_eq!(own["me"]["globe_techs"], json!(GLOBE_TECHS));
        assert_eq!(own["me"]["globe_great_person"], json!(GLOBE_GREAT_PERSON));
        assert_eq!(own["me"]["exoplanet_eye"], json!(EXOPLANET_EYE));
        assert_eq!(
            observation_player_view(&game, 0)["me"]["knows_globe"],
            json!(false),
        );
        // A spectator is above the world and has never had to prove anything
        // about it, so the whole system is open from the first turn.
        let watching = observation_spectator(&game, 0);
        assert_eq!(watching["me"]["knows_globe"], json!(true));
        assert_eq!(watching["me"]["sees_exoplanet"], json!(true));

        // Each road on its own is enough.
        for tech in GLOBE_TECHS {
            game.players[0].techs.insert(tech.to_string());
            assert_eq!(observation(&game, 0)["me"]["knows_globe"], json!(true));
            game.players[0].techs.remove(tech);
        }
        game.players[0]
            .great_people
            .push(GLOBE_GREAT_PERSON.to_string());
        assert_eq!(observation(&game, 0)["me"]["knows_globe"], json!(true));

        // The other star stays out of reach until something has actually been
        // put up there to see it with.
        assert_eq!(observation(&game, 0)["me"]["sees_exoplanet"], json!(false));
        game.players[0]
            .science_projects
            .insert(EXOPLANET_EYE.to_string());
        assert_eq!(observation(&game, 0)["me"]["sees_exoplanet"], json!(true));
    }

    #[test]
    fn the_spectator_feed_trades_per_item_research_for_era_firsts() {
        let mut game = Game::new(2, 18, 12, 7, 25, 0);
        game.note(0, "Science", "researched pottery", None);
        game.note(0, "Culture", "adopted code of laws", None);
        game.note(0, "War", "declared war on somebody", None);

        let categories = |observed: &Value| -> Vec<String> {
            observed["events"]
                .as_array()
                .unwrap()
                .iter()
                .map(|event| event["category"].as_str().unwrap().to_string())
                .collect()
        };

        let spectator = categories(&observation_spectator(&game, 0));
        assert!(!spectator.iter().any(|category| category == "Science"));
        assert!(!spectator.iter().any(|category| category == "Culture"));
        assert!(spectator.iter().any(|category| category == "War"));

        let personal = categories(&observation(&game, 0));
        assert!(personal.iter().any(|category| category == "Science"));
        assert!(personal.iter().any(|category| category == "Culture"));
    }

    /// The browser draws its blast, writes its log entry and marks its war card
    /// from these three fields and nothing else. A detonation the client cannot
    /// see is the same as one that did not happen, so the wire shape is pinned
    /// here rather than left to a screenshot.
    #[test]
    fn a_detonation_reaches_the_wire_whole() {
        let mut game = Game::new_full(2, 24, 16, 91_806, 200, 0, false);
        let mut capitals = Vec::new();
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            capitals.push(game.found_city_for(pid, game.units[&settler].pos, None));
            game.remove_unit(settler);
        }
        let target = game.cities[&capitals[1]].pos;
        let city_name = game.cities[&capitals[1]].name.clone();
        game.at_war.insert((0, 1));
        // Whether two randomly placed capitals sit inside 12 tiles or 15 is a
        // property of the seed, so the device is chosen to reach rather than the
        // fixture bent to suit one.
        let spec = game.rules.wmds["thermonuclear_device"].clone();
        let distance = game.wdist(game.cities[&capitals[0]].pos, target);
        assert!(
            distance <= spec.icbm_strike_range,
            "fixture capitals must sit within ICBM range ({distance})"
        );
        game.players[0]
            .counters
            .insert("project_effect:thermonuclear_devices".to_string(), 1);
        for position in game.wdisk(target, spec.blast_radius) {
            game.players[0].explored.insert(position);
        }
        game.apply(
            0,
            &crate::game::Action::WmdStrike {
                city: capitals[0],
                target,
                thermonuclear: true,
            },
        )
        .expect("the fixture capitals are inside ICBM range");

        let observed = observation_spectator(&game, 0);

        // 1. The strike ledger, which is what the animation reads.
        let strikes = observed["nuclear_strikes"].as_array().unwrap();
        assert_eq!(strikes.len(), 1);
        let strike = &strikes[0];
        assert!(strike["id"].as_u64().is_some(), "a blast needs an identity");
        assert_eq!(strike["attacker"], serde_json::json!(0));
        assert_eq!(strike["turn"], serde_json::json!(game.turn));
        assert_eq!(
            strike["target"],
            serde_json::json!([target.0, target.1]),
            "ground zero cannot be inferred from a halved city"
        );
        assert_eq!(strike["thermonuclear"], serde_json::json!(true));
        assert_eq!(strike["platform"], serde_json::json!("city"));
        assert_eq!(strike["blast_radius"], serde_json::json!(spec.blast_radius));
        assert_eq!(strike["cities"], serde_json::json!([city_name]));
        assert_eq!(strike["victims"], serde_json::json!([1]));
        assert!(strike["fallout_until"].as_u64().unwrap() > game.turn as u64);

        // 2. The notification, flagged so the log pins it.
        let detonation = observed["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|event| event["category"] == "Nuclear")
            .expect("the log hears about it");
        assert_eq!(detonation["important"], serde_json::json!(true));
        assert!(detonation["text"]
            .as_str()
            .unwrap()
            .contains("thermonuclear device"));
        assert_eq!(
            detonation["pos"],
            serde_json::json!([target.0, target.1]),
            "the entry points at the crater"
        );

        // 3. The war moment, so the card can badge the war as nuclear.
        let moment = observed["wars"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|war| war["highlights"].as_array().unwrap())
            .find(|moment| moment["kind"] == "nuclear_strike")
            .expect("the war ledger names the detonation");
        assert_eq!(moment["actor"], serde_json::json!(0));
        assert_eq!(moment["subject"], serde_json::json!(1));
        assert_eq!(moment["city"], serde_json::json!(city_name));

        // The victim's own view carries the same detonation, marked the same way.
        let victim = observation(&game, 1);
        let told = victim["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|event| event["category"] == "Nuclear")
            .expect("the civilization it landed on is told");
        assert_eq!(told["important"], serde_json::json!(true));
    }

    #[test]
    fn observation_exposes_compact_hud_and_victory_race_metrics() {
        let mut game = Game::new_full(2, 20, 14, 81_004, 120, 1, false);
        let capital_position = game
            .player_unit_ids(0)
            .into_iter()
            .find_map(|unit| {
                let unit = &game.units[&unit];
                (unit.kind == "settler").then_some(unit.pos)
            })
            .unwrap();
        game.found_city_for(0, capital_position, None);
        let observed = observation_spectator(&game, 0);
        assert_eq!(observed["max_turns"], serde_json::json!(120));

        let player = observed["players"]
            .as_array()
            .unwrap()
            .iter()
            .find(|player| player["id"] == serde_json::json!(0))
            .unwrap();
        assert_eq!(
            player["cities"],
            serde_json::json!(game.player_city_ids(0).len()),
        );
        assert!(player["suzerain_count"].is_number());
        assert!(player["wonder_count"].is_number());

        let free_cities = observed["players"]
            .as_array()
            .unwrap()
            .iter()
            .find(|player| player["is_free_city"] == serde_json::json!(true))
            .unwrap();
        assert_eq!(free_cities["alive"], serde_json::json!(false));

        let city_id = game.player_city_ids(0)[0];
        let source_city = &game.cities[&city_id];
        let city = observed["cities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|city| city["id"] == serde_json::json!(city_id))
            .unwrap();
        assert_eq!(
            city["amenities"],
            serde_json::json!(game.city_amenities(source_city))
        );
        assert_eq!(
            city["amenities_required"],
            serde_json::json!(Game::city_amenities_required(source_city))
        );
        assert_eq!(
            city["amenity_surplus"],
            serde_json::json!(game.city_amenity_surplus(source_city))
        );
        assert_eq!(
            city["happiness"],
            serde_json::json!(game.city_happiness(source_city))
        );
        assert!(city["loyalty_per_turn"].is_number());
        assert!(city["loyalty_state"].is_string());
        assert_eq!(city["free_city"], serde_json::json!(false));

        let victories = player["victories"].as_object().unwrap();
        for victory in [
            "science",
            "culture",
            "religious",
            "diplomatic",
            "domination",
            "score",
        ] {
            assert!(victories[victory]["progress"].is_number(), "{victory}");
        }
    }

    /// The victory ribbon carries the researched technology and civic counts
    /// behind the science and culture races. Domination counts every original
    /// capital in the world, a civilization's own included, which is how
    /// `check_domination` reads the board.
    #[test]
    fn victory_metrics_carry_researched_trees_and_every_capital() {
        let players = 6;
        let game = Game::new_full(players, 26, 18, 81_005, 120, 1, false);
        let observed = observation_spectator(&game, 0);
        let victories = observed["players"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|player| player["victories"].is_object())
            .map(|player| player["victories"].clone())
            .collect::<Vec<_>>();
        assert_eq!(victories.len(), players);

        for victory in &victories {
            assert_eq!(victory["domination"]["capitals"], serde_json::json!(1));
            assert_eq!(victory["domination"]["target"], serde_json::json!(players));
            assert_eq!(
                victory["science"]["tech_total"],
                serde_json::json!(game.rules.techs.len()),
            );
            assert!(victory["science"]["techs"].is_number());
            assert_eq!(
                victory["culture"]["civic_total"],
                serde_json::json!(game.rules.civics.len()),
            );
            assert!(victory["culture"]["civics"].is_number());
            assert!(victory["culture"]["domestic"].is_number());
            assert!(victory["culture"]["leading_domestic"].is_number());
        }
    }

    /// The score race ends on the turn limit rather than at a threshold, so
    /// its meter is the clock: the leader is exactly as far along as the game
    /// is, and every rival is that same distance cut by their share of the
    /// leader's score.
    #[test]
    fn score_meter_is_the_turn_clock_scaled_by_the_share_of_the_leader() {
        let mut game = Game::new_full(4, 26, 18, 81_006, 100, 1, false);
        game.turn = 25;
        let leading = game
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .map(|player| game.team_score_rank_key(player.id).0)
            .max()
            .unwrap();
        assert!(leading > 0);

        let observed = observation_spectator(&game, 0);
        let mut saw_leader = false;
        for player in observed["players"].as_array().unwrap() {
            let Some(score) = player["victories"]["score"].as_object() else {
                continue;
            };
            let points = score["points"].as_i64().unwrap();
            let expected = round1(100.0 * 0.25 * points as f64 / leading as f64);
            assert_eq!(score["progress"], serde_json::json!(expected));
            if points == leading {
                saw_leader = true;
                // A quarter of the turns played puts the leader a quarter up.
                assert_eq!(score["progress"], serde_json::json!(25.0));
            }
        }
        assert!(saw_leader);

        // The last playable turn fills the leader's meter: the score victory
        // is awarded once the turn passes the limit.
        game.turn = game.max_turns;
        let observed = observation_spectator(&game, 0);
        let best = observed["players"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|player| player["victories"]["score"]["progress"].as_f64())
            .fold(0.0_f64, f64::max);
        assert_eq!(best, 100.0);
    }
}
