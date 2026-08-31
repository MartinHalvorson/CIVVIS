//! The research-order probe, moved out of the controller it measures.
//!
//! ⚠ A verbatim move, indentation included — see `tests.rs` for why nothing was
//! dedented. `super` still resolves to `advanced`.

use super::*;
use crate::ai::Ai;
use crate::game::{Action, Game};
use crate::setup::GameSpeed;

/// Paired A/B at the *deployment* shape, not the unit-test shape: six
/// players, Online speed, 250 turns, a Small world.
///
/// This is the instrument behind PR #958's numbers, kept so they can be
/// reproduced rather than taken on trust. The same treatment measured at
/// 4p/28x18/140 turns was byte-identical on 13 of 20 seeds — those empires
/// finish with 6-25 techs and mostly never build a Campus, so the treatment
/// barely fires and the run says almost nothing. Measure where it runs.
#[test]
#[ignore = "census, not an assertion; run explicitly with --nocapture"]
fn research_economy_ab_at_deployment_scale() {
    let play = |seed: u64, treated: bool| {
        let mut g = Game::new(6, 74, 46, seed, 250, 9);
        g.game_speed = GameSpeed::Online;
        let mut me = AdvancedAi::new();
        me.research_economy = treated;
        let mut others = AdvancedAi::fleet(&g);
        while g.winner.is_none() && g.turn <= g.max_turns {
            let pid = g.current;
            if pid == 0 {
                me.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
        }
        let cities = g.player_city_ids(0);
        let science: f64 = cities.iter().map(|c| g.city_yields(*c).science).sum();
        let campuses = cities
            .iter()
            .filter(|c| {
                g.cities[c]
                    .districts
                    .keys()
                    .any(|d| g.district_family(*d) == crate::name!("campus"))
            })
            .count();
        (
            cities.len(),
            campuses,
            g.players[0].techs.len(),
            science,
            g.score(0),
        )
    };
    let (mut n, mut ts, mut cs, mut tt, mut tc, mut tsc, mut csc, mut tcp, mut ccp) =
        (0, 0.0, 0.0, 0, 0, 0i64, 0i64, 0, 0);
    for seed in 9_200..9_208u64 {
        let t = play(seed, true);
        let c = play(seed, false);
        println!(
                "seed {seed}: treated cities={} campus={} techs={} sci={:.1} score={} | control cities={} campus={} techs={} sci={:.1} score={}",
                t.0, t.1, t.2, t.3, t.4, c.0, c.1, c.2, c.3, c.4
            );
        n += 1;
        ts += t.3;
        cs += c.3;
        tt += t.2;
        tc += c.2;
        tsc += t.4;
        csc += c.4;
        tcp += t.1;
        ccp += c.1;
    }
    println!(
            "TOTALS n={n} science {ts:.1} vs {cs:.1} | techs {tt} vs {tc} | campuses {tcp} vs {ccp} | score {tsc} vs {csc}"
        );
}
/// Off by default, set only by the live bridge, and holdable off on its own
/// so the arm is a controlled comparison — which is what makes the repair
/// measurable rather than merely deployed.
#[test]
fn only_the_live_bridge_discounts_a_stranded_settler() {
    assert!(
        !AdvancedAi::new().base.settler_strand_discount,
        "the frozen tournament controller must keep its recorded ladders"
    );
    assert!(!AdvancedAi::new().settler_founds_when_stalled);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(
        live.base.settler_strand_discount,
        "the deployment turns it on"
    );
    assert!(
        live.settler_founds_when_stalled,
        "a safe legal city must finish the stranded Settler's conversion"
    );
    live.disable_stranded_settler_discount();
    assert!(
        !live.base.settler_strand_discount,
        "and the control arm holds it off"
    );
    assert!(!live.settler_founds_when_stalled);
}

/// ⚠ The whole point: a Campus with a hundred turns left must not be priced
/// as if the game were nearly over. The old game-fraction horizon said 0.40
/// at turn 150 of 250; the payback horizon says full value, and only ramps
/// down inside the window where a Campus genuinely cannot repay.
#[test]
fn the_campus_horizon_measures_payback_not_the_fraction_of_the_game_left() {
    let mut g = crate::game::Game::new(2, 16, 12, 5, 250, 0);

    for (turn, want_full) in [(1u32, true), (150, true), (209, true)] {
        g.turn = turn;
        let payback = AdvancedAi::campus_payback_horizon(&g);
        assert_eq!(
            payback, 1.0,
            "at turn {turn} of 250 a Campus still repays; got {payback}"
        );
        assert!(want_full);
    }

    // At turn 150 the OLD shape had already discounted it to 0.40, against
    // rivals that never decay at all — a Theatre Square is a flat 850.
    g.turn = 150;
    let old = AdvancedAi::research_horizon(&g);
    assert!(
        old < 0.45,
        "the game-fraction horizon is what starved the late cities: {old}"
    );

    // Inside the payback window it still ramps to nothing, which is why the
    // horizon existed: a Campus begun at turn 245 must not outbid a defender.
    g.turn = 230;
    let late = AdvancedAi::campus_payback_horizon(&g);
    assert!(
        (0.0..=0.6).contains(&late),
        "turn 230 of 250 ramps down: {late}"
    );
    g.turn = 250;
    assert_eq!(AdvancedAi::campus_payback_horizon(&g), 0.0);
    g.turn = 260;
    assert_eq!(
        AdvancedAi::campus_payback_horizon(&g),
        0.0,
        "and it never goes negative past the budget"
    );
}

/// Off by default, set only by the live bridge, holdable off on its own.
#[test]
fn only_the_live_bridge_aims_research_at_the_housing_ceiling() {
    assert!(!AdvancedAi::new().housing_research);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(live.housing_research);
    live.disable_housing_research();
    assert!(!live.housing_research);
}

/// ⚠ It must be INERT for an empire that is not paying the ceiling, and it
/// must name the Aqueduct's tech — asked of the ruleset, not hardcoded.
#[test]
fn the_housing_goal_fires_only_while_the_ceiling_is_being_paid() {
    let mut game = crate::game::Game::new(2, 32, 24, 9_101, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &crate::game::Action::FoundCity { unit: settler })
        .expect("found city");
    let cid = game.player_city_ids(0)[0];
    // The repaired gate (2026-08-19) needs the ceiling to bind the EMPIRE —
    // one capped city of many measured −2.2 pp wins hijacking research — so
    // this probe's empire is two cities, capped together below.
    let second_site = {
        let home = game.cities[&cid].pos;
        game.map
            .tiles
            .iter()
            .filter(|(_, tile)| {
                tile.owner_city.is_none()
                    && game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
            })
            .map(|(position, _)| *position)
            .find(|position| {
                (4..=10).contains(&game.wdist(home, *position))
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(city.pos, *position) >= 4)
            })
            .expect("test map has a nearby city site")
    };
    game.found_city_for(0, second_site, None);
    let second = game
        .player_city_ids(0)
        .into_iter()
        .find(|other| *other != cid)
        .expect("two cities");

    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge_universe();

    // A size-1 city has room to grow, so nothing is being paid.
    game.cities.get_mut(&cid).unwrap().pop = 1;
    assert!(
        game.city_housing_headroom(&game.cities[&cid]) >= HOUSING_CARD_HEADROOM,
        "a new city is not housing-capped"
    );
    assert_eq!(
        ai.unreachable_housing_tech(&game, 0),
        None,
        "a comfortable empire must not divert a beaker to this"
    );

    // Grow BOTH cities until the engine's own band bites: the repaired gate
    // fires only when the cap binds the empire, not one town.
    for city in [cid, second] {
        let capped = (2..40)
            .find(|pop| {
                game.cities.get_mut(&city).unwrap().pop = *pop;
                game.city_housing_headroom(&game.cities[&city]) < HOUSING_CARD_HEADROOM
            })
            .expect("some population overruns this city's housing");
        game.cities.get_mut(&city).unwrap().pop = capped;
    }
    assert_eq!(
        ai.unreachable_housing_tech(&game, 0),
        Some("engineering"),
        "and a capped one is sent at the Aqueduct's tech"
    );

    // And the whole path short-circuits for a frozen controller.
    let legacy = AdvancedAi::legacy();
    assert_eq!(legacy.unreachable_housing_tech(&game, 0), None);
}

/// Off by default, set only by the live bridge, holdable off on its own —
/// the recon-replacement arm follows the same contract as every other
/// bridge repair, so the frozen `advanced_v1` anchor keeps its ladder.
#[test]
fn only_the_live_bridge_replaces_the_recon_arm() {
    // On in production since the 2026-08-17 recon-fleet promotion; the
    // frozen anchor keeps its ladder.
    assert!(AdvancedAi::new().base.recon_replacement);
    assert!(!AdvancedAi::legacy().base.recon_replacement);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(live.base.recon_replacement);
    live.disable_recon_replacement();
    assert!(!live.base.recon_replacement);
}

/// A settler standing on the site it chose, which the host or the mirror
/// has since blocked, must retire that site and pick another — not stand
/// there journaling "already standing on its target" until it dies, as
/// the settler of run civvis-20260815T190904Z did for 25 turns.
#[test]
fn a_settler_on_a_site_it_can_no_longer_found_retires_it_and_moves_on() {
    let mut game = Game::new_full(2, 40, 24, 91_774, 250, 0, false);
    game.current = 0;
    let capital_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(
        0,
        &Action::FoundCity {
            unit: capital_settler,
        },
    )
    .unwrap();
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    // Flat, open, explored land everywhere so the choice is about the
    // block and nothing else.
    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for position in positions {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        game.players[0].explored.insert(position);
    }
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| game.wdist(*position, home) == 5)
        .collect();
    sites.sort_unstable();
    let site = sites[0];
    let settler = game.spawn_test_unit("settler", 0, site);
    assert!(
        game.can_found_city(settler),
        "the fixture site is foundable before the block"
    );

    let mut ai = AdvancedAi::new();
    assert!(
        ai.settler_commit,
        "the deployed controller carries settler_commit"
    );
    assert!(
        !AdvancedAi::legacy().settler_commit,
        "the frozen anchor never reaches this path"
    );
    ai.settler_targets.insert(settler, site);
    // The mirror's loyalty forecast, a host `found_refused`, or a
    // city-state's border all arrive through this one set.
    std::sync::Arc::make_mut(&mut game.blocked_city_sites).insert(site);
    assert!(!game.can_found_city(settler));

    game.units.get_mut(&settler).unwrap().moves_left = 2.0;
    let _ = ai.advanced_settler_step(&mut game, 0, settler);

    assert_ne!(
        ai.settler_targets.get(&settler),
        Some(&site),
        "the blocked site must not survive as the settler's target"
    );
    assert!(
        ai.settler_site_is_dead(settler, site),
        "the retired site is dead for a bounded while so the next pick cannot be it"
    );
    let expiry = ai.settler_dead_sites[&settler][&site];
    assert!(
        expiry > game.turn + game.standard_duration(8),
        "the retirement must outlast the stall counter's own eight-turn avoidance"
    );
    assert!(
        game.units
            .get(&settler)
            .is_some_and(|unit| unit.pos != site)
            || ai
                .settler_targets
                .get(&settler)
                .is_some_and(|next| *next != site),
        "the settler either stepped off the dead site or holds a new target"
    );
}

/// A live settler must reject a frontier site whose speculative city would
/// immediately lose Loyalty, while normal and frozen controllers preserve
/// their established selection path.
#[test]
fn a_live_settler_refuses_a_loyalty_doomed_target_before_walking_there() {
    let mut game = Game::new_full(2, 40, 24, 91_775, 250, 0, false);
    game.current = 0;
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let pos = game.units[&settler].pos;
        game.remove_unit(settler);
        game.found_city_for(pid, pos, None);
    }
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    let ours = game.player_city_ids(0)[0];
    let theirs = game.player_city_ids(1)[0];
    let home = game.cities[&ours].pos;
    let rival = game.cities[&theirs].pos;
    assert!(
        game.wdist(home, rival) >= 12,
        "fixture needs a distant rival"
    );
    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for position in positions {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        game.players[0].explored.insert(position);
    }
    game.cities.get_mut(&theirs).unwrap().pop = 12;
    game.cities.get_mut(&ours).unwrap().pop = 6;

    let mut beside_rival: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(*pos, rival) == 4 && game.wdist(*pos, home) >= 4)
        .collect();
    beside_rival.sort_unstable();
    let doomed = beside_rival[0];
    assert!(
        AdvancedAi::new()
            .settle_site_loyalty_verdict(&game, 0, doomed)
            .is_some(),
        "the fixture must put the candidate below the live emergency threshold"
    );
    let mut beside_home: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(*pos, home) == 4 && game.wdist(*pos, rival) >= 10)
        .collect();
    beside_home.sort_unstable();
    assert!(
        !AdvancedAi::new()
            .settle_site_loyalty_verdict(&game, 0, beside_home[0])
            .is_some(),
        "the fixture must retain a holdable alternative"
    );

    let settler = game.spawn_test_unit("settler", 0, doomed);
    game.units.get_mut(&settler).unwrap().moves_left = 2.0;
    let mut default_board = game.clone();

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    // The loyalty forecast under test rides on `loyalty-rate-alarm`, which the
    // batch rule holds off as of the 2026-08-25 batches; the arm turns it on.
    live.enable_loyalty_rate_alarm();
    assert!(live.base.loyalty_rate_alarm);
    let _ = live.advanced_settler_step(&mut game, 0, settler);
    let retired = live
        .settler_dead_sites
        .get(&settler)
        .cloned()
        .unwrap_or_default();
    assert!(
        !retired.is_empty(),
        "the live bridge retires a doomed candidate before the Settler walks"
    );
    if let Some(target) = live.settler_targets.get(&settler).copied() {
        assert!(!retired.contains_key(&target));
        assert!(
            !live.settle_site_loyalty_verdict(&game, 0, target).is_some(),
            "a retained target must survive the same forecast"
        );
    }

    let mut default = AdvancedAi::new();
    assert!(
        default.settler_commit,
        "the normal controller does use settler_commit"
    );
    assert!(!default.base.loyalty_rate_alarm);
    let _ = default.advanced_settler_step(&mut default_board, 0, settler);
    assert!(
        default
            .settler_dead_sites
            .get(&settler)
            .is_none_or(|sites| sites.is_empty()),
        "the normal controller must not enter the live-only forecast"
    );
}

/// A target forecast at selection cannot see Loyalty pressure that changes
/// during a long march. At the arrival branch, the live controller must
/// recheck the cached site before it spends a Settler founding a city that
/// will immediately revolt; normal and frozen evaluators retain their
/// historical cached-target behavior.
#[test]
fn a_live_settler_rechecks_cached_loyalty_when_it_arrives() {
    let mut game = Game::new_full(2, 40, 24, 91_778, 250, 0, false);
    game.current = 0;
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let pos = game.units[&settler].pos;
        game.remove_unit(settler);
        game.found_city_for(pid, pos, None);
    }
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    let ours = game.player_city_ids(0)[0];
    let theirs = game.player_city_ids(1)[0];
    let home = game.cities[&ours].pos;
    let distant_rival = game.cities[&theirs].pos;
    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for position in positions {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        game.players[0].explored.insert(position);
    }
    game.cities.get_mut(&ours).unwrap().pop = 6;
    // Build a rival outpost eight tiles from our capital and found at the
    // midpoint. The original rival capital remains outside the target's
    // pressure radius, so changing this outpost's population is the only
    // reason the forecast changes between selection and arrival.
    let (near_rival_pos, target) = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|rival| game.wdist(home, *rival) == 8 && game.wdist(distant_rival, *rival) >= 4)
        .find_map(|rival| {
            game.map
                .tiles
                .keys()
                .copied()
                .find(|target| {
                    game.wdist(home, *target) == 4
                        && game.wdist(rival, *target) == 4
                        && game.wdist(distant_rival, *target) > 9
                })
                .map(|target| (rival, target))
        })
        .expect("fixture needs a midpoint outside the distant capital's pressure radius");
    let near_rival = game.found_city_for(1, near_rival_pos, None);
    let settler = game.spawn_test_unit("settler", 0, target);
    game.units.get_mut(&settler).unwrap().moves_left = 2.0;
    assert!(game.can_found_city(settler));

    // This is the board that created the cache. The rival is not yet large
    // enough to make the target untenable.
    game.cities.get_mut(&theirs).unwrap().pop = 1;
    game.cities.get_mut(&near_rival).unwrap().pop = 1;
    assert!(
        !AdvancedAi::new()
            .settle_site_loyalty_verdict(&game, 0, target)
            .is_some(),
        "the cached target must begin viable"
    );

    // A five-turn march is enough for a nearby city to grow or appear in
    // the mirror. At arrival the same target is now a clear loss.
    game.cities.get_mut(&near_rival).unwrap().pop = 20;
    assert!(
        AdvancedAi::new()
            .settle_site_loyalty_verdict(&game, 0, target)
            .is_some(),
        "the arrival board must make the formerly safe target untenable"
    );
    let cities_before = game.player_city_ids(0).len();
    let mut default_board = game.clone();
    let mut frozen_board = game.clone();

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    // The loyalty forecast under test rides on `loyalty-rate-alarm`, which the
    // batch rule holds off as of the 2026-08-25 batches; the arm turns it on.
    live.enable_loyalty_rate_alarm();
    live.settler_targets.insert(settler, target);
    assert!(
        !live.advanced_settler_step(&mut game, 0, settler),
        "the live arrival check must retire the target rather than found"
    );
    assert_eq!(game.player_city_ids(0).len(), cities_before);
    assert!(live.settler_site_is_dead(settler, target));
    assert!(
        !live.settler_targets.contains_key(&settler),
        "the next turn must select a distinct target"
    );

    let mut default = AdvancedAi::new();
    default.settler_targets.insert(settler, target);
    assert!(
        default.advanced_settler_step(&mut default_board, 0, settler),
        "the normal controller keeps its historical cached-target behavior"
    );
    assert_eq!(default_board.player_city_ids(0).len(), cities_before + 1);

    let mut frozen = AdvancedAi::legacy();
    frozen.settler_targets.insert(settler, target);
    assert!(
        frozen.advanced_settler_step(&mut frozen_board, 0, settler),
        "the frozen evaluator is unchanged by the live arrival guard"
    );
    assert_eq!(frozen_board.player_city_ids(0).len(), cities_before + 1);
}

/// When every candidate examined by the live forecast is doomed, holding
/// is safer than falling through to `BasicAi`, which cannot see the
/// retired-site set and may found on the very plot just rejected.
#[test]
fn a_live_settler_holds_when_every_new_target_is_loyalty_doomed() {
    let mut game = Game::new_full(2, 40, 24, 91_776, 250, 0, false);
    game.current = 0;
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let pos = game.units[&settler].pos;
        game.remove_unit(settler);
        game.found_city_for(pid, pos, None);
    }
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    let ours = game.player_city_ids(0)[0];
    let theirs = game.player_city_ids(1)[0];
    let home = game.cities[&ours].pos;
    let rival = game.cities[&theirs].pos;
    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for position in positions {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        game.players[0].explored.insert(position);
    }
    game.cities.get_mut(&theirs).unwrap().pop = 12;
    game.cities.get_mut(&ours).unwrap().pop = 6;
    let mut beside_rival: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(*pos, rival) == 4 && game.wdist(*pos, home) >= 4)
        .collect();
    beside_rival.sort_unstable();
    let doomed = beside_rival[0];
    assert!(AdvancedAi::new()
        .settle_site_loyalty_verdict(&game, 0, doomed)
        .is_some());
    // Give the sole candidate a deliberately strong first two rings. The
    // test must exercise a real target pick, not pass because the normal
    // value threshold rejects every flat, frontier site before Loyalty is
    // considered.
    for position in game.wdisk(doomed, 2) {
        if let Some(tile) = game.map.tiles.get_mut(&position) {
            tile.resource = Some(crate::name!("rice"));
        }
    }
    for position in game.map.tiles.keys().copied().collect::<Vec<_>>() {
        if position != doomed {
            std::sync::Arc::make_mut(&mut game.blocked_city_sites).insert(position);
        }
    }
    let settler = game.spawn_test_unit("settler", 0, doomed);
    game.units.get_mut(&settler).unwrap().moves_left = 2.0;
    let cities_before = game.player_city_ids(0).len();

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    // The loyalty forecast under test rides on `loyalty-rate-alarm`, which the
    // batch rule holds off as of the 2026-08-25 batches; the arm turns it on.
    live.enable_loyalty_rate_alarm();
    assert!(
        !game.blocked_city_sites.contains(&doomed),
        "the lone candidate must remain available to the target picker"
    );
    assert_eq!(
        live.best_settler_target(&game, 0, settler, 8, None)
            .map(|(position, _)| position),
        Some(doomed),
        "the fixture must force the forecast to examine its lone candidate"
    );
    assert!(
        !live.advanced_settler_step(&mut game, 0, settler),
        "the safe exhaustion is a no-op, not an unfiltered fallback"
    );
    assert_eq!(game.player_city_ids(0).len(), cities_before);
    assert_eq!(game.units[&settler].pos, doomed);
    assert!(live.settler_site_is_dead(settler, doomed));
    assert!(
        !live.settler_targets.contains_key(&settler),
        "no doomed target is retained after exhaustion"
    );
}

/// A global site ranking can quite reasonably put three rich, distant colonies
/// ahead of a merely adequate nearby city. When all three are beyond the
/// unexplored Loyalty horizon, the live controller must still try the nearby
/// ranked site before leaving the Settler parked indefinitely.
#[test]
fn live_settler_uses_a_ranked_local_site_after_distant_loyalty_failures() {
    let mut game = Game::new_full(2, 40, 24, 91_780, 250, 0, false);
    game.current = 0;
    let founding_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let home = game.units[&founding_settler].pos;
    game.remove_unit(founding_settler);
    game.found_city_for(0, home, None);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    for position in game.map.tiles.keys().copied().collect::<Vec<_>>() {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.river_edges = [false; 6];
        tile.riverside = false;
    }
    let known_home = game.wdisk(home, 6);
    game.players[0].explored.clear();
    game.players[0].explored.extend(known_home);

    let local = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.wdist(home, *position) == 6)
        .expect("fixture needs a local settlement ring");
    let source = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.wdist(local, *position) == 8 && game.wdist(home, *position) >= 6)
        .expect("fixture needs a starting tile within the local fallback radius");
    let mut distant = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| {
            game.wdist(home, *position) >= 7
                && (9..=12).contains(&game.wdist(source, *position))
                && game.wdist(local, *position) >= 8
                && game
                    .wdisk(*position, 2)
                    .into_iter()
                    .all(|ring| game.map.tiles.contains_key(&ring))
        })
        .collect::<Vec<_>>();
    distant.sort_unstable();
    let distant = distant.into_iter().step_by(8).take(3).collect::<Vec<_>>();
    assert_eq!(distant.len(), 3, "fixture needs three distant colonies");
    for site in &distant {
        for position in game.wdisk(*site, 2) {
            if let Some(tile) = game.map.tiles.get_mut(&position) {
                tile.hills = true;
                tile.resource = Some(crate::name!("rice"));
            }
        }
        assert!(
            AdvancedAi::beyond_loyalty_reach(&game, 0, *site),
            "each rich distant site must be outside the known Loyalty halo"
        );
    }
    for site in &distant {
        let sight = game
            .nbrs(*site)
            .into_iter()
            .find(|position| !distant.contains(position) && *position != local)
            .expect("fixture needs a distinct scouting tile beside each distant site");
        game.spawn_test_unit("scout", 0, sight);
    }
    for position in game.map.tiles.keys().copied().collect::<Vec<_>>() {
        if position != local && !distant.contains(&position) {
            std::sync::Arc::make_mut(&mut game.blocked_city_sites).insert(position);
        }
    }
    let settler = game.spawn_test_unit("settler", 0, source);
    game.units.get_mut(&settler).unwrap().moves_left = 2.0;

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    live.enable_loyalty_rate_alarm();
    assert!(live.frontier_loyalty);
    let selected = live
        .best_settler_target(&game, 0, settler, 8, None)
        .map(|(position, _)| position);
    let ranking = live.settle_ranking(&game, 0, source, 12);
    assert!(
        selected.is_some_and(|position| distant.contains(&position)),
        "the main ranking must try a richer distant site before the local fallback; \
         home {home:?}, source {source:?}, selected {selected:?}, local {local:?}, \
         distant {distant:?}, ranking {ranking:?}"
    );
    assert!(
        live.advanced_settler_step(&mut game, 0, settler),
        "a verified-safe local site must replace a string of doomed distant targets"
    );
    assert_eq!(live.settler_targets.get(&settler), Some(&local));
    for site in distant {
        assert!(
            live.settler_site_is_dead(settler, site),
            "the live Loyalty gate must still retire every distant failure"
        );
    }
    assert!(
        !live.settler_site_is_dead(settler, local),
        "the safe local fallback must remain viable"
    );
}

/// A stalled Settler may finish a safe fallback such as Cumae, where its
/// own support was closer than Khmer's city. It must not turn the opposite
/// geometry into Puteoli: on t175 it stood six tiles from Rome's nearest
/// city and five from Khmer's Ostia, then was captured before it paid back.
#[test]
fn live_stalled_settler_refuses_an_unsupported_hostile_frontier() {
    let mut game = Game::new_full(2, 40, 24, 91_777, 250, 0, false);
    game.current = 0;
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    let (here, friendly, hostile) = positions
        .iter()
        .copied()
        .find_map(|here| {
            game.wring(here, 6).into_iter().find_map(|friendly| {
                game.wring(here, 5)
                    .into_iter()
                    .find(|hostile| game.wdist(friendly, *hostile) >= 10)
                    .map(|hostile| (here, friendly, hostile))
            })
        })
        .expect("the fixture needs a settled side of the map with five- and six-tile rings");
    for position in positions {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        game.players[0].explored.insert(position);
    }
    game.found_city_for(0, friendly, None);
    game.found_city_for(1, hostile, None);
    game.at_war.insert((0, 1));
    assert_eq!(game.wdist(here, friendly), 6);
    assert_eq!(game.wdist(here, hostile), 5);
    let settler = game.spawn_test_unit("settler", 0, here);
    assert!(
        game.can_found_city(settler),
        "the frontier remains legal to found"
    );
    let mut control_board = game.clone();

    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(live.settler_founds_when_stalled);
    assert_eq!(
        live.live_stalled_settlement_is_unsupported_frontline(&game, 0, here),
        Some((5, 6)),
        "the revealed hostile city is closer than friendly support"
    );
    let cities_before = game.player_city_ids(0).len();
    assert!(
        !live.founds_where_it_stands(&mut game, 0, settler, here),
        "the live fallback must not found the unsupported outpost"
    );
    assert_eq!(game.player_city_ids(0).len(), cities_before);
    assert!(
        live.settler_site_is_dead(settler, here),
        "the exposed fallback is retired long enough to require a genuine re-plan"
    );

    let mut control = AdvancedAi::new();
    control.enable_settler_founds_when_stalled();
    assert_eq!(
        control.live_stalled_settlement_is_unsupported_frontline(&control_board, 0, here),
        None,
        "the evaluator-only fallback preserves its prior behavior without the live gate"
    );
    assert!(
        control.founds_where_it_stands(&mut control_board, 0, settler, here),
        "outside the live bridge the historical stalled-settler fallback still applies"
    );
    assert_eq!(control_board.player_city_ids(0).len(), cities_before + 1);
}

/// Cumae showed that the stalled-settler fallback could bypass the ordinary
/// Loyalty forecast: t63 rejected this fogged tile as beyond Rome's support,
/// yet t72 founded there after the walk stalled; it revolted at t82. A city
/// that cannot hold is not the wide expansion the fallback is meant to save.
#[test]
fn live_stalled_settler_refuses_a_loyalty_doomed_fallback() {
    let mut game = Game::new_full(2, 40, 24, 91_779, 250, 0, false);
    game.current = 0;
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    let (here, friendly, fogged) = positions
        .iter()
        .copied()
        .find_map(|here| {
            game.wring(here, 8)
                .into_iter()
                .filter(|friendly| game.map.tiles.contains_key(friendly))
                .find_map(|friendly| {
                    game.wdisk(here, 9)
                        .into_iter()
                        .find(|fogged| *fogged != here && game.map.tiles.contains_key(fogged))
                        .map(|fogged| (here, friendly, fogged))
                })
        })
        .expect("fixture needs a frontier eight tiles from its friendly city");
    for position in positions.iter() {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
    }
    game.found_city_for(0, friendly, None);
    assert_eq!(game.wdist(here, friendly), 8);
    let settler = game.spawn_test_unit("settler", 0, here);
    // Founding refreshes ordinary city vision, so set the single fogged
    // pressure tile after every fixture unit has refreshed its vision rather
    // than accidentally testing a revealed frontier.
    game.players[0].explored.clear();
    game.players[0]
        .explored
        .extend(positions.into_iter().filter(|position| *position != fogged));
    assert!(
        game.can_found_city(settler),
        "the fallback tile remains legal"
    );
    let own_city_distances: Vec<i32> = game
        .cities
        .values()
        .filter(|city| city.owner == 0)
        .map(|city| game.wdist(city.pos, here))
        .collect();
    let nearby_fog = game
        .wdisk(here, 9)
        .into_iter()
        .filter(|position| {
            game.map.tiles.contains_key(position) && !game.players[0].explored.contains(position)
        })
        .count();
    assert!(
        AdvancedAi::beyond_loyalty_reach(&game, 0, here),
        "the fixture needs no friendly city inside the seven-tile reach and nearby fog; \
         distances={own_city_distances:?}, fogged={fogged:?}, nearby_fog={nearby_fog}"
    );
    let mut control_board = game.clone();

    // ⚠ THE UNIVERSE, NOT THE DEPLOYMENT GENOME. What this pins is the live
    // guard on the stalled-settler fallback, so the fallback has to exist: it
    // is switched on as a side effect of `enable_stranded_settler_discount`,
    // and on 2026-08-23 the standard screen took that gene out of the shipped
    // genome, which turned `settler_founds_when_stalled` off and made
    // `founds_where_it_stands` return `false` at its first line — the assertion
    // below still passed, for entirely the wrong reason, and the site was never
    // retired. A behaviour test must not be able to pass because a gene stopped
    // shipping. Its sibling above already reads the universe; this one was
    // missed, and only worked while the gene happened to be on.
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(live.base.loyalty_rate_alarm);
    assert!(live.frontier_loyalty);
    assert!(
        live.settler_founds_when_stalled,
        "the fallback under test must be reachable, whatever the ledger ships"
    );
    assert!(
        live.settle_site_loyalty_verdict(&game, 0, here).is_some(),
        "the live forecast must identify the unsupported fogged frontier"
    );
    let cities_before = game.player_city_ids(0).len();
    assert!(
        !live.founds_where_it_stands(&mut game, 0, settler, here),
        "the live fallback must not found a city that the same forecast rejects"
    );
    assert_eq!(game.player_city_ids(0).len(), cities_before);
    assert!(
        live.settler_site_is_dead(settler, here),
        "the doomed fallback is retired so the Settler can make a genuinely safe plan"
    );

    let mut control = AdvancedAi::new();
    control.enable_settler_founds_when_stalled();
    assert!(!control.base.loyalty_rate_alarm);
    assert!(
        control.founds_where_it_stands(&mut control_board, 0, settler, here),
        "the evaluator-only fallback retains its historical behavior"
    );
    assert_eq!(control_board.player_city_ids(0).len(), cities_before + 1);
}

/// Off by default, set only by the live bridge, holdable off on its own —
/// the wonder race follows the same contract as every other bridge
/// repair, so the frozen `advanced_v1` anchor keeps its ladder.
#[test]
fn only_the_live_bridge_races_for_wonders() {
    assert!(!AdvancedAi::new().live_wonder_race);
    assert!(!AdvancedAi::legacy().live_wonder_race);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert!(live.live_wonder_race);
    live.disable_live_wonder_race();
    assert!(!live.live_wonder_race);
}

/// The live seat had never ordered a wonder because the `Item::Wonder`
/// arm returns −10 000 for every plan but Culture and every lane but
/// Score. On the live seat a developed city in a three-city empire prices
/// the wonder at the Score-lane valuation; a second city may not open a
/// second race while the first is still building; and the stock
/// controller keeps refusing exactly as it always did.
#[test]
fn live_seat_races_for_one_wonder_at_a_time_and_stock_still_refuses() {
    let mut game = Game::new_full(1, 32, 20, 91_772, 250, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = game.player_city_ids(0)[0];
    let techs: Vec<Name> = game.rules.techs.keys().cloned().collect();
    let civics: Vec<Name> = game.rules.civics.keys().cloned().collect();
    game.players[0].techs.extend(techs);
    game.players[0].civics.extend(civics);
    for position in game.cities[&capital].owned_tiles.clone() {
        if position == game.cities[&capital].pos {
            continue;
        }
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("desert");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
    }
    // Three buildings make the capital "developed"; two more cities make
    // the empire one that has begun expanding.
    for building in ["monument", "granary", "water_mill"] {
        game.cities
            .get_mut(&capital)
            .unwrap()
            .buildings
            .push(Name::new(building));
    }
    let home = game.cities[&capital].pos;
    let owned_tiles = game.cities[&capital].owned_tiles.clone();
    let (stonehenge_site, stone) = owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != home)
        .find_map(|site| {
            game.nbrs(site)
                .iter()
                .copied()
                .find(|neighbor| *neighbor != home && owned_tiles.contains(neighbor))
                .map(|stone| (site, stone))
        })
        .expect("the capital owns neighboring non-center tiles");
    for position in [stonehenge_site, stone] {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
    }
    game.map.tiles.get_mut(&stone).unwrap().resource = Some(Name::new("stone"));
    let mut founded = 0;
    let mut sites: Vec<Pos> = game.map.tiles.keys().copied().collect();
    sites.sort_unstable();
    for site in sites {
        if founded == 2 {
            break;
        }
        let distant = game.wdist(site, home) >= 5
            && game
                .player_city_ids(0)
                .iter()
                .all(|c| game.wdist(site, game.cities[c].pos) >= 5);
        let tile = &game.map.tiles[&site];
        if distant && game.rules.is_passable(tile) && !game.rules.is_water(tile) {
            game.found_city_for(0, site, None);
            founded += 1;
        }
    }
    assert_eq!(
        game.player_city_ids(0).len(),
        3,
        "fixture must hold three cities"
    );
    let pyramids = game
        .producible_items(0, capital)
        .into_iter()
        .find(|item| matches!(item, Item::Wonder { wonder, .. } if wonder == "pyramids"))
        .expect("the desert capital can place the Pyramids");
    assert!(
        game.rules.wonders["stonehenge"]
            .effects
            .get("religion_founding_site")
            .is_some_and(|value| *value > 0.0),
        "the fixture's founding-site marker remains data-driven"
    );
    let stonehenge = game
        .producible_items(0, capital)
        .into_iter()
        .find(|item| matches!(item, Item::Wonder { wonder, .. } if wonder == "stonehenge"))
        .expect("the grassland next to stone permits Stonehenge");
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: game.turn,
        rush: false,
    };

    let stock = AdvancedAi::new();
    assert!(
        stock.production_value(&game, 0, capital, &pyramids, &plan, &stock.counts(&game, 0))
            < -1_000.0,
        "the stock controller keeps refusing wonders outside the Culture plan"
    );

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    // This test isolates the live-only race's era and queue limits. The
    // separately scored tally lane intentionally does not share them, so it
    // is held off here whatever the deployment selection says about it —
    // the operator pinned it on (2026-08-27) and then off (2026-08-28), and
    // neither reading is this test's subject.
    live.disable_wonder_score_tally();
    assert!(!live.wonder_score_tally);
    let raced = live.production_value(&game, 0, capital, &pyramids, &plan, &live.counts(&game, 0));
    assert!(raced > 0.0, "the live seat opens the wonder arm: {raced}");

    // An explicit Science target keeps the default live wonder treatment from
    // opening a generic score race. Science-specific wonders remain available
    // through their strategic lane value; this check is the generic fallback.
    let mut science_target = AdvancedAi::targeting(VictoryTarget::Science);
    science_target.enable_live_bridge();
    let science_plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        ..plan.clone()
    };
    let refused = science_target.production_value(
        &game,
        0,
        capital,
        &pyramids,
        &science_plan,
        &science_target.counts(&game, 0),
    );
    assert!(
        refused <= -10_000.0,
        "a targeted Science lane refuses the generic wonder race: {refused}"
    );

    // ★★★★ And prices it higher as the tally approaches: ×1 at the start,
    // ×2 at the midpoint, ×3 at the limit. See `live_wonder_race_scale`.
    let turn_before = game.turn;
    game.turn = 0;
    assert!((AdvancedAi::live_wonder_race_scale(&game) - 1.0).abs() < 1e-9);
    let raced = live.production_value(&game, 0, capital, &pyramids, &plan, &live.counts(&game, 0));
    game.turn = game.max_turns / 2;
    assert!((AdvancedAi::live_wonder_race_scale(&game) - 2.0).abs() < 1e-9);
    let raced_midgame =
        live.production_value(&game, 0, capital, &pyramids, &plan, &live.counts(&game, 0));
    assert!(
        raced_midgame > raced * 1.5,
        "the midgame wonder is worth more than the opening one: {raced_midgame} vs {raced}"
    );
    game.turn = game.max_turns;
    assert!((AdvancedAi::live_wonder_race_scale(&game) - 3.0).abs() < 1e-9);
    game.turn = turn_before;
    let stonehenge_before_founding = live.production_value(
        &game,
        0,
        capital,
        &stonehenge,
        &plan,
        &live.counts(&game, 0),
    );
    assert!(
            stonehenge_before_founding > 0.0,
            "the same legal founding-site wonder remains available before religion: {stonehenge_before_founding}"
        );
    game.players[0].religion = Some("Already Founded".to_string());
    let stonehenge_after_founding = live.production_value(
        &game,
        0,
        capital,
        &stonehenge,
        &plan,
        &live.counts(&game, 0),
    );
    assert!(
            stonehenge_after_founding < -1_000.0,
            "the live race does not sink production into a spent founding effect: {stonehenge_after_founding}"
        );
    game.players[0].religion = None;
    let recovery_plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        ..plan.clone()
    };
    assert!(
        live.production_value(
            &game,
            0,
            capital,
            &pyramids,
            &recovery_plan,
            &live.counts(&game, 0),
        ) < -1_000.0,
        "Recovery must reserve even a locally safe city's production for defense"
    );
    let mut score_lane = AdvancedAi::targeting(VictoryTarget::Score);
    score_lane.enable_live_bridge();
    let scored = score_lane.production_value(
        &game,
        0,
        capital,
        &pyramids,
        &plan,
        &score_lane.counts(&game, 0),
    );
    assert!(
        raced > scored && scored > 0.0,
        "the race must out-price the Score lane's token wonder appetite: {raced} vs {scored}"
    );

    // A second city may not open a second race while the capital builds.
    game.cities.get_mut(&capital).unwrap().queue = vec![pyramids.clone()];
    let other = game
        .player_city_ids(0)
        .into_iter()
        .find(|c| *c != capital)
        .unwrap();
    for building in ["monument", "granary", "water_mill"] {
        game.cities
            .get_mut(&other)
            .unwrap()
            .buildings
            .push(Name::new(building));
    }
    // The cheapest other wonder the second city can place, so a young
    // city's build time is not what refuses it below.
    let another = game
        .producible_items(0, other)
        .into_iter()
        .filter(|item| matches!(item, Item::Wonder { wonder, .. } if wonder != "pyramids"))
        .min_by(|left, right| {
            game.item_remaining_cost_for_city(0, other, left)
                .total_cmp(&game.item_remaining_cost_for_city(0, other, right))
        });
    if let Some(another) = &another {
        assert!(
            live.production_value(&game, 0, other, another, &plan, &live.counts(&game, 0))
                < -1_000.0,
            "one wonder in production empire-wide at a time in a three-city empire"
        );
    }

    // ★★★★ A second lane opens at six cities: the same second city may
    // then race while the capital builds, and a third city may not open a
    // third. See `live_wonder_race_lanes`.
    assert_eq!(AdvancedAi::live_wonder_race_lanes(3), 1);
    assert_eq!(AdvancedAi::live_wonder_race_lanes(5), 1);
    assert_eq!(AdvancedAi::live_wonder_race_lanes(6), 2);
    assert_eq!(AdvancedAi::live_wonder_race_lanes(11), 2);
    assert_eq!(AdvancedAi::live_wonder_race_lanes(12), 3);
    let mut more_sites: Vec<Pos> = game.map.tiles.keys().copied().collect();
    more_sites.sort_unstable();
    for site in more_sites {
        if game.player_city_ids(0).len() == 6 {
            break;
        }
        let distant = game
            .player_city_ids(0)
            .iter()
            .all(|c| game.wdist(site, game.cities[c].pos) >= 5);
        let tile = &game.map.tiles[&site];
        if distant && game.rules.is_passable(tile) && !game.rules.is_water(tile) {
            game.found_city_for(0, site, None);
        }
    }
    assert_eq!(
        game.player_city_ids(0).len(),
        6,
        "fixture must reach six cities"
    );
    if let Some(another) = &another {
        // The second city is young and slow; lengthen the game so the
        // wonder's build time, not the lane count, is not what refuses it.
        let max_turns = game.max_turns;
        game.max_turns = 1_000;
        let second_lane =
            live.production_value(&game, 0, other, another, &plan, &live.counts(&game, 0));
        assert!(
            second_lane > 0.0,
            "six cities open a second wonder lane while the capital builds: {second_lane} \
                 ({another:?})"
        );
        game.cities.get_mut(&other).unwrap().queue = vec![another.clone()];
        let third = game
            .player_city_ids(0)
            .into_iter()
            .find(|c| *c != capital && *c != other)
            .unwrap();
        for building in ["monument", "granary", "water_mill"] {
            game.cities
                .get_mut(&third)
                .unwrap()
                .buildings
                .push(Name::new(building));
        }
        if let Some(yet_another) = game
            .producible_items(0, third)
            .into_iter()
            .find(|item| matches!(item, Item::Wonder { .. }))
        {
            assert!(
                live.production_value(&game, 0, third, &yet_another, &plan, &live.counts(&game, 0))
                    < -1_000.0,
                "two lanes are full: a third city does not open a third race"
            );
        }
        game.cities.get_mut(&other).unwrap().queue.clear();
        game.max_turns = max_turns;
    }

    // A wonder three eras behind the world is one the Firaxis rivals have
    // long since finished: the race does not ask for it.
    game.cities.get_mut(&capital).unwrap().queue.clear();
    game.world_era = 3;
    assert!(
        live.production_value(&game, 0, capital, &pyramids, &plan, &live.counts(&game, 0))
            < -1_000.0,
        "an ancient wonder is not raced once the world is medieval"
    );
    game.world_era = 2;
    assert!(
        live.production_value(&game, 0, capital, &pyramids, &plan, &live.counts(&game, 0)) > 0.0,
        "wonders two eras behind the world are still raced"
    );
    game.world_era = 0;

    // Withholding the treatment restores the stock refusal.
    live.disable_live_wonder_race();
    assert!(
        live.production_value(&game, 0, capital, &pyramids, &plan, &live.counts(&game, 0))
            < -1_000.0
    );
}

/// Off by default, set only by the live bridge, each holdable off on its
/// own — the war-conversion pair follows the same contract as every other
/// bridge repair.
#[test]
fn only_the_live_bridge_fights_the_war_conversion_pair() {
    let fresh = AdvancedAi::new();
    assert!(!fresh.war_economy);
    assert!(!fresh.war_reinforcement);
    let legacy = AdvancedAi::legacy();
    assert!(!legacy.war_economy && !legacy.war_reinforcement);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(live.war_economy && live.war_reinforcement);
    live.disable_war_economy();
    live.disable_war_reinforcement();
    assert!(!live.war_economy && !live.war_reinforcement);
}

/// The removal note stands where the routing stood: `take_turn`'s dispatch
/// records the three repairs and the numbers that ended them.
#[test]
fn the_war_economy_conquest_routing_is_gone_from_the_dispatch() {
    let src = include_str!("../advanced.rs");
    let block = src
        .split("|| adaptive_expansion_dispatch")
        .nth(1)
        .expect("the production routing condition exists")
        .split("self.advanced_production(g, pid, &plan, adaptive_expansion_dispatch);")
        .next()
        .expect("the routing block ends at the production call");
    assert!(
        block.contains("CONQUEST ROUTING WAS REMOVED"),
        "the dispatch must carry the removal note where the routing stood"
    );
    assert!(
        !block.contains("self.war_economy"),
        "no war-economy disjunct may return to the dispatch"
    );
}

/// The live mirror can know a leader's public victory clock before it has
/// any city coordinate for that leader. That warning must not replace a
/// campaign against somebody else. See `conquest_denial_actionable`.
#[test]
fn a_live_conquest_denial_needs_a_known_city_target() {
    let mut game = Game::new_full(3, 24, 16, 7_932, 300, 0, false);
    for pid in 0..3 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    for _ in 0..3 {
        game.spawn_test_unit("modern_armor", 0, home);
    }
    let leader_capital = game.player_city_ids(2)[0];
    game.players[2]
        .science_projects
        .insert("exoplanet_expedition".to_string());
    game.current = 0;
    game.turn = 60;
    game.record_contact(0, 1);
    game.record_contact(0, 2);
    game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    game.turn = 100;

    // The host has announced player 2's victory progress, but not a city
    // position. Move its capital into player 1's public city list to model
    // that omitted rival-city export without leaving a dangling map tile.
    game.cities.get_mut(&leader_capital).unwrap().owner = 1;
    assert!(game.player_city_ids(2).is_empty());

    let mut live = AdvancedAi::new();
    assert_eq!(
        live.victory_denial(&game, 0),
        Some((2, GrandStrategy::Conquest)),
        "the raw public warning remains available to non-military responses"
    );
    assert_eq!(
        live.denial_target(&game, 0),
        None,
        "a live Conquest response needs a known city to campaign against"
    );
    let plan = live.assess(&game, 0);
    assert_eq!(
        plan.target_player,
        Some(1),
        "the cityless leader cannot replace the active campaign"
    );
    live.plan = Some(plan);
    assert!(
        !live.plan_stale(&game, 0),
        "the raw warning must not interrupt the economic plan every turn"
    );

    // Restoring the leader's actual city makes the same emergency a valid
    // Conquest denial again.
    game.cities.get_mut(&leader_capital).unwrap().owner = 2;
    assert_eq!(
        live.denial_target(&game, 0),
        Some((2, GrandStrategy::Conquest))
    );
    let actionable = live.assess(&game, 0);
    assert_eq!(actionable.strategy, GrandStrategy::Conquest);
    assert_eq!(
        actionable.target_player,
        Some(1),
        "the active campaign still finishes before the denial can redirect it"
    );
    assert!(
        live.plan_stale(&game, 0),
        "an actionable leader still interrupts the cached economic plan"
    );

    // `advanced_v1` retains the historical all-information response.
    game.cities.get_mut(&leader_capital).unwrap().owner = 1;
    assert_eq!(
        AdvancedAi::legacy().denial_target(&game, 0),
        Some((2, GrandStrategy::Conquest))
    );
}

/// The "strong enough to take what a neighbour has" branch fired in every
/// one of the last eight live runs and never once produced a captured
/// city; sixteen cities were lost. See `no_elective_war`.
#[test]
fn the_live_seat_does_not_open_an_elective_war() {
    let mut game = Game::new_full(2, 24, 16, 7_932, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    // Two cities, past turn 55, an army that dwarfs the neighbour's: the
    // stock premise of the elective branch.
    let mut sites = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            tile.owner_city.is_none()
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && (4..=10).contains(&game.wdist(home, **position))
        })
        .map(|(position, _)| *position)
        .collect::<Vec<_>>();
    sites.sort();
    let second = sites
        .iter()
        .copied()
        .find(|site| {
            game.cities
                .values()
                .all(|city| game.wdist(city.pos, *site) >= 4)
        })
        .expect("the test map has a second city site");
    game.found_city_for(0, second, None);
    for _ in 0..3 {
        game.spawn_test_unit("modern_armor", 0, home);
    }
    game.current = 0;
    game.turn = 60;
    game.record_contact(0, 1);
    assert!(!game.is_at_war(0, 1));

    let mut stock = AdvancedAi::new();
    assert!(!stock.no_elective_war);
    let journal = crate::reasoning::Journal::recording();
    stock.attach_journal(journal.handle());
    assert_eq!(
        stock.assess(&game, 0).strategy,
        GrandStrategy::Conquest,
        "the stock chain takes the elective branch"
    );
    assert!(journal.since(0).thoughts.iter().any(|thought| thought
        .detail
        .contains("strong enough to take what a neighbour has")));
    let frozen = AdvancedAi::legacy();
    assert!(!frozen.no_elective_war);
    assert_eq!(frozen.assess(&game, 0).strategy, GrandStrategy::Conquest);

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert!(live.no_elective_war);
    assert!(
        live.elective_war_yields_to_a_lane,
        "the deployment routes elective-war yields through the active lane"
    );
    let journal = crate::reasoning::Journal::recording();
    live.attach_journal(journal.handle());
    assert_ne!(
        live.assess(&game, 0).strategy,
        GrandStrategy::Conquest,
        "the live seat does not open an elective war"
    );
    assert!(!journal.since(0).thoughts.iter().any(|thought| thought
        .detail
        .contains("strong enough to take what a neighbour has")));

    // The control arm takes the branch again when both independent policies
    // that suppress its old stock heuristic are withheld.
    live.disable_no_elective_war();
    live.disable_elective_war_yields_to_a_lane();
    assert_eq!(live.assess(&game, 0).strategy, GrandStrategy::Conquest);
    live.enable_elective_war_yields_to_a_lane();
    live.enable_no_elective_war();

    // A war the neighbour opens is still answered: "already at war" is
    // not the elective branch.
    game.current = 1;
    game.apply(1, &Action::DeclareWar { player: 0 }).unwrap();
    game.current = 0;
    assert!(game.is_at_war(0, 1));
    // ⚠ `unchosen_war_keeps_the_lane` (deployed 2026-08-26) reaches this same
    // branch from the other side: a war we did NOT declare, over a lane that
    // is live, names the lane rather than Conquest. Withhold it to read the
    // branch this test is about, then restore it and read what the deployed
    // seat actually plays — the two are different answers to the same board.
    live.disable_unchosen_war_keeps_the_lane();
    assert_eq!(
        live.assess(&game, 0).strategy,
        GrandStrategy::Conquest,
        "a war the neighbour opens is still answered by ordinary war planning"
    );
    live.enable_unchosen_war_keeps_the_lane();
    assert_ne!(
        live.assess(&game, 0).strategy,
        GrandStrategy::Conquest,
        "the deployed seat keeps the live lane through a war it did not declare"
    );
}

/// A settle site pressed against a rival's border is a provocation the
/// live seat has paid for with cities three times in five games. See
/// `foreign_border_pressure`.
#[test]
fn a_site_inside_a_rivals_border_is_priced_as_a_provocation() {
    let mut game = Game::new_full(2, 32, 20, 7_934, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let theirs = game.player_city_ids(1)[0];
    let their_home = game.cities[&theirs].pos;
    // A plot two steps outside their centre, ringed by their owned tiles.
    let pressed = game
        .wring(their_home, 3)
        .into_iter()
        .find(|pos| {
            game.map.get(*pos).is_some_and(|t| {
                !game.rules.is_water(t) && game.rules.is_passable(t) && t.owner_city.is_none()
            }) && game
                .wdisk(*pos, FOREIGN_BORDER_RADIUS)
                .iter()
                .any(|near| game.map.get(*near).and_then(|t| t.owner_city) == Some(theirs))
        })
        .expect("some open plot near the rival's border");
    let owned_near = game
        .wdisk(pressed, FOREIGN_BORDER_RADIUS)
        .iter()
        .filter(|near| game.map.get(**near).and_then(|t| t.owner_city) == Some(theirs))
        .count();
    assert!(owned_near > 0);
    // A plot well away from anyone's border.
    let ours = game.cities[&game.player_city_ids(0)[0]].pos;
    let quiet = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|t| !game.rules.is_water(t) && game.rules.is_passable(t))
                && game
                    .wdisk(*pos, FOREIGN_BORDER_RADIUS)
                    .iter()
                    .all(|near| game.map.get(*near).and_then(|t| t.owner_city).is_none())
        })
        .min_by_key(|pos| game.wdist(*pos, ours))
        .expect("some plot clear of every border");

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert!(live.settlement_safety);
    let pressure = live.foreign_border_pressure(&game, 0, pressed);
    assert!(
        (pressure
            - (owned_near as f64 * FOREIGN_BORDER_TILE_PENALTY).min(FOREIGN_BORDER_PENALTY_CAP))
        .abs()
            < 1e-9,
        "priced per rival-owned tile within {FOREIGN_BORDER_RADIUS}, capped"
    );
    assert_eq!(live.foreign_border_pressure(&game, 0, quiet), 0.0);
    // Our own border is not a provocation.
    let home_ring = game
        .wring(ours, 2)
        .into_iter()
        .find(|pos| game.map.get(*pos).is_some_and(|t| !game.rules.is_water(t)))
        .expect("a plot in our own territory");
    assert_eq!(live.foreign_border_pressure(&game, 0, home_ring), 0.0);
    // The penalty reaches the site value.
    let visible = game.player_vision_frame(0);
    let with = live.settlement_safety_penalty(&game, 0, pressed, &visible);
    live.settlement_safety = false;
    let without = live.settlement_safety_penalty(&game, 0, pressed, &visible);
    assert!(
        (with - without - pressure).abs() < 1e-9,
        "the safety penalty carries exactly the border term under settlement_safety"
    );
    // The frozen anchor never prices it.
    let frozen = AdvancedAi::legacy();
    assert!(!frozen.settlement_safety);
    assert_eq!(
        frozen.settlement_safety_penalty(&game, 0, pressed, &visible),
        without
    );
}

/// Every live game read `Grand strategy: religion` from turn 19–26 with one
/// or two cities, and the capital built the Holy Site while the second
/// city could not make a settler. See `expansion_before_prophet`.
#[test]
fn the_third_city_comes_before_the_prophet_on_the_live_seat() {
    let mut game = Game::new_full(2, 24, 16, 7_931, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    // Sites 4–10 tiles from home, clear of every city, on walkable ground.
    let mut sites = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            tile.owner_city.is_none()
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && (4..=10).contains(&game.wdist(home, **position))
        })
        .map(|(position, _)| *position)
        .collect::<Vec<_>>();
    sites.sort();
    let found_next = |game: &mut Game| {
        let site = sites
            .iter()
            .copied()
            .find(|site| {
                game.cities
                    .values()
                    .all(|city| game.wdist(city.pos, *site) >= 4)
            })
            .expect("the test map has another city site");
        game.found_city_for(0, site, None)
    };
    found_next(&mut game);
    assert_eq!(game.player_city_ids(0).len(), 2);
    // The Prophet race is on: a Prophet is already pending for the seat.
    game.players[0].prophet_pending = true;
    game.current = 0;
    game.turn = 26;
    assert!(AdvancedAi::expansion_window_open(&game));

    let mut stock = AdvancedAi::new();
    assert!(!stock.expansion_before_prophet);
    assert!(stock.religious_opening_viable(&game, 0));
    assert!(
        stock.best_settle_site(&game, 0, home, 10).is_some(),
        "the fixture needs a reachable settle site"
    );
    let journal = crate::reasoning::Journal::recording();
    stock.attach_journal(journal.handle());
    assert_eq!(
        stock.assess(&game, 0).strategy,
        GrandStrategy::Religion,
        "the stock chain enters the Prophet race with two cities"
    );
    assert!(journal
        .since(0)
        .thoughts
        .iter()
        .any(|thought| thought.detail.contains("a Prophet is a finite race")));

    let frozen = AdvancedAi::legacy();
    assert!(!frozen.expansion_before_prophet);
    assert_eq!(frozen.assess(&game, 0).strategy, GrandStrategy::Religion);

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert!(live.expansion_before_prophet);
    let journal = crate::reasoning::Journal::recording();
    live.attach_journal(journal.handle());
    assert_eq!(
        live.assess(&game, 0).strategy,
        GrandStrategy::Expansion,
        "the live seat settles its third city before racing for the Prophet"
    );
    assert!(journal.since(0).thoughts.iter().any(|thought| thought
        .detail
        .contains("the third city comes before the Prophet")));

    // The control arm holds it off.
    live.disable_expansion_before_prophet();
    assert_eq!(live.assess(&game, 0).strategy, GrandStrategy::Religion);
    live.enable_expansion_before_prophet();
    assert_eq!(live.assess(&game, 0).strategy, GrandStrategy::Expansion);

    // At three cities the race resumes exactly as before.
    found_next(&mut game);
    assert_eq!(game.player_city_ids(0).len(), 3);
    assert_eq!(live.assess(&game, 0).strategy, GrandStrategy::Religion);
}

/// The standing rear marches: a unit whose clique cannot advance walks to
/// the campaign objective instead of holding at home, and every guard the
/// step declines for — flag off, threatened homeland, enemy contact —
/// stands it down.
#[test]
fn wartime_reinforcement_marches_the_standing_rear_to_the_objective() {
    let mut game = Game::new_full(2, 24, 16, 7_922, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let objective_city = game.player_city_ids(1)[0];
    let objective = game.cities[&objective_city].pos;
    game.current = 0;
    game.turn = 60;
    game.record_contact(0, 1);
    game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    let soldier = game.spawn_test_unit("warrior", 0, home);
    let start = game.units[&soldier].pos;
    assert!(
        game.wdist(start, objective) > REINFORCEMENT_FRONT_RADIUS,
        "precondition: the unit starts in the rear"
    );

    let campaign = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(objective_city),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };

    // Off: the step declines and the unit keeps its shipped behaviour.
    let mut stock = AdvancedAi::new();
    assert_eq!(
        stock.wartime_reinforcement_step(&mut game, 0, soldier, &campaign, None, &[1]),
        None,
        "off by default"
    );

    // A threatened homeland stands the march down empire-wide.
    let mut threatened_plan = campaign.clone();
    threatened_plan.threatened_city = game.player_city_ids(0).into_iter().next();
    let mut marching = AdvancedAi::new();
    marching.enable_war_reinforcement();
    assert_eq!(
        marching.wartime_reinforcement_step(&mut game, 0, soldier, &threatened_plan, None, &[1]),
        None,
        "the homeland keeps first claim"
    );

    // On, with the homeland quiet: the rear unit walks toward the war.
    assert_eq!(
        marching.wartime_reinforcement_step(&mut game, 0, soldier, &campaign, None, &[1]),
        Some(true),
        "the standing rear must march"
    );
    assert_ne!(game.units[&soldier].pos, start, "the first step moved");
    // Walked turn by turn, the march delivers the unit to the staging
    // ring: the route can step laterally around terrain, so the claim is
    // arrival, not per-step monotony. Once inside the front radius the
    // rear gate itself stands the step down.
    let mut steps = 0;
    while game.wdist(game.units[&soldier].pos, objective) > 5 && steps < 60 {
        game.units.get_mut(&soldier).unwrap().moves_left = 2.0;
        if marching
            .wartime_reinforcement_step(&mut game, 0, soldier, &campaign, None, &[1])
            .is_none()
        {
            break;
        }
        steps += 1;
    }
    let after = game.units[&soldier].pos;
    assert!(
        game.wdist(after, objective) <= 5,
        "the march must reach the staging ring; stalled at {:?}, {} from the \
             objective after {steps} steps",
        after,
        game.wdist(after, objective)
    );

    // In contact, the tactical layer owns the unit and the march declines.
    let adjacent = game
        .wdisk(after, 2)
        .into_iter()
        .find(|pos| {
            *pos != after
                && game.unit_ids_at(*pos).is_empty()
                && game.city_at(*pos).is_none()
                && game
                    .map
                    .get(*pos)
                    .is_some_and(|tile| !game.rules.is_water(tile))
        })
        .expect("open land beside the marcher");
    game.spawn_test_unit("warrior", 1, adjacent);
    assert_eq!(
        marching.wartime_reinforcement_step(&mut game, 0, soldier, &campaign, None, &[1]),
        None,
        "a unit in contact belongs to the tactical layer"
    );
}
