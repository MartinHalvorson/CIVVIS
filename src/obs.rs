//! JSON observation builder (fog-of-war view for a player) — feeds the GUI
//! and any external agent speaking the JSON protocol.
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::game::{
    City, Game, Item, RememberedCity, Unit, UnitMoveTrail, DIPLOMATIC_VICTORY_POINTS,
    EXOPLANET_TARGETS,
};
use crate::name::Name;
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
        || GLOBE_TECHS
            .iter()
            .any(|tech| p.techs.contains(&Name::new(tech)))
        || p.great_people
            .iter()
            .any(|id| id.as_str() == GLOBE_GREAT_PERSON)
}

/// The instruments that reach past what an eye can see.
///
/// The five wandering stars are naked-eye objects and always were: Mercury,
/// Venus, Mars, Jupiter and Saturn are in every sky anybody has ever looked at,
/// and a people who know their world is a ball can place all five of them. The
/// rung above is the one the telescope opened, and it opened as one rung —
/// Uranus in 1781, Ceres in 1801, Neptune in 1846 by being predicted before it
/// was looked for, and in 1838 the first measured distance to another star. So
/// this gate hands over the outer system and a neighbourhood with real
/// distances in it at the same moment, because that is when both arrived.
///
/// Newton is the recruit who does it without the discovery, for the same reason
/// Hypatia opens the globe: he built the reflecting telescope every one of
/// those findings was made with a descendant of.
pub const OUTER_SYSTEM_TECHS: [&str; 1] = ["scientific_theory"];
pub const OUTER_SYSTEM_GREAT_PERSON: &str = "isaac_newton";

/// Whether this civilization can see past the five wanderers. A people who have
/// not proved their world round are not handed the outer planets by a tech:
/// there is no system to put them in yet.
pub fn sees_outer_system(p: &crate::game::Player) -> bool {
    knows_globe(p)
        && (OUTER_SYSTEM_TECHS
            .iter()
            .any(|tech| p.techs.contains(&Name::new(tech)))
            || p.great_people
                .iter()
                .any(|id| id.as_str() == OUTER_SYSTEM_GREAT_PERSON)
            || p.science_projects.contains(EXOPLANET_EYE))
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

/// What can be read off one unit by somebody looking at it: how far it
/// reaches on a turn's movement, and what it can see from where it stands.
///
/// The observation deliberately does not carry this for every unit — one
/// flow per unit per frame dominated late-game spectator responses, which is
/// why `reachable` is built only for the seat that can actually act. So it is
/// asked for one unit at a time, when a viewer points at one.
///
/// `viewer` is the seat the question is asked on behalf of, or `None` for the
/// omniscient spectator, and it is the *same* seat the observation was built
/// for — otherwise this hands back ground that view has never been shown. Two
/// rules follow from that and neither is optional:
///
/// * A unit nobody in that view can see is not a unit to ask about. The answer
///   is an error rather than an empty range, because "no range" is itself a
///   claim about a unit the viewer is not entitled to have located.
/// * Both sets are clipped to ground that view has explored. A Knight's reach
///   runs over country its owner knows and the watcher may not, and the
///   perimeter of that reach would draw the coastline of a continent nobody
///   in this game has sailed to.
///
/// The movement set spends a whole turn's allowance and reads through other
/// units on purpose — see [`Game::threat_reach`]. The sight set is real,
/// terrain and promotions included, which is what makes a hill worth pointing
/// at.
pub fn unit_intel(g: &Game, viewer: Option<usize>, uid: u32) -> Value {
    let Some(unit) = g.units.get(&uid) else {
        return json!({"error": "no such unit"});
    };
    let seen: Option<BTreeSet<Pos>> = match viewer {
        None => None,
        Some(pid) => {
            let (vis, explored) = visibility(g, pid);
            let at = unit.air_patrol_pos.unwrap_or(unit.pos);
            let watched = unit.owner == pid
                || (vis.contains(&at)
                    && g.visibility_viewers(pid)
                        .into_iter()
                        .any(|watcher| g.unit_visible_to(uid, watcher)));
            if !watched {
                return json!({"error": "that unit is not in view"});
            }
            Some(explored)
        }
    };
    let known = |tiles: Vec<Pos>| -> Vec<Value> {
        tiles
            .into_iter()
            .filter(|pos| seen.as_ref().is_none_or(|explored| explored.contains(pos)))
            .map(|pos| json!([pos.0, pos.1]))
            .collect()
    };
    json!({
        "error": Value::Null,
        "unit": uid,
        "owner": unit.owner,
        "kind": unit.kind,
        "pos": [unit.pos.0, unit.pos.1],
        "moves_left": round1(unit.moves_left),
        "sight": g.unit_sight(uid),
        "reachable": known(g.threat_reach(uid)),
        "vision": known(g.unit_visible_tiles(uid).into_iter().collect()),
    })
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
    // `vis` is the exact frame used for rules and for the map's sight
    // perimeter. During the acting player's turn, keep everything seen in an
    // earlier frame in the lighter map region until End Turn; exact last-seen
    // snapshots supply its contents without reading changing state under fog.
    let mut turn_vis = vis.clone();
    if !omniscient && g.current == pid {
        turn_vis.extend(p.turn_visible.iter().copied());
    }
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
    // A seated player gets only their own Tourism sources, matching Civ VI's
    // Tourism lens. The omniscient spectator combines every major empire so
    // the same lens remains useful while watching the whole world.
    let mut tourism_by_tile = BTreeMap::new();
    let tourism_players: Vec<usize> = if omniscient {
        g.players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect()
    } else {
        vec![pid]
    };
    for player in tourism_players {
        for (position, amount) in g.tourism_by_tile(player) {
            *tourism_by_tile.entry(position).or_default() += amount;
        }
    }
    let revealed = revealed_resources(g, pid, omniscient);
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
                tile,
                owner,
                &revealed,
                live,
                &tourism_by_tile,
                planned.get(pos).copied().filter(|_| live),
            ))
        })
        .collect();
    let mut known_units: BTreeMap<u32, &Unit> = g
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
        .map(|unit| (unit.id, unit))
        .collect();
    if !omniscient && g.current == pid {
        for unit in p.turn_units.values() {
            let observed_pos = unit.air_patrol_pos.unwrap_or(unit.pos);
            if !vis.contains(&observed_pos) {
                known_units.entry(unit.id).or_insert(unit);
            }
        }
    }
    let units: Vec<Value> = known_units
        .into_values()
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
                // The per-edge companion: which single steps the remaining
                // movement affords, so a client can draw how the range is
                // entered. Only the engine knows step costs, rivers and ZOC,
                // so the client must not re-derive these from `reachable`.
                v["reach_steps"] = json!(g
                    .reach_steps(u.id)
                    .iter()
                    .map(|(a, b)| json!([[a.0, a.1], [b.0, b.1]]))
                    .collect::<Vec<_>>());
                if let Some((target, gold, _)) = g.unit_gold_upgrade_offer(pid, u.id) {
                    v["upgrade"] = json!({ "to": target, "gold": gold });
                }
            }
            // Whether the unit has been left behind by the ruleset is worth
            // showing even when the upgrade itself is out of reach this turn.
            if u.owner == pid || omniscient {
                v["obsolete"] = json!(g.unit_is_obsolete(u.owner, u.kind));
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
    if let Some(adjustment) = g.observed_yield_adjustments.get(&pid) {
        empire[0] += adjustment.food;
        empire[1] += adjustment.production;
        empire[2] += adjustment.gold;
        empire[3] += adjustment.science;
        empire[4] += adjustment.culture;
        empire[5] += adjustment.faith;
    }
    // Founder-belief income and the Faith paid for unused Great Person
    // points are collected by the empire, not by any city; the top bar
    // shows them with the city sum, and so does this.
    {
        let extras = g.player_yield_extras(pid);
        empire[2] += extras.gold;
        empire[3] += extras.science;
        empire[4] += extras.culture;
        empire[5] += extras.faith;
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
            KnownCity::Remembered(city) => {
                let mut memory = city.clone();
                g.refresh_defending_district_memory(&mut memory, &vis);
                remembered_city_json(&memory)
            }
            KnownCity::Live(city) => live_city_json(g, pid, city, omniscient, &vis),
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
    // The three big arrays go in afterwards. `json!` copies whatever is named in
    // it by serializing it again, and every frame of a spectated game was paying
    // to rebuild thousands of tile, unit and city documents it had just made.
    let mut observation = json!({
        "turn": g.turn,
        "max_turns": g.max_turns,
        // `max_turns` remains the setup value a successor game should inherit.
        // This nullable field is the live rule: playing on explicitly removes
        // that cap, so clients must display infinity rather than a stale turn.
        "turn_limit": g.turn_limit(),
        "seed": g.seed,
        "game_speed": g.game_speed.id(),
        // The setup panel can faithfully read a running arena back after a
        // reload, including its selected deadline. Worlds carry the stock
        // value but never consult it.
        "tactics": g.tactics,
        // Where each side's flag stands, on the arena shape that plants
        // them. Published to every viewer **through the fog**, deliberately:
        // a flag is the battle's objective and its position is common
        // ground — both commanders marched in knowing where the other's
        // flag was, and a capture-the-flag battle in which you must first go
        // and find the thing you are capturing is a different game. What the
        // fog still hides is everything around it: whose army is standing
        // there and in what strength.
        "arena_flags": g.arena_flags
            .iter()
            .map(|(owner, at)| json!({"owner": owner, "pos": at}))
            .collect::<Vec<_>>(),
        // The handicap the game is being played on. The save list has always
        // reported this for games nobody is playing; without it here the setup
        // panel could not tell a reloaded page which difficulty the game on
        // screen was started at, and offered to restart it at the stock one.
        "difficulty": g.difficulty,
        "world_era": g.world_era,
        // The bomb is invented once, for the whole world. This says only that
        // somebody has finished the research that unlocks nuclear devices —
        // never who — so it is a fact about the age like `world_era` beside it
        // rather than a look inside a rival's tech tree. Nothing un-invents it:
        // a world that has built the bomb has built it even after the empire
        // that did is gone. Which research it is comes from the rules, so the
        // tech's name is not written down a second time here.
        "nuclear_weapons_unlocked": g.rules.projects.get("build_nuclear_device")
            .and_then(|spec| spec.tech.as_ref())
            .is_some_and(|tech| g.players.iter().any(|p| p.techs.contains(tech))),
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
            // Whether east and west are the same edge. A Tactics arena is the
            // one shape where they are not, and the browser cannot infer that
            // from `shape`: an arena is flat like every stock script and only
            // its walls tell it apart. Without this the chart unrolls a walled
            // field, and a unit at the east wall is drawn past the west one.
            "wrap_x": g.map.wraps_east_west(),
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
        },
        "visible": vis.iter().filter(|v| g.map.tiles.contains_key(v))
            .map(|v| json!([v.0, v.1])).collect::<Vec<_>>(),
        "turn_visible": turn_vis.iter().filter(|v| g.map.tiles.contains_key(v))
            .map(|v| json!([v.0, v.1])).collect::<Vec<_>>(),
        "camps": camps,
        "spies": spies,
        "me": {
            "team": p.team,
            "gold": round1(p.gold), "faith": round1(p.faith),
            "gold_per_turn": round1(p.gold_per_turn),
            "bankruptcy_amenity_penalty": p.bankruptcy_amenity_penalty,
            "techs": p.techs, "research": p.research,
            "research_progress": round1(p.research_progress),
            // A spectator watches from above the world rather than inside it,
            // so it is never the party that has to find north.
            "found_north": omniscient || p.techs.contains(&Name::new(NORTH_TECH)),
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
            // The middle rung: the outer system, and the neighbourhood with
            // real distances on it. Reported the same way and for the same
            // reason as the two either side of it.
            "sees_outer_system": omniscient || sees_outer_system(p),
            "outer_system_techs": OUTER_SYSTEM_TECHS,
            "outer_system_great_person": OUTER_SYSTEM_GREAT_PERSON,
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
            // What each met city-state is asking this civilization for, and
            // the Envoy it pays. Per pair: a rival's quest from the same
            // city-state is its own business.
            "city_state_quests": p.quests.iter().map(|(minor, quest)| {
                serde_json::json!({
                    "city_state": minor,
                    "kind": quest.kind,
                    "target": quest.target,
                    "name": Game::quest_name(&quest.kind),
                    "description": g.quest_description(quest),
                })
            }).collect::<Vec<_>>(),
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
            // Every Great Work this empire holds, as the piece it is — kind,
            // the era it was made in, who made it — so the Great Works
            // screen can say what the collection is rather than how big.
            "great_work_pieces": p.great_work_pieces,
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
            "denounced_since": p.denounced_since,
            "friends_until": p.friends_until,
            "open_borders_until": p.open_borders_until,
            "alliances": p.alliances,
            "diplomatic_missions": p.diplomatic_missions,
            "defensive_pacts": p.defensive_pacts,
            "promises": p.promises,
            "diplomatic_incidents": p.diplomatic_incidents,
            "broken_promises_until": p.broken_promises_until,
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
            "exoplanet_distance": round1(g.science_victory_points(pid)),
            "exoplanet_speed": round1(g.exoplanet_speed(pid)),
            // Which world this expedition is aimed at, and how much of the
            // neighbourhood this civilization has found. Before the launch the
            // target is what it *would* set out for, which is what makes the
            // survey legible while there is still time to deepen it.
            "exoplanet_target": g.exoplanet_target(pid).id,
            "exoplanet_target_name": g.exoplanet_target(pid).name,
            "exoplanet_target_ly": g.exoplanet_target(pid).light_years,
            "exoplanet_launched": p.exoplanet_target.is_some(),
            "exoplanet_surveyed": g.exoplanet_survey(pid).len(),
            "exoplanet_roster": EXOPLANET_TARGETS.len(),
            // The Moon, in the Modified Future Era. The piles are one shared
            // body rather than this civilization's holdings, so they are
            // reported to everybody who can see the Moon at all — knowing how
            // much a rival has already taken out is most of what makes the
            // race a race. Empty in a classic game, where the Moon has no ore.
            "moon_deposits": g.moon_ores().iter()
                .map(|ore| json!({"id": ore, "left": round1(g.moon_deposit(ore))}))
                .collect::<Vec<_>>(),
            "mass_drivers": g.mass_drivers(pid),
            "mass_driver_site": p.mass_driver_site,
            "mass_driver_ore": p.mass_driver_ore,
            "mass_driver_shots": p.mass_driver_shots,
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
            let city_ids = g.player_city_ids(o.id);
            let public_stats = g.observed_public_empire_stats.get(&o.id);
            let mut output = crate::rules::Yields::default();
            for &cid in &city_ids {
                output.add(g.city_yields(cid));
            }
            if let Some(adjustment) = g.observed_yield_adjustments.get(&o.id) {
                output.add(*adjustment);
            }
            output.add(g.player_yield_extras(o.id));
            // An arena grants Gold and Science to the side rather than to a
            // city, so summing cities alone would report zero of both while
            // the treasury filled and the tree opened — and would report a
            // side with no city as having no economy at all. Add the grants
            // where they are actually collected, so the panel says what the
            // turn is about to do.
            output.add(g.arena_side_yields(o.id));
            let modeled_population: i32 = city_ids
                .iter()
                .map(|&cid| g.cities[&cid].pop)
                .sum();
            let city_count = public_stats
                .and_then(|stats| stats.city_count)
                .unwrap_or(city_ids.len());
            let total_population = public_stats
                .and_then(|stats| stats.population)
                .unwrap_or(modeled_population);
            let military = g.military_power(o.id).round() as i64;
            // A finished device is an empire-scale fact of the same kind as
            // military strength, and it goes on the public ledger for the same
            // reason: the standings ribbon compares empires somebody has met.
            // The two classes are reported apart because they are two different
            // weapons — one blast ring against two, and a maintenance bill to
            // match — and the browser adds them for the one figure it shows.
            let devices = |key: &str| o.counters.get(key).copied().unwrap_or(0).max(0);
            // The FPT asterisk marks every civilization that founded a
            // religion. It used to require a city that still follows the
            // faith, and in a mirrored game the seat cannot see a fogged
            // rival city's religion: G235353Z at t149 marked Portugal (our
            // own cities are Catholic) and missed Nubia's Islam and Hungary's
            // Confucianism. Founding is public history, so the host's founder
            // list (`apply_player_religion`) is the whole answer. The key
            // keeps its name for recorded replays.
            let founded_religion_exists = o.religion.is_some();
            json!({
                "id": o.id, "civ": o.civ,
                "met": true,
                "leader": g.observed_leader_types.get(&o.id)
                    .map(|leader| leader
                        .trim_start_matches("LEADER_")
                        .replace('_', " ")
                        .to_lowercase()
                        .split_whitespace()
                        .map(|word| {
                            let mut chars = word.chars();
                            chars.next().map(|first| first.to_uppercase().collect::<String>()
                                + chars.as_str()).unwrap_or_default()
                        })
                        .collect::<Vec<_>>()
                        .join(" "))
                    .or_else(|| g.rules.civs.get(&o.civ).map(|c| c.leader.clone())),
                "leader_type": g.observed_leader_types.get(&o.id),
                // A leader's agenda is public knowledge in Civ VI once you
                // have met them, and so is roughly how they feel about you.
                "agenda": g.agenda_of(o.id).map(|agenda| json!({
                    "name": agenda.name,
                    "description": agenda.description,
                })),
                "opinion_of_me": round1(g.agenda_opinion(o.id, pid)),
                "relationship_opinion_of_me": round1(g.relationship_opinion(o.id, pid)),
                "alive": o.alive,
                "is_minor": o.is_minor,
                "is_barbarian": o.is_barbarian,
                "is_free_city": o.is_free_city,
                "cs_type": if o.is_minor && !o.is_barbarian {
                    Some(g.cs_type(&o.civ))
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
                "exoplanet_distance": round1(g.science_victory_points(o.id)),
                "government": o.government,
                "anarchy_turns": o.anarchy_turns,
                "age": o.age,
                "score": g.score(o.id),
                "cities": city_count,
                "population": total_population,
                "suzerain_count": public_stats
                    .and_then(|stats| stats.suzerain_count)
                    .unwrap_or_else(|| g.players.iter()
                        .filter(|minor| minor.alive && minor.is_minor && !minor.is_barbarian)
                        .filter(|minor| g.suzerain_of(minor.id) == Some(o.id))
                        .count()),
                "wonder_count": public_stats
                    .and_then(|stats| stats.wonder_count)
                    .unwrap_or_else(|| city_ids.iter()
                        .map(|city| g.cities[city].wonders.len())
                        .sum::<usize>()),
                "victories": if !o.is_minor && !o.is_barbarian {
                    Some(victory_progress_json(g, o.id, leading_score))
                } else {
                    None
                },
                "gold": round1(o.gold),
                "gold_per_turn": round1(o.gold_per_turn),
                "bankruptcy_amenity_penalty": o.bankruptcy_amenity_penalty,
                "faith": round1(o.faith),
                "founded_religion_exists": founded_religion_exists,
                "yields": yields_json(&output),
                "tourism_per_turn": round1(public_stats
                    .and_then(|stats| stats.tourism_per_turn)
                    .unwrap_or_else(|| g.tourism_per_turn(o.id))),
                "military": military,
                "nuclear_devices": public_stats
                    .and_then(|stats| stats.nuclear_devices)
                    .unwrap_or_else(|| devices("project_effect:nuclear_devices")),
                "thermonuclear_devices": public_stats
                    .and_then(|stats| stats.thermonuclear_devices)
                    .unwrap_or_else(|| devices("project_effect:thermonuclear_devices")),
                // ⚠ WHAT THE HOST SAID, kept apart from what CIVVIS MODELS.
                // `military` above is `military_power`, which is deliberately
                // `max(observed, our own strength sum)` — so for our OWN seat,
                // where we can see every unit, our sum can win the max and the
                // figure legitimately exceeds the host's. For a RIVAL the sum is
                // fog-limited, so the host's number wins.
                //
                // That makes `military` the wrong thing for `civ6_mirror_check`
                // to compare: it reported `seat 0 military Civ6=520 CIVVIS=545`
                // as a DISAGREEMENT when the mapping was perfect and only the
                // model differed. A check that cries wolf gets ignored.
                //
                // This is the mapped value alone, so the checker can verify the
                // BRIDGE (did the export land?) rather than the MODEL.
                "observed_military": g
                    .observed_military_power
                    .get(&o.id)
                    .copied()
                    .map(|value| value.round() as i64),
                // The domination lane's own number on the shipped overview:
                // the army without the treasury, where `military` folds Gold in.
                "military_no_treasury": public_stats
                    .and_then(|stats| stats.military_no_treasury)
                    .map(|value| value.round() as i64),
                "team": o.team,
                "teammate": g.same_team(pid, o.id),
                "at_war_with_me": g.is_at_war(pid, o.id),
                "grievances_against_me": o.grievances.get(&pid).copied().unwrap_or(0.0),
                "my_grievances": p.grievances.get(&o.id).copied().unwrap_or(0.0),
                "friend": g.are_friends(pid, o.id),
                "allied": g.are_allied(pid, o.id),
                "alliance": g.alliance_with(pid, o.id),
                "relationship": g.relationship_state(pid, o.id),
                "their_relationship": g.relationship_state(o.id, pid),
                "defensive_pact": g.defensive_pact_until(pid, o.id),
                // Keep the old delegation-named fields for API consumers,
                // but publish neutral names too: an Embassy replaces a
                // Delegation and is not a second kind of delegation.
                "mission_to_them": g.diplomatic_mission_to(pid, o.id),
                "mission_to_me": g.diplomatic_mission_to(o.id, pid),
                "delegation_to_them": g.diplomatic_mission_to(pid, o.id),
                "delegation_to_me": g.diplomatic_mission_to(o.id, pid),
                "diplomatic_visibility": round1(g.diplomatic_visibility(pid, o.id)),
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
        "wars": wars_json(g, &explored),
        // Every detonation, newest last. Shown whole for the same reason wars
        // are: a mushroom cloud is not a thing one civilization keeps to
        // itself, and a client needs the account to place the blast on the map
        // rather than inferring one from a ring of fallout.
        "nuclear_strikes": nuclear_strikes_json(g),
        // Where each unit has walked lately, oldest first. The client draws
        // these as movement tails and walks its move animations along them.
        // Unlike wars and strikes this is exactly the kind of thing fog is
        // for, so a seated view gets its own routes whole and everyone
        // else's clipped to the stretches it can currently see.
        "unit_move_trails": unit_move_trails_json(g, pid, omniscient, &vis),
        "winner": g.winner,
        "winners": g.winning_players(),
        // A terminal Tactics draw has no winner, so clients must not infer
        // liveness from `winner` alone. `draw` makes that one result explicit;
        // `finished` is the generic lifecycle answer every driver needs.
        "finished": g.is_finished(),
        "draw": g.is_draw(),
        "victory_type": g.victory_type,
        // How that result is written: the type for every ordinary victory, and
        // the Mercy Rule's lane notation for a mercy ending, composed here so
        // every surface says the same thing about the same game. Empty while a
        // game is live, exactly as `victory_type` is.
        "victory_label": g.victory_label(),
        // The turn a finished game is reported on, which is usually the live
        // turn of a victory. A score tiebreak or Tactics draw is settled on
        // the wrap out of the final turn, so a 250-turn game reads turn 250
        // and not the turn 251 nobody plays. Empty while a game is live.
        "victory_turn": g.is_finished().then(|| g.reported_turn()),
        // The result this world was already given, if it was asked for one
        // more turn. The game is live again, so `winner` is empty; this is how
        // a viewer is still told whose victory the extra turns are borrowed
        // from, and what can end the continuation.
        "decided": g.decided.as_ref().map(|decided| json!({
            "winner": decided.winner,
            "civ": g.players.get(decided.winner).map(|player| player.civ.clone()),
            "victory_type": decided.victory_type,
            "victory_label": decided.victory_label(),
            "turn": decided.turn,
            "mode": decided.mode.as_str(),
        })),
        // What has happened to this civilization lately, newest last. An
        // omniscient viewer watches whichever seat it is observing, so the
        // spectator log follows the same seat as the rest of the frame.
        "events": recent_events(g, pid, omniscient),
    });
    observation["map"]["tiles"] = Value::Array(tiles);
    observation["units"] = Value::Array(units);
    observation["cities"] = Value::Array(cities);
    observation
}

/// Every conflict in progress, longest-running first, followed by the most
/// recently concluded conflicts. A declaration can open several bilateral
/// fronts through teams and defensive alliances; `WarRecord::conflict` folds
/// those fronts into one durable, Wikipedia-style account here rather than
/// asking each browser to guess which records belong together.
fn wars_json(g: &Game, explored: &BTreeSet<Pos>) -> Vec<Value> {
    const RECENT_CONFLICTS: usize = 12;
    let mut grouped: BTreeMap<u32, Vec<&crate::game::WarRecord>> = BTreeMap::new();
    for war in g.wars.values().chain(g.concluded_wars.iter()) {
        grouped.entry(war.conflict).or_default().push(war);
    }
    let mut conflicts: Vec<Vec<&crate::game::WarRecord>> = grouped.into_values().collect();
    conflicts.sort_by_key(|records| {
        let ongoing = records.iter().any(|war| war.ended.is_none());
        let started = records.iter().map(|war| war.started).min().unwrap_or(0);
        let ended = records
            .iter()
            .filter_map(|war| war.ended)
            .max()
            .unwrap_or(0);
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
            let casus_belli = records.iter().find_map(|war| war.casus_belli.as_deref());
            let joint_war_until = records.iter().filter_map(|war| war.joint_war_until).max();

            let mut losses: BTreeMap<usize, crate::game::WarLosses> = BTreeMap::new();
            for war in &records {
                for (player, toll) in &war.losses {
                    let total = losses.entry(*player).or_default();
                    total.units += toll.units;
                    total.cities += toll.cities;
                    total.city_names.extend(toll.city_names.iter().cloned());
                    total.city_losses.extend(toll.city_losses.iter().cloned());
                    for (kind, count) in &toll.unit_kinds {
                        *total.unit_kinds.entry(kind.clone()).or_insert(0) += count;
                    }
                }
            }

            // One belligerent is one entry, however many fronts it fought on and
            // however many times it entered. A participant appearing in several
            // fronts at once is the same interval seen twice; a city-state whose
            // Suzerain changes leaves the war and comes back, which is the same
            // belligerent with a second interval. Both merge here so the log can
            // give an entity one section listing its whole involvement, and so
            // the effort it spent is counted per unit across every interval
            // rather than once per row.
            let mut participation: BTreeMap<(usize, bool), Vec<&crate::game::WarParticipation>> =
                BTreeMap::new();
            for participant in records.iter().flat_map(|war| &war.participants) {
                participation
                    .entry((participant.player, participant.declarer_side))
                    .or_default()
                    .push(participant);
            }
            let mut side_losses = [
                crate::game::WarLosses::default(),
                crate::game::WarLosses::default(),
            ];
            for (player, toll) in &losses {
                let declarer_side = participation
                    .keys()
                    .find(|(participant, _)| participant == player)
                    .map(|(_, side)| *side)
                    .unwrap_or(*player == anchor.declarer);
                let total = &mut side_losses[if declarer_side { 0 } else { 1 }];
                total.units += toll.units;
                total.cities += toll.cities;
            }
            let parties = participation
                .into_iter()
                .map(|((player, declarer_side), entries)| {
                    // Keyed by entry turn, so the two copies of one interval a
                    // pair of fronts produces collapse while a real re-entry
                    // stays its own interval. Still being in on one front is
                    // still being in.
                    let mut intervals: BTreeMap<u32, Option<u32>> = BTreeMap::new();
                    let mut saw_action_units: BTreeMap<u32, i64> = BTreeMap::new();
                    let mut peak_strength: Option<i64> = None;
                    for participant in &entries {
                        intervals
                            .entry(participant.entered)
                            .and_modify(|exited| {
                                *exited = match (*exited, participant.exited) {
                                    (Some(first), Some(second)) => Some(first.max(second)),
                                    _ => None,
                                };
                            })
                            .or_insert(participant.exited);
                        peak_strength = match (peak_strength, participant.peak_strength) {
                            (Some(first), Some(second)) => Some(first.max(second)),
                            (known, None) | (None, known) => known,
                        };
                        for (unit, value) in &participant.saw_action_units {
                            saw_action_units
                                .entry(*unit)
                                .and_modify(|known| *known = (*known).max(*value))
                                .or_insert(*value);
                        }
                    }
                    let entered = intervals.keys().next().copied().unwrap_or(started);
                    // Strength on first entering the war, not the largest of
                    // several entries: this is the "before" of the comparison.
                    let strength = entries
                        .iter()
                        .filter(|participant| participant.entered == entered)
                        .map(|participant| participant.strength)
                        .max()
                        .unwrap_or(0);
                    // A belligerent has left only if its latest interval closed.
                    let exited = intervals.values().next_back().copied().flatten();
                    let suzerain = entries
                        .iter()
                        .filter(|participant| participant.entered == entered)
                        .find_map(|participant| participant.suzerain)
                        .or_else(|| entries.iter().find_map(|participant| participant.suzerain));
                    let toll = losses.get(&player).cloned().unwrap_or_default();
                    let saw_action_strength = saw_action_units.values().sum::<i64>();
                    json!({
                        "player": player,
                        "declarer_side": declarer_side,
                        "suzerain": suzerain,
                        "entered": entered,
                        "exited": exited,
                        "intervals": intervals
                            .iter()
                            .map(|(entered, exited)| json!({
                                "entered": entered,
                                "exited": exited,
                            }))
                            .collect::<Vec<_>>(),
                        "strength": strength,
                        "strength_start": strength,
                        "strength_peak": peak_strength,
                        "strength_saw_action": saw_action_strength,
                        "units_lost": toll.units,
                        "cities_lost": toll.cities,
                        "unit_kinds": toll.unit_kinds,
                        "city_names": toll.city_names,
                        "city_losses": toll.city_losses,
                    })
                })
                .collect::<Vec<_>>();

            let mut highlights = records
                .iter()
                .flat_map(|war| war.highlights.iter())
                .collect::<Vec<_>>();
            highlights.sort_by_key(|moment| {
                (
                    moment.turn,
                    moment.kind.as_str(),
                    moment.actor,
                    moment.subject,
                )
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
                    .entry((
                        peace.turn,
                        peace.first,
                        peace.second,
                        peace.terms.join("\u{1f}"),
                    ))
                    .or_insert(peace);
            }
            // The ledger's bounded action tail remains in the save, but the
            // browser needs the current/final theater rather than a map-wide
            // tour of every place a long war ever touched.  Keep the last
            // visit to each explored site, then the last eight turns of action
            // overall; after the war ends this exact tail remains stable while
            // the world continues around it.
            let mut theater_by_pos = BTreeMap::new();
            for site in records
                .iter()
                .flat_map(|war| war.theater.iter().copied())
                // A public war is not public reconnaissance.  A seated player
                // may return only to battlefield ground their civilization has
                // actually explored; the omniscient spectator's explored set
                // already contains the complete world.
                .filter(|site| explored.contains(&site.pos))
            {
                theater_by_pos
                    .entry(site.pos)
                    .and_modify(|turn: &mut u32| *turn = (*turn).max(site.turn))
                    .or_insert(site.turn);
            }
            let mut theater = theater_by_pos
                .into_iter()
                .map(|(pos, turn)| crate::game::WarTheaterSite { turn, pos })
                .collect::<Vec<_>>();
            theater.sort_by_key(|site| (site.turn, site.pos));
            if let Some(latest) = theater.last().map(|site| site.turn) {
                let cutoff = latest.saturating_sub(8);
                theater.retain(|site| site.turn >= cutoff);
            }
            const THEATER_SITES_SENT: usize = 24;
            if theater.len() > THEATER_SITES_SENT {
                theater.drain(..theater.len() - THEATER_SITES_SENT);
            }
            json!({
                "conflict": anchor.conflict,
                "aggressor": anchor.declarer,
                "defender": anchor.target,
                "casus_belli": casus_belli,
                "joint_war_until": joint_war_until,
                "started": started,
                "ended": ended,
                "turns": ended.unwrap_or(g.turn).saturating_sub(started),
                "outcome": outcome,
                "victor": victor,
                "theater": theater.into_iter().map(|site| json!({
                    "turn": site.turn,
                    "pos": [site.pos.0, site.pos.1],
                })).collect::<Vec<_>>(),
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
                // Whose event this is. The omniscient feed rotates through the
                // seats, so "the civilization being observed" is a different
                // answer on the next frame and the browser cannot infer this
                // from the frame it arrived on — which is what the event log's
                // civ filter needs, and the text only spells out in prose.
                "player": event.player,
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

/// The walked-route ledger, through the viewer's fog. The omniscient
/// spectator and a unit's own player read a route whole; anyone else gets it
/// cut down to the visible stretches, each emitted as its own trail so a
/// client never has to reason about gaps — a route that dips into fog simply
/// arrives as two shorter ones.
fn unit_move_trails_json(
    g: &Game,
    pid: usize,
    omniscient: bool,
    vis: &BTreeSet<Pos>,
) -> Vec<Value> {
    let trail_json = |trail: &UnitMoveTrail, path: &[Pos]| {
        json!({
            "unit": trail.unit,
            "owner": trail.owner,
            "turn": trail.turn,
            "path": path.iter().map(|p| [p.0, p.1]).collect::<Vec<_>>(),
        })
    };
    let mut out = Vec::new();
    for trail in &g.unit_move_trails {
        if omniscient || trail.owner == pid {
            out.push(trail_json(trail, &trail.path));
            continue;
        }
        let mut run: Vec<Pos> = Vec::new();
        for &pos in &trail.path {
            if vis.contains(&pos) {
                run.push(pos);
                continue;
            }
            if run.len() > 1 {
                out.push(trail_json(trail, &run));
            }
            run.clear();
        }
        if run.len() > 1 {
            out.push(trail_json(trail, &run));
        }
    }
    out
}

/// Public victory-screen metrics. Each progress value is normalized to
/// 0..100 for sorting and meter width, while the underlying counts let the UI
/// describe the actual victory requirement instead of showing a vague percent.
fn victory_progress_json(g: &Game, pid: usize, leading_score: i64) -> Value {
    // The arithmetic lives on `Game` so the victory tracker and the AI read the
    // same standings; this only formats them.
    let r = g.victory_races(pid, leading_score);
    json!({
        "science": {
            "progress": round1(r.science),
            "projects": r.science_projects,
            "project_target": r.science_project_target,
            "distance": round1(r.exoplanet_distance),
            "distance_target": round1(r.exoplanet_distance_target),
            "speed": round1(r.exoplanet_speed),
            "techs": r.techs,
            "tech_total": r.tech_total,
        },
        "culture": {
            "progress": round1(r.culture),
            "tourists": r.foreign_tourists,
            "target": r.culture_target,
            "civics": r.civics,
            "civic_total": r.civic_total,
            "domestic": r.domestic_tourists,
            "rival_domestic": r.rival_domestic,
            "leading_domestic": r.leading_domestic,
        },
        "religious": {
            "progress": round1(r.religious),
            "converted": r.converted_civs,
            "target": r.religious_target,
            "cities_following": r.cities_following_religion,
        },
        "diplomatic": {
            "progress": round1(r.diplomatic),
            "points": r.diplomatic_points,
            "target": DIPLOMATIC_VICTORY_POINTS,
        },
        "domination": {
            "progress": round1(r.domination),
            "capitals": r.controlled_capitals,
            "target": r.capital_target,
        },
        "score": {
            "progress": round1(r.score),
            "points": r.score_points,
            "leader": leading_score,
        },
    })
}

/// The resources a tile view may name. A seated player is shown what its own
/// research has uncovered; the omniscient spectator is shown what the *first*
/// civilization to get there has uncovered, so an Iron deposit is nowhere on
/// the world map until somebody researches Bronze Working. Tournament Civ VI
/// hands its spectator every deposit at once, but a map that fills in as the
/// world learns to read it is the more honest picture of what the players are
/// actually deciding on.
///
/// Only majors count: a city-state never leads anyone to a resource. Whether
/// the discoverer is still alive does not matter — knowledge does not leave
/// the world with the civilization that found it, and a deposit that vanished
/// when its finder was conquered would be a very odd map.
///
/// Computed once per observation because it is read for every tile on the map.
fn revealed_resources(g: &Game, pid: usize, omniscient: bool) -> BTreeSet<&str> {
    g.rules
        .resources
        .keys()
        .filter(|resource| {
            if omniscient {
                g.players
                    .iter()
                    .filter(|player| !player.is_minor && !player.is_barbarian)
                    .any(|player| g.resource_visible_to(player.id, resource))
            } else {
                g.resource_visible_to(pid, resource)
            }
        })
        .map(|name| name.as_str())
        .collect()
}

fn tile_json(
    g: &Game,
    tile: &Tile,
    owner: Option<usize>,
    revealed: &BTreeSet<&str>,
    live: bool,
    tourism_by_tile: &BTreeMap<Pos, f64>,
    planned: Option<&str>,
) -> Value {
    let resource = tile
        .resource
        .as_deref()
        .filter(|resource| revealed.contains(resource));
    // Adjacency is read off the *current* neighbors, so it may only be sent
    // for a tile being looked at right now. A remembered district would
    // otherwise report yields from tiles the player cannot see.
    let district_yields = tile
        .district
        .filter(|district| live && g.rules.districts.contains_key(district.as_str()))
        .map(|district| {
            (
                g.district_yields(Name::new(district.as_str()), tile.pos),
                g.district_adjacency_sources(Name::new(district.as_str()), tile.pos),
            )
        });
    let (district_yields, adjacency) = match district_yields {
        Some((yields, sources)) => (json!(yields), json!(sources)),
        None => (Value::Null, Value::Null),
    };
    // A district still in the production queue reports what its site would pay
    // today — the figure a player is actually deciding on.
    let planned = planned
        .filter(|district| tile.district.is_none() && g.rules.districts.contains_key(district))
        .map(|district| {
            json!({
                "district": district,
                "yields": g.district_yields(Name::new(district), tile.pos),
                "adjacency": g.district_adjacency_sources(Name::new(district), tile.pos),
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
    // A visible border identifies the city that owns it. Remembered tiles
    // retain only the city id, so a client may join it to a city the viewer
    // already knows without learning a current name through fog of war.
    let owner_city_name = live
        .then(|| {
            tile.owner_city
                .and_then(|city| g.cities.get(&city))
                .map(|city| city.name.as_str())
        })
        .flatten();
    // ★★★★★ THE ENGINE'S OWN NUMBER FOR THIS PLOT, SO THE BOARD CANNOT DISAGREE
    // WITH THE GAME IT IS DRAWING.
    //
    // The client used to derive the pips and the tooltip itself — terrain plus
    // Hills plus feature plus resource plus improvement, read out of the shipped
    // ruleset. That sum is not what a citizen collects, and it is silently short
    // by every term the ground carries and the catalogue does not:
    //
    //   - the fertility a flood or an eruption left behind (`disaster_yields`,
    //     exported here since the disasters shipped and never once added up);
    //   - the host's own per-plot reading on a mirrored board
    //     (`observed_tile_yield_adjustments`), which is the only place a live
    //     game's Volcanic Soil is written down at all;
    //   - a neighbouring feature's `adjacent_yields` (Vesuvius, Yosemite);
    //   - drought's -1 Food, and the nothing-at-all a flooded, irradiated or
    //     just-placed-district plot pays;
    //   - a pillaged improvement, which the client paid in full.
    //
    // The operator's report was a 3 Food 3 Production tile on the live map that
    // the board drew as 2 Food 2 Production, after a volcano went off beside it.
    // `Game::workable_tile_yields` is the function the citizen planner and the
    // city's own yields go through, so publishing it is what makes the two agree
    // by construction instead of by two catalogues staying in step.
    //
    // ⚠ GROUND ONLY — no district, no wonder. `Rules::worked_tile_yields`
    // stops at the improvement and the client adds `district_yields`,
    // `planned_district` and the wonder on top; sending a total that already
    // held them would count them twice.
    //
    // Live tiles only, for the same reason as `appeal` and `adjacency` above: a
    // remembered tile is a copy of what the seat once saw, and the engine's
    // answer is about the plot as it stands now.
    let yields = if live && g.map.tiles.contains_key(&tile.pos) {
        json!(g.workable_tile_yields(tile.pos))
    } else {
        Value::Null
    };
    json!({
        "pos": [tile.pos.0, tile.pos.1],
        "terrain": tile.terrain,
        "yields": yields,
        "appeal": appeal,
        "tourism": live.then(|| round1(tourism_by_tile.get(&tile.pos).copied().unwrap_or(0.0))),
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
        "owner_city": tile.owner_city,
        "owner_city_name": owner_city_name,
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
    defending_districts: &'a [crate::game::DefendingDistrict],
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
        "defending_districts": city.defending_districts.iter().map(|d| json!({
            "kind": d.kind, "pos": d.pos, "hp": d.hp, "wall_hp": d.wall_hp, "pillaged": d.pillaged,
        })).collect::<Vec<_>>(),
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
        defending_districts: &city.defending_districts,
        religion: city.religion.as_deref(),
    })
}

/// The Governor this viewer has posted to a city, including whether that
/// Governor's local effects are live yet. Assignments belong to the player
/// rather than to public map knowledge, so the omniscient spectator does not
/// need a city-by-city copy of them; a player does, including Amani's posting
/// to a city-state the player can currently see.
fn viewer_governor_json(g: &Game, pid: usize, city: u32, omniscient: bool) -> Value {
    if omniscient {
        return Value::Null;
    }
    let Some((id, state)) = g.players[pid]
        .governor_roster
        .iter()
        .find(|(_, governor)| governor.city == Some(city))
    else {
        return Value::Null;
    };
    let Some(spec) = g.rules.governors.get(id) else {
        return Value::Null;
    };
    let establishes_turn = state.assigned_turn + g.standard_duration(spec.establish_turns);
    let active_turn = establishes_turn.max(state.disabled_until);
    let status = if state.disabled_until > g.turn {
        "disabled"
    } else if establishes_turn > g.turn {
        "establishing"
    } else {
        "established"
    };
    json!({
        "id": id,
        "name": spec.name,
        "title": spec.title,
        "status": status,
        "established": status == "established",
        "active_turn": active_turn,
        "turns_remaining": active_turn.saturating_sub(g.turn),
        "promotions": state.promotions,
    })
}

fn live_city_json(
    g: &Game,
    pid: usize,
    city: &City,
    omniscient: bool,
    visible: &BTreeSet<Pos>,
) -> Value {
    let memory =
        (city.owner != pid && !omniscient).then(|| g.remember_city_for_viewer(city, pid, visible));
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
        defending_districts: memory
            .as_ref()
            .map_or(city.defending_districts.as_slice(), |known| {
                known.defending_districts.as_slice()
            }),
        religion: g.city_religion(city),
    });
    // Religious pressure is visible with the city itself. Remembered cities
    // intentionally omit it, so a lens never reveals conversions under fog.
    value["religious_pressure"] = json!(city.pressure);
    value["atheist_pressure"] = json!(round1(city.atheist_pressure));
    let governor_assignment = viewer_governor_json(g, pid, city.id, omniscient);
    if !governor_assignment.is_null() {
        value["governor_assignment"] = governor_assignment;
    }
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
        "defense": round1(g.city_strength(city.id)),
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
mod tests;
