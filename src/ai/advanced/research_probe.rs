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
/// Off by default and set only by the live bridge, so the control arm can
/// hold exactly this one mechanism off — which is what makes
/// `live_without_housing_districts` a controlled comparison rather than a
/// second implementation.
#[test]
fn only_the_live_bridge_raises_the_housing_ceiling() {
    assert!(
        !AdvancedAi::new().base.housing_districts,
        "the frozen tournament controller must keep its recorded ladders"
    );
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(live.base.housing_districts, "the deployment turns it on");
    live.disable_housing_districts();
    assert!(
        !live.base.housing_districts,
        "and the control arm holds it off"
    );
}
/// Off by default, set only by the live bridge, and holdable off on its own.
#[test]
fn only_the_live_bridge_lets_a_capped_city_buy_its_ceiling() {
    assert!(
        !AdvancedAi::new().base.housing_buildings,
        "the frozen tournament controller must keep its recorded ladders"
    );
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(live.base.housing_buildings, "the deployment turns it on");
    live.disable_housing_buildings();
    assert!(
        !live.base.housing_buildings,
        "and the control arm holds it off"
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

/// Off by default, set only by the live bridge, and holdable off on its own
/// so the arm is a controlled comparison.
#[test]
fn only_the_live_bridge_keeps_asking_for_a_campus() {
    assert!(!AdvancedAi::new().campus_every_city);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(live.campus_every_city);
    live.disable_campus_every_city();
    assert!(!live.campus_every_city);
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

/// The `balanced_core` cliff is exempted for the two research/civic trees
/// ONLY, and each exemption must be gated on its own named treatment. The
/// Commercial Hub, Harbor and Industrial Zone keep the half-empire cap they
/// were written with: their effects are empire-wide, so half the empire
/// really is enough. The Campus and the Theater Square each carry their own
/// building chain and a tree that cascades out of them, and the empire
/// stopping at half is what measured 50 of 100 cities for the Campus and 27%
/// for the Theater Square. See `campus_every_city` and `culture_coverage`.
#[test]
fn only_the_two_trees_are_exempt_from_the_half_empire_cliff() {
    let src = include_str!("../advanced.rs");
    let block = src
        .split("let core_capped = district_count * 2 >= city_count;")
        .nth(1)
        .expect("the cliff is computed once")
        .split("};")
        .next()
        .expect("the balanced_core block ends");
    // The Campus exemption gained a population floor on 2026-08-19 (see
    // `campus_every_city`: −2.8 pp wins over 4,000 screened pairs when a
    // size-three town's Campus outbid its growth buildings), so its gate is
    // a three-term conjunction; the Theater Square keeps the original pair.
    assert!(
        block.contains("self.campus_every_city")
            && block.contains("&& family == \"campus\"")
            && block.contains("&& city.pop >= CAMPUS_EVERY_CITY_POP_FLOOR"),
        "the campus exemption must be gated on its treatment and the pop floor"
    );
    assert!(
        block.contains("self.culture_coverage && family == \"theater_square\""),
        "the theater_square exemption must be gated on self.culture_coverage"
    );
    for family in ["commercial_hub", "harbor", "industrial_zone"] {
        assert!(
            !block.contains(&format!("family == \"{family}\"")),
            "{family} must keep the half-empire cap"
        );
    }
    // Both exemptions are live-bridge treatments; a stock controller keeps
    // the cliff for every family.
    let stock = AdvancedAi::new();
    assert!(!stock.campus_every_city);
    assert!(!stock.culture_coverage);
}
/// Off by default, set only by the live bridge, holdable off on its own.
#[test]
fn only_the_live_bridge_plays_the_housing_cards() {
    assert!(!AdvancedAi::new().housing_cards);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(live.housing_cards);
    live.disable_housing_cards();
    assert!(!live.housing_cards);
}

/// ⚠ The two cards must be the ones the empire can actually unlock, and
/// they must be REAL rows in the shipped ruleset — a card name the game does
/// not have is the defect class this project has hit six times.
#[test]
fn the_housing_cards_exist_and_are_the_reachable_pair() {
    let rules = crate::rules::Rules::shipped();
    for card in ["medina_quarter", "insulae"] {
        let spec = rules
            .policies
            .get(&crate::name::Name::new(card))
            .unwrap_or_else(|| panic!("{card} is a shipped policy"));
        assert_eq!(
            spec.slot, "economic",
            "{card} competes for an economic slot, which is what the insert \
                 point assumes"
        );
    }
    // `new_deal` is the strongest housing card and is ALREADY in every
    // strategic list — it is excluded here because it needs `suffrage`,
    // which these games do not reach (slotted in 1 of 107 live runs).
    assert!(rules
        .policies
        .contains_key(&crate::name::Name::new("new_deal")));
}

/// The insert sits behind the lane's leading cards, exactly like the
/// research pair — a lane keeps its opening and pays for this from the tail.
#[test]
fn the_housing_cards_go_behind_the_lanes_own_opening() {
    assert_eq!(HOUSING_DECK_INSERT, RESEARCH_DECK_INSERT);
    assert_eq!(
        HOUSING_CARD_HEADROOM, 2.0,
        "the break-even of the engine's own growth band, not a margin"
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

/// A city under attack that cannot wall itself researches the wall first:
/// civvis-20260816T040537Z lost its unwalled capital on t79 with Masonry
/// "at 16" behind Celestial Navigation. See `walls_tech_goal`.
#[test]
fn a_major_war_sends_an_unwalled_empire_at_the_wall_tech() {
    let mut game = crate::game::Game::new(2, 32, 24, 9_103, 250, 0);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("starting settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    game.record_contact(0, 1);
    game.players[0].techs.remove(&crate::name!("masonry"));
    assert!(
        game.rules.buildings["walls"].tech.as_deref() == Some("masonry"),
        "the ruleset unlocks the first Walls with Masonry"
    );

    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(
        live.base.garrison_walls,
        "the walls doctrine is on for the live seat"
    );
    // At peace, with no threatened city: nothing to force.
    assert_eq!(live.walls_tech_goal(&game, 0), None);

    // The neighbour declares: the unwalled empire is sent at Masonry.
    game.current = 1;
    game.apply(1, &Action::DeclareWar { player: 0 }).unwrap();
    game.current = 0;
    assert_eq!(live.walls_tech_goal(&game, 0), Some("masonry"));
    // And it drives the research decision itself.
    game.players[0].research = None;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    live.advanced_research(&mut game, 0, &plan);
    let picked = game.players[0].research.clone().expect("a tech is chosen");
    assert!(
            live.tech_leads_to(&game, picked.as_str(), "masonry"),
            "under a major war the cheapest step toward the wall tech is researched first, not {picked}"
        );

    // Walls known: the goal is spent.
    game.players[0].techs.insert(crate::name!("masonry"));
    assert_eq!(live.walls_tech_goal(&game, 0), None);
    game.players[0].techs.remove(&crate::name!("masonry"));
    // Every city walled: nothing to force either.
    let capital = game.player_city_ids(0)[0];
    game.cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    assert_eq!(live.walls_tech_goal(&game, 0), None);
    game.cities.get_mut(&capital).unwrap().buildings.pop();

    // The stock controller and the frozen anchor never enter it.
    let stock = AdvancedAi::new();
    assert!(!stock.base.garrison_walls);
    assert_eq!(stock.walls_tech_goal(&game, 0), None);
    let frozen = AdvancedAi::legacy();
    assert_eq!(frozen.walls_tech_goal(&game, 0), None);
}

/// The live Campus treatment used to be unable to act during a religious
/// opening: Astronomy won correctly, then the housing goal pulled the next
/// slot to Engineering and Writing waited behind ordinary scores. Keep the
/// finite Prophet opener, but make the first Campus legal before that
/// optional detour consumes its handoff.
#[test]
fn live_first_campus_writing_precedes_the_housing_research_detour() {
    let setup = || {
        let mut game = Game::new(2, 32, 24, 9_102, 250, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("starting settler");
        game.apply(0, &Action::FoundCity { unit: settler })
            .expect("found city");
        let capital = game.player_city_ids(0)[0];
        let home = game.cities[&capital].pos;
        let second_site = game
            .map
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
            .expect("test map has a nearby city site");
        game.found_city_for(0, second_site, None);
        game.players[0].techs.extend([
            crate::name!("mining"),
            crate::name!("pottery"),
            crate::name!("astrology"),
        ]);
        // The repaired gate (2026-08-19) needs the cap to bind the empire
        // AND to be tech-bound: both cities are capped, and both already
        // hold the granary production could otherwise answer with — the
        // live run's own shape (relief built, ceiling still paid).
        for city in game.player_city_ids(0) {
            game.cities
                .get_mut(&city)
                .unwrap()
                .buildings
                .push(crate::name!("granary"));
            let capped = (2..40)
                .find(|pop| {
                    game.cities.get_mut(&city).unwrap().pop = *pop;
                    game.city_housing_headroom(&game.cities[&city]) < HOUSING_CARD_HEADROOM
                })
                .expect("some population overruns this city's housing");
            game.cities.get_mut(&city).unwrap().pop = capped;
        }
        game
    };
    let plan = StrategicPlan {
        strategy: GrandStrategy::Religion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: 1,
        rush: false,
    };

    let mut housing_game = setup();
    let mut housing_only = AdvancedAi::new();
    housing_only.enable_housing_research();
    assert_eq!(
        housing_only.unreachable_housing_tech(&housing_game, 0),
        Some("engineering"),
        "the fixture must reproduce the competing live housing goal"
    );
    housing_only.advanced_research(&mut housing_game, 0, &plan);
    let housing_pick = housing_game.players[0]
        .research
        .as_deref()
        .expect("the housing goal selects a prerequisite");
    assert!(
        housing_only.tech_leads_to(&housing_game, housing_pick, "engineering"),
        "with Campus coverage off, the existing housing route remains available"
    );

    let mut live_game = setup();
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert_eq!(
        live.first_campus_tech(&live_game, 0),
        Some("writing"),
        "two cities with a legal Campus site make the live coverage promise actionable"
    );
    assert_eq!(
        AdvancedAi::legacy().first_campus_tech(&live_game, 0),
        None,
        "the frozen controller does not acquire the Writing handoff"
    );
    live.advanced_research(&mut live_game, 0, &plan);
    assert_eq!(
        live_game.players[0].research.as_deref(),
        Some("writing"),
        "Writing must beat the post-Astrology housing detour"
    );
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

/// Off by default, set only by the live bridge, holdable off on its own —
/// the wonder-ring settle credit follows the same contract as every other
/// bridge repair, so the frozen `advanced_v1` anchor keeps its ladder.
#[test]
fn only_the_live_bridge_prices_the_wonder_ring_into_settling() {
    assert!(!AdvancedAi::new().base.wonder_ring_settle_value);
    assert!(!AdvancedAi::legacy().base.wonder_ring_settle_value);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(live.base.wonder_ring_settle_value);
    live.disable_wonder_ring_settle_value();
    assert!(!live.base.wonder_ring_settle_value);
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
    game.blocked_city_sites.insert(site);
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
            game.blocked_city_sites.insert(position);
        }
    }
    let settler = game.spawn_test_unit("settler", 0, doomed);
    game.units.get_mut(&settler).unwrap().moves_left = 2.0;
    let cities_before = game.player_city_ids(0).len();

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
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
    let raced = live.production_value(&game, 0, capital, &pyramids, &plan, &live.counts(&game, 0));
    assert!(raced > 0.0, "the live seat opens the wonder arm: {raced}");
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
/// own — the war-conversion quartet follows the same contract as every other
/// bridge repair.
#[test]
fn only_the_live_bridge_fights_the_war_conversion_quartet() {
    let fresh = AdvancedAi::new();
    assert!(!fresh.war_economy);
    assert!(!fresh.war_reinforcement);
    assert!(!fresh.war_patience);
    assert!(!fresh.endgame_war_runway);
    let legacy = AdvancedAi::legacy();
    assert!(
        !legacy.war_economy
            && !legacy.war_reinforcement
            && !legacy.war_patience
            && !legacy.endgame_war_runway
    );
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(
        live.war_economy && live.war_reinforcement && live.war_patience && live.endgame_war_runway
    );
    live.disable_war_economy();
    live.disable_war_reinforcement();
    live.disable_war_patience();
    live.disable_endgame_war_runway();
    assert!(
        !live.war_economy
            && !live.war_reinforcement
            && !live.war_patience
            && !live.endgame_war_runway
    );
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

/// The strategic governor runs under the Science lane on the treated seat:
/// civvis-20260816T054344Z ran Rome on the baseline picker for the whole
/// of a Science stretch and started no wonder in 57 turns. See
/// `governor_every_lane`.
/// The strategic governor runs under the Science lane on the treated seat:
/// civvis-20260816T054344Z ran Rome on the baseline picker for the whole
/// of a Science stretch and started no wonder in 57 turns. See
/// `governor_every_lane`. Same source-pinned shape as
/// `the_war_economy_routes_an_adaptive_conquest_plan_to_war_production`:
/// the routing block is the only place the choice is made.
#[test]
fn the_strategic_governor_runs_under_every_lane_on_the_treated_seat() {
    let src = include_str!("../advanced.rs");
    let block = src
        .split("|| adaptive_expansion_dispatch")
        .nth(1)
        .expect("the production routing condition exists")
        .split("self.advanced_production(g, pid, &plan, adaptive_expansion_dispatch);")
        .next()
        .expect("the routing block ends at the production call");
    assert!(
        block.contains("|| every_lane"),
        "the every-lane arm reaches the same production call"
    );
    // The arm is two flags since the bisect: the composite still covers the
    // same five lanes, but each half can be measured on its own.
    let arm = src
        .split("let every_lane = (self.governor_victory_lanes")
        .nth(1)
        .expect("the arm is gated by the flags")
        .split(';')
        .next()
        .expect("the arm ends");
    for lane in ["Science", "Culture", "Religion", "Diplomacy", "Expansion"] {
        assert!(
            arm.contains(&format!("GrandStrategy::{lane}")),
            "the {lane} lane is covered"
        );
    }
    assert!(
        arm.contains("self.governor_expansion_lane"),
        "the Expansion lane hangs off its own half"
    );
    assert!(
        !arm.contains("GrandStrategy::Conquest") && !arm.contains("GrandStrategy::Recovery"),
        "war lanes keep their own routing"
    );
    // Off by default, set only by the live bridge and the native repair
    // bundle, holdable off on its own.
    assert!(!AdvancedAi::new().governor_every_lane);
    assert!(!AdvancedAi::legacy().governor_every_lane);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(live.governor_every_lane);
    live.disable_governor_every_lane();
    assert!(!live.governor_every_lane);
    let mut repaired = AdvancedAi::new();
    repaired.enable_engine_repairs_economy();
    assert!(repaired.governor_every_lane);
}

/// War patience measures a campaign, not ordinary expansion.
///
/// On live run `civvis-20260816T003229Z`, Rome was at war with Indonesia
/// without taking a city, then founded population-1 Arretium on turn 201.
/// The former city-count clock read that settlement as campaign progress
/// and gave the stalled war another full patience window.
#[test]
fn only_a_foreign_city_acquisition_refreshes_live_war_patience() {
    let mut game = Game::new_full(2, 24, 16, 7_924, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let enemy_city = game.player_city_ids(1)[0];
    game.current = 0;
    game.turn = 60;
    game.record_contact(0, 1);
    game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();

    let mut ai = AdvancedAi::new();
    ai.enable_war_patience();
    ai.observe_campaign(&game, 0);
    assert_eq!(ai.last_campaign_progress, 60);
    let mut frozen = AdvancedAi::legacy();
    frozen.observe_campaign(&game, 0);
    assert_eq!(frozen.last_campaign_progress, 60);

    game.turn = 100;
    let founded_site = game
        .map
        .tiles
        .values()
        .find(|tile| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && tile.owner_city.is_none()
                && game.city_at(tile.pos).is_none()
                && game.units_at(tile.pos).is_empty()
                && game
                    .cities
                    .values()
                    .all(|city| game.wdist(city.pos, tile.pos) >= 4)
        })
        .map(|tile| tile.pos)
        .expect("open site for a fresh Roman settlement");
    let founded = game.found_city_for(0, founded_site, None);
    assert_eq!(game.cities[&founded].owner, 0);
    ai.observe_campaign(&game, 0);
    assert_eq!(
        ai.last_campaign_progress, 60,
        "founding a new city cannot make an unproductive war patient again"
    );
    assert!(ai.war_patience_exhausted(&game));
    frozen.observe_campaign(&game, 0);
    assert_eq!(
        frozen.last_campaign_progress, 100,
        "the frozen controller retains its historical city-count clock"
    );

    game.turn = 101;
    game.cities.get_mut(&enemy_city).unwrap().owner = 0;
    ai.observe_campaign(&game, 0);
    assert_eq!(
        ai.last_campaign_progress, 101,
        "a known foreign city changing hands is real campaign progress"
    );
    assert!(!ai.war_patience_exhausted(&game));
}

/// A stalled war against an overwhelmed campaign target is neither offered
/// away nor accepted away while `war_patience` is on; a stalled war
/// against anyone else keeps the shipped fatigue shape.
/// Once a war's patience has lapsed and no city of ours is in danger, the
/// plan stops reading Conquest merely because the enemy will not make
/// peace; a foreign city acquired (recent campaign progress) keeps the war
/// posture.
#[test]
fn a_stalemated_war_no_longer_sets_the_grand_strategy() {
    let mut game = Game::new_full(2, 24, 16, 7_923, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let staging = game.cities[&game.player_city_ids(0)[0]].pos;
    for _ in 0..3 {
        game.spawn_test_unit("modern_armor", 0, staging);
    }
    game.current = 0;
    game.turn = 60;
    game.record_contact(0, 1);
    game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    game.turn = 100;
    assert!(game.military_power(0) >= game.military_power(1) * OVERWHELMING_WAR_RATIO);

    // Patience lapsed: no foreign city acquired in forty turns.
    let mut stale = AdvancedAi::new();
    stale.enable_war_patience();
    stale.major_war_since = Some(60);
    stale.last_campaign_progress = 60;
    assert!(stale.war_patience_exhausted(&game));
    let journal = crate::reasoning::Journal::recording();
    stale.attach_journal(journal.handle());
    let plan = stale.assess(&game, 0);
    assert_ne!(
        plan.strategy,
        GrandStrategy::Conquest,
        "a stalemate the empire cannot end no longer pins the plan on Conquest"
    );
    assert!(
        journal
            .since(0)
            .thoughts
            .iter()
            .any(|thought| thought.headline.starts_with("The war is a stalemate")),
        "the posture is journaled"
    );

    // Recent progress: the same war keeps its Conquest posture.
    let mut pressing = AdvancedAi::new();
    pressing.enable_war_patience();
    pressing.major_war_since = Some(60);
    pressing.last_campaign_progress = 95;
    assert!(!pressing.war_patience_exhausted(&game));
    assert_eq!(pressing.assess(&game, 0).strategy, GrandStrategy::Conquest);

    // Without the flag (default and frozen controllers) the shipped
    // "already at war" arm is unchanged.
    let mut plain = AdvancedAi::new();
    plain.major_war_since = Some(60);
    plain.last_campaign_progress = 60;
    assert!(!plain.war_patience);
    assert_eq!(plain.assess(&game, 0).strategy, GrandStrategy::Conquest);
    let mut frozen = AdvancedAi::legacy();
    frozen.major_war_since = Some(60);
    frozen.last_campaign_progress = 60;
    assert_eq!(frozen.assess(&game, 0).strategy, GrandStrategy::Conquest);
}

/// The live mirror can know a leader's public victory clock before it has
/// any city coordinate for that leader. That warning must not turn a
/// stalled war with somebody else back into Conquest. See
/// `conquest_denial_actionable`.
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
    live.enable_war_patience();
    live.major_war_since = Some(60);
    live.last_campaign_progress = 60;
    assert!(live.war_patience_exhausted(&game));
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
    assert_ne!(
        plan.strategy,
        GrandStrategy::Conquest,
        "the cityless leader cannot re-arm a war whose patience expired"
    );
    assert_eq!(plan.target_player, Some(1));
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
    assert_eq!(live.assess(&game, 0).strategy, GrandStrategy::Conquest);
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

    // The control arm takes the branch again.
    live.disable_no_elective_war();
    assert_eq!(live.assess(&game, 0).strategy, GrandStrategy::Conquest);
    live.enable_no_elective_war();

    // A war the neighbour opens is still answered: "already at war" is
    // not the elective branch.
    game.current = 1;
    game.apply(1, &Action::DeclareWar { player: 0 }).unwrap();
    game.current = 0;
    assert!(game.is_at_war(0, 1));
    // A fresh war: patience has not lapsed (see `war_patience`).
    live.major_war_since = Some(60);
    live.last_campaign_progress = 60;
    assert_eq!(live.assess(&game, 0).strategy, GrandStrategy::Conquest);
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
    let visible = game.player_vision_now(0);
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

/// A war declared today is not a stalemate: patience counts from the
/// later of the war's start and its last capture. See
/// `war_patience_exhausted` (civvis-20260816T030249Z: declared t71,
/// "stalemate" t72).
#[test]
fn a_fresh_war_starts_its_own_patience_window() {
    let mut game = Game::new_full(2, 24, 16, 7_933, 300, 0, false);
    game.turn = 100;
    let limit = game.standard_duration(WAR_PATIENCE_LIMIT_TURNS).max(1);
    let mut ai = AdvancedAi::new();
    ai.enable_war_patience();
    // The last capture (or first observation) is long past…
    ai.last_campaign_progress = 20;
    ai.major_war_since = None;
    assert!(
        ai.war_patience_exhausted(&game),
        "a war with no start on record and no capture for eighty turns has lapsed"
    );
    // …but a war that opened this turn has a full window ahead of it.
    ai.major_war_since = Some(100);
    assert!(
        !ai.war_patience_exhausted(&game),
        "a war one turn old is not a stalemate"
    );
    ai.major_war_since = Some(100 - limit + 1);
    assert!(
        !ai.war_patience_exhausted(&game),
        "one turn inside the window"
    );
    ai.major_war_since = Some(100 - limit);
    assert!(
        ai.war_patience_exhausted(&game),
        "the window lapses at the limit"
    );
    // A capture inside an old war extends it the same way.
    ai.major_war_since = Some(20);
    ai.last_campaign_progress = 95;
    assert!(
        !ai.war_patience_exhausted(&game),
        "a recent capture keeps an old war alive"
    );
}

#[test]
fn war_patience_holds_the_stall_clause_only_over_an_overwhelmed_target() {
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
    let staging = game.cities[&game.player_city_ids(0)[0]].pos;
    for _ in 0..3 {
        game.spawn_test_unit("modern_armor", 0, staging);
    }
    // The stall offer only goes to a target still holding more than one
    // city (a one-city empire is finished, not sued); give the defender
    // its second city before the fatigue arms below.
    let their_capital = game.cities[&game.player_city_ids(1)[0]].pos;
    let second_site = game
        .wdisk(their_capital, 4)
        .into_iter()
        .find(|pos| {
            game.wdist(*pos, their_capital) >= 3
                && game.units_at(*pos).is_empty()
                && game.city_at(*pos).is_none()
                && game
                    .map
                    .get(*pos)
                    .is_some_and(|tile| !game.rules.is_water(tile))
        })
        .expect("open land for the defender's second city");
    game.found_city_for(1, second_site, None);
    game.current = 0;
    game.turn = 60;
    game.record_contact(0, 1);
    game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    // War age 40 with no campaign progress in that time: the stall
    // clause's own conditions hold for every arm below.
    game.turn = 100;
    assert!(
        game.military_power(0) >= game.military_power(1) * OVERWHELMING_WAR_RATIO,
        "precondition: the attacker overwhelms the defender"
    );
    let campaign = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: game.player_city_ids(1).into_iter().next(),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let white_peace = DiplomaticDeal {
        id: 7,
        from: 1,
        to: 0,
        give_gold: 0.0,
        request_gold: 0.0,
        open_borders: false,
        friendship: false,
        peace: true,
        alliance: None,
        defensive_pact: false,
        joint_war_target: None,
        promise: None,
        demand: false,
        expires: game.turn + 10,
    };

    // Shipped shape: fatigue accepts the white peace and offers its own.
    let mut fatigued = AdvancedAi::new();
    fatigued.major_war_since = Some(60);
    assert!(
        fatigued.incoming_deal_value(&game, 0, &white_peace, &campaign) > 0.0,
        "without the flag, a stalled war accepts white peace"
    );
    let mut offering = game.clone();
    fatigued.advanced_diplomacy(&mut offering, 0, &campaign);
    assert!(
        fatigued.peace_offers.contains(&1),
        "without the flag, a stalled war offers peace to its own target"
    );

    // With the flag: the stall clause stands down against the overwhelmed
    // campaign target, and the denied-partner guard refuses the deal —
    // while the campaign is still gaining ground (a foreign city acquired
    // fifteen turns ago is inside the patience window).
    let mut patient = AdvancedAi::new();
    patient.enable_war_patience();
    patient.major_war_since = Some(60);
    patient.last_campaign_progress = 85;
    assert!(!patient.war_patience_exhausted(&game));
    assert!(
        patient.incoming_deal_value(&game, 0, &white_peace, &campaign) < 0.0,
        "with the flag, an overwhelming attacker keeps prosecuting"
    );
    let mut pressed = game.clone();
    patient.advanced_diplomacy(&mut pressed, 0, &campaign);
    assert!(
        !patient.peace_offers.contains(&1),
        "with the flag, no stall offer goes to the overwhelmed target"
    );

    // ★ Patience is bounded: forty standard turns without a foreign city
    // acquired and the same overwhelming attacker sues, accepts, and
    // journals why.
    let mut worn_out = AdvancedAi::new();
    worn_out.enable_war_patience();
    worn_out.major_war_since = Some(60);
    worn_out.last_campaign_progress = 60;
    assert!(worn_out.war_patience_exhausted(&game));
    assert!(
        worn_out.incoming_deal_value(&game, 0, &white_peace, &campaign) > 0.0,
        "patience lapsed: the white peace is accepted"
    );
    let journal = crate::reasoning::Journal::recording();
    worn_out.attach_journal(journal.handle());
    let mut suing = game.clone();
    worn_out.advanced_diplomacy(&mut suing, 0, &campaign);
    assert!(
        worn_out.peace_offers.contains(&1),
        "patience lapsed: the stall offer goes to the target"
    );
    assert!(
        journal
            .since(0)
            .thoughts
            .iter()
            .any(|thought| thought.headline.starts_with("Patience with the war on")),
        "the lapse is journaled by name"
    );
    // A gained city resets the clock: progress five turns ago and the
    // same attacker is patient again.
    worn_out.last_campaign_progress = 95;
    assert!(!worn_out.war_patience_exhausted(&game));
    assert!(worn_out.incoming_deal_value(&game, 0, &white_peace, &campaign) < 0.0);

    // Scope: patience covers exactly the campaign target. The same
    // stalled war read against a plan that is not fighting player 1
    // keeps the shipped fatigue acceptance.
    let unrelated = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    assert!(
        patient.incoming_deal_value(&game, 0, &white_peace, &unrelated) > 0.0,
        "patience must not outlive the campaign that justified it"
    );
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
                && game.units_at(*pos).is_empty()
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
