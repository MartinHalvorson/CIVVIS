use super::super::test_support::opt_in_off_in_both_controllers;
use super::super::AdvancedAi;
use super::*;
use crate::game::{Action, Game};
use crate::name;

#[test]
fn chase_every_boost_versions_are_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("chase-every-boost", |ai| ai.chase_every_boost);
    opt_in_off_in_both_controllers("chase-every-boost-2", |ai| ai.chase_every_boost_2);
}

/// One founded capital and nothing else on the board.
fn capital_board(seed: u64) -> Game {
    let mut game = Game::new_full(1, 20, 14, seed, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    game
}

/// A second city of ours, founded on the first legal site four or more
/// tiles from the capital.
fn second_city(game: &mut Game) -> u32 {
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    let sites: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for site in sites {
        if game.wdist(site, capital) < 4 {
            continue;
        }
        let settler = game.spawn_unit("settler", 0, site);
        if game.apply(0, &Action::FoundCity { unit: settler }).is_ok() {
            return *game
                .player_city_ids(0)
                .iter()
                .find(|cid| game.cities[cid].pos == site)
                .expect("the city stands where it was founded");
        }
        game.remove_unit(settler);
    }
    panic!("no legal second site on the board");
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_chase_every_boost();
    ai
}

fn armed_v2() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_chase_every_boost_2();
    ai
}

#[test]
fn chase_every_boost_versions_ship_off_and_are_mutually_exclusive() {
    let mut ai = AdvancedAi::new();
    assert!(!ai.chase_every_boost, "the broad control ships off");
    assert!(!ai.chase_every_boost_2, "the selective successor ships off");

    ai.enable_chase_every_boost();
    assert!(ai.chase_every_boost);
    assert!(!ai.chase_every_boost_2);
    ai.enable_chase_every_boost_2();
    assert!(!ai.chase_every_boost, "one family version plays");
    assert!(ai.chase_every_boost_2);
    ai.enable_chase_every_boost();
    assert!(ai.chase_every_boost);
    assert!(
        !ai.chase_every_boost_2,
        "version one also selects its family"
    );
}

fn spec(game: &Game, node: &str, techs: bool) -> crate::rules::BoostSpec {
    let boost = if techs {
        game.rules.techs[node].boost.clone()
    } else {
        game.rules.civics[node].boost.clone()
    };
    boost.expect("the node carries a boost row")
}

#[test]
fn boost_progress_is_the_engines_own_met_test_for_every_row() {
    let game = capital_board(57_001);
    let rows: Vec<(String, crate::rules::BoostSpec)> = game
        .rules
        .techs
        .iter()
        .filter_map(|(n, s)| s.boost.clone().map(|b| (n.to_string(), b)))
        .chain(
            game.rules
                .civics
                .iter()
                .filter_map(|(n, s)| s.boost.clone().map(|b| (n.to_string(), b))),
        )
        .collect();
    assert!(
        rows.len() > 100,
        "both trees carry boost rows: {}",
        rows.len()
    );
    for (node, boost) in rows {
        let (have, need) = game.boost_progress(0, &boost);
        assert!(need >= 1, "{node}: need {need}");
        assert_eq!(
            game.boost_met(0, &boost),
            have >= need,
            "{node} ({}) met must be have >= need: {have} >= {need}",
            boost.trigger
        );
    }
}

#[test]
fn boost_progress_counts_the_state_the_planner_can_change() {
    let mut game = capital_board(57_002);
    // Early Empire: six citizens across the empire, one city of one.
    let (have, need) = game.boost_progress(0, &spec(&game, "early_empire", false));
    assert_eq!((have, need), (1, 6));
    // Guilds: two Markets, none standing.
    assert_eq!(
        game.boost_progress(0, &spec(&game, "guilds", false)),
        (0, 2)
    );
    // Political Philosophy: three city-states met, none on a lone board.
    assert_eq!(
        game.boost_progress(0, &spec(&game, "political_philosophy", false)),
        (0, 3)
    );
    // Archery: a Slinger kill the counters have not recorded.
    assert_eq!(
        game.boost_progress(0, &spec(&game, "archery", true)),
        (0, 1)
    );
    // A yes/no trigger is (0 or 1, 1): Sailing wants a coastal city.
    let (have, need) = game.boost_progress(0, &spec(&game, "sailing", true));
    assert_eq!(need, 1);
    assert!(have == 0 || have == 1);
    // Bronze Working: three barbarians, and the counter moves the count.
    game.players[0]
        .counters
        .insert("barbs_killed".to_string(), 2);
    assert_eq!(
        game.boost_progress(0, &spec(&game, "bronze_working", true)),
        (2, 3)
    );
}

#[test]
fn off_the_chase_table_knows_only_the_six_buildable_prefixes() {
    let game = capital_board(57_003);
    let plain = AdvancedAi::new();
    let chases = plain.eureka_chases(&game, 0);
    assert!(chases.iter().all(|chase| {
        let t = chase.trigger.as_str();
        t.starts_with("improvement")
            || t.starts_with("improve_resource:")
            || t.starts_with("units_of:")
            || t.starts_with("building:")
            || t.starts_with("district:")
    }));
    assert!(!chases.iter().any(|chase| chase.node == name!("archery")));
    assert!(!chases
        .iter()
        .any(|chase| chase.node == name!("political_philosophy")));
}

#[test]
fn on_every_trigger_in_reach_is_a_chase_in_both_trees() {
    let game = capital_board(57_004);
    let chases = armed().eureka_chases(&game, 0);
    let chase = |node: &str| {
        chases
            .iter()
            .find(|chase| chase.node.as_str() == node)
            .unwrap_or_else(|| panic!("{node} is chased"))
            .clone()
    };
    // A kill trigger the counters cannot yet show: one step open.
    assert_eq!(chase("archery").remaining, 1);
    assert_eq!(chase("bronze_working").remaining, 3);
    // Inspirations from contacts, wonders, districts and population.
    assert_eq!(chase("political_philosophy").remaining, 3);
    assert_eq!(chase("drama_poetry").remaining, 1);
    assert_eq!(chase("military_training").remaining, 1);
    assert_eq!(chase("early_empire").remaining, 5);
    // The old buildable prefixes are still there.
    assert_eq!(chase("guilds").remaining, 2);
    assert_eq!(chase("masonry").remaining, 1);
    // Research is the boost's own fraction of the node's cost.
    let guilds = chase("guilds");
    assert!((guilds.research - game.game_speed.scale(420.0) * 0.4).abs() < 1e-9);
}

#[test]
fn a_banked_boost_drops_out_of_the_chase_table() {
    let mut game = capital_board(57_005);
    game.players[0].boosted_civics.insert(name!("drama_poetry"));
    game.players[0].boosted_techs.insert(name!("archery"));
    let chases = armed().eureka_chases(&game, 0);
    assert!(!chases
        .iter()
        .any(|chase| chase.node == name!("drama_poetry")));
    assert!(!chases.iter().any(|chase| chase.node == name!("archery")));
}

#[test]
fn a_kill_trigger_is_actionable_only_with_the_named_unit_in_hand() {
    let mut game = capital_board(57_006);
    let ai = armed();
    let archery = ai
        .chase_for(&game, 0, "archery")
        .expect("archery is chased");
    assert!(
        !ai.chase_one_action_away(&game, 0, &archery),
        "no Slinger, no action"
    );
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    game.spawn_unit("slinger", 0, capital);
    assert!(ai.chase_one_action_away(&game, 0, &archery));
    // Growth is not an action: Early Empire is never "one step away".
    let early = ai
        .chase_for(&game, 0, "early_empire")
        .expect("early empire is chased");
    assert!(!ai.chase_actionable(&game, 0, &early));
}

#[test]
fn the_beeline_walks_the_boosted_prerequisite_first() {
    let mut game = capital_board(57_007);
    let plain = AdvancedAi::new();
    let ai = armed();
    let printed = game.rules.techs["masonry"].cost;
    assert_eq!(plain.beeline_step_cost(&game, 0, "masonry", true), printed);
    assert_eq!(ai.beeline_step_cost(&game, 0, "masonry", true), printed);
    game.players[0].boosted_techs.insert(name!("masonry"));
    assert_eq!(
        plain.beeline_step_cost(&game, 0, "masonry", true),
        printed,
        "off, the beeline is blind to the boost"
    );
    assert!((ai.beeline_step_cost(&game, 0, "masonry", true) - printed * 0.6).abs() < 1e-9);
    // The civic half through the same code path.
    let civic = game.rules.civics["drama_poetry"].cost;
    game.players[0].boosted_civics.insert(name!("drama_poetry"));
    assert!((ai.beeline_step_cost(&game, 0, "drama_poetry", false) - civic * 0.6).abs() < 1e-9);
    assert_eq!(
        plain.beeline_step_cost(&game, 0, "drama_poetry", false),
        civic
    );
}

#[test]
fn a_boost_one_action_away_halves_its_discount_on_the_beeline() {
    let mut game = capital_board(57_008);
    let ai = armed();
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    game.spawn_unit("slinger", 0, capital);
    let printed = game.rules.techs["archery"].cost;
    let cost = ai.beeline_step_cost(&game, 0, "archery", true);
    assert!((cost - printed * (1.0 - 0.4 * CHASE_ONE_STEP_FRACTION)).abs() < 1e-9);
}

#[test]
fn production_pays_an_inspiration_trigger_and_stops_when_others_have_it_queued() {
    let mut game = capital_board(57_009);
    let ai = armed();
    let cid = game.player_city_ids(0)[0];
    let encampment = Item::District {
        district: name!("encampment"),
        pos: game.cities[&cid].pos,
    };
    let market = Item::Building {
        building: name!("market"),
    };
    assert!(
        ai.chase_production_premium(&game, 0, cid, &encampment) > 0.0,
        "Military Training's Encampment is priced"
    );
    assert!(
        ai.chase_production_premium(&game, 0, cid, &market) > 0.0,
        "Guilds' Market is priced"
    );
    assert_eq!(
        AdvancedAi::new().production_boost_premium(&game, 0, cid, &market, 100.0),
        0.0,
        "off, the premium is the old genes' zero"
    );
    // A second city with two Markets queued exhausts the two-Market chase.
    let second = second_city(&mut game);
    game.cities
        .get_mut(&second)
        .unwrap()
        .queue
        .push(market.clone());
    game.cities
        .get_mut(&second)
        .unwrap()
        .queue
        .push(market.clone());
    assert_eq!(ai.chase_production_premium(&game, 0, cid, &market), 0.0);
    // With one queued elsewhere, this city is paid for the second, at
    // the full step (nothing left after it).
    game.cities.get_mut(&second).unwrap().queue.pop();
    let one_left = ai.chase_production_premium(&game, 0, cid, &market);
    game.cities.get_mut(&second).unwrap().queue.clear();
    let two_left = ai.chase_production_premium(&game, 0, cid, &market);
    assert!(
        one_left > two_left,
        "{one_left} > {two_left}: the further step is discounted"
    );
}

#[test]
fn chase_every_boost_v2_only_prices_a_timely_active_final_city_trigger() {
    let mut game = capital_board(57_014);
    let cid = game.player_city_ids(0)[0];
    let encampment = Item::District {
        district: name!("encampment"),
        pos: game.cities[&cid].pos,
    };
    // Military Training needs precisely one Encampment. It is the civic
    // under study, and the small capital finishes the district well before
    // the slow early-culture study completes.
    game.players[0].civic = Some("military_training".to_string());
    game.players[0].civic_progress = 0.0;
    let ai = armed_v2();
    let premium = ai.chase_current_production_premium(&game, 0, cid, &encampment);
    assert!(
        premium > 0.0,
        "the concrete final step earns its active civic"
    );
    assert!(
        (ai.production_boost_premium(&game, 0, cid, &encampment, 1_000.0) - premium).abs() < 1e-9,
        "the production path uses version two's narrow tiebreak"
    );

    // V2 does not acquire v1's global research ordering hook, even for a
    // banked boost, and a future or unrelated civic earns no city premium.
    let masonry = game.rules.techs["masonry"].cost;
    game.players[0].boosted_techs.insert(name!("masonry"));
    assert_eq!(ai.beeline_step_cost(&game, 0, "masonry", true), masonry);
    game.players[0].civic = Some("craftsmanship".to_string());
    assert_eq!(
        ai.chase_current_production_premium(&game, 0, cid, &encampment),
        0.0,
        "a future Military Training trigger cannot buy an Encampment"
    );

    // A study that will finish first cannot cash the later build, and a
    // front queue in another city has already claimed the one remaining
    // trigger step.
    game.players[0].civic = Some("military_training".to_string());
    game.players[0].civic_progress = game.civic_cost("military_training") - 0.25;
    assert_eq!(
        ai.chase_current_production_premium(&game, 0, cid, &encampment),
        0.0,
        "a boost after its study finishes is worthless"
    );
    game.players[0].civic_progress = 0.0;
    let wonder = Item::Wonder {
        wonder: name!("pyramids"),
        pos: game.cities[&cid].pos,
    };
    assert_eq!(
        ai.chase_current_production_premium(&game, 0, cid, &wonder),
        0.0,
        "version two keeps broad wonder chasing in the v1 control"
    );
    let other = second_city(&mut game);
    game.cities
        .get_mut(&other)
        .expect("the second city exists")
        .queue
        .push(encampment.clone());
    assert_eq!(
        ai.chase_current_production_premium(&game, 0, cid, &encampment),
        0.0,
        "another city already owns the final trigger"
    );
}

#[test]
fn active_study_boost_saving_cannot_exceed_remaining_research() {
    let mut game = capital_board(57_019);
    game.players[0].civic = Some("military_training".to_string());
    game.players[0].civic_progress = game.civic_cost("military_training") - 3.0;
    let (_, payout, _) = armed_v2()
        .active_study_production_trigger(&game, 0, "military_training", false)
        .expect("one Encampment completes this inspiration");
    assert_eq!(payout, 3.0);
}

#[test]
fn active_study_trigger_deadline_includes_production_modifiers() {
    let mut game = capital_board(57_020);
    let cid = game.player_city_ids(0)[0];
    let item = Item::District {
        district: name!("encampment"),
        pos: game.cities[&cid].pos,
    };
    let ai = armed_v2();
    game.players[0].civic = Some("military_training".to_string());
    // The same almost-finished district misses the study at the raw rate,
    // but finishes before it under Veterancy's actual production bonus.
    let production = game.city_yields(cid).production;
    let cost = game.item_cost_for_city(0, cid, &item);
    game.cities.get_mut(&cid).unwrap().production = cost - production * 2.4;
    let rate = AdvancedAi::research_rate(&game, 0, false);
    game.players[0].civic_progress = game.civic_cost("military_training") - rate * 2.0;
    assert_eq!(
        ai.chase_current_production_premium(&game, 0, cid, &item),
        0.0
    );
    game.players[0].policies.insert(name!("veterancy"));
    assert!(game.item_prod_mult(0, cid, Some(&item)) > 1.0);
    assert!(ai.chase_current_production_premium(&game, 0, cid, &item) > 0.0);
}

#[test]
fn the_premiums_never_exceed_half_the_items_own_value() {
    let game = capital_board(57_013);
    let ai = armed();
    let cid = game.player_city_ids(0)[0];
    let market = Item::Building {
        building: name!("market"),
    };
    let open = ai.chase_production_premium(&game, 0, cid, &market);
    assert!(open > 0.0);
    // A marginal item (raw 10) can gain at most 5; a strong one keeps
    // the whole premium, under the absolute cap.
    assert!(
        (ai.production_boost_premium(&game, 0, cid, &market, 10.0) - open.min(5.0)).abs() < 1e-9
    );
    assert!((ai.production_boost_premium(&game, 0, cid, &market, 1_000.0) - open).abs() < 1e-9);
    assert!(open <= CHASE_PRODUCTION_VALUE_CAP);
    // A refused item (raw below zero) gains nothing.
    assert_eq!(
        ai.production_boost_premium(&game, 0, cid, &market, -5.0),
        0.0
    );
    // The builder premium under this gene alone is held the same way.
    let capital = game.cities[&cid].pos;
    let tile = game.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != capital)
        .expect("the capital owns a work tile");
    let raw = ai.eureka_builder_premium(&game, tile, "farm");
    let held = ai.builder_boost_premium(&game, tile, "farm", 4.0);
    assert!(held <= raw && held <= 2.0);
    let mut both = armed();
    both.enable_eureka_chasing_builder();
    assert!((both.builder_boost_premium(&game, tile, "farm", 4.0) - raw).abs() < 1e-9);
}

#[test]
fn a_wonder_is_worth_the_inspiration_it_completes() {
    let game = capital_board(57_010);
    let ai = armed();
    let cid = game.player_city_ids(0)[0];
    let wonder = Item::Wonder {
        wonder: name!("pyramids"),
        pos: game.cities[&cid].pos,
    };
    assert!(ai.chase_production_premium(&game, 0, cid, &wonder) > 0.0);
    assert_eq!(
        AdvancedAi::new().production_boost_premium(&game, 0, cid, &wonder, 100.0),
        0.0
    );
}

#[test]
fn a_slinger_kill_on_a_barbarian_earns_archery_and_bronze_working() {
    // A barbarian-seated board, emptied of every unit and camp.
    let mut game = Game::new_full(1, 20, 14, 57_011, 200, 0, true);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    game.barb_camps.clear();
    let ai = armed();
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    let slinger = game.spawn_unit("slinger", 0, capital);
    let warrior = game.spawn_unit("warrior", 0, capital);
    let barbarian = game.barb_pid.expect("a barbarian seat");
    let camp = (capital.0 + 3, capital.1);
    game.spawn_unit("warrior", barbarian, camp);
    let slinger_pays = ai.chase_kill_premium(&game, 0, slinger, camp);
    let warrior_pays = ai.chase_kill_premium(&game, 0, warrior, camp);
    assert!(
        slinger_pays > warrior_pays,
        "{slinger_pays} > {warrior_pays}"
    );
    assert!(
        warrior_pays > 0.0,
        "Bronze Working's three barbarians pay any kill"
    );
    assert!(slinger_pays <= CHASE_KILL_VALUE_CAP);
    assert_eq!(
        AdvancedAi::new().chase_kill_premium(&game, 0, slinger, camp),
        0.0
    );
    // Archery banked: the Slinger's edge is gone. A fresh controller, because
    // the chase table is memoised per turn and the board changed under it.
    game.players[0].boosted_techs.insert(name!("archery"));
    let ai = armed();
    let banked = ai.chase_kill_premium(&game, 0, slinger, camp);
    assert!((banked - ai.chase_kill_premium(&game, 0, warrior, camp)).abs() < 1e-9);
}

/// Diagnostic, never in CI: one screen-shaped game (the probe's own
/// board) with the gene on seat 0 against the same seed with it off,
/// printing the boosted share and every unboosted node's trigger, plus
/// the chase table at three turns. `cargo test --profile ci --lib --
/// --ignored diagnostic_boost_share --nocapture`.
#[test]
#[ignore]
fn diagnostic_boost_share_on_vs_off() {
    use crate::ai::Ai;
    use crate::game::GameOptions;
    use crate::setup::MapScript;
    let seed: u64 = std::env::var("CIVVIS_DIAG_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(26090140);
    for on in [false, true] {
        if !on && std::env::var("CIVVIS_DIAG_SKIP_OFF").is_ok() {
            continue;
        }
        let mut world = Game::new_with(GameOptions {
            speed: "online".to_string(),
            map_script: MapScript::Continents,
            difficulty: "prince".to_string(),
            ..GameOptions::new(6, 74, 46, seed, 250, 9)
        });
        let mut ais: Vec<AdvancedAi> = (0..world.players.len())
            .map(|pid| {
                let mut ai = AdvancedAi::new();
                if on && pid == 0 {
                    ai.enable_chase_every_boost();
                }
                ai
            })
            .collect();
        world.set_fog_memory(false);
        world.set_war_ledger(false);
        let mut reported = std::collections::BTreeSet::new();
        while world.winner.is_none() && world.turn <= world.max_turns {
            let pid = world.current;
            if pid == 0
                && [60u32, 120, 180].contains(&world.turn)
                && !reported.contains(&world.turn)
            {
                reported.insert(world.turn);
                let chases = ais[0].eureka_chases(&world, 0);
                let mut brief: Vec<String> = chases
                    .iter()
                    .map(|c| format!("{}[{} left {}]", c.node, c.trigger, c.remaining))
                    .collect();
                brief.sort();
                if std::env::var("CIVVIS_DIAG_PRODUCTION").is_ok() {
                    let cid = world.player_city_ids(0)[0];
                    let plan =
                        ais[0]
                            .current_plan()
                            .cloned()
                            .unwrap_or(super::super::StrategicPlan {
                                strategy: super::super::GrandStrategy::Science,
                                target_player: None,
                                target_city: None,
                                threatened_city: None,
                                desired_cities: 4,
                                assessed_turn: world.turn,
                                rush: false,
                            });
                    let counts = ais[0].counts(&world, 0);
                    let items = world.producible_items(0, cid);
                    let values = ais[0].production_values(&world, 0, cid, &items, &plan, counts);
                    let mut table: Vec<(f64, f64, String)> = items
                        .iter()
                        .zip(values.iter())
                        .map(|(item, value)| {
                            (
                                *value,
                                ais[0].chase_production_premium(&world, 0, cid, item),
                                format!("{item:?}"),
                            )
                        })
                        .collect();
                    table.sort_by(|a, b| b.0.total_cmp(&a.0));
                    let brief: Vec<String> = table
                        .iter()
                        .take(14)
                        .map(|(v, prem, name)| {
                            format!(
                                "{}={:.0}(+{:.0})",
                                name.replace("Item::", "").replace(' ', ""),
                                v,
                                prem
                            )
                        })
                        .collect();
                    eprintln!("on={on} t{} PRODUCTION {}", world.turn, brief.join(" "));
                }
                let p = &world.players[0];
                eprintln!(
                        "on={on} t{} cities {} score {} techs {} boosted∩ {} | civics {} inspired∩ {} | chases {}: {}",
                        world.turn,
                        world.player_city_ids(0).len(),
                        world.score(0),
                        p.techs.len(),
                        p.techs.iter().filter(|t| p.boosted_techs.contains(t)).count(),
                        p.civics.len(),
                        p.civics.iter().filter(|c| p.boosted_civics.contains(c)).count(),
                        chases.len(),
                        brief.join(" ")
                    );
            }
            ais[pid].take_turn(&mut world, pid);
            if world.winner.is_none() && world.current == pid {
                let _ = world.apply(pid, &Action::EndTurn);
            }
        }
        world.finish_at_turn_limit();
        let p = &world.players[0];
        let mut unboosted: Vec<String> = p
            .techs
            .iter()
            .filter(|t| !p.boosted_techs.contains(t))
            .map(|t| {
                let trig = world.rules.techs[t]
                    .boost
                    .as_ref()
                    .map(|b| format!("{}x{}", b.trigger, b.count))
                    .unwrap_or_else(|| "no-boost".into());
                format!("{t}[{trig}]")
            })
            .collect();
        unboosted.sort();
        let mut unboosted_civics: Vec<String> = p
            .civics
            .iter()
            .filter(|c| !p.boosted_civics.contains(c))
            .map(|c| {
                let trig = world.rules.civics[c]
                    .boost
                    .as_ref()
                    .map(|b| format!("{}x{}", b.trigger, b.count))
                    .unwrap_or_else(|| "no-boost".into());
                format!("{c}[{trig}]")
            })
            .collect();
        unboosted_civics.sort();
        eprintln!(
            "on={on} END t{} score {} cities {} techs {} boosted∩ {} civics {} inspired∩ {}",
            world.turn,
            world.score(0),
            world.player_city_ids(0).len(),
            p.techs.len(),
            p.techs
                .iter()
                .filter(|t| p.boosted_techs.contains(t))
                .count(),
            p.civics.len(),
            p.civics
                .iter()
                .filter(|c| p.boosted_civics.contains(c))
                .count(),
        );
        eprintln!("on={on} UNBOOSTED TECHS: {}", unboosted.join(" "));
        eprintln!("on={on} UNINSPIRED CIVICS: {}", unboosted_civics.join(" "));
    }
}

#[test]
fn the_research_hooks_read_the_gene_as_the_union_of_the_old_family() {
    let mut game = capital_board(57_012);
    game.players[0].boosted_techs.insert(name!("masonry"));
    game.players[0]
        .boosted_civics
        .insert(name!("craftsmanship"));
    let plain = AdvancedAi::new();
    let ai = armed();
    assert_eq!(plain.boost_in_hand_scale(&game, 0, "masonry", true), 1.0);
    // The in-hand scale is the one old-family hook the gene does NOT arm:
    // see `boost_in_hand_scale` for the isolation that removed it.
    assert_eq!(ai.boost_in_hand_scale(&game, 0, "masonry", true), 1.0);
    assert!((ai.boost_in_hand_scale(&game, 0, "craftsmanship", false) - 1.0).abs() < 1e-9);
}
