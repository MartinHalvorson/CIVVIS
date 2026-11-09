//! Our cities: writing what the seat reports about its own cities into the
//! rebuilt game.
//!
//! `apply_observed_city_economy` and `apply_observed_city_infrastructure`
//! (the host's yields, buildings and districts), `apply_observed_city_facts`,
//! `apply_host_district_progress`, city and Encampment health, a city's
//! religion, and pillaged buildings.
//!
//! Moved verbatim out of `mirror.rs` (2026-11-05). `mirror.rs` re-exports
//! everything here, so every path is unchanged. Items the rest of `mirror`
//! uses are `pub(super)`; the code is otherwise unchanged.

use super::*;

/// Bind native district work only to the city's exact observed foundation.
/// The ordinary menu sites are possible NEW placements, not invested plots.
/// Unit formation balances and unfinished Wonder locations have different
/// identity rules and are deliberately not inferred here.
pub(super) fn apply_host_district_progress(
    game: &mut crate::game::Game,
    cities: &[StateCity],
    city_ids: &BTreeMap<u32, i64>,
) {
    use crate::game::{Game, Item};
    for (cid, host_id) in city_ids {
        let Some(state) = cities.iter().find(|city| city.id == *host_id) else {
            continue;
        };
        for observed in &state.districts {
            let Some(name) = civvis_node_name(&game.rules.districts, &observed.kind, "DISTRICT_")
            else {
                continue;
            };
            let district = crate::name::Name::new(&name);
            let pos = crate::hex::offset_to_axial(observed.x, observed.y);
            let item = Item::District { district, pos };
            let key = Game::item_progress_key(&item);
            let active_head = game.cities[cid].queue.first() == Some(&item);
            if observed.complete || active_head {
                // Current-head work already lives in City::production. Never
                // add the same total again as a paused investment.
                game.cities
                    .get_mut(cid)
                    .unwrap()
                    .production_progress
                    .remove(&key);
                if observed.complete {
                    continue;
                }
            }
            let unique = state
                .districts
                .iter()
                .filter(|other| {
                    civvis_node_name(&game.rules.districts, &other.kind, "DISTRICT_").as_deref()
                        == Some(name.as_str())
                })
                .count()
                == 1;
            let exact_foundation = game.map.tiles.get(&pos).is_some_and(|tile| {
                tile.owner_city == Some(*cid)
                    && tile
                        .district_foundation
                        .as_ref()
                        .is_some_and(|foundation| foundation.district == district)
            });
            if !unique || !exact_foundation {
                continue;
            }
            // Prefer the current positive menu. A tail row can also carry an
            // observation, but a duplicate/conflicting type is not an identity.
            let rows = state
                .buildable
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .filter(|row| row.t.eq_ignore_ascii_case(&observed.kind))
                .collect::<Vec<_>>();
            let menu_progress = (rows.len() == 1).then(|| rows[0].pr).flatten();
            let tail = state
                .queue
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .filter(|row| row.t.eq_ignore_ascii_case(&observed.kind))
                .collect::<Vec<_>>();
            let tail_progress = (tail.len() == 1).then(|| tail[0].pr).flatten();
            let reading = |value: f64| value.is_finite() && value >= 0.0;
            let Some(progress) = menu_progress
                .filter(|value| reading(*value))
                .or_else(|| tail_progress.filter(|value| reading(*value)))
            else {
                continue; // Missing is unknown, not a fabricated zero or ETA-derived work.
            };
            // Fresh reconstruction plants the foundation before the host menu
            // arrives. Its model-derived locked cost must not override the
            // native price paired with this exact work observation. The host
            // keeps the placed Campus quote fixed as research advances.
            if rows.len() == 1 && rows[0].c.is_finite() && rows[0].c > 0.0 {
                game.map
                    .tiles
                    .get_mut(&pos)
                    .unwrap()
                    .district_foundation
                    .as_mut()
                    .unwrap()
                    .cost = rows[0].c;
            }
            if active_head {
                continue;
            }
            let saved = &mut game.cities.get_mut(cid).unwrap().production_progress;
            if progress == 0.0 {
                saved.remove(&key);
            } else {
                saved.insert(key, progress);
            }
        }
    }
}

pub(super) fn apply_city_religion(live: &mut crate::game::City, state: &StateCity) {
    // `City::pressure` is the model's only existing representation of a
    // conversion in progress.  The host gives us the more useful clock, but
    // not the individual pressure values, so retain the exact majority above
    // and add only a bounded warning signal for a flip that is close enough to
    // require a response.  A marker of 1.0 keeps a religionless city below its
    // 50-point atheist pressure and therefore cannot invent a majority; when a
    // city already has a majority, 60.0 is the smallest value that reaches
    // AdvancedAi's existing 60%-of-top-pressure defense threshold while the
    // observed majority remains at 100.0.
    const ACTIONABLE_CONVERSION_TURNS: i64 = 20;
    const RELIGIONLESS_CONVERSION_MARKER: f64 = 1.0;
    const MAJORITY_CONVERSION_MARKER: f64 = 60.0;

    let current = state.religion.as_deref().and_then(civvis_religion_name);
    // `founder-holds-the-holy-city` reads the host's next faith raw, before
    // the conversion clock below decides whether it is actionable.
    live.religion_next_seen = state
        .religion_next
        .as_deref()
        .and_then(civvis_religion_name)
        .filter(|next| current.as_deref() != Some(next.as_str()));
    live.pressure.clear();
    match current.as_ref() {
        Some(religion) => {
            live.atheist_pressure = 0.0;
            live.pressure.insert(religion.clone(), 100.0);
        }
        None => live.atheist_pressure = 50.0,
    }

    let Some(next) = state
        .religion_next
        .as_deref()
        .and_then(civvis_religion_name)
    else {
        return;
    };
    if current.as_deref() == Some(next.as_str())
        || state.religion_turns < 0
        || state.religion_turns > ACTIONABLE_CONVERSION_TURNS
    {
        return;
    }
    let marker = if current.is_some() {
        MAJORITY_CONVERSION_MARKER
    } else {
        RELIGIONLESS_CONVERSION_MARKER
    };
    live.pressure.insert(next, marker);
}

/// Apply a city's districts and wonders to both representations CIVVIS uses.
/// City collections drive yields; tile fields drive placement and Builder legality.
/// Give a mirrored city's Encampment the health Firaxis reports for it.
///
/// ★★★★★ WITHOUT THIS THE CITY BUILDS NOTHING, FOREVER. `City::encampment_hp`
/// defaults to 0 and nothing ever set it on a reconstructed board.
/// `Game::can_produce` gates `repair_encampment` on `encampment_hp < 100`, so
/// that test passed permanently for every city holding an Encampment: the AI
/// queued the repair every turn, `civvis_orders` correctly declined to translate
/// a project Civilization VI does not have, the order was discarded, and no
/// other production was chosen for that city. Two of five cities in live run
/// `civvis-20260810T040916Z` produced nothing from turn 67 to turn 188 this way.
///
/// The **default when the host does not answer is full health, not zero** — that
/// asymmetry is the whole point. A wrong "healthy" costs one skipped repair; a
/// wrong "destroyed" costs the city's entire production for the rest of the game.
pub(super) fn apply_encampment_health(game: &mut crate::game::Game, state: &StateCity, cid: u32) {
    let Some(city) = game.cities.get(&cid) else {
        return;
    };
    let max_wall = game.city_max_wall_hp(city);
    let forts: Vec<_> = state
        .districts
        .iter()
        .filter(|d| d.complete)
        .filter_map(|district| {
            let kind = civvis_node_name(&game.rules.districts, &district.kind, "DISTRICT_")?;
            let kind = crate::name::Name::new(&kind);
            let pos = crate::hex::offset_to_axial(district.x, district.y);
            (game.district_has_defenses(kind)
                && city.districts.iter().any(|(k, p)| *k == kind && *p == pos))
            .then_some((kind, pos, district))
        })
        .collect();
    if !forts
        .iter()
        .any(|(kind, _, _)| game.district_family(*kind) == crate::name!("encampment"))
    {
        game.cities.get_mut(&cid).unwrap().encampment_hp = 100;
    }
    for (kind, pos, district) in forts {
        let mut observed = game
            .defending_district_state(cid, pos)
            .unwrap_or_else(|| crate::game::DefendingDistrict::healthy(kind, pos, max_wall));
        observed.pillaged = district.pillaged;
        observed.hp = district_garrison_hp(district.damage, district.max_damage);
        observed.wall_hp =
            district_outer_hp(district.wall_damage, district.max_wall_damage, max_wall);
        game.set_defending_district_state(cid, observed);
    }
    retain_observed_defending_districts(game, cid);
}

/// Carry the host's per-building pillage state onto a reconstructed city.
///
/// The engine already refuses to pay a building in `pillaged_buildings`
/// (yields, housing, amenities, great-work slots all test it), so this is the
/// import matching a rule that was there; only the export was missing. An
/// older export (`None`) leaves the city's own set alone rather than clearing
/// it — unknown is not "nothing pillaged".
pub(super) fn apply_pillaged_buildings(
    rules: &crate::rules::Rules,
    city: &mut crate::game::City,
    state: &StateCity,
) {
    let Some(pillaged) = &state.pillaged_buildings else {
        return;
    };
    city.pillaged_buildings.clear();
    for civ6 in pillaged {
        if let Some(name) = civvis_node_name(&rules.buildings, civ6, "BUILDING_") {
            if name == "palace" {
                continue;
            }
            let named = crate::name::Name::new(&name);
            if city.buildings.contains(&named) {
                city.pillaged_buildings.insert(named);
            }
        }
    }
}

pub(super) fn apply_observed_city_infrastructure(
    game: &mut crate::game::Game,
    cid: u32,
    state: &StateCity,
    unmapped: &mut Vec<String>,
) {
    // Rival/public city records do not carry this private roster, and the own-city
    // exporter also leaves both fields absent when Firaxis refuses the plot query.
    // A living Civ VI city cannot lose a completed district or wonder, so an empty
    // observation is not authority to erase remembered infrastructure.
    if state.districts.is_empty() && state.wonders.is_empty() {
        for civ6 in &state.buildings {
            if civvis_node_name(&game.rules.buildings, civ6, "BUILDING_").is_none()
                && civvis_node_name(&game.rules.wonders, civ6, "BUILDING_").is_some()
            {
                let issue = format!("{civ6}:wonder_missing_plot");
                if !unmapped.contains(&issue) {
                    unmapped.push(issue);
                }
            }
        }
        return;
    }
    let Some(city) = game.cities.get(&cid) else {
        return;
    };
    let owner = city.owner;
    let old_districts: Vec<(crate::name::Name, crate::Pos)> = city
        .districts
        .iter()
        .map(|(name, pos)| (*name, *pos))
        .collect();
    let old_wonders: Vec<(crate::name::Name, crate::Pos)> = city
        .wonders
        .iter()
        .map(|(name, pos)| (*name, *pos))
        .collect();
    let old_foundations: Vec<(crate::name::Name, crate::Pos)> = city
        .queue
        .iter()
        .filter_map(|item| match item {
            crate::game::Item::District { district, pos } => Some((*district, *pos)),
            _ => None,
        })
        .collect();

    // Clear only markers this city previously supplied. owner_city can change
    // during capture reconciliation, so it cannot identify infrastructure safely.
    for (name, pos) in old_districts {
        if let Some(tile) = game.map.tiles.get_mut(&pos) {
            if tile.district.as_ref() == Some(&name) {
                tile.district = None;
                tile.pillaged = false;
            }
        }
    }
    for (name, pos) in old_foundations {
        if let Some(tile) = game.map.tiles.get_mut(&pos) {
            if tile
                .district_foundation
                .as_ref()
                .is_some_and(|foundation| foundation.district == name)
            {
                tile.district_foundation = None;
            }
        }
    }
    for (name, pos) in old_wonders {
        if let Some(tile) = game.map.tiles.get_mut(&pos) {
            if tile.wonder.as_ref() == Some(&name) {
                tile.wonder = None;
            }
        }
    }

    let remember_issue = |unmapped: &mut Vec<String>, issue: String| {
        if !unmapped.contains(&issue) {
            unmapped.push(issue);
        }
    };
    let mut completed = Vec::new();
    let mut foundations = Vec::new();
    for district in &state.districts {
        if district.kind.eq_ignore_ascii_case("DISTRICT_CITY_CENTER")
            || district.kind.eq_ignore_ascii_case("DISTRICT_WONDER")
        {
            continue;
        }
        let Some(name) = civvis_node_name(&game.rules.districts, &district.kind, "DISTRICT_")
        else {
            remember_issue(unmapped, format!("{}:district", district.kind));
            continue;
        };
        let pos = crate::hex::offset_to_axial(district.x, district.y);
        if !game.map.tiles.contains_key(&pos) {
            remember_issue(
                unmapped,
                format!(
                    "{}@{},{}:district_plot_missing",
                    district.kind, district.x, district.y
                ),
            );
            continue;
        }
        let observed = (crate::name::Name::new(&name), pos, district.pillaged);
        if district.complete {
            completed.push(observed);
        } else {
            foundations.push(observed);
        }
    }

    let mut wonders = Vec::new();
    for wonder in &state.wonders {
        let Some(name) = civvis_node_name(&game.rules.wonders, &wonder.kind, "BUILDING_") else {
            remember_issue(unmapped, format!("{}:wonder", wonder.kind));
            continue;
        };
        let pos = crate::hex::offset_to_axial(wonder.x, wonder.y);
        if !game.map.tiles.contains_key(&pos) {
            remember_issue(
                unmapped,
                format!(
                    "{}@{},{}:wonder_plot_missing",
                    wonder.kind, wonder.x, wonder.y
                ),
            );
            continue;
        }
        wonders.push((crate::name::Name::new(&name), pos));
    }
    for civ6 in &state.buildings {
        if civvis_node_name(&game.rules.buildings, civ6, "BUILDING_").is_none() {
            if let Some(name) = civvis_node_name(&game.rules.wonders, civ6, "BUILDING_") {
                if !wonders.iter().any(|(wonder, _)| wonder == &name) {
                    remember_issue(unmapped, format!("{civ6}:wonder_missing_plot"));
                }
            }
        }
    }

    if let Some(city) = game.cities.get_mut(&cid) {
        city.districts.clear();
        city.wonders.clear();
        for (name, pos, _) in &completed {
            city.districts.insert(*name, *pos);
        }
        for (name, pos) in &wonders {
            city.wonders.insert(*name, *pos);
        }
    }

    apply_encampment_health(game, state, cid);

    for (name, pos, pillaged) in completed {
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.improvement = None;
        tile.district = Some(name);
        tile.district_foundation = None;
        tile.wonder = None;
        tile.pillaged = pillaged;
    }
    for (name, pos, _) in foundations {
        let item = crate::game::Item::District {
            district: name,
            pos,
        };
        let cost = game.item_cost_for_city(owner, cid, &item);
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = Some(crate::world::DistrictFoundation {
            district: name,
            cost,
        });
        tile.wonder = None;
        tile.pillaged = false;
    }
    for (name, pos) in wonders {
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = Some(name);
        tile.pillaged = false;
    }
}

/// Apply the two health bars Firaxis exposes on a city banner.
fn apply_city_health(game: &mut crate::game::Game, cid: u32, state: &StateCity) {
    if state.damage.is_finite()
        && state.max_damage.is_finite()
        && state.damage >= 0.0
        && state.max_damage > 0.0
    {
        let remaining = (state.max_damage - state.damage).clamp(0.0, state.max_damage);
        if let Some(city) = game.cities.get_mut(&cid) {
            // CIVVIS and the stock City Center both use 200 garrison HP. Scale
            // anyway so a scenario-specific maximum remains truthful.
            city.hp = (200.0 * remaining / state.max_damage)
                .round()
                .clamp(1.0, 200.0) as i32;
        }
    }

    if state.wall_damage.is_finite()
        && state.max_wall_damage.is_finite()
        && state.wall_damage >= 0.0
        && state.max_wall_damage >= 0.0
    {
        let max_wall = state.max_wall_damage.round().max(0.0) as i32;
        Arc::make_mut(&mut game.observed_city_max_wall_hp).insert(cid, max_wall);
        if let Some(city) = game.cities.get_mut(&cid) {
            city.wall_hp = (state.max_wall_damage - state.wall_damage)
                .round()
                .clamp(0.0, state.max_wall_damage) as i32;
        }
    }
}

/// Restore exact private city facts before deriving host-to-model corrections.
pub(super) fn apply_observed_city_economy(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    snapshot: Option<&Snapshot>,
    unmapped: &mut Vec<String>,
) {
    Arc::make_mut(&mut game.observed_city_yield_adjustments).clear();
    // Clear first: the previous correction is part of `city_amenities` and
    // `city_housing`, and using it while deriving this turn's delta would
    // compound it forever.
    Arc::make_mut(&mut game.observed_city_amenity_adjustments).clear();
    Arc::make_mut(&mut game.observed_city_housing_adjustments).clear();
    Arc::make_mut(&mut game.observed_tile_yield_adjustments).clear();
    Arc::make_mut(&mut game.observed_city_worked_tiles).clear();
    Arc::make_mut(&mut game.observed_city_specialists).clear();

    for observed in &state.cities {
        let pos = crate::hex::offset_to_axial(observed.x, observed.y);
        let Some(cid) = game.city_at(pos) else {
            continue;
        };
        if let Some(worked) = &observed.worked {
            let positions = worked
                .iter()
                .map(|plot| crate::hex::offset_to_axial(plot.x, plot.y))
                // Firaxis includes the city centre in GetWorkedPlots(), but
                // CIVVIS accounts for it separately before assigning citizens.
                // Treating that implicit plot as a citizen assignment rejected
                // the whole authoritative list and silently restored CIVVIS's
                // own governor instead.
                .filter(|worked_pos| *worked_pos != pos)
                // ★★★★★ A DISTRICT PLOT IN THE WORKED LIST IS A SPECIALIST, NOT
                // A TILE. `Citizens:IsPlotWorked` answers true for a Campus a
                // citizen staffs, and the export already names that citizen in
                // `specialists`. Passing the plot through as a worked tile made
                // the model pay the specialist twice — once from `specialists`
                // (+2 Science, correctly) and once as the ground under the
                // district (its terrain Food/Production, which Firaxis removes
                // when the district is placed). Measured on live run
                // civvis-20260816T011314Z: Cumae with two Campus specialists and
                // one Industrial Zone specialist read **+2 Food, +4 Production**
                // over the host for twenty turns; every specialist city showed
                // the same signature. CIVVIS's own governor never offers a
                // district, foundation or wonder plot as a tile job, so this is
                // the import matching the engine, not a new rule.
                .filter(|worked_pos| {
                    game.map.get(*worked_pos).is_none_or(|tile| {
                        tile.district.is_none()
                            && tile.district_foundation.is_none()
                            && tile.wonder.is_none()
                    })
                })
                .collect::<Vec<_>>();
            let all_valid = positions.iter().all(|worked_pos| {
                game.map.get(*worked_pos).is_some() && game.city_at(*worked_pos).is_none()
            });
            if all_valid {
                // Firaxis lets the player swap an empire tile between nearby
                // cities. The tile export carries only player ownership, so the
                // initial mirror assigns it to a nearby city heuristically. A
                // current worked-plot record is stronger evidence: move that
                // tile to the city actually working it before preserving the
                // citizen plan. Without this, Lugdunum's real (58,25) was owned
                // by neighboring Rome in the mirror and the entire observed
                // list was discarded for CIVVIS's freshly optimized substitute.
                for &worked_pos in &positions {
                    let previous = game.map.tiles[&worked_pos].owner_city;
                    if previous == Some(cid) {
                        continue;
                    }
                    if let Some(previous) = previous {
                        if let Some(city) = game.cities.get_mut(&previous) {
                            city.owned_tiles.retain(|tile| *tile != worked_pos);
                        }
                    }
                    if !game.cities[&cid].owned_tiles.contains(&worked_pos) {
                        game.cities
                            .get_mut(&cid)
                            .unwrap()
                            .owned_tiles
                            .push(worked_pos);
                    }
                    game.map.tiles.get_mut(&worked_pos).unwrap().owner_city = Some(cid);
                }
                Arc::make_mut(&mut game.observed_city_worked_tiles).insert(cid, positions);
            } else {
                let issue = format!("{}:worked_plot", observed.name);
                if !unmapped.contains(&issue) {
                    unmapped.push(issue);
                }
            }
        }
        if let Some(specialists) = &observed.specialists {
            let mut translated = Vec::new();
            let mut all_valid = true;
            for civ6 in specialists {
                match civvis_node_name(&game.rules.districts, civ6, "DISTRICT_") {
                    Some(name) => translated.push(
                        game.district_family(crate::name::Name::new(&name))
                            .to_string(),
                    ),
                    None => {
                        all_valid = false;
                        let issue = format!("{civ6}:specialist");
                        if !unmapped.contains(&issue) {
                            unmapped.push(issue);
                        }
                    }
                }
            }
            if all_valid {
                Arc::make_mut(&mut game.observed_city_specialists).insert(cid, translated);
            }
        }
    }

    // Replace only when every own-city query succeeded. A partial export is
    // unknown, not authority to erase works housed in the omitted city.
    if !state.cities.is_empty() && state.cities.iter().all(|city| city.great_works.is_some()) {
        for kind in [
            "writing",
            "art",
            "religious_art",
            "artifact",
            "music",
            "relic",
        ] {
            game.players[0]
                .counters
                .insert(format!("great_work:{kind}"), 0);
        }
        game.players[0].great_work_pieces.clear();
        let mut seen = std::collections::BTreeSet::new();
        // ...and WHERE the host keeps each one. The model's own housing picks
        // the best slot for a work (a Relic goes to St. Basil's over the
        // Palace); the host's placement is what pays, and it read "+6 from
        // GreatWorks" in Rome while the model paid Mediolanum (run
        // civvis-20260816T233226Z t154+).
        let mut housing: std::collections::BTreeMap<
            u32,
            std::collections::BTreeMap<String, usize>,
        > = Default::default();
        for city in &state.cities {
            let cid = game.city_at(crate::hex::offset_to_axial(city.x, city.y));
            for work in city.great_works.as_deref().unwrap_or_default() {
                if !seen.insert(work.kind.clone()) {
                    continue;
                }
                match great_work_kind(&work.object) {
                    Some(kind) => {
                        game.grant_great_work(
                            0,
                            kind,
                            great_work_era(work.era.as_deref()),
                            &work.creator,
                        );
                        if let Some(cid) = cid {
                            *housing
                                .entry(cid)
                                .or_default()
                                .entry(kind.to_string())
                                .or_insert(0) += 1;
                        }
                    }
                    None => {
                        let issue = format!("{}:{}:great_work", work.kind, work.object);
                        if !unmapped.contains(&issue) {
                            unmapped.push(issue);
                        }
                    }
                }
            }
        }
        game.observed_great_work_housing = Some(Arc::new(housing));
    }

    // ★★★★★ THE HOST'S OWN PER-PLOT YIELDS, WHERE THE EXPORT CARRIES THEM.
    //
    // A city total says by how much the model is off; the plot says where. Some
    // of what a tile pays only the host can know — the fertility an eruption or
    // a flood left behind (Rome on run civvis-20260816T003229Z: **+12 Food and
    // +5 Production** over the model for forty turns, all of it volcanic soil),
    // a plantation pillaged between two tile exports, a modifier CIVVIS has no
    // row for. This is the tile-level twin of the city correction below: the
    // difference between `Plot:GetYield` and CIVVIS's own tile model, on the
    // centre and every plot the host says this city works, added inside
    // `workable_tile_yields`. Derived first, so the city correction that
    // follows carries only what is left (buildings, routes, policies).
    //
    // Deltas, never overrides, for the same reason as every other correction
    // here: a Builder's counterfactual mine still moves the tile by its modeled
    // amount. Cleared each turn above, so a plot the host stops working — or a
    // repaired plantation — carries no stale correction. Skipped for district,
    // foundation and wonder plots (specialists, imported separately) and for
    // any plot the mirror lacks. An export older than the field leaves the map
    // empty and everything below exactly as it was.
    for observed in &state.cities {
        let pos = crate::hex::offset_to_axial(observed.x, observed.y);
        let Some(_cid) = game.city_at(pos) else {
            continue;
        };
        let finite = |yields: &crate::rules::Yields| {
            [
                yields.food,
                yields.production,
                yields.gold,
                yields.science,
                yields.culture,
                yields.faith,
            ]
            .iter()
            .all(|value| value.is_finite())
        };
        let delta =
            |host: crate::rules::Yields, model: crate::rules::Yields| crate::rules::Yields {
                food: host.food - model.food,
                production: host.production - model.production,
                gold: host.gold - model.gold,
                science: host.science - model.science,
                culture: host.culture - model.culture,
                faith: host.faith - model.faith,
            };
        if let Some(host) = observed.center_yields.filter(finite) {
            // Against the RAW tile model, not the floored centre: the
            // correction is added before `city_yields_inner` applies its 2 Food
            // / 1 Production floor, and the host's own centre figure already
            // sits at or above that floor, so raw + correction = host survives
            // the floor unchanged.
            let model = game.modeled_tile_yields(pos);
            Arc::make_mut(&mut game.observed_tile_yield_adjustments)
                .insert(pos, delta(host, model));
        }
        for plot in observed.worked.iter().flatten() {
            let Some(host) = plot.yields.filter(finite) else {
                continue;
            };
            let plot_pos = crate::hex::offset_to_axial(plot.x, plot.y);
            if plot_pos == pos {
                continue;
            }
            let Some(tile) = game.map.get(plot_pos) else {
                continue;
            };
            if tile.district.is_some()
                || tile.district_foundation.is_some()
                || tile.wonder.is_some()
                || game.city_at(plot_pos).is_some()
            {
                continue;
            }
            let model = game.modeled_tile_yields(plot_pos);
            Arc::make_mut(&mut game.observed_tile_yield_adjustments)
                .insert(plot_pos, delta(host, model));
        }
    }

    // ...and the same correction for every OTHER plot the sweep read, which is
    // the set a Builder, a Settler and the citizen governor are choosing
    // between. Second, so the worked and centre readings above — taken from
    // this turn's state export rather than the last sweep — are the ones that
    // stand where both exist. See `apply_observed_plot_yields`.
    if let Some(snapshot) = snapshot {
        apply_observed_plot_yields(game, snapshot);
    }

    // Exact citizen assignments and durable Great Work state are now in place.
    // Calibrate the actual happiness band before deriving yield corrections.
    // The correction is a delta, not a host-value override, so a simulated Arena
    // still supplies its modeled Amenities on top of what Firaxis reported now.
    for observed in &state.cities {
        let Some(host_surplus) = host_city_amenity_surplus(observed) else {
            continue;
        };
        let pos = crate::hex::offset_to_axial(observed.x, observed.y);
        let Some(cid) = game.city_at(pos) else {
            continue;
        };
        let modeled_surplus = game.city_amenity_surplus(&game.cities[&cid]);
        Arc::make_mut(&mut game.observed_city_amenity_adjustments)
            .insert(cid, host_surplus - modeled_surplus);
    }

    // Housing the same way: the host's ceiling is what the city grows against,
    // and the board showed CIVVIS's own derivation beside the host's population
    // (Rome 8.5 modelled against 10 reported, Ostia 7.5 against 6, on run
    // civvis-20260816T011314Z t169). `-1` is the mod's could-not-read sentinel
    // and `None` an older export; neither is a claim about the ceiling.
    for observed in &state.cities {
        let Some(host_housing) = observed.housing.filter(|value| *value >= 0.0) else {
            continue;
        };
        let pos = crate::hex::offset_to_axial(observed.x, observed.y);
        let Some(cid) = game.city_at(pos) else {
            continue;
        };
        let modeled_housing = game.city_housing(&game.cities[&cid]);
        Arc::make_mut(&mut game.observed_city_housing_adjustments)
            .insert(cid, host_housing - modeled_housing);
    }

    // What remains is a local correction for host rules CIVVIS has not modeled.
    for observed in &state.cities {
        let Some(host) = observed.yields else {
            continue;
        };
        if ![
            host.food,
            host.production,
            host.gold,
            host.science,
            host.culture,
            host.faith,
        ]
        .iter()
        .all(|value| value.is_finite())
        {
            continue;
        }
        let pos = crate::hex::offset_to_axial(observed.x, observed.y);
        let Some(cid) = game.city_at(pos) else {
            continue;
        };
        let model = game.city_yields_model(cid);
        let adjustment = crate::rules::Yields {
            food: host.food - model.food,
            production: host.production - model.production,
            gold: host.gold - model.gold,
            science: host.science - model.science,
            culture: host.culture - model.culture,
            faith: host.faith - model.faith,
        };
        Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(cid, adjustment);
    }
}

/// Apply city facts that affect `city_yields_model` before deriving the
/// host-to-model correction. A fresh reconstruction initially marks the first
/// planted city as the capital; the host can have moved its Palace elsewhere.
/// Population, loyalty and pillage state also affect the modeled total.
pub(super) fn apply_observed_city_facts(game: &mut crate::game::Game, state: &StateSnapshot) {
    capital_identity::apply(game, state);
    let cities = state
        .cities
        .iter()
        .chain(state.rivals.iter().flat_map(|rival| rival.cities.iter()))
        .chain(state.minors.iter().flat_map(|minor| minor.cities.iter()));
    for observed in cities {
        let pos = crate::hex::offset_to_axial(observed.x, observed.y);
        let Some(cid) = game.city_at(pos) else {
            continue;
        };
        // Population drives Loyalty pressure in a nine-tile radius. Rival and
        // city-state cities are planted at population one, so this has to land
        // before any yield or pressure correction is measured.
        if observed.pop > 0 {
            game.cities.get_mut(&cid).unwrap().pop = observed.pop;
        }
        // The foreign-city exporter marks a successful, currently visible
        // majority read, including a confirmed lack of majority. Missing or
        // fogged observations must not manufacture a conversion or erase one.
        if game.cities[&cid].owner != 0 && observed.religion_observed == Some(true) {
            apply_city_religion(game.cities.get_mut(&cid).unwrap(), observed);
            Arc::make_mut(&mut game.observed_city_religion_markers).insert(cid);
        }
        apply_city_health(game, cid, observed);
        if observed.loyalty_per_turn.is_finite() {
            Arc::make_mut(&mut game.observed_city_loyalty_per_turn)
                .insert(cid, observed.loyalty_per_turn);
        }
        if let Some(strength) = observed
            .ranged_strength
            .filter(|v| v.is_finite() && *v >= 0.0)
        {
            Arc::make_mut(&mut game.observed_city_ranged_strength).insert(cid, strength);
        }
        if observed.defense.is_finite() && observed.defense >= 0.0 {
            Arc::make_mut(&mut game.observed_city_strength).insert(cid, observed.defense);
        }
    }
    // The owned-city list is complete and current, unlike the revealed foreign
    // subset. Record its strict majority explicitly so the host-only country
    // verdict rule does not hide our own known conversion state.
    let mut own_following: std::collections::BTreeMap<String, usize> = Default::default();
    for city in &state.cities {
        if let Some(faith) = city.religion.as_deref().and_then(civvis_religion_name) {
            *own_following.entry(faith).or_default() += 1;
        }
    }
    let own_majority = own_following
        .into_iter()
        .find(|(_, count)| count * 2 > state.cities.len())
        .map(|(faith, _)| faith);
    let observed = Arc::make_mut(&mut game.observed_majority_religion);
    observed.remove(&0);
    if let Some(faith) = own_majority {
        observed.insert(0, faith);
    }
}

/// Tell the seat which cities it has seen, so fog does not erase them.
///
/// ★★★★★ **THE SAME DEFECT AS `apply_tile_memory`, ONE FIELD OVER, AND I ONLY FIXED
/// THE TILES.** The operator's report was *"civvis sometimes only shows current
/// visibility"*; that was true of ground and it is equally true of cities.
///
/// `obs.rs` includes a city in a seated observation when it is currently visible, or
/// when the seat REMEMBERS it:
///
/// ```ignore
/// for memory in g.players[*viewer].remembered_cities.values() {
///     if explored.contains(&memory.pos) && !vis.contains(&memory.pos) { … }
/// ```
///
/// `grep -c remembered_cities src/mirror.rs` answered **0**, so every enemy city
/// outside the seat's current sight vanished from the board it is shown and reasons
/// over.
///
/// Measured on live run `civvis-20260801T045406Z` at turn 198, at war and losing:
///
/// | | |
/// |---|---|
/// | enemy cities in the export | **7** |
/// | …revealed, on land, unoccupied | **7** |
/// | placed on the reconstruction | **7** (`follow.log`: "7 rival cities") |
/// | **visible in the seated observation** | **1** |
///
/// ⚠ That last row is the one that matters strategically: `findWarTarget` needs a
/// revealed rival city, and this project has already recorded "no enemy city is ever
/// revealed … domination is arithmetically impossible" as a standing blocker. The
/// cities were on the board the whole time; the seat could not remember them.
///
/// Built through `Game::remember_city` rather than assembled here, so the bridge's
/// memory has exactly the shape the engine's own fog bookkeeping produces and the two
/// cannot drift.
///
/// ⚠ Replaces rather than extends, for the same reason [`apply_explored`] does:
/// `Game::new` generates a world with cities of its own, and a memory of THOSE is a
/// memory of somewhere the real seat has never been.
pub(crate) fn apply_city_memory(game: &mut crate::game::Game) {
    let turn = game.turn.max(1);
    let seen: Vec<(u32, crate::game::RememberedCity)> = game
        .cities
        .values()
        .map(|city| {
            let mut memory = game.remember_city(city);
            memory.seen_turn = turn;
            (city.id, memory)
        })
        .collect();
    let Some(seat) = game.players.get_mut(0) else {
        return;
    };
    seat.remembered_cities.clear();
    for (id, memory) in seen {
        seat.remembered_cities.insert(id, memory);
    }
}

/// ★★★ THE CAPTURE DECISION REACHES THE BOARD. `captured_from` is assigned
/// from every export — the host names the loser only while its own
/// keep/raze/liberate decision is open for this city (`GetNextCapturedCity()`,
/// `Popups/RazeCity.lua:71`), so an export without it CLEARS the flag and the
/// board stops asking. `original_owner` moves only when the founder sits on a
/// mirrored seat; an unmapped founder (never met, or no seat left) keeps the
/// board's default, under which `city_can_be_razed_by` and the liberate
/// clause both say no and only Keep is offered. A loser that maps to no seat
/// leaves the flag clear: the engine's `do_keep_city` pays the capture
/// rewards to `players[defeated]`, and a seat that is not there is a panic,
/// not a decision.
pub(super) fn apply_city_capture(
    live: &mut crate::game::City,
    state: &StateCity,
    seat_of: &std::collections::BTreeMap<i64, usize>,
) {
    if let Some(founder) = state
        .original_owner
        .filter(|player| *player >= 0)
        .and_then(|player| seat_of.get(&player))
    {
        live.original_owner = *founder;
    }
    live.captured_from = state
        .captured_from
        .filter(|player| *player >= 0)
        .and_then(|player| seat_of.get(&player).copied())
        .filter(|seat| *seat != live.owner);
}
