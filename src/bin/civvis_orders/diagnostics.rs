//! Diagnostic commands of the orders bridge: `--explain-settler`,
//! `--explain-assault` and `--dump-mirror`.
//!
//! Both load one turn of the host's export, build the mirror, print what they
//! find and exit, without deciding anything. Moved verbatim out of `main` in
//! `civvis_orders.rs` (2026-11-06).

use super::*;

/// ★★★★★ WHY THERE IS NO SETTLER, ANSWERED FOR A RECORDED RUN.
///
/// The empire-level failure this project keeps measuring is EXPANSION: runs
/// end with one city while the plan asks for seven or eight. The Strategy
/// journal says "short of cities with land still open" every turn, and the
/// Cities journal says "starts heavy chariot" every turn, and NOTHING says
/// which of the settler gate's five conditions refused.
///
/// Live on `civvis-20260804T041525Z`: one settler in 130 turns (built t9,
/// lost t28), then 90 consecutive turns of military while `desired_cities`
/// stood at 7. Reading the code cannot distinguish "the site search found
/// nothing" from "the branch was never reached", because the gate is a
/// single `&&` chain with no journal line of its own.
///
/// This evaluates each condition separately on the reconstructed board and
/// prints them, so the answer is a measurement rather than an inference.
pub(super) fn explain_settler(
    args: Vec<String>,
    players: usize,
    max_turns: u32,
    frontier: u32,
    load: &impl Fn(Option<u32>) -> Option<(civvis::mirror::Snapshot, civvis::mirror::StateSnapshot)>,
) {
    let want_turn: Option<u32> = arg_text(&args, "--turn").and_then(|v| v.parse().ok());
    let Some((snapshot, state)) = load(want_turn) else {
        println!("no revealed terrain or no state yet");
        return;
    };
    let (mirror_players, mirror_turns) = mirror_setup(&state, players, max_turns);
    let live = civvis::mirror::LiveMirror::new(
        &snapshot,
        &state,
        mirror_players,
        1,
        mirror_turns,
        frontier,
    );
    let g = &live.game;
    // ⚠⚠ THE PROBE MUST PLAY THE GENOME THE LIVE RUN PLAYED.
    //
    // The first version of this diagnostic used stock `AdvancedAi::new()`
    // weights and reported `mil_per_city` 1.0, while the live journal for the
    // same run printed **1.4** — a league genome the live seat had been handed.
    // The league is retired (#2357) and the live seat plays `AdvancedAi::new`
    // itself, so the probe and the deployment are the same agent again; the
    // header line still names it so nobody has to take that on trust.
    let advanced = civvis::ai::AdvancedAi::new();
    refuse_retired_strategy_flag(&args);
    println!("genome: stock AdvancedAi::new — the deployment genome");
    let w = advanced.weights().clone();
    let mut ai = civvis::ai::BasicAi::with_weights(w.clone());
    // The live decider plays with the second pipeline slot open, the
    // host's Settler population floor, and the land grab's wider pipeline
    // and later window (see `AdvancedAi::land_grab`).
    ai.enable_parallel_settlers();
    ai.enable_host_settler_pop();
    ai.enable_land_grab();
    let pid = 0usize;
    let n_cities = g.player_city_ids(pid).len();
    let settlers = g
        .units
        .values()
        .filter(|u| u.owner == pid && u.kind == "settler")
        .count();
    let city_pop = g
        .player_city_ids(pid)
        .into_iter()
        .map(|cid| g.cities[&cid].pop)
        .max()
        .unwrap_or(0);
    println!(
        "turn {}  cities {n_cities}  settlers {settlers}  best city pop {city_pop}",
        g.turn
    );
    println!("settler gate, condition by condition:");
    println!(
        "  (cities+settlers) < city_target      {:>5}   {} < {:.1}",
        ((n_cities + settlers) as f64) < w.city_target,
        n_cities + settlers,
        w.city_target
    );
    // The live seat's pipeline width (see `AdvancedAi::land_grab`, and
    // `AdvancedAi::parallel_settlers` beneath it): two walkers from the
    // first city, one more per three cities, never more than the seats
    // still short of the target.
    let seats_short = (w.city_target.ceil().max(0.0) as usize).saturating_sub(n_cities);
    let pipeline = if seats_short > 0 {
        (civvis::ai::LAND_GRAB_PIPELINE_BASE + n_cities / 3).min(seats_short)
    } else if n_cities >= 2 && ((n_cities + settlers + 1) as f64) < w.city_target {
        2
    } else {
        1
    };
    println!(
        "  settlers < pipeline ({pipeline})            {:>5}   {settlers}",
        settlers < pipeline
    );
    // The live seat's floor is the host's (see `BasicAi::host_settler_pop`).
    let settler_min_pop = w.settler_min_pop.min(2.0);
    println!(
        "  city_pop >= settler_min_pop          {:>5}   {city_pop} >= {:.1} (genome {:.3}, host floor 2)",
        (city_pop as f64) >= settler_min_pop,
        settler_min_pop,
        w.settler_min_pop
    );
    // The land grab's window: a settler must still repay before the turn
    // limit (see `BasicAi::land_grab`); the genome's stop turn alone no
    // longer closes it.
    println!(
        "  turn < settler_stop_turn             {:>5}   {} < {:.0} (land grab keeps the window open while a settler still repays: {})",
        (g.turn as f64) < w.settler_stop_turn,
        g.turn,
        w.settler_stop_turn,
        g.turn + g.standard_duration(18) < g.max_turns
    );
    // The military branch sits ABOVE the settler gate in `pick_item` and
    // returns first when it fires, so the gate passing means nothing on its
    // own. Recompute its floor from outside: `mil_per_city * cities`, raised
    // by visible besiegers within 3 of a city, capped at +3.
    let military = g
        .units
        .values()
        .filter(|u| u.owner == pid && g.rules.units[u.kind].class == "military")
        .count();
    let besiegers = {
        let homes: Vec<_> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        g.units
            .values()
            .filter(|u| u.owner != pid && g.rules.units[u.kind].class == "military")
            .filter(|u| homes.iter().any(|h| g.wdist(*h, u.pos) <= 3))
            .count()
    };
    let floor = (w.mil_per_city * n_cities as f64).max(if besiegers == 0 {
        0.0
    } else {
        w.mil_per_city * n_cities as f64 + besiegers.min(3) as f64
    });
    println!(
        "military branch: hold {military}, floor {floor:.1} (besiegers within 3: {besiegers}) -> fires {}",
        (military as f64) < floor
    );
    // End the inference: ask the chooser itself.
    {
        let cid = g.player_city_ids(pid).into_iter().next();
        if let Some(cid) = cid {
            let probe = g.clone();
            let melee = g
                .units
                .values()
                .filter(|u| u.owner == pid && g.rules.units[u.kind].class == "military")
                .count();
            let chosen = ai.pick_item(
                &probe, pid, cid, n_cities, settlers, 0, 0, 0, military, melee, 0,
            );
            println!("pick_item returns: {chosen:?}");
        }
    }
    let practical = ai.has_practical_settle_site(g, pid);
    println!("  has_practical_settle_site            {practical:>5}");
    // When the site search refuses, say WHICH rule refused each candidate:
    // "no site" and "every site too close to a city we already own" are
    // different defects and only one of them is about the map.
    if !practical {
        let mut water = 0;
        let mut impassable = 0;
        let mut too_close = 0;
        let mut foreign = 0;
        let mut wonder = 0;
        let mut ok = 0;
        for (pos, tile) in g.map.tiles.iter() {
            if g.rules.is_water(tile) {
                water += 1;
            } else if !g.rules.is_passable(tile) {
                impassable += 1;
            } else if g.tile_is_natural_wonder(tile) {
                wonder += 1;
            } else if g
                .cities
                .values()
                .any(|city| (g.wdist(city.pos, *pos) as f64) < w.min_city_dist)
            {
                too_close += 1;
            } else if tile
                .owner_city
                .is_some_and(|cid| g.cities[&cid].owner != pid)
            {
                foreign += 1;
            } else {
                ok += 1;
            }
        }
        println!(
            "  known tiles {}: water {water}, impassable {impassable}, natural wonder {wonder}, within min_city_dist {:.0} of a city {too_close}, foreign-owned {foreign}, VALID {ok}",
            g.map.tiles.len(),
            w.min_city_dist
        );
        println!("  ⚠ VALID > 0 with practical=false means the site exists but the 8-step walk cannot REACH it");
    }
}

/// ★★★★★ PRINT THE BOARD CIVVIS IS ACTUALLY ANSWERING, so it can be diffed against
/// the one Civilization VI exported.
///
/// ⚠ THIS EXISTS BECAUSE "the mirror is 1:1" HAS ALREADY BEEN CLAIMED AND BEEN
/// FALSE. It was rendering the right terrain at the wrong hexes: Civ 6 speaks
/// OFFSET, CIVVIS stores AXIAL, both are pairs of small integers, and nothing
/// complains when they are mixed. A capital at offset (56,28) had NO TILE in the
/// reconstruction and the only symptom was CIVVIS reporting "no legal revealed
/// site" on a map with 323 revealed plots — it blamed the map.
///
/// The dump is keyed back in OFFSET (`hex::axial_to_offset`) precisely so the
/// round trip is exercised: a plot the export named and this dump cannot produce
/// is the coordinate bug's signature, and it shows as an ABSENT tile rather than a
/// wrong value.
///
/// Not everything here is an independent check — terrain names are written from
/// this same export through this same vocabulary, so they must agree. What IS
/// independent, and is where the defects have been:
///
///   w    Civ 6 answers `IsWater()`; CIVVIS derives water from its own ruleset via
///        the translated terrain. This is the "unrevealed ground reads as OCEAN"
///        family, which cost a seat its whole continent.
///   h    the export encodes hills in the terrain NAME (`TERRAIN_*_HILLS`) and
///        CIVVIS carries a separate flag. Disagreement here is the standing
///        explanation for improvement orders refused and re-issued forever.
///   res  whether the name resolved at all. An unresolved name does not error: the
///        tile silently keeps whatever `Game::new` generated, which is a wrong
///        terrain wearing a right one's clothes.
pub(super) fn dump_mirror(
    args: Vec<String>,
    players: usize,
    max_turns: u32,
    frontier: u32,
    load: &impl Fn(Option<u32>) -> Option<(civvis::mirror::Snapshot, civvis::mirror::StateSnapshot)>,
) {
    let want_turn: Option<u32> = arg_text(&args, "--turn").and_then(|v| v.parse().ok());
    let Some((snapshot, state)) = load(want_turn) else {
        println!("{{\"plots\":[],\"note\":\"no revealed terrain or no state yet\"}}");
        return;
    };
    let (mirror_players, mirror_turns) = mirror_setup(&state, players, max_turns);
    let live = civvis::mirror::LiveMirror::new(
        &snapshot,
        &state,
        mirror_players,
        1,
        mirror_turns,
        frontier,
    );
    let game = &live.game;
    let vocab = civvis::mirror::Vocabulary::embedded();
    let width = snapshot.width.max(1);
    let height = snapshot.height.max(1);
    let mut plots: Vec<String> = Vec::new();
    let mut unresolved: std::collections::BTreeMap<String, usize> = Default::default();
    for y in 0..height {
        for x in 0..width {
            let pos = civvis::hex::offset_to_axial(x, y);
            let Some(tile) = game.map.get(pos) else {
                // Deliberately NOT skipped silently: a plot with no tile is the
                // whole reason this dump exists. It is reported as absent by
                // simply not appearing, and the diff counts it.
                continue;
            };
            let exported = snapshot.plot((x, y));
            // Only dump ground either side has an opinion about. The far unknown
            // is ocean filler on both and would drown the diff in agreement.
            if exported.is_none() && !snapshot.is_revealed((x, y)) {
                continue;
            }
            let mut resolved = true;
            if let Some(plot) = exported {
                if let Some(name) = &plot.t {
                    match vocab.terrain(name) {
                        civvis::mirror::Resolved::Known(_) => {}
                        _ => {
                            resolved = false;
                            *unresolved.entry(name.clone()).or_default() += 1;
                        }
                    }
                }
            }
            let field = |value: &Option<civvis::name::Name>| match value {
                Some(name) => format!("\"{}\"", name.as_str()),
                None => "null".to_string(),
            };
            // Whose ground the mirror thinks this is, as a CIVVIS seat index.
            // The export gives a Civ 6 player id and our seat is always its
            // local player 0, so "ours" is the part that compares cleanly; a
            // rival's id does not, because rivals are remapped on the way in.
            let owner = tile
                .owner_city
                .and_then(|cid| game.cities.get(&cid))
                .map(|city| city.owner as i64);
            let foundation = tile
                .district_foundation
                .as_ref()
                .map(|placed| format!("\"{}\"", placed.district.as_str()))
                .unwrap_or_else(|| "null".to_string());
            // What the board PAYS this plot, and what the host said it
            // pays. The dump named every row of the ruleset either side
            // reads and not the one number they are read for, so the
            // mapping could be verified tile by tile while the yields
            // derived from it went unchecked — which is how disaster
            // fertility stayed invisible on the board for as long as it
            // did. `ty` is `Game::workable_tile_yields`; `hy` is the
            // export's own `Plot:GetYield` tuple where the sweep carried
            // one, so `ty` minus `hy` is the residual to explain.
            //
            // ⚠ `ty`, not `y`: `y` is this plot's row coordinate three
            // fields to the left, and a duplicate key in one JSON object
            // silently keeps whichever a reader saw last.
            let paid = serde_json::to_string(&game.workable_tile_yields(pos))
                .unwrap_or_else(|_| "null".to_string());
            let host = exported
                .and_then(civvis::mirror::Plot::host_yields)
                .map(|yields| serde_json::to_string(&yields).unwrap_or_else(|_| "null".into()))
                .unwrap_or_else(|| "null".to_string());
            plots.push(format!(
                "{{\"x\":{},\"y\":{},\"t\":\"{}\",\"h\":{},\"w\":{},\"f\":{},\"r\":{},\
                 \"im\":{},\"d\":{},\"df\":{},\"wo\":{},\"p\":{},\"own\":{},\"res\":{},\
                 \"ty\":{},\"hy\":{}}}",
                x,
                y,
                tile.terrain.as_str(),
                tile.hills,
                game.rules.is_water(tile),
                field(&tile.feature),
                field(&tile.resource),
                field(&tile.improvement),
                field(&tile.district),
                foundation,
                field(&tile.wonder),
                tile.pillaged,
                owner.map(|o| o == 0).unwrap_or(false),
                resolved,
                paid,
                host,
            ));
        }
    }
    let unresolved_json: Vec<String> = unresolved
        .iter()
        .map(|(name, count)| format!("\"{name}\":{count}"))
        .collect();
    let mut cities = Vec::new();
    for city in game.cities.values().filter(|city| city.owner == 0) {
        let (x, y) = civvis::hex::axial_to_offset(city.pos.0, city.pos.1);
        let plan = game.city_citizen_plan(city.id);
        let worked = plan
            .worked_tiles
            .iter()
            .map(|pos| {
                let (wx, wy) = civvis::hex::axial_to_offset(pos.0, pos.1);
                serde_json::json!({"x": wx, "y": wy})
            })
            .collect::<Vec<_>>();
        // The tile-level ledger behind the model total, in the host's
        // offset coordinates so `tools/civ6_yield_drift.py` can diff it
        // against the export's per-plot yields tile for tile.
        let ledger = game.city_yield_ledger(city.id);
        let offset = |pos: civvis::Pos| {
            let (px, py) = civvis::hex::axial_to_offset(pos.0, pos.1);
            serde_json::json!({"x": px, "y": py})
        };
        let ledger_json = serde_json::json!({
            "center": ledger.center,
            "tiles": ledger
                .tiles
                .iter()
                .map(|(pos, yields)| {
                    let mut entry = offset(*pos);
                    entry["yields"] = serde_json::json!(yields);
                    entry
                })
                .collect::<Vec<_>>(),
            "tile_adjustments": ledger
                .tile_adjustments
                .iter()
                .map(|(pos, yields)| {
                    let mut entry = offset(*pos);
                    entry["yields"] = serde_json::json!(yields);
                    entry
                })
                .collect::<Vec<_>>(),
            "specialists": ledger
                .specialists
                .iter()
                .map(|(family, yields)| serde_json::json!({"district": family, "yields": yields}))
                .collect::<Vec<_>>(),
            "districts": ledger
                .districts
                .iter()
                .map(|(name, pos, yields)| {
                    let mut entry = offset(*pos);
                    entry["district"] = serde_json::json!(name);
                    entry["yields"] = serde_json::json!(yields);
                    entry
                })
                .collect::<Vec<_>>(),
        });
        // Board values carry the mirror's host-to-model correction; the
        // `model_*` twins are what CIVVIS derives on its own, which is the
        // number the fidelity instrument scores.
        let housing = game.city_housing(city);
        let housing_adjustment = game
            .observed_city_housing_adjustments
            .get(&city.id)
            .copied()
            .unwrap_or(0.0);
        let housing_sources = serde_json::json!(game
            .city_housing_sources(city)
            .named()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>());
        let amenities = game.city_amenities(city);
        let amenity_adjustment = game
            .observed_city_amenity_adjustments
            .get(&city.id)
            .copied()
            .unwrap_or(0);
        cities.push(serde_json::json!({
            "x": x,
            "y": y,
            "name": city.name,
            "pop": city.pop,
            "production_progress": city.production,
            "worked": worked,
            "specialists": plan.specialists,
            "yields": game.city_yields(city.id),
            "model_yields": game.city_yields_model(city.id),
            "ledger": ledger_json,
            "housing": housing,
            "model_housing": housing - housing_adjustment,
            // The host exports `housing_from_water`, `_buildings`,
            // `_districts` and the rest on every city of every state. Until
            // this twin existed the instrument could only compare the two
            // totals, so a persistent gap named no rule and nobody could
            // act on it. Same categories, same names.
            "model_housing_sources": housing_sources,
            "amenities": amenities,
            "model_amenities": amenities - amenity_adjustment,
            "amenities_required": civvis::game::Game::city_amenities_required(city),
            "amenity_surplus": game.city_amenity_surplus(city),
        }));
    }
    let great_works = [
        "writing",
        "art",
        "religious_art",
        "artifact",
        "music",
        "relic",
    ]
    .into_iter()
    .map(|kind| {
        (
            kind,
            game.players[0]
                .counters
                .get(&format!("great_work:{kind}"))
                .copied()
                .unwrap_or(0),
        )
    })
    .collect::<std::collections::BTreeMap<_, _>>();
    let governors = game.players[0]
        .governor_roster
        .iter()
        .filter_map(|(name, governor)| {
            let host_type = mirror::civ6_governor_name(name.as_str())?;
            let city = governor.city.and_then(|cid| game.cities.get(&cid));
            let (x, y) = city
                .map(|city| civvis::hex::axial_to_offset(city.pos.0, city.pos.1))
                .unwrap_or((-1, -1));
            let establish_turns = game
                .rules
                .governors
                .get(name)
                .map(|spec| game.standard_duration(spec.establish_turns))
                .unwrap_or(0);
            let mut promotions = governor
                .promotions
                .iter()
                .filter_map(|promotion| mirror::civ6_governor_promotion(promotion))
                .collect::<Vec<_>>();
            promotions.sort_unstable();
            Some(serde_json::json!({
                "type": host_type,
                "x": x,
                "y": y,
                "established": city.is_some()
                    && game.turn >= governor.assigned_turn.saturating_add(establish_turns),
                "neutralized": game.turn < governor.disabled_until,
                "promotions": promotions,
            }))
        })
        .collect::<Vec<_>>();
    let mut empire_yields = civvis::rules::Yields::default();
    let mut model_empire_yields = civvis::rules::Yields::default();
    for city in game.cities.values().filter(|city| city.owner == 0) {
        empire_yields.add(game.city_yields(city.id));
        model_empire_yields.add(game.city_yields_model(city.id));
    }
    // What the empire collects beside its cities — founder beliefs and
    // the Faith Firaxis pays for Great Person points nobody can spend —
    // is part of the model's per-turn figure and of the board's, and the
    // player-level correction (`observed_yield_adjustments`) is measured
    // against the same sum, so the board reads the host's number.
    let player_extras = game.player_yield_extras(0);
    empire_yields.add(player_extras);
    model_empire_yields.add(player_extras);
    if let Some(adjustment) = game.observed_yield_adjustments.get(&0) {
        empire_yields.add(*adjustment);
    }
    let great_person_points_per_turn = game.great_person_points_per_turn(0);
    let unused_great_person_classes = great_person_points_per_turn
        .keys()
        .filter(|kind| !game.great_person_class_earnable(0, kind))
        .cloned()
        .collect::<Vec<_>>();
    // What crossed for the first time on 2026-08-26, read back from the
    // board it landed on: the host's climate beside the phase it set, the
    // request each city-state is making, and the routes the host priced.
    let quests: Vec<serde_json::Value> = game.players[0]
        .quests
        .iter()
        .map(|(minor, quest)| {
            serde_json::json!({
                "minor": game.players.get(*minor).map(|player| player.civ.clone()),
                "kind": quest.kind,
                "target": quest.target,
            })
        })
        .collect();
    let route_options: Vec<serde_json::Value> = game
        .observed_route_options
        .iter()
        .map(|((origin, destination), yields)| {
            serde_json::json!({
                "origin": game.cities.get(origin).map(|city| city.name.to_string()),
                "dest": game.cities.get(destination).map(|city| city.name.to_string()),
                "yields": yields,
            })
        })
        .collect();
    println!(
        "{{\"turn\":{},\"width\":{},\"height\":{},\"revealed\":{},\
         \"unresolved_terrain\":{{{}}},\"plots\":[{}],\"cities\":{},\
         \"great_works\":{},\"governors\":{},\"governor_points\":{},\
         \"governor_points_spent\":{},\"governor_points_available\":{},\
         \"empire_yields\":{},\"model_empire_yields\":{},\
         \"player_extras\":{},\"unused_great_person_faith\":{},\
         \"great_person_points_per_turn\":{},\"unused_great_person_classes\":{},\
         \"host_faith_per_turn\":{},\"climate_phase\":{},\"climate\":{},\
         \"quests\":{},\"route_options\":{},\"observed_appeal\":{},\
         \"host_observed\":{}}}",
        state.turn,
        width,
        height,
        snapshot.revealed_count(),
        unresolved_json.join(","),
        plots.join(","),
        serde_json::to_string(&cities).unwrap(),
        serde_json::to_string(&great_works).unwrap(),
        serde_json::to_string(&governors).unwrap(),
        game.governor_titles(0),
        game.players[0].governor_titles_spent,
        game.governor_titles_available(0),
        serde_json::to_string(&empire_yields).unwrap(),
        serde_json::to_string(&model_empire_yields).unwrap(),
        serde_json::to_string(&player_extras).unwrap(),
        game.unused_great_person_faith(0),
        serde_json::to_string(&great_person_points_per_turn).unwrap(),
        serde_json::to_string(&unused_great_person_classes).unwrap(),
        serde_json::to_string(&state.faith_per_turn).unwrap(),
        game.climate_phase,
        serde_json::to_string(&game.observed_climate).unwrap(),
        serde_json::to_string(&quests).unwrap(),
        serde_json::to_string(&route_options).unwrap(),
        game.observed_appeal.len(),
        game.host_observed.len(),
    );
}

/// `--explain-assault CORPS [--reach N] [--arrive T] [--turn N]`: the assault
/// census of one recorded turn.
///
/// `CORPS` is `kind:count[,kind:count...]`, for example
/// `legion:4,battering_ram:1`; every recruit reaches the board's edge nearest
/// our cities `--arrive` turns from now (default 4). For each charted city of
/// a living major or city-state within `--reach` tiles (default 12) of one of
/// our cities it prints the forecast's raw chance of capture within 4, 8 and
/// 12 turns of a war that began this turn, with the land soldiers the seat
/// holds and again with the recruits added. One tab-separated line a city,
/// best first; it decides nothing. See `ai/advanced/assault_census.rs`.
pub(super) fn explain_assault(
    args: Vec<String>,
    players: usize,
    max_turns: u32,
    frontier: u32,
    load: &impl Fn(Option<u32>) -> Option<(civvis::mirror::Snapshot, civvis::mirror::StateSnapshot)>,
) {
    let want_turn: Option<u32> = arg_text(&args, "--turn").and_then(|v| v.parse().ok());
    let Some((snapshot, state)) = load(want_turn) else {
        println!("no revealed terrain or no state yet");
        return;
    };
    let reach: i32 = arg_text(&args, "--reach")
        .and_then(|v| v.parse().ok())
        .unwrap_or(12);
    let arrive: u32 = arg_text(&args, "--arrive")
        .and_then(|v| v.parse().ok())
        .unwrap_or(4);
    let recruits: Vec<(String, u32, u32)> = arg_text(&args, "--explain-assault")
        .unwrap_or_default()
        .split(',')
        .filter_map(|part| {
            let (kind, count) = part.split_once(':')?;
            Some((kind.trim().to_string(), count.trim().parse().ok()?, arrive))
        })
        .collect();
    let (mirror_players, mirror_turns) = mirror_setup(&state, players, max_turns);
    let live = civvis::mirror::LiveMirror::new(
        &snapshot,
        &state,
        mirror_players,
        1,
        mirror_turns,
        frontier,
    );
    let g = &live.game;
    let pid = 0usize;
    let ai = civvis::ai::AdvancedAi::new();
    let ours = g.player_city_ids(pid).len();
    let soldiers = g
        .player_unit_ids(pid)
        .into_iter()
        .filter(|uid| g.rules.units[g.units[uid].kind].class == "military")
        .count();
    println!(
        "assault-census\tturn {}\tour cities {ours}\tour military units {soldiers}\trecruits {}\tarrive {arrive}\treach {reach}",
        g.turn,
        recruits
            .iter()
            .map(|(kind, count, _)| format!("{kind}:{count}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    println!(
        "city\towner\tkind\tcapital\tpop\tdist\tstrength\twalls\thostiles\theld4\theld8\theld12\trec4\trec8\trec12\tcal8\tlost\tshort"
    );
    for row in ai.assault_census(g, pid, reach, &recruits) {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{:.0}\t{}/{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.1}\t{}",
            row.city,
            row.owner,
            if row.city_state { "city-state" } else { "major" },
            row.capital,
            row.pop,
            row.distance,
            row.strength,
            row.walls,
            row.max_walls,
            row.hostiles,
            row.held[0],
            row.held[1],
            row.held[2],
            row.recruited[0],
            row.recruited[1],
            row.recruited[2],
            row.calibrated,
            row.units_lost,
            row.short_of
        );
    }
}
