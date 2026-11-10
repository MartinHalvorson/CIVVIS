//! Tests of `advanced/battle_planner.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

#[test]
fn a_bleeding_city_is_besieged_whatever_the_fog_says() {
    // The t115 shape from run civvis-20260807T181839Z: city under fire,
    // hostile list empty, production about to pick a culture building.
    let mut game = Game::new_full(2, 28, 18, 7, 1_000, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("seat starts with a settler");
    let position = game.units[&settler].pos;
    let capital = game.found_city_for(0, position, None);
    game.remove_unit(settler);
    game.players[0].techs.insert(crate::name!("masonry"));
    game.cities.get_mut(&capital).unwrap().hp = 165; // damage 35 of 200

    let mut treated = AdvancedAi::new();
    treated.enable_garrison_under_fire();
    let item = treated.base.besieged_city_item(&game, 0, capital);
    assert_eq!(
        item,
        Some(Item::Building {
            building: crate::name!("walls")
        }),
        "a bleeding city must reach for walls with zero visible besiegers"
    );

    let stock = AdvancedAi::new();
    assert_eq!(
        stock.base.besieged_city_item(&game, 0, capital),
        None,
        "the frozen controllers keep the two-visible-besiegers gate"
    );

    let mut bridged = AdvancedAi::new();
    bridged.enable_live_bridge_universe();
    assert!(
        bridged.base.garrison_under_fire,
        "the screen universe carries the treatment even when the ledger withholds it"
    );
    bridged.disable_garrison_under_fire();
    assert!(!bridged.base.garrison_under_fire);
}

/// ★★★★ One barbarian scout pinned a whole opening (civvis-20260816T151716Z
/// t15–t35: the settler held fourteen turns, our scout fled every frame,
/// warrior and builder went nowhere). See `barbarian_scouts_are_scouts`.
/// On the live seat a barbarian scout two tiles from a settler prices no
/// risk; a barbarian warrior at the same spot still does; stock prices
/// both, and a rival's scout at war is still priced.
#[test]
fn a_barbarian_scout_is_not_a_threat_the_settler_prices() {
    let (game0, _capital, home) = empire_with_a_capital(71_117);
    // `empire_with_a_capital` puts players 0 and 1 at war; make 1 the
    // barbarian seat.
    let mut game = game0.clone();
    game.players[1].is_barbarian = true;
    game.barb_pid = Some(1);
    let post = anchor_at(&game, home, 3);
    let settler = game.spawn_test_unit("settler", 0, post);
    // A land tile two steps off that the settler can actually see.
    let two_off = {
        let mut ring: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| {
                game.wdist(*pos, post) == 2
                    && game
                        .map
                        .get(*pos)
                        .is_some_and(|t| game.rules.is_passable(t) && !game.rules.is_water(t))
                    && game.player_can_see(0, *pos)
            })
            .collect();
        ring.sort_unstable();
        ring.into_iter()
            .next()
            .expect("a visible land tile two steps off")
    };
    let scout = game.spawn_test_unit("scout", 1, two_off);

    let stock = AdvancedAi::new();
    let mut withheld = AdvancedAi::new();
    withheld.disable_barbarian_scouts_are_scouts();
    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    live.enable_barbarian_scouts_are_scouts();
    assert!(
        live.barbarian_scouts_are_scouts,
        "the focused arm carries the treatment"
    );
    let visible = game.player_vision_now(0);
    let stock_risk = stock.settlement_tile_risk(&game, 0, Some(settler), post, &visible);
    let withheld_risk = withheld.settlement_tile_risk(&game, 0, Some(settler), post, &visible);
    let live_risk = live.settlement_tile_risk(&game, 0, Some(settler), post, &visible);
    assert!(
        withheld_risk > 0.0,
        "the withheld arm keeps pricing the barbarian scout ({withheld_risk})"
    );
    assert_eq!(
        stock_risk, 0.0,
        "production prices a barbarian scout at nothing: it cannot act in \
         either regime"
    );
    assert_eq!(
        live_risk, 0.0,
        "the live seat prices a barbarian scout at nothing"
    );

    // A barbarian warrior at the same spot is a threat to both.
    game.remove_unit(scout);
    game.spawn_test_unit("warrior", 1, two_off);
    let visible = game.player_vision_now(0);
    assert!(live.settlement_tile_risk(&game, 0, Some(settler), post, &visible) > 0.0);
    assert!(stock.settlement_tile_risk(&game, 0, Some(settler), post, &visible) > 0.0);

    // A rival's scout at war (not barbarian) is still priced on the live seat.
    let mut rival = game0.clone();
    let settler = rival.spawn_test_unit("settler", 0, post);
    rival.spawn_test_unit("scout", 1, two_off);
    let visible = rival.player_vision_now(0);
    assert!(live.settlement_tile_risk(&rival, 0, Some(settler), post, &visible) > 0.0);

    let mut withheld = AdvancedAi::new();
    withheld.enable_live_bridge();
    withheld.disable_barbarian_scouts_are_scouts();
    let mut again = game0.clone();
    again.players[1].is_barbarian = true;
    again.barb_pid = Some(1);
    let settler = again.spawn_test_unit("settler", 0, post);
    again.spawn_test_unit("scout", 1, two_off);
    let visible = again.player_vision_now(0);
    assert!(withheld.settlement_tile_risk(&again, 0, Some(settler), post, &visible) > 0.0);
    assert!(AdvancedAi::new().barbarian_scouts_are_scouts);
    assert!(!AdvancedAi::legacy().barbarian_scouts_are_scouts);
}

/// V2 reserves one Builder only for a concrete first-copy luxury debt, after
/// expansion is covered, and keeps a receipt after paying it once.
#[test]
fn first_builder_reserve_2_is_a_one_shot_after_expansion() {
    let (mut game, capital, home) = empire_with_a_capital(71_155);
    clear_barbarian_fixture(&mut game);
    game.at_war.clear();
    game.turn = 30;
    game.players[0]
        .techs
        .extend([crate::name!("mining"), crate::name!("irrigation")]);
    let work = game.cities[&capital]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != home)
        .expect("the capital owns a workable plot");
    {
        let tile = game.map.tiles.get_mut(&work).expect("work plot");
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = true;
        tile.resource = Some(crate::name!("silk"));
        tile.improvement = None;
        tile.pillaged = false;
    }
    game.cities.get_mut(&capital).unwrap().pop = 6;
    assert!(
        game.valid_improvements(0, work)
            .contains(&crate::name!("plantation")),
        "Silk is immediately connectable"
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.enable_first_builder_reserve_2();
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let cities = game.player_city_ids(0);
    let counts = ai.counts(&game, 0);
    assert_eq!(counts.builders, 0);
    assert!(
        ai.empire_amenity_deficit(&game, 0) > 0.0,
        "the first luxury has an immediate empire payoff"
    );
    assert_eq!(
        ai.first_builder_reserve_2_city(&game, 0, &plan, &counts, &cities),
        None,
        "the Builder cannot consume the queue that owes the next Settler"
    );

    let settler = game.spawn_test_unit("settler", 0, home);
    let counts = ai.counts(&game, 0);
    let target = ai
        .first_builder_reserve_2_city(&game, 0, &plan, &counts, &cities)
        .expect("a Settler in flight exposes the first-copy luxury debt");
    ai.advanced_production(&mut game, 0, &plan, false);
    assert_eq!(game.cities[&target].queue.first(), Some(&builder));
    assert!(
        ai.first_builder_reserve_2_paid,
        "paying the luxury debt records the one-shot receipt"
    );
    game.remove_unit(settler);
    game.cities.get_mut(&target).unwrap().queue.clear();
    let counts = ai.counts(&game, 0);
    assert_eq!(
        ai.first_builder_reserve_2_city(&game, 0, &plan, &counts, &cities),
        None,
        "spending or losing that Builder must not reserve replacements"
    );

    ai.enable_first_builder_reserve();
    assert!(ai.first_builder_reserve && !ai.first_builder_reserve_2);
    ai.enable_first_builder_reserve_2();
    assert!(!ai.first_builder_reserve && ai.first_builder_reserve_2);
    assert!(!AdvancedAi::legacy().first_builder_reserve_2);
    let gene = crate::ai::gene("first-builder-reserve-2").expect("registered v2");
    assert!(gene.opt_in() && gene.screenable());
}

/// `border-parity`: a met major's city six tiles from our capital, fielding
/// six Warriors against our none, makes the seat buy its capital a defender
/// with Gold above the reserve; at parity, or off, or with the treasury at
/// the reserve, it buys nothing.
#[test]
fn a_weak_seat_beside_a_strong_neighbour_buys_the_contact_citys_defender() {
    let mut game = Game::new_full(2, 40, 24, 91_776, 250, 0, false);
    game.current = 0;
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("a starting settler");
        let pos = game.units[&settler].pos;
        game.remove_unit(settler);
        game.found_city_for(pid, pos, None);
    }
    for pid in 0..2 {
        for unit in game.player_unit_ids(pid) {
            game.remove_unit(unit);
        }
    }
    let ours = game.player_city_ids(0)[0];
    let home = game.cities[&ours].pos;
    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for position in &positions {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        game.players[0].explored.insert(*position);
    }
    // A second rival city six tiles from our capital, with a Warrior stack.
    let mut ring: Vec<Pos> = positions
        .iter()
        .copied()
        .filter(|pos| game.wdist(*pos, home) == 6)
        .collect();
    ring.sort_unstable();
    let border = ring[0];
    game.found_city_for(1, border, None);
    for _ in 0..6 {
        game.spawn_test_unit("warrior", 1, border);
    }
    game.record_contact(0, 1);
    game.players[0].gold = 1_000.0;
    assert!(
        game.military_power(0) < BORDER_PARITY_RATIO * game.military_power(1),
        "fixture: the seat is below parity"
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let _ = &plan;
    let units_before = game.player_unit_ids(0).len();
    let mut ai = AdvancedAi::new();
    assert!(!ai.border_parity, "the gene ships off");
    assert!(!AdvancedAi::legacy().border_parity);
    let mut untouched = game.clone();
    assert!(
        !ai.border_parity_purchase(&mut untouched, 0),
        "off, nothing is bought"
    );
    ai.enable_border_parity();
    assert!(ai.border_parity);
    let gold_before = game.players[0].gold;
    assert!(
        ai.border_parity_purchase(&mut game, 0),
        "below parity beside a strong neighbour, the contact city buys a defender"
    );
    assert_eq!(game.player_unit_ids(0).len(), units_before + 1);
    assert!(game.players[0].gold < gold_before);
    // At the reserve, nothing.
    let mut poor = untouched.clone();
    poor.players[0].gold = BORDER_PARITY_RESERVE;
    assert!(
        !ai.border_parity_purchase(&mut poor, 0),
        "the reserve is kept"
    );
    // At parity, nothing.
    let mut armed = untouched.clone();
    for _ in 0..8 {
        armed.spawn_test_unit("warrior", 0, home);
    }
    assert!(
        armed.military_power(0) >= BORDER_PARITY_RATIO * armed.military_power(1),
        "fixture: eight Warriors reach parity"
    );
    assert!(
        !ai.border_parity_purchase(&mut armed, 0),
        "at parity, nothing is bought"
    );
    ai.disable_border_parity();
    assert!(!ai.border_parity);
}

/// `age-closer`: three points short of a Normal Age names the shortfall;
/// at the bar, eight short, or off, nothing; and the ordinary closeness
/// limit stands for the buyer to lift.
#[test]
fn a_seat_a_few_era_points_short_names_the_shortfall() {
    let (mut game, _city, _) = empire_with_a_capital(71_119);
    game.players[0].era_score = 10;
    game.players[0].normal_age_threshold = 13;
    let mut ai = AdvancedAi::new();
    assert!(!ai.age_closer, "the gene ships off");
    assert!(!AdvancedAi::legacy().age_closer);
    assert_eq!(ai.era_points_short(&game, 0), None, "off, nothing");
    ai.enable_age_closer();
    assert!(ai.age_closer);
    assert_eq!(ai.era_points_short(&game, 0), Some(3), "three points short");
    game.players[0].era_score = 13;
    assert_eq!(ai.era_points_short(&game, 0), None, "at the bar");
    game.players[0].era_score = 5;
    assert_eq!(
        ai.era_points_short(&game, 0),
        None,
        "eight short is past a moment's reach"
    );
    assert_eq!(ai.patronage_close_limit(100.0), 0.15);
    assert_eq!(ai.patronage_close_limit(500.0), 0.40);
    ai.disable_age_closer();
    assert!(!ai.age_closer);
}

#[test]
fn boosted_bargain_versions_are_registered_reversible_and_exclusive() {
    for (tag, field) in [
        ("boosted-bargain-first", "boosted_bargain_first"),
        ("boosted-bargain-first-2", "boosted_bargain_first_2"),
        ("boosted-bargain-first-3", "boosted_bargain_first_3"),
    ] {
        assert!(
            GENES
                .iter()
                .any(|gene| gene.opt_in() && gene.tag == tag && gene.field == field),
            "{tag} must be a registered native opt-in"
        );
        assert!(
            crate::ai::advanced::gene_ledger::screenable(tag),
            "the ledger must be able to price {tag} independently"
        );
    }
    let mut ai = AdvancedAi::new();
    ai.enable_boosted_bargain_first();
    assert!(ai.boosted_bargain_first && !ai.boosted_bargain_first_2);
    ai.enable_boosted_bargain_first_2();
    assert!(!ai.boosted_bargain_first && ai.boosted_bargain_first_2);
    ai.enable_boosted_bargain_first();
    assert!(ai.boosted_bargain_first && !ai.boosted_bargain_first_2);
    ai.disable_boosted_bargain_first();
    assert!(!ai.boosted_bargain_first);
    ai.enable_boosted_bargain_first_2();
    ai.disable_boosted_bargain_first_2();
    assert!(!ai.boosted_bargain_first_2);
    ai.enable_boosted_bargain_first_2();
    ai.enable_boosted_bargain_first_3();
    assert!(!ai.boosted_bargain_first && !ai.boosted_bargain_first_2 && ai.boosted_bargain_first_3);
    ai.enable_boosted_bargain_first();
    assert!(ai.boosted_bargain_first && !ai.boosted_bargain_first_3);
    ai.enable_boosted_bargain_first_3();
    ai.disable_boosted_bargain_first_3();
    assert!(!ai.boosted_bargain_first_3);
}

/// `cheapest-wonder-first`: a one-city empire whose capital can finish a
/// wonder within twelve turns opens the live race for it under the gene;
/// off, the three-city guard refuses it. The bargain-city test and the
/// toggles are checked on the same board.
#[test]
fn a_wonder_twelve_turns_from_done_opens_the_race_in_a_one_city_empire() {
    let (mut game, city, _) = empire_with_a_capital(71_121);
    {
        let capital = game.cities.get_mut(&city).expect("capital");
        capital.pop = 30;
        capital
            .buildings
            .extend([crate::name!("monument"), crate::name!("granary")]);
    }
    game.players[0].techs.extend([
        crate::name!("pottery"),
        crate::name!("mining"),
        crate::name!("masonry"),
        crate::name!("astrology"),
        crate::name!("writing"),
        crate::name!("archery"),
        crate::name!("bronze_working"),
        crate::name!("irrigation"),
    ]);
    game.players[0].civics.extend([
        crate::name!("code_of_laws"),
        crate::name!("mysticism"),
        crate::name!("craftsmanship"),
        crate::name!("foreign_trade"),
        crate::name!("political_philosophy"),
    ]);
    let production = game.city_yields(city).production.max(1.0);
    let mut wonders: Vec<(f64, Item)> = game
        .producible_items(0, city)
        .into_iter()
        .filter(|item| matches!(item, Item::Wonder { .. }))
        .map(|item| {
            (
                game.item_remaining_cost_for_city(0, city, &item) / production,
                item,
            )
        })
        .collect();
    wonders.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (_, wonder) = wonders
        .first()
        .cloned()
        .expect("fixture: the capital can build some wonder");
    // Bank most of the wonder's cost as the idle queue's overflow, the way a
    // resumed race stands: three turns of production remain.
    let full_cost = game.item_remaining_cost_for_city(0, city, &wonder);
    game.cities.get_mut(&city).expect("capital").production = full_cost - 3.0 * production;
    let turns = game.item_remaining_cost_for_city(0, city, &wonder) / production;
    assert!(
        turns <= CHEAPEST_WONDER_TURNS,
        "fixture: the wonder {wonder:?} needs {turns:.1} turns at {production:.1} production"
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    assert!(!ai.cheapest_wonder_first, "the gene ships off");
    assert!(!AdvancedAi::legacy().cheapest_wonder_first);
    ai.enable_live_wonder_race();
    assert!(
        ai.wonder_bargain_city(&game, 0, city),
        "the only city is the strongest"
    );
    let counts = ai.counts(&game, 0);
    let refused = ai.production_value(&game, 0, city, &wonder, &plan, &counts);
    assert!(
        refused <= -10_000.0,
        "off, a one-city empire fails the three-city guard: {refused}"
    );
    ai.enable_cheapest_wonder_first();
    assert!(ai.cheapest_wonder_first);
    let opened = ai.production_value(&game, 0, city, &wonder, &plan, &counts);
    assert!(
        opened > 0.0,
        "under the gene the twelve-turn wonder opens the race: {opened}"
    );
    ai.disable_cheapest_wonder_first();
    assert!(!ai.cheapest_wonder_first);
}

/// `border-parity-3`: whole-empire weakness and a nearby rival city are no
/// longer enough. Two actually visible non-recon land bodies staged beside an
/// ungarrisoned city open exactly one local defender purchase; neither an idle
/// queue nor an active Settler is touched.
#[test]
fn visible_border_staging_fills_one_local_garrison_without_preemption() {
    let mut game = Game::new_full(2, 40, 24, 91_779, 250, 0, false);
    game.current = 0;
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("a starting settler");
        let pos = game.units[&settler].pos;
        game.remove_unit(settler);
        game.found_city_for(pid, pos, None);
    }
    for pid in 0..2 {
        for unit in game.player_unit_ids(pid) {
            game.remove_unit(unit);
        }
    }
    let ours = game.player_city_ids(0)[0];
    let home = game.cities[&ours].pos;
    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for position in &positions {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        game.players[0].explored.insert(*position);
    }
    let mut city_ring: Vec<Pos> = positions
        .iter()
        .copied()
        .filter(|pos| game.wdist(*pos, home) == 6)
        .collect();
    city_ring.sort_unstable();
    game.found_city_for(1, city_ring[0], None);
    let mut staging_ring: Vec<Pos> = positions
        .iter()
        .copied()
        .filter(|pos| game.wdist(*pos, home) == 2)
        .collect();
    staging_ring.sort_unstable();
    let staged = staging_ring[0];
    let first = game.spawn_test_unit("warrior", 1, staged);
    let second = game.spawn_test_unit("warrior", 1, staged);
    game.record_contact(0, 1);
    game.players[0].gold = 1_000.0;
    assert!(
        game.player_can_see(0, staged),
        "fixture: the staging tile is in current city vision"
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    assert!(!ai.border_parity_3, "the new version ships off");
    assert!(!AdvancedAi::legacy().border_parity_3);
    assert_eq!(ai.border_parity_3_target(&game, 0), None, "off, nothing");
    ai.enable_border_parity_3();
    assert!(ai.border_parity_3);
    assert!(
        !ai.border_parity && !ai.border_parity_2,
        "one family version"
    );
    let (strength, rival, contact, bodies) = ai
        .border_parity_3_target(&game, 0)
        .expect("two visible bodies stage beside the empty city");
    assert!(strength > 0.0);
    assert_eq!((rival, contact, bodies), (1, ours, 2));

    let mut passing = game.clone();
    passing.remove_unit(second);
    assert_eq!(
        ai.border_parity_3_target(&passing, 0),
        None,
        "one passing body is not a staged army"
    );
    assert!(passing.units.contains_key(&first));

    let mut idle = game.clone();
    idle.players[0].gold = 0.0;
    let mut idle_control = idle.clone();
    AdvancedAi::new().advanced_production(&mut idle_control, 0, &plan, false);
    ai.advanced_production(&mut idle, 0, &plan, false);
    assert_eq!(
        idle.cities[&ours].queue, idle_control.cities[&ours].queue,
        "version three leaves an idle queue to ordinary production"
    );

    let mut committed = game.clone();
    committed.players[0].gold = 0.0;
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    // This is a legal commitment, not a population-one blocked Settler.
    committed.cities.get_mut(&ours).expect("capital").pop = 2;
    committed
        .apply(
            0,
            &Action::Produce {
                city: ours,
                item: settler.clone(),
            },
        )
        .unwrap();
    ai.advanced_production(&mut committed, 0, &plan, false);
    assert_eq!(
        committed.cities[&ours].queue.first(),
        Some(&settler),
        "version three never preempts an active queue"
    );

    let units_before = game.player_unit_ids(0).len();
    assert!(
        ai.border_parity_3_purchase(&mut game, 0),
        "the exposed city buys its one garrison"
    );
    assert_eq!(game.player_unit_ids(0).len(), units_before + 1);
    assert_eq!(
        ai.border_parity_3_target(&game, 0),
        None,
        "the first local defender closes the bounded debt"
    );
    assert!(
        !ai.border_parity_3_purchase(&mut game, 0),
        "there is no second purchase"
    );
    ai.enable_border_parity_2();
    assert!(ai.border_parity_2);
    assert!(
        !ai.border_parity && !ai.border_parity_3,
        "v2 selects itself"
    );
    ai.enable_border_parity();
    assert!(ai.border_parity);
    assert!(
        !ai.border_parity_2 && !ai.border_parity_3,
        "v1 selects itself"
    );
}

#[test]
fn loyalty_endangered_cities_pause_settlers_before_they_revolt() {
    // Antium founded at t23 of civvis-20260827T203744Z and began a Settler at
    // t29.  Its host-reported Loyalty collapsed from -0.4 to -10.4/turn, yet
    // the default production margin kept the queue through its Free City flip
    // at t44.  The live rate is the fact this handoff must act on; it should
    // pause the build before the city spends its remaining usable turns on a
    // civilian it cannot finish safely.
    let (mut game, city, _) = empire_with_a_capital(71_144);
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    let city_state = game.cities.get_mut(&city).expect("capital exists");
    city_state.pop = 3;
    city_state.loyalty = 48.0;
    assert!(game.can_produce(0, city, &settler));
    game.apply(
        0,
        &Action::Produce {
            city,
            item: settler.clone(),
        },
    )
    .expect("queue the unsafe Settler");
    game.cities.get_mut(&city).unwrap().production = 17.0;
    std::sync::Arc::make_mut(&mut game.observed_city_loyalty_per_turn).insert(city, -8.0);

    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let ai = AdvancedAi::new();
    let completion_turns = ai.settler_factory_turns(&game, 0, city);
    let (_, turns_to_flip) = AdvancedAi::settler_queue_loyalty_risk(&game, city, completion_turns)
        .expect("the observed collapse beats the Settler deadline");
    assert!(turns_to_flip < completion_turns);

    // The strategic scorer must refuse a fresh queue too, not merely repair
    // a Settler that was already committed through the delegated governor.
    let mut empty = game.clone();
    empty.cities.get_mut(&city).unwrap().queue.clear();
    assert!(
        ai.production_value(&empty, 0, city, &settler, &plan, &ai.counts(&empty, 0))
            < PRODUCTION_VETO_FLOOR,
        "a city that will flip before completion cannot start another Settler"
    );

    let live = AdvancedAi::new();
    live.redirect_loyalty_endangered_settler_queues(&mut game, 0, &plan);
    let replacement = game.cities[&city]
        .queue
        .first()
        .cloned()
        .expect("the city receives a productive non-Settler replacement");
    assert_ne!(replacement, settler, "the doomed Settler must be paused");
    assert_eq!(
        game.cities[&city].production_progress.get("unit:settler"),
        Some(&17.0),
        "switching keeps the Settler's invested Production banked"
    );

    // A slow, recoverable drift remains an expansion city.  This safety floor
    // is about a real imminent flip, not a generic anti-growth penalty.
    let mut recoverable = empty;
    recoverable.cities.get_mut(&city).unwrap().queue = vec![settler.clone()];
    std::sync::Arc::make_mut(&mut recoverable.observed_city_loyalty_per_turn).insert(city, -0.5);
    assert!(
        AdvancedAi::settler_queue_loyalty_risk(
            &recoverable,
            city,
            ai.settler_factory_turns(&recoverable, 0, city),
        )
        .is_none(),
        "a city with ample time to stabilize keeps its Settler"
    );
    live.redirect_loyalty_endangered_settler_queues(&mut recoverable, 0, &plan);
    assert_eq!(recoverable.cities[&city].queue.first(), Some(&settler));
}

/// `settler-backlog-brake`: with three cities and a Settler parked six turns
/// the pipeline is closed; off, the land grab's three walkers stand. Below
/// three cities the brake never speaks.
#[test]
fn a_parked_settler_closes_the_pipeline_under_the_brake() {
    let mut game = Game::new(2, 24, 16, 71, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("a starting settler");
    let mut ai = AdvancedAi::new();
    ai.enable_land_grab();
    assert!(!ai.settler_backlog_brake && !ai.base.settler_backlog_brake);
    ai.enable_settler_backlog_brake();
    assert!(ai.settler_backlog_brake && ai.base.settler_backlog_brake);
    for turn in 1..=BasicAi::SETTLER_BACKLOG_IDLE_TURNS + 1 {
        game.turn = turn;
        ai.base.refresh_settler_idle(&game, 0);
    }
    assert_eq!(
        ai.base
            .parked_settlers(&game, 0, BasicAi::SETTLER_BACKLOG_IDLE_TURNS),
        1,
        "fixture: the starting settler never moved"
    );
    let _ = settler;
    assert_eq!(
        ai.settler_pipeline_width(&game, 0, 16, 3, 1),
        0,
        "three cities and a parked settler: the pipeline is closed"
    );
    assert_eq!(
        ai.settler_pipeline_width(&game, 0, 16, 2, 1),
        2,
        "the opening is untouched"
    );
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 16, 3, 1),
        3,
        "the ungated width is the land grab's three walkers"
    );
    ai.disable_settler_backlog_brake();
    assert!(!ai.settler_backlog_brake && !ai.base.settler_backlog_brake);
    assert_eq!(
        ai.settler_pipeline_width(&game, 0, 16, 3, 1),
        3,
        "off, the land grab's three walkers stand"
    );
}

/// V2 preserves V1's full stop after six cities while removing the veto from
/// the high-leverage three-to-five-city growth wave.
#[test]
fn settler_backlog_v2_waits_for_a_mature_empire() {
    let mut game = Game::new(2, 24, 16, 71, 250, 0);
    let mut ai = AdvancedAi::new();
    ai.enable_land_grab();
    ai.enable_settler_backlog_brake_2();
    assert!(!ai.settler_backlog_brake && ai.settler_backlog_brake_2);
    assert!(!ai.base.settler_backlog_brake && ai.base.settler_backlog_brake_2);

    for turn in 1..=BasicAi::SETTLER_BACKLOG_IDLE_TURNS {
        game.turn = turn;
        ai.base.refresh_settler_idle(&game, 0);
    }
    assert_eq!(
        ai.settler_pipeline_width(&game, 0, 16, 3, 1),
        3,
        "the patience window leaves the whole pipeline alone"
    );
    game.turn = BasicAi::SETTLER_BACKLOG_IDLE_TURNS + 1;
    ai.base.refresh_settler_idle(&game, 0);
    assert_eq!(
        ai.base
            .parked_settlers(&game, 0, BasicAi::SETTLER_BACKLOG_IDLE_TURNS),
        1,
        "fixture: one starting Settler is parked"
    );
    assert_eq!(
        ai.settler_pipeline_width(&game, 0, 16, 3, 1),
        3,
        "a parked walker does not end the three-city growth wave"
    );
    assert_eq!(
        ai.settler_pipeline_width(&game, 0, 16, 6, 1),
        0,
        "the full brake returns once the empire holds six cities"
    );
    assert_eq!(
        ai.settler_pipeline_width(&game, 0, 16, 2, 1),
        2,
        "the opening remains untouched"
    );
    ai.enable_settler_backlog_brake();
    assert!(ai.settler_backlog_brake && !ai.settler_backlog_brake_2);
    assert!(ai.base.settler_backlog_brake && !ai.base.settler_backlog_brake_2);
    assert_eq!(
        ai.settler_pipeline_width(&game, 0, 16, 3, 1),
        0,
        "V1 still closes the entire pipeline"
    );
}

/// The land grab's pipeline: two walkers from the first city, one more per
/// three cities, never more than the seats still short. Every default
/// constructor and the frozen anchor keep the one-at-a-time gate.
#[test]
fn the_land_grab_pipeline_widens_with_the_empire() {
    // The pipeline reads the board only for `expansion_schedule`'s pace,
    // which is off here; every answer below is the land grab's own.
    let game = Game::new(2, 24, 16, 71, 250, 0);
    let mut ai = AdvancedAi::new();
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 16, 1, 0),
        1,
        "off unless the bridge asks"
    );
    ai.enable_land_grab();
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 16, 1, 0),
        2,
        "one city: two walkers"
    );
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 16, 1, 1),
        2,
        "the second may start while the first walks"
    );
    assert_eq!(ai.settler_in_flight_allowed(&game, 16, 2, 1), 2);
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 16, 3, 0),
        3,
        "three cities: three walkers"
    );
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 16, 6, 2),
        4,
        "six cities: four"
    );
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 16, 9, 0),
        5,
        "nine cities: five"
    );
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 16, 14, 0),
        2,
        "two seats short: two walkers"
    );
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 16, 15, 0),
        1,
        "one seat short: one walker"
    );
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 16, 15, 1),
        1,
        "the walker covers the last seat; the target stays the hard cap"
    );
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 16, 16, 0),
        1,
        "at the target the arm's own gate refuses"
    );
    assert_eq!(
        AdvancedAi::legacy().settler_in_flight_allowed(&game, 16, 3, 0),
        1
    );
}

#[test]
fn timed_war_target_and_breakthrough_survive_ordinary_reassessment() {
    let (game, _, objective) = timed_war_fixture(940_004);
    let mut ai = AdvancedAi::new();
    ai.timed_war = true;
    ai.war_plan = Some(timed_plan(
        &game,
        objective,
        crate::name!("military_tactics"),
        crate::name!("pikeman"),
        None,
        None,
        WarPhase::Research,
    ));
    for assessed_turn in [game.turn, game.turn + 5] {
        let mut ordinary = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 6,
            assessed_turn,
            rush: false,
        };
        ai.apply_war_plan_to_strategy(&mut ordinary);
        assert_eq!(ordinary.strategy, GrandStrategy::Conquest);
        assert_eq!(ordinary.target_player, Some(1));
        assert_eq!(ordinary.target_city, Some(objective));
        assert_eq!(
            ai.war_plan.as_ref().unwrap().breakthrough_tech,
            crate::name!("military_tactics")
        );
    }
}

#[test]
fn timed_war_prebuild_floor_counts_units_and_queues() {
    let (mut game, home, objective) = timed_war_fixture(940_006);
    clear_military(&mut game, 0);
    let city_pos = game.cities[&home].pos;
    game.spawn_test_unit("warrior", 0, city_pos);
    game.spawn_test_unit("warrior", 0, city_pos);
    let mut ai = AdvancedAi::new();
    ai.timed_war = true;
    ai.war_plan = Some(timed_plan(
        &game,
        objective,
        crate::name!("iron_working"),
        crate::name!("swordsman"),
        Some(crate::name!("warrior")),
        None,
        WarPhase::Research,
    ));
    game.cities.get_mut(&home).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("warrior"),
    }];
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(objective),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    let counts = ai.counts(&game, 0);
    let candidate = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let missing = ai.production_value(&game, 0, home, &candidate, &plan, &counts);
    assert!(missing > 7_000.0, "the fourth body outranks infrastructure");
    game.spawn_test_unit("warrior", 0, city_pos);
    let filled = ai.production_value(&game, 0, home, &candidate, &plan, &ai.counts(&game, 0));
    assert!(
        filled < 7_000.0,
        "the exact four-body vacancy disappears once filled"
    );
}

#[test]
fn legacy_controller_keeps_battlefront_observation_off() {
    assert!(AdvancedAi::new().battlefront_observation);
    assert!(!AdvancedAi::legacy().battlefront_observation);
}

#[test]
fn advanced_formations_link_breach_support_to_a_military_escort() {
    let mut game = Game::new_full(1, 20, 14, 71_001, 30, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let position = *game
        .map
        .tiles
        .iter()
        .find(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .map(|(position, _)| position)
        .unwrap();
    let ram = game.spawn_test_unit("battering_ram", 0, position);
    let cavalry = game.spawn_test_unit("heavy_chariot", 0, position);
    let spear = game.spawn_test_unit("spearman", 0, position);

    let ai = AdvancedAi::new();
    ai.advanced_formations(&mut game, 0);

    let escort = game.units[&ram]
        .linked_to
        .expect("the support unit is paired with an available military escort");
    assert!(
        [cavalry, spear].contains(&escort),
        "the generic support rule selects one of the available military escorts"
    );
    assert_eq!(game.units[&escort].linked_to, Some(ram));
    let unpaired = if escort == cavalry { spear } else { cavalry };
    assert_eq!(game.units[&unpaired].linked_to, None);
}

/// The same separation applies on land, where the current live game was
/// repeatedly issuing `ENTER_FORMATION`, `EXIT_FORMATION`, then a delayed
/// march.  A deployment controller keeps the native gene off and instead
/// sends the guard through the ordinary shadow path.
#[test]
fn deployed_live_shadow_does_not_link_a_land_settler_after_the_native_gene_is_withheld() {
    let mut game = Game::new_full(1, 20, 14, 71_002, 30, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let land = *game
        .map
        .tiles
        .iter()
        .find(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .map(|(position, _)| position)
        .expect("the board has passable land");
    let settler = game.spawn_test_unit("settler", 0, land);
    let escort = game.spawn_test_unit("heavy_chariot", 0, land);

    let mut historical = AdvancedAi::new();
    historical.settlement_safety = true;
    historical.settler_targets.insert(settler, land);
    historical.settler_blocked_turns.insert(settler, 1);
    historical.advanced_formations(&mut game, 0);
    assert_eq!(game.units[&settler].linked_to, Some(escort));
    let _ = game.apply(0, &Action::UnlinkUnits { unit: settler });

    let mut deployed = AdvancedAi::new();
    deployed.enable_live_bridge();
    deployed.settler_targets.insert(settler, land);
    deployed.settler_blocked_turns.insert(settler, 1);
    assert!(deployed.live_formationless_settler_shadow);
    deployed.advanced_formations(&mut game, 0);
    assert_eq!(game.units[&settler].linked_to, None);
    assert_eq!(game.units[&escort].linked_to, None);
}

/// The measured t174 purchase: five gold in the treasury, one turn after
/// Civilization VI disbanded a third of the army to pay the upkeep bill,
/// and CIVVIS bought another Field Cannon because the gate only ever asked
/// about faith.
#[test]
fn a_broke_empire_stops_buying_soldiers_with_faith() {
    let mut game = Game::new(2, 20, 14, 91_501, 80, 0);
    let unit = game
        .rules
        .units
        .iter()
        .find(|(_, spec)| spec.class == "military" && spec.maintenance > 0.0)
        .map(|(name, _)| name.to_string())
        .expect("the ruleset has a military unit that costs upkeep");
    let upkeep = game.rules.units[unit.as_str()].maintenance;

    let mut ai = AdvancedAi::new();
    ai.enable_solvent_faith_army();

    // The measured state: broke, and losing money.
    game.players[0].gold = 5.0;
    game.players[0].gold_per_turn = -3.0;
    assert!(
        !ai.faith_military_is_affordable(&game, 0, &unit),
        "an empire on five gold and falling must not take on more upkeep"
    );

    // Funding only the new unit does not carry the already-negative income.
    game.players[0].gold = upkeep * FAITH_ARMY_SOLVENCY_TURNS + 1.0;
    assert!(
        !ai.faith_military_is_affordable(&game, 0, &unit),
        "the existing deficit still drains a bank that funds only new upkeep"
    );

    // A deep balance carries both bills even while income is short.
    game.players[0].gold = (upkeep + 3.0) * FAITH_ARMY_SOLVENCY_TURNS + 1.0;
    assert!(
        ai.faith_military_is_affordable(&game, 0, &unit),
        "a treasury that can pay for {FAITH_ARMY_SOLVENCY_TURNS} turns may still arm"
    );

    // And a surplus never needs the balance at all.
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = upkeep + 1.0;
    assert!(
        ai.faith_military_is_affordable(&game, 0, &unit),
        "income that covers the new bill is sufficient on its own"
    );
}

#[test]
fn production_advanced_reflects_the_bounded_recovery_selection() {
    let bare = AdvancedAi::new();
    assert!(
        !bare.bounded_recovery,
        "the bare controller remains opt-in-off"
    );
    assert!(bare.envoy_priority);
    let selected = crate::ai::ledger_default_on("bounded-recovery")
        .expect("bounded-recovery has a deployment selection");

    let mut production = AdvancedAi::new();
    production.enable_engine_repairs();
    assert_eq!(
        production.bounded_recovery, selected,
        "production must follow the recorded deployment selection"
    );

    let mut live_bridge = AdvancedAi::new();
    live_bridge.enable_live_bridge();
    assert_eq!(
        live_bridge.bounded_recovery, selected,
        "the live bridge must follow the recorded deployment selection"
    );
}

#[test]
fn production_advanced_scales_cities_and_development_together() {
    let production = AdvancedAi::new();
    // ⚠ Three, not six. The production floor was removed on 2026-08-10
    // after the promotion matrix returned PASS for withholding it:
    // deployment-online 55.9%, Elo +41 (CI +7..+76), 125/65, p=0.0000.
    // `advanced_wide_opening` still carries six, so the axis stays
    // reachable.
    assert_eq!(production.city_target_floor, 3);
    assert_ne!(production.city_target_floor, PRODUCTION_CITY_TARGET_FLOOR);
    assert!(production.plan_city_target);
    assert_eq!(production.base.w.city_target, 4.0);
    assert_eq!(production.base.w.builder_per_city, 0.5);
    assert!(!production.bounded_recovery);
    assert!(production.settlement_safety);
    assert!(production.adjacency_site_planning);

    // Frozen and evaluator controls keep the old policy exactly, including
    // its smaller economy and default-off live-defense adapters.
    let control = AdvancedAi::pre_policy_envoy();
    assert_eq!(control.city_target_floor, 3);
    assert!(!control.plan_city_target);
    assert_eq!(control.base.w.city_target, 4.0);
    assert_eq!(control.base.w.builder_per_city, 0.5);
    assert!(!control.bounded_recovery);

    let frozen = AdvancedAi::legacy();
    assert_eq!(frozen.city_target_floor, 3);
    assert!(!frozen.plan_city_target);
    assert_eq!(frozen.base.w.city_target, 4.0);
    assert_eq!(frozen.base.w.builder_per_city, 0.5);
    assert!(!frozen.bounded_recovery);

    // Production policy never mutates a wider, better-developed genome.
    let wide = Weights {
        city_target: 10.0,
        builder_per_city: 0.9,
        ..Weights::default()
    };
    let weighted = AdvancedAi::with_weights(wide);
    assert_eq!(weighted.base.w.city_target, 10.0);
    assert_eq!(weighted.base.w.builder_per_city, 0.9);
}

#[test]
fn production_policy_envoy_default_is_distinct_from_evaluator_and_legacy_controls() {
    let production = AdvancedAi::new();
    assert_eq!(production.weights().policy_deck, PolicyDeck::Live);
    // ⚠ Asserted, not argued: the withhold added on 2026-08-11 must not be
    // able to move the frozen anchor, and the anchor must genuinely be on
    // the deck `Weights::default()` describes even though production is not.
    assert_eq!(
        AdvancedAi::legacy().weights().policy_deck,
        PolicyDeck::Legacy,
        "advanced_v1 must stay on the deck it was rated with"
    );
    assert_eq!(
        AdvancedAi::with_legacy_policy_deck().weights().policy_deck,
        PolicyDeck::Legacy,
        "the withhold must actually withhold, or its number measures nothing"
    );
    assert!(production.envoy_priority);

    let pre_promotion = AdvancedAi::pre_policy_envoy();
    assert_eq!(pre_promotion.weights().policy_deck, PolicyDeck::Legacy);
    assert!(!pre_promotion.envoy_priority);
    assert!(pre_promotion.victory_planning);

    let legacy = AdvancedAi::legacy();
    assert_eq!(legacy.weights().policy_deck, PolicyDeck::Legacy);
    assert!(!legacy.envoy_priority);
    assert!(!legacy.victory_planning);

    let weighted = Weights {
        pol_influence: 4.0,
        ..Weights::default()
    };
    let weighted_production = AdvancedAi::with_weights(weighted);
    assert_eq!(weighted_production.weights().policy_deck, PolicyDeck::Live);
    assert_eq!(weighted_production.weights().pol_influence, 4.0);
}

/// The research floor is a ramp on the remaining game and is absent from
/// the frozen controls, so an ablation measures a real difference.
#[test]
fn the_research_floor_ramps_with_the_horizon_and_stays_out_of_the_controls() {
    let mut early = Game::new(2, 24, 16, 5_411, 250, 0);
    early.turn = 1;
    let mut late = early.clone();
    late.turn = 240;
    let mut finished = early.clone();
    finished.turn = 251;

    let mut ai = AdvancedAi::new();
    ai.refresh_research_weight(&early);
    let early_weight = ai.research_weight;
    ai.refresh_research_weight(&late);
    let late_weight = ai.research_weight;
    ai.refresh_research_weight(&finished);
    let finished_weight = ai.research_weight;

    assert!(
        early_weight > 2.9 && early_weight <= RESEARCH_FLOOR_EARLY,
        "a beaker bought on turn 1 compounds for the whole game: {early_weight}"
    );
    assert!(
        late_weight < 1.2,
        "a beaker bought on turn 240 buys nothing that can be built: {late_weight}"
    );
    assert!(
        early_weight > late_weight,
        "the floor must decay, not sit flat"
    );
    assert_eq!(
        finished_weight, RESEARCH_FLOOR_LATE,
        "past the turn limit the horizon clamps at zero rather than going negative"
    );

    for mut control in [AdvancedAi::legacy(), AdvancedAi::pre_policy_envoy()] {
        control.refresh_research_weight(&early);
        assert_eq!(
            control.research_weight, 0.0,
            "the evaluation controls must keep the pre-treatment pricing"
        );
    }
}

/// The defect this repairs: every lane except Science priced a beaker below
/// its own least valuable ordinary yield, and the live Kongo run spent 89 of
/// 106 sampled turns in exactly those lanes.
#[test]
fn beakers_keep_a_floor_in_every_lane_but_never_outbid_the_science_lane() {
    let mut game = Game::new(2, 24, 16, 5_412, 250, 0);
    game.turn = 40;
    let mut ai = AdvancedAi::new();
    ai.refresh_research_weight(&game);
    let legacy = AdvancedAi::legacy();
    let one_science = Yields {
        science: 1.0,
        ..Yields::default()
    };

    for strategy in [
        GrandStrategy::Religion,
        GrandStrategy::Conquest,
        GrandStrategy::Recovery,
        GrandStrategy::Expansion,
        GrandStrategy::Culture,
        GrandStrategy::Diplomacy,
    ] {
        let treated = ai.yield_value(one_science, strategy);
        let control = legacy.yield_value(one_science, strategy);
        assert!(
            treated > control,
            "{strategy:?} must value research above the lane-only anchor: \
                 {treated} vs {control}"
        );
        assert!(
            treated < ai.yield_value(one_science, GrandStrategy::Science),
            "{strategy:?} must not outbid the Science lane's own currency"
        );
    }

    assert_eq!(
        ai.yield_value(one_science, GrandStrategy::Science),
        legacy.yield_value(one_science, GrandStrategy::Science),
        "the Science lane's promoted weight is untouched"
    );
    let one_faith = Yields {
        faith: 1.0,
        ..Yields::default()
    };
    assert_eq!(
        ai.yield_value(one_faith, GrandStrategy::Religion),
        legacy.yield_value(one_faith, GrandStrategy::Religion),
        "the floor is science-only: no other yield moves"
    );
}

/// The live run's second city held an Industrial Zone and a Mbanza and
/// never built a Campus, because a Religion empire's Campus bonus is zero.
#[test]
fn a_religion_empire_still_wants_a_campus_in_a_city_that_has_none() {
    let mut game = Game::new(2, 32, 24, 5_413, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    game.players[0].techs.insert(crate::name!("writing"));
    game.turn = 60;

    let mut ai = AdvancedAi::new();
    ai.refresh_research_weight(&game);
    let legacy = AdvancedAi::legacy();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Religion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let counts = ai.counts(&game, 0);
    let campus_site = game
        .district_sites(city, crate::name!("campus"))
        .into_iter()
        .next()
        .expect("a legal Campus site");
    let campus = Item::District {
        district: crate::name!("campus"),
        pos: campus_site,
    };

    let treated = ai.production_value(&game, 0, city, &campus, &plan, &counts);
    let control = legacy.production_value(&game, 0, city, &campus, &plan, &counts);
    assert!(
        treated > control * 1.5,
        "a Religion empire must still pay for its first Campus: {treated} vs {control}"
    );
}

/// A Campus with no Library is the expensive half of a research city bought
/// and the cheap half declined. That debt was only ever paid at war.
#[test]
fn a_campus_without_its_library_carries_a_debt_in_peacetime_too() {
    let mut game = Game::new(2, 32, 24, 5_414, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    install_ai_test_district(&mut game, city, "campus");
    game.players[0].techs.insert(crate::name!("writing"));
    game.turn = 60;

    let mut ai = AdvancedAi::new();
    ai.refresh_research_weight(&game);
    let legacy = AdvancedAi::legacy();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Religion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let counts = ai.counts(&game, 0);
    let library = Item::Building {
        building: crate::name!("library"),
    };
    assert!(game.can_produce(0, city, &library));

    let treated = ai.production_value(&game, 0, city, &library, &plan, &counts);
    let control = legacy.production_value(&game, 0, city, &library, &plan, &counts);
    assert!(
        treated > control * 2.0,
        "the peacetime Campus-building debt must be paid: {treated} vs {control}"
    );

    // And it is owed for the Campus, not for every building in the city.
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    let treated_monument = ai.production_value(&game, 0, city, &monument, &plan, &counts);
    let control_monument = legacy.production_value(&game, 0, city, &monument, &plan, &counts);
    assert!(
        treated_monument <= control_monument * 1.05,
        "an unrelated building must not collect the research debt: \
             {treated_monument} vs {control_monument}"
    );
}

/// Host competitions move the diplomatic race outside the ordinary Congress
/// ballot. The exact score table must therefore make World Games and either
/// Aid Request project legal and compelling, while the World's Fair rewards
/// the Great Person points existing district projects already produce.
#[test]
fn live_competitions_price_host_projects_and_worlds_fair_production() {
    let mut game = Game::new(2, 32, 24, 5_415, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    game.turn = 180;
    game.players[0].techs.insert(crate::name!("pottery"));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.refresh_research_weight(&game);
    let athlete = Item::Project {
        project: crate::name!("train_athletes"),
    };
    let aid = Item::Project {
        project: crate::name!("send_aid"),
    };
    let granary = Item::Building {
        building: crate::name!("granary"),
    };
    assert!(game.can_produce(0, city, &granary));
    assert!(
        !game.can_produce(0, city, &athlete),
        "the host-only project cannot escape into a native menu"
    );
    assert!(
        !game.can_produce(0, city, &aid),
        "the shared Aid Request project cannot escape into a native menu"
    );

    game.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_WORLD_GAMES".to_string(),
        ends: game.turn + 50,
        ours: 0.0,
        leader: 50.0,
    }]);
    assert!(game.can_produce(0, city, &athlete));
    let resumed: Game = serde_json::from_value(
        serde_json::to_value(&game).expect("the live competition checkpoint saves"),
    )
    .expect("the live competition checkpoint reloads");
    assert_eq!(resumed.host_competitions, game.host_competitions);
    assert!(
        resumed.can_produce(0, city, &athlete),
        "a resumed live mirror keeps the host-granted project legal"
    );
    let counts = ai.counts(&game, 0);
    let athlete_value = ai.production_value(&game, 0, city, &athlete, &plan, &counts);
    let granary_value = ai.production_value(&game, 0, city, &granary, &plan, &counts);
    assert!(
        athlete_value > granary_value,
        "a 50-point project that catches the World Games leader must outrank ordinary infrastructure: {athlete_value} vs {granary_value}"
    );

    // Once the host competition ends, the bridge withdraws both legality and
    // its valuation; a stale project must never survive as a neutral fallback.
    game.replace_host_competitions(Vec::new());
    assert!(!game.can_produce(0, city, &athlete));
    assert!(ai.production_value(&game, 0, city, &athlete, &plan, &counts) < -1_000.0);

    // Firaxis grants the same 200-point Send Aid project for either kind of
    // request. Each individual host kind must make it legal and compelling;
    // treating the plural rules row as an AND would silently lose one aid
    // emergency.
    for kind in ["EMERGENCY_SEND_AID", "EMERGENCY_SEND_MILITARY_AID"] {
        game.replace_host_competitions(vec![crate::game::HostCompetition {
            kind: kind.to_string(),
            ends: game.turn + 50,
            ours: 0.0,
            leader: 200.0,
        }]);
        assert!(
            game.can_produce(0, city, &aid),
            "{kind} grants the shared Send Aid project"
        );
        let aid_value = ai.production_value(&game, 0, city, &aid, &plan, &counts);
        assert!(
            aid_value > granary_value,
            "the 200-point Send Aid project must outrank ordinary infrastructure during {kind}: {aid_value} vs {granary_value}"
        );
    }
    game.replace_host_competitions(vec![
        crate::game::HostCompetition {
            kind: "EMERGENCY_SEND_AID".to_string(),
            ends: game.turn + 1,
            ours: 0.0,
            leader: 200.0,
        },
        crate::game::HostCompetition {
            kind: "EMERGENCY_SEND_MILITARY_AID".to_string(),
            ends: game.turn + 50,
            ours: 0.0,
            leader: 200.0,
        },
    ]);
    let best_live_aid_value = ai.production_value(&game, 0, city, &aid, &plan, &counts);
    assert!(
        best_live_aid_value > granary_value,
        "when both kinds are active, the viable military request must win over an expiring ordinary request: {best_live_aid_value} vs {granary_value}"
    );
    game.replace_host_competitions(Vec::new());
    assert!(
        !game.can_produce(0, city, &aid),
        "with neither aid emergency live the shared project is withdrawn"
    );
    assert!(ai.production_value(&game, 0, city, &aid, &plan, &counts) < -1_000.0);

    install_ai_test_district(&mut game, city, "campus");
    let grants = Item::Project {
        project: crate::name!("campus_research_grants"),
    };
    assert!(game.can_produce(0, city, &grants));
    let baseline = ai.production_value(&game, 0, city, &grants, &plan, &counts);
    game.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_WORLDS_FAIR".to_string(),
        ends: game.turn + 50,
        ours: 0.0,
        leader: 10.0,
    }]);
    let boosted = ai.production_value(&game, 0, city, &grants, &plan, &counts);
    assert!(
        boosted > baseline + 50.0,
        "a Campus project's Great Scientist points must close the live World's Fair race: {boosted} vs {baseline}"
    );
}

/// Firaxis counts selected Great Person points in its Nobel competitions just
/// as it counts every class for the World's Fair. A hosted Nobel Physics race
/// should therefore pull a Scientist project, while Literature should pull a
/// Writer/Artist/Musician project—and neither race may price the other class.
#[test]
fn live_nobel_competitions_price_only_matching_great_person_projects() {
    let mut game = Game::new(2, 32, 24, 5_416, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    game.turn = 180;
    install_ai_test_district(&mut game, city, "campus");
    install_ai_test_district(&mut game, city, "theater_square");

    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.refresh_research_weight(&game);
    let scientist = Item::Project {
        project: crate::name!("campus_research_grants"),
    };
    let literature = Item::Project {
        project: crate::name!("theater_square_festival"),
    };
    assert!(game.can_produce(0, city, &scientist));
    assert!(game.can_produce(0, city, &literature));
    let counts = ai.counts(&game, 0);
    let scientist_baseline = ai.production_value(&game, 0, city, &scientist, &plan, &counts);
    let literature_baseline = ai.production_value(&game, 0, city, &literature, &plan, &counts);

    game.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_PHYSICS".to_string(),
        ends: game.turn + 50,
        ours: 0.0,
        leader: 10.0,
    }]);
    let physics_scientist = ai.production_value(&game, 0, city, &scientist, &plan, &counts);
    let physics_literature = ai.production_value(&game, 0, city, &literature, &plan, &counts);
    assert!(
        physics_scientist > scientist_baseline + 50.0,
        "Nobel Physics must price a project's Great Scientist points: {physics_scientist} vs {scientist_baseline}"
    );
    assert_eq!(
        physics_literature, literature_baseline,
        "Nobel Physics must not claim Writer/Artist/Musician points"
    );

    game.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_LITERATURE".to_string(),
        ends: game.turn + 50,
        ours: 0.0,
        leader: 15.0,
    }]);
    let literature_scientist = ai.production_value(&game, 0, city, &scientist, &plan, &counts);
    let literature_project = ai.production_value(&game, 0, city, &literature, &plan, &counts);
    assert_eq!(
        literature_scientist, scientist_baseline,
        "Nobel Literature must not claim Great Scientist points"
    );
    assert!(
        literature_project > literature_baseline + 50.0,
        "Nobel Literature must price Writer/Artist/Musician project points: {literature_project} vs {literature_baseline}"
    );
}

/// Nobel Peace is unlike the other Nobel competitions: its score source is
/// Favor generated during the competition.  A live seat that can finish the
/// Foreign Ministry in time must therefore value its +3-per-turn stream, but
/// no unrelated competition or already-expired Peace contest may borrow it.
#[test]
fn live_nobel_peace_prices_favor_buildings_only_before_its_deadline() {
    let mut game = Game::new(2, 32, 24, 5_417, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    game.turn = 180;
    install_ai_test_district(&mut game, city, "government_plaza");
    game.cities
        .get_mut(&city)
        .expect("capital")
        .buildings
        .push(crate::name!("ancestral_hall"));
    // Keep the fixture inside Nobel Peace's short clock without changing the
    // Foreign Ministry's real cost or its prerequisite chain.
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        city,
        Yields {
            production: 120.0,
            ..Yields::default()
        },
    );
    let foreign_ministry = Item::Building {
        building: crate::name!("foreign_ministry"),
    };
    assert!(
        game.can_produce(0, city, &foreign_ministry),
        "the completed Plaza and its first tier unlock Foreign Ministry"
    );
    assert_eq!(
        game.rules.buildings[&crate::name!("foreign_ministry")]
            .effects
            .get("diplomatic_favor"),
        Some(&3.0),
        "the fixture uses the host's real +3 Favor-per-turn source"
    );

    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.refresh_research_weight(&game);
    let counts = ai.counts(&game, 0);
    let baseline = ai.production_value(&game, 0, city, &foreign_ministry, &plan, &counts);

    game.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_LITERATURE".to_string(),
        ends: game.turn + 30,
        ours: 0.0,
        leader: 50.0,
    }]);
    assert_eq!(
        ai.production_value(&game, 0, city, &foreign_ministry, &plan, &counts),
        baseline,
        "Nobel Literature scores Great Person points, not generated Favor"
    );

    game.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_PEACE".to_string(),
        ends: game.turn + 30,
        ours: 0.0,
        leader: 50.0,
    }]);
    let peace = ai.production_value(&game, 0, city, &foreign_ministry, &plan, &counts);
    assert!(
        peace > baseline + 50.0,
        "a timely +3 Favor stream must close the Nobel Peace race: {peace} vs {baseline}"
    );

    game.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_PEACE".to_string(),
        ends: game.turn + 1,
        ours: 0.0,
        leader: 50.0,
    }]);
    assert_eq!(
        ai.production_value(&game, 0, city, &foreign_ministry, &plan, &counts),
        baseline,
        "a building that cannot generate Favor before the deadline receives no stale score"
    );
}

/// Monarchy's Star Fort modifier is not stored on Renaissance Walls itself:
/// the engine grants its Favor from the government to every completed member
/// of that building family.  Nobel Peace must price that generated stream for
/// both the generic building and Georgia's Tsikhe replacement, but no earlier
/// wall tier or other government can claim it.
#[test]
fn live_nobel_peace_prices_monarchy_renaissance_wall_favor() {
    let mut game = Game::new(2, 32, 24, 5_418, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    game.turn = 180;
    game.players[0].government = Some("monarchy".to_string());
    game.players[0].techs.insert(crate::name!("siege_tactics"));
    game.cities
        .get_mut(&city)
        .expect("capital")
        .buildings
        .extend([crate::name!("walls"), crate::name!("medieval_walls")]);
    let wall_hp = game.city_max_wall_hp(&game.cities[&city]);
    game.cities.get_mut(&city).expect("capital").wall_hp = wall_hp;
    // Keep this defensive building inside Nobel Peace's short clock without
    // changing its real cost or prerequisite chain.
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        city,
        Yields {
            production: 120.0,
            ..Yields::default()
        },
    );
    let renaissance_walls = Item::Building {
        building: crate::name!("renaissance_walls"),
    };
    assert!(game.can_produce(0, city, &renaissance_walls));
    assert_eq!(
        game.rules.buildings[&crate::name!("renaissance_walls")]
            .effects
            .get("diplomatic_favor"),
        None,
        "the source is Monarchy's Star Fort modifier, not a field on the wall"
    );
    assert_eq!(
        game.gov_effects(0).walled_city_diplomatic_favor,
        2.0,
        "the fixture uses Monarchy's real +2 Favor-per-wall-per-turn modifier"
    );

    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.refresh_research_weight(&game);
    let counts = ai.counts(&game, 0);
    let baseline = ai.production_value(&game, 0, city, &renaissance_walls, &plan, &counts);
    let medieval_walls = Item::Building {
        building: crate::name!("medieval_walls"),
    };
    let medieval_baseline = ai.production_value(&game, 0, city, &medieval_walls, &plan, &counts);

    game.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_LITERATURE".to_string(),
        ends: game.turn + 30,
        ours: 0.0,
        leader: 50.0,
    }]);
    assert_eq!(
        ai.production_value(&game, 0, city, &renaissance_walls, &plan, &counts),
        baseline,
        "Nobel Literature must not claim Monarchy's generated Favor"
    );

    game.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_PEACE".to_string(),
        ends: game.turn + 30,
        ours: 0.0,
        leader: 50.0,
    }]);
    let peace = ai.production_value(&game, 0, city, &renaissance_walls, &plan, &counts);
    assert!(
        peace > baseline + 50.0,
        "a timely Monarchy wall must close the Nobel Peace race: {peace} vs {baseline}"
    );
    assert_eq!(
        ai.production_value(&game, 0, city, &medieval_walls, &plan, &counts),
        medieval_baseline,
        "the earlier wall tier must not claim Monarchy's Star Fort Favor"
    );

    game.players[0].government = Some("merchant_republic".to_string());
    let other_government_peace =
        ai.production_value(&game, 0, city, &renaissance_walls, &plan, &counts);
    game.replace_host_competitions(Vec::new());
    let other_government_baseline =
        ai.production_value(&game, 0, city, &renaissance_walls, &plan, &counts);
    assert_eq!(
        other_government_peace, other_government_baseline,
        "another government must not borrow Monarchy's wall Favor"
    );

    game.players[0].government = Some("monarchy".to_string());
    game.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_PEACE".to_string(),
        ends: game.turn + 1,
        ours: 0.0,
        leader: 50.0,
    }]);
    assert_eq!(
        ai.production_value(&game, 0, city, &renaissance_walls, &plan, &counts),
        baseline,
        "a wall that cannot generate Favor before the deadline receives no stale score"
    );

    game.players[0].civ = "Georgia".to_string();
    let tsikhe = Item::Building {
        building: crate::name!("tsikhe"),
    };
    assert!(game.can_produce(0, city, &tsikhe));
    assert!(
        game.building_is_family(crate::name!("tsikhe"), crate::name!("renaissance_walls")),
        "Tsikhe is the real Renaissance Walls replacement the engine checks"
    );
    let tsikhe_baseline = ai.production_value(&game, 0, city, &tsikhe, &plan, &counts);
    game.replace_host_competitions(vec![crate::game::HostCompetition {
        kind: "EMERGENCY_NOBEL_PRIZE_PEACE".to_string(),
        ends: game.turn + 30,
        ours: 0.0,
        leader: 50.0,
    }]);
    let tsikhe_peace = ai.production_value(&game, 0, city, &tsikhe, &plan, &counts);
    assert!(
        tsikhe_peace > tsikhe_baseline + 50.0,
        "the Tsikhe replacement must receive the same Nobel Peace Favor score: {tsikhe_peace} vs {tsikhe_baseline}"
    );
}

#[test]
fn conquest_without_city_objective_holds_a_garrison_until_the_target_exists() {
    let mut game = Game::new_full(2, 24, 16, 73_102, 250, 0, false);
    for pid in 0..2 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("starting settler");
        game.apply(pid, &Action::FoundCity { unit: settler })
            .expect("found city");
    }
    game.current = 0;
    let home = game.player_city_ids(0)[0];
    let target = game.player_city_ids(1)[0];
    for unit in game.player_unit_ids(0) {
        if game.rules.units[game.units[&unit].kind].class == "military" {
            game.remove_unit(unit);
        }
    }
    let position = game.cities[&home].pos;
    game.spawn_test_unit("warrior", 0, position);
    game.spawn_test_unit("warrior", 0, position);
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = -8.0;

    let no_objective = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let appointed = StrategicPlan {
        target_city: Some(target),
        ..no_objective.clone()
    };
    let ai = AdvancedAi::new();
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let counts = ai.counts(&game, 0);

    assert_eq!(counts.military, 2);
    assert!(
        ai.production_value(&game, 0, home, &warrior, &no_objective, &counts) < -1_000.0,
        "a conquest label without a city objective must not keep buying units"
    );
    assert!(
        ai.production_value(&game, 0, home, &warrior, &appointed, &counts) > 0.0,
        "appointing a real city must reopen the offensive force package"
    );
}

#[test]
fn recovery_reads_the_war_v2_is_a_reversible_exclusive_opt_in() {
    let gene = GENES
        .iter()
        .find(|gene| gene.tag == "recovery-reads-the-war-2")
        .expect("version two is published for gene_screen");
    assert!(gene.opt_in() && gene.screenable() && !gene.live());
    assert!(!AdvancedAi::new().recovery_reads_the_war_2);
    assert!(!AdvancedAi::legacy().recovery_reads_the_war_2);

    let mut family = AdvancedAi::new();
    family.enable_recovery_reads_the_war();
    assert!(family.recovery_reads_the_war && !family.recovery_reads_the_war_2);
    family.enable_recovery_reads_the_war_2();
    assert!(!family.recovery_reads_the_war && family.recovery_reads_the_war_2);
    family.enable_recovery_reads_the_war();
    assert!(family.recovery_reads_the_war && !family.recovery_reads_the_war_2);
    family.enable_recovery_reads_the_war_2();
    family.disable_recovery_reads_the_war_2();
    assert!(!family.recovery_reads_the_war && !family.recovery_reads_the_war_2);
}

#[test]
fn rush_route_targets_are_frozen_after_the_first_complete_capital_state() {
    let mut game = connected_rush_fixture();
    let mut connected = AdvancedAi::new();
    connected.early_rush = true;
    connected.route_connected_rush = true;
    assert!(connected.freeze_rush_route_targets(&game, 0));
    let frozen = connected.rush_route_targets.clone();
    assert_eq!(frozen, Some(BTreeSet::from([1])));

    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    assert!(!connected.freeze_rush_route_targets(&game, 0));
    assert_eq!(connected.rush_route_targets, frozen);
}

#[test]
fn island_expansion_genes_are_reversible_opt_ins() {
    let mut ai = AdvancedAi::new();
    assert!(!ai.base.island_exploration);
    assert!(!ai.overseas_settlement);

    ai.enable_island_exploration();
    ai.enable_overseas_settlement();
    assert!(ai.base.island_exploration);
    assert!(ai.overseas_settlement);

    ai.disable_island_exploration();
    ai.disable_overseas_settlement();
    assert!(!ai.base.island_exploration);
    assert!(!ai.overseas_settlement);
}

#[test]
fn expansion_dispatch_and_late_window_are_independent_on_online() {
    let mut game = Game::new_with(GameOptions {
        speed: "online".to_string(),
        ..GameOptions::new(1, 30, 18, 7_114, 250, 0)
    });
    // These are intentionally derived from the live duration curve. The
    // earlier preregistration prose named an obsolete 150..225 range,
    // while the engine's fidelity-correct duration scaling resolves the
    // shipped formulas to this 198..217 window.
    assert_eq!(game.standard_duration(300), 198);
    assert_eq!(game.standard_duration(50), 33);
    assert_eq!(AdvancedAi::stock_expansion_deadline(&game), 198);
    assert_eq!(AdvancedAi::late_expansion_deadline(&game), 217);

    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: 0,
        rush: false,
    };
    let stock = AdvancedAi::new();
    let mut dispatcher = AdvancedAi::new();
    dispatcher.expansion_dispatch = true;
    let mut late = AdvancedAi::new();
    late.late_expansion = true;
    let mut combined = AdvancedAi::new();
    combined.expansion_dispatch = true;
    combined.late_expansion = true;

    game.turn = 197;
    assert!(stock.adaptive_expansion_window_open(&game));
    assert!(late.adaptive_expansion_window_open(&game));
    game.turn = 198;
    assert!(!stock.adaptive_expansion_window_open(&game));
    assert!(late.adaptive_expansion_window_open(&game));
    game.turn = 216;
    assert!(combined.adaptive_expansion_window_open(&game));
    game.turn = 217;
    assert!(!combined.adaptive_expansion_window_open(&game));

    assert!(!stock.adaptive_expansion_dispatches(&plan, None));
    assert!(dispatcher.adaptive_expansion_dispatches(&plan, None));
    assert!(!late.adaptive_expansion_dispatches(&plan, None));
    assert!(combined.adaptive_expansion_dispatches(&plan, None));
    assert!(!combined.adaptive_expansion_dispatches(&plan, Some(VictoryTarget::Culture)));

    let targeted = AdvancedAi::targeting(VictoryTarget::Culture);
    game.turn = game.standard_duration(175) - 1;
    assert!(targeted.settler_expansion_window_open(&game, 0, 0));
    game.turn = game.standard_duration(175);
    assert!(!targeted.settler_expansion_window_open(&game, 0, 0));
}

/// Off by default, set only by the live bridge, holdable off on its own —
/// the siege-commitment term follows the same contract as every other
/// bridge repair, so the frozen `advanced_v1` anchor keeps its ladder.
#[test]
fn only_the_live_bridge_finishes_a_siege() {
    let fresh = AdvancedAi::new();
    assert!(!fresh.siege_commitment);
    assert!(!AdvancedAi::legacy().siege_commitment);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(live.siege_commitment);
    live.disable_siege_commitment();
    assert!(!live.siege_commitment);
}

#[test]
fn strategic_plan_is_stable_inside_assessment_window() {
    let mut g = Game::new(2, 24, 16, 72, 30, 0);
    let mut ai = AdvancedAi::new();
    ai.take_turn(&mut g, 0);
    let first = ai.current_plan().unwrap().clone();
    assert!(!ai.plan_stale(&g, 0));
    assert_eq!(ai.current_plan(), Some(&first));
}

#[test]
fn surprise_wars_and_imminent_victories_interrupt_the_planning_window() {
    let mut game = Game::new_full(3, 30, 18, 721, 300, 0, false);
    for pid in 0..3 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    game.turn = 190;
    for rival in 1..game.players.len() {
        game.record_contact(0, rival);
    }
    let mut ai = AdvancedAi::new();
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: Some(2),
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    });
    assert!(!ai.plan_stale(&game, 0));

    game.at_war.insert((0, 1));
    assert!(ai.plan_stale(&game, 0), "a surprise war must replan now");

    game.at_war.clear();
    ai.plan.as_mut().unwrap().target_player = Some(1);
    game.players[2].science_projects.extend([
        "launch_earth_satellite".to_string(),
        "launch_moon_landing".to_string(),
        "launch_mars_colony".to_string(),
        "exoplanet_expedition".to_string(),
    ]);
    game.players[2].exoplanet_distance = 42.0;
    assert!(
        ai.plan_stale(&game, 0),
        "an imminent rival victory must replan now"
    );
}

#[test]
fn religious_denial_triggers_with_one_unconverted_civilization() {
    let mut game = Game::new_full(4, 30, 18, 7_215, 300, 0, false);
    for pid in 0..4 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    game.players[1].religion = Some("Rival Faith".to_string());
    for owner in [1, 2, 3] {
        let city = game.player_city_ids(owner)[0];
        game.cities
            .get_mut(&city)
            .unwrap()
            .pressure
            .insert("Rival Faith".to_string(), 1_000.0);
    }

    let ai = AdvancedAi::new();
    let pressure = ai.rival_victory_pressure(&game, 1);
    assert_eq!(pressure.strategy, GrandStrategy::Religion);
    assert_eq!(pressure.progress, 75);
    assert_eq!(
        ai.victory_denial(&game, 0),
        Some((1, GrandStrategy::Conquest))
    );

    // The `advanced_blind_to_leaders` ablation is silent on the same
    // position, and silent about its urgency, while reading the identical
    // pressure. An ablation that changed what the empire *sees* would
    // measure something other than the response.
    let mut blind = AdvancedAi::new();
    blind.deny_leaders = false;
    assert_eq!(blind.rival_victory_pressure(&game, 1).progress, 75);
    assert_eq!(blind.victory_denial(&game, 0), None);
    assert!(!blind.urgent_victory_threat(&game, 1));
    assert!(ai.urgent_victory_threat(&game, 1));
}

/// ★★★ A stock cannot be pumped at match point. Of the thirteen live
/// games this seat finished AHEAD on score (2026-08-16/17), FIVE were
/// ended at t229-245 by a rival Culture or Diplomatic victory while the
/// general 90 alarm had not fired. Those two counters are accumulations,
/// so they need Science's lead time, and the Culture counter additionally
/// needs OUR culture to be the bar the leader must clear.
#[test]
fn the_stock_denial_lanes_get_lead_time_and_refuse_an_inert_counter() {
    let mut game = Game::new_full(3, 30, 18, 7_219, 300, 0, false);
    for pid in 0..3 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    game.victory_conditions = crate::game::VictoryConditions::default();

    // Drive the race through the OBSERVED counters — the live path the
    // mirror fills from Firaxis' own World Rankings screen (#1855/#1860).
    // Player 1's tourists sit at 80% of the bar: past `STOCK_DENIAL_BAR`,
    // short of the general 90 alarm. WE hold the bar, so racing culture
    // moves it.
    {
        let observed = std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
            .entry(1)
            .or_default();
        observed.foreign_tourists = Some(80);
    }
    {
        let observed = std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
            .entry(0)
            .or_default();
        observed.domestic_tourists = Some(100);
    }
    {
        let observed = std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
            .entry(2)
            .or_default();
        observed.domestic_tourists = Some(10);
    }

    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_deny_while_targeted();
    let pressure = ai.rival_victory_pressure(&game, 1);
    assert_eq!(pressure.strategy, GrandStrategy::Culture);
    assert!(
        (78..90).contains(&pressure.progress),
        "fixture must sit between the stock bar and the general alarm, got {}",
        pressure.progress
    );

    assert!(
        !ai.urgent_victory_threat(&game, 1),
        "without the flag a Culture race below 90 raises no alarm at all"
    );
    assert_eq!(
        ai.victory_denial(&game, 0),
        None,
        "without the early alarm, the ordinary race margin keeps the assigned lane focused"
    );
    ai.enable_stock_denial_lead_time();
    assert!(
        ai.urgent_victory_threat(&game, 1),
        "the stock lane must alarm at its own bar"
    );
    assert_eq!(
        ai.victory_denial(&game, 0),
        Some((1, GrandStrategy::Culture)),
        "an urgent Culture stock must interrupt even a completed assigned race"
    );
    assert!(
        ai.culture_denial_actionable(&game, 0, 1, GrandStrategy::Culture),
        "we hold the bar, so racing culture moves it"
    );

    // Now a third civilization holds the bar, just above ours, so the
    // leader is still past the stock bar but our own culture cannot move
    // its denominator. The counter is inert and the lane keeps its focus.
    {
        let observed = std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
            .entry(2)
            .or_default();
        observed.domestic_tourists = Some(110);
    }
    {
        let observed = std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
            .entry(1)
            .or_default();
        observed.foreign_tourists = Some(88);
    }
    assert!(
        (78..90).contains(&ai.rival_victory_pressure(&game, 1).progress),
        "the leader must still be past the stock bar in the inert case"
    );
    assert!(
        !ai.culture_denial_actionable(&game, 0, 1, GrandStrategy::Culture),
        "a bar we do not hold makes the Culture counter inert"
    );
    assert!(
        ai.culture_denial_actionable(&game, 0, 1, GrandStrategy::Conquest),
        "the gate speaks only for the Culture counter"
    );

    // Default off, and the live bridge turns it on.
    assert!(!AdvancedAi::new().stock_denial_lead_time);
    assert!(!AdvancedAi::legacy().stock_denial_lead_time);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert!(live.stock_denial_lead_time);
    let mut withheld = AdvancedAi::new();
    withheld.enable_live_bridge();
    withheld.disable_stock_denial_lead_time();
    assert!(
        !withheld.urgent_victory_threat(&game, 1),
        "the withhold arm returns the lane to the general 90 alarm"
    );
}

#[test]
fn settlement_atlas_parallel_misses_match_uncached_values_and_invalidate_on_map_write() {
    let mut game = Game::new_full(1, 60, 40, 8_123, 120, 0, false);
    let positions = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| !game.rules.is_water(tile) && game.rules.is_passable(tile))
        })
        .take(48)
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 48, "the fixture needs a wide score batch");

    let visible = game.player_vision_frame(0);
    let units = game
        .units
        .values()
        .filter(|unit| game.sees(&visible, unit.pos) && game.unit_visible_to(unit.id, 0))
        .map(|unit| unit.id)
        .collect();
    let mut ai = AdvancedAi::new();
    ai.battlefront_frame = Some(BattlefrontFrame { visible, units });
    ai.work_pool = Some(Arc::new(WorkPool::new(4)));

    let expected = positions
        .iter()
        .map(|position| ai.settlement_static_value_uncached(&game, 0, *position))
        .collect::<Vec<_>>();
    let cloned_game = game.clone();
    let cloned = positions
        .iter()
        .map(|position| ai.settlement_static_value_uncached(&cloned_game, 0, *position))
        .collect::<Vec<_>>();
    assert_eq!(
        cloned, expected,
        "a worker game clone must preserve static site values"
    );
    let cloned_ai = ai.clone();
    let cloned_controller = positions
        .iter()
        .map(|position| cloned_ai.settlement_static_value_uncached(&game, 0, *position))
        .collect::<Vec<_>>();
    assert_eq!(
        cloned_controller, expected,
        "a worker controller clone must preserve static site values"
    );
    ai.settlement_atlas_values(&game, 0, &positions);
    let actual = positions
        .iter()
        .map(|position| ai.settlement_atlas_value(&game, 0, *position))
        .collect::<Vec<_>>();
    assert_eq!(
        actual, expected,
        "parallel atlas scoring must be bit-identical"
    );
    assert_eq!(ai.settlement_atlas.borrow().values.len(), positions.len());

    let changed = positions[0];
    game.map.tiles.get_mut(&changed).unwrap().terrain = Name::new("desert");
    let uncached = ai.settlement_static_value_uncached(&game, 0, changed);
    let refreshed = ai.settlement_atlas_value(&game, 0, changed);
    assert_eq!(
        refreshed, uncached,
        "a map epoch change must refresh the atlas"
    );
}

#[test]
fn settlement_forecast_values_the_actual_first_four_jobs_across_both_rings() {
    let (mut game, center, first, second) = settlement_forecast_fixture(8_116);
    shape_forecast_tile(&mut game, first[0], "grassland", false, Some("rice"));
    shape_forecast_tile(&mut game, first[1], "plains", true, None);
    shape_forecast_tile(&mut game, second[0], "grassland", false, Some("horses"));
    shape_forecast_tile(&mut game, second[1], "grassland", true, None);
    assert!(game.map.set_river_edge(center, first[5], true));

    let forecast = AdvancedAi::new().settlement_growth_forecast(&game, 0, center);
    assert_eq!(forecast.reached_population, 4);
    assert_eq!(forecast.worked.len(), 4);
    assert_eq!(forecast.worked[0].pos, first[0]);
    assert_eq!(forecast.worked[0].ring, 1);
    assert!(forecast.worked.iter().any(|tile| tile.ring == 2));
    assert!(
        forecast.turns_to_four.is_some_and(|turns| turns < 30.0),
        "the four useful jobs should grow the city promptly: {forecast:?}"
    );
}

#[test]
fn settlement_forecast_prefers_four_sustainable_jobs_to_one_spike() {
    let (mut spike, center, first, second) = settlement_forecast_fixture(8_117);
    let mut broad = spike.clone();
    shape_forecast_tile(&mut spike, first[0], "grassland", false, Some("rice"));
    for position in [first[0], first[1], second[0], second[1]] {
        shape_forecast_tile(&mut broad, position, "grassland", true, None);
    }
    assert!(spike.map.set_river_edge(center, first[5], true));
    assert!(broad.map.set_river_edge(center, first[5], true));

    let ai = AdvancedAi::new();
    let spike = ai.settlement_growth_forecast(&spike, 0, center);
    let broad = ai.settlement_growth_forecast(&broad, 0, center);
    assert!(
        spike.reached_population < 4,
        "one tile should stall: {spike:?}"
    );
    assert_eq!(
        broad.reached_population, 4,
        "four jobs should mature: {broad:?}"
    );
    assert!(
        broad.score > spike.score,
        "a useful four-pop city should outrank a one-tile mirage: broad={broad:?}, spike={spike:?}"
    );
}

#[test]
fn settlement_forecast_delays_second_ring_jobs_and_models_water_housing() {
    let (mut game, center, first, second) = settlement_forecast_fixture(8_118);
    shape_forecast_tile(&mut game, first[0], "grassland", false, None);
    shape_forecast_tile(&mut game, second[0], "grassland", false, Some("citrus"));
    let forecast = AdvancedAi::new().settlement_growth_forecast(&game, 0, center);
    assert_eq!(
        forecast.worked.first().map(|tile| tile.pos),
        Some(first[0]),
        "the rich second-ring tile is not owned for the first citizen"
    );
    assert_eq!(AdvancedAi::settlement_base_housing(&game, 0, center), 2.0);

    let mut coast = game.clone();
    shape_forecast_tile(&mut coast, first[5], "coast", false, None);
    assert_eq!(AdvancedAi::settlement_base_housing(&coast, 0, center), 3.0);

    let mut fresh = game;
    assert!(fresh.map.set_river_edge(center, first[5], true));
    assert_eq!(AdvancedAi::settlement_base_housing(&fresh, 0, center), 5.0);
}

/// The live seat walks two Settlers at once from two cities on, while the
/// empire is at least two seats short; every default constructor and the
/// frozen anchor keep the one-at-a-time gate in both settler routes.
#[test]
fn parallel_settlers_open_a_second_pipeline_slot() {
    // As above: the board is read only by `expansion_schedule`, which is off.
    let game = Game::new(2, 24, 16, 71, 250, 0);
    let mut ai = AdvancedAi::new();
    assert!(
        !ai.parallel_settlers && !ai.base.parallel_settlers,
        "off unless the bridge asks"
    );
    assert_eq!(ai.settler_in_flight_allowed(&game, 8, 3, 1), 1);
    assert!(!AdvancedAi::legacy().parallel_settlers);
    assert!(!AdvancedAi::legacy().base.parallel_settlers);

    ai.enable_parallel_settlers();
    assert!(
        ai.parallel_settlers && ai.base.parallel_settlers,
        "both routes see the flag"
    );
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 8, 3, 1),
        2,
        "three cities, one walker, five short"
    );
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 8, 2, 0),
        2,
        "two cities open the second slot"
    );
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 8, 1, 1),
        1,
        "one city keeps a single walker"
    );
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 3, 2, 0),
        1,
        "one seat short: one walker covers it"
    );
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 4, 2, 1),
        1,
        "the walker already covers the seat but one"
    );
    assert_eq!(ai.settler_in_flight_allowed(&game, 5, 2, 1), 2);
}

#[test]
fn settler_factory_coordination_keeps_the_fast_pair_and_gives_them_distinct_sites() {
    let mut game = Game::new_full(1, 30, 18, crate::rng::fixture_seed("AUTO_23", 8_124), 160, 0, false);
    let opening_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("the opening has a Settler");
    game.apply(
        0,
        &Action::FoundCity {
            unit: opening_settler,
        },
    )
    .unwrap();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.wonder = None;
        tile.hills = false;
    }
    let capital = game.player_city_ids(0)[0];
    let capital_pos = game.cities[&capital].pos;
    let second = found_nearby_test_city(&mut game, 0, capital_pos);
    let second_pos = game.cities[&second].pos;
    let third = found_nearby_test_city(&mut game, 0, second_pos);
    for city in [capital, second, third] {
        game.cities.get_mut(&city).unwrap().pop = 3;
    }
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    let shape_turns = |game: &mut Game, city: u32, turns: f64| {
        let cost = game.item_cost_for_city(0, city, &settler);
        let production = game.city_yields(city).production.max(1.0);
        game.cities.get_mut(&city).unwrap().production = (cost - turns * production).max(0.0);
    };
    shape_turns(&mut game, capital, 3.0);
    shape_turns(&mut game, second, 7.0);
    game.cities.get_mut(&third).unwrap().production = 0.0;

    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 8,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.land_grab = true;
    ai.enable_settler_factory_coordination();
    let counts = ai.counts(&game, 0);
    assert_eq!(
        ai.settler_in_flight_allowed(&game, plan.desired_cities, 3, counts.settlers),
        3,
        "coordination must preserve the widened early pipeline"
    );
    assert!(ai.settler_factory_is_competitive(&game, 0, capital, &plan, &counts));
    assert!(ai.settler_factory_is_competitive(&game, 0, second, &plan, &counts));
    assert!(
        !ai.settler_factory_is_competitive(&game, 0, third, &plan, &counts),
        "the fresh slow satellite must not consume a slot beside 3/7-turn factories"
    );

    let mut untreated = AdvancedAi::new();
    untreated.land_grab = true;
    assert!(
        untreated.production_value(&game, 0, third, &settler, &plan, &counts) > 0.0,
        "the ordinary controller still prices the slow factory"
    );
    ai.advanced_production(&mut game, 0, &plan, false);
    assert_eq!(game.cities[&capital].queue.first(), Some(&settler));
    assert_eq!(game.cities[&second].queue.first(), Some(&settler));
    assert_ne!(game.cities[&third].queue.first(), Some(&settler));

    let first_site = ai
        .coordinated_settler_site(&game, 0, capital)
        .expect("the first factory has a practical claim")
        .0;
    let second_site = ai
        .coordinated_settler_site(&game, 0, second)
        .expect("the second factory has a practical claim")
        .0;
    assert!(
        game.wdist(first_site, second_site) >= SETTLER_FACTORY_SITE_SPACING,
        "queued factories must not plan mutually invalidating sites: {first_site:?} / {second_site:?}"
    );

    assert!(!AdvancedAi::new().settler_factory_coordination);
    assert!(!AdvancedAi::legacy().settler_factory_coordination);
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("settler-factory-coordination")
            .is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let gene = GENES
        .iter()
        .find(|gene| gene.tag == "settler-factory-coordination")
        .expect("the coordinator is published for gene_screen");
    assert!(gene.opt_in() && gene.screenable() && !gene.live());
}

#[test]
fn settler_factory_coordination_requires_a_route_but_accepts_shipbuilding() {
    let mut game = Game::new_full(1, 30, 18, 8_125, 160, 0, false);
    let opening_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("the opening has a Settler");
    game.apply(
        0,
        &Action::FoundCity {
            unit: opening_settler,
        },
    )
    .unwrap();
    let city = game.player_city_ids(0)[0];
    let origin = game.cities[&city].pos;
    let remote = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.wdist(origin, *position) == 7 && game.wdisk(*position, 2).len() == 19)
        .expect("the map has room for an isolated island");
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("coast");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.wonder = None;
        tile.hills = false;
    }
    game.map.tiles.get_mut(&origin).unwrap().terrain = crate::name!("grassland");
    for position in game.wdisk(remote, 2) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = true;
    }

    let ai = AdvancedAi::new();
    assert!(
        ai.best_settle_site(&game, 0, origin, 11).is_some(),
        "the ordinary straight-line scorer sees the attractive island"
    );
    assert!(
        ai.coordinated_site_from(&game, 0, origin, &[]).is_none(),
        "a pre-Shipbuilding Settler cannot cross the coastal moat"
    );
    game.players[0].techs.insert(crate::name!("shipbuilding"));
    assert!(game.tree_effect(0, "land_unit_embark") > 0.0);
    let reachable = ai
        .coordinated_site_from(&game, 0, origin, &[])
        .expect("Shipbuilding makes the same island practical")
        .0;
    assert!(game.wdisk(remote, 2).contains(&reachable));
}

#[test]
fn a_repeatedly_blocked_settler_releases_only_the_next_missing_slot() {
    // The board is read only by `expansion_schedule`, which is off here.
    let game = Game::new(2, 24, 16, 71, 250, 0);
    let mut ai = AdvancedAi::new();
    ai.settler_blocked_turns.insert(41, 2);
    assert_eq!(ai.settler_in_flight_allowed(&game, 3, 1, 1), 1);
    ai.settler_blocked_turns.insert(41, 3);
    assert_eq!(ai.settler_in_flight_allowed(&game, 3, 1, 1), 2);
    assert_eq!(
        ai.settler_in_flight_allowed(&game, 2, 1, 1),
        1,
        "a Settler already covers the final missing city"
    );

    let mut legacy = AdvancedAi::legacy();
    legacy.settler_blocked_turns.insert(41, 99);
    assert_eq!(legacy.settler_in_flight_allowed(&game, 4, 1, 1), 1);
}

#[test]
fn settlement_safety_is_shared_by_top_advanced_constructors() {
    assert!(AdvancedAi::new().settlement_safety);
    assert!(AdvancedAi::targeting(VictoryTarget::Culture).settlement_safety);
    assert!(AdvancedAi::with_weights(Weights::default()).settlement_safety);
    assert!(!AdvancedAi::legacy().settlement_safety);
}

#[test]
fn religious_denial_warns_early_but_never_ignores_match_point() {
    let mut game = Game::new_full(4, 30, 18, 7_216, 300, 0, false);
    for pid in 0..4 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    game.players[1].religion = Some("Rival Faith".to_string());
    for owner in [1, 2] {
        let city = game.player_city_ids(owner)[0];
        game.cities
            .get_mut(&city)
            .unwrap()
            .pressure
            .insert("Rival Faith".to_string(), 1_000.0);
    }

    let ai = AdvancedAi::new();
    assert_eq!(ai.rival_victory_pressure(&game, 1).progress, 50);
    assert_eq!(
        ai.victory_denial(&game, 0),
        Some((1, GrandStrategy::Conquest)),
        "two remaining holdouts leave time to build and route a defense"
    );

    game.players[0].dvp = 13;
    assert_eq!(ai.victory_focus(&game, 0).progress, 65);
    assert_eq!(
        ai.victory_denial(&game, 0),
        None,
        "an early warning need not derail a meaningfully closer race"
    );

    let last_converted = game.player_city_ids(3)[0];
    game.cities
        .get_mut(&last_converted)
        .unwrap()
        .pressure
        .insert("Rival Faith".to_string(), 1_000.0);
    assert_eq!(ai.rival_victory_pressure(&game, 1).progress, 75);
    assert_eq!(
        ai.victory_denial(&game, 0),
        Some((1, GrandStrategy::Conquest)),
        "a one-conversion match point must interrupt even a close own race"
    );
}

#[test]
fn victory_focus_tracks_religious_diplomatic_and_culture_races() {
    let ai = AdvancedAi::new();

    let mut religion = Game::new(2, 24, 16, 74, 80, 0);
    religion.players[0].religion = Some("Test Faith".to_string());
    assert_eq!(
        ai.victory_focus(&religion, 0).strategy,
        GrandStrategy::Religion
    );
    assert_eq!(
        AdvancedAi::legacy().victory_focus(&religion, 0).strategy,
        GrandStrategy::Science
    );

    let mut diplomacy = Game::new(2, 24, 16, 75, 80, 0);
    diplomacy.players[0].dvp = 14;
    assert_eq!(
        ai.victory_focus(&diplomacy, 0).strategy,
        GrandStrategy::Diplomacy
    );

    let mut culture = Game::new(2, 24, 16, 76, 80, 0);
    culture.players[0].tourism_lifetime = 100_000.0;
    culture.players[1].culture_lifetime = 100.0;
    assert_eq!(
        ai.victory_focus(&culture, 0).strategy,
        GrandStrategy::Culture
    );
}

#[test]
fn bulk_rival_culture_pressure_matches_individual_victory_scans() {
    let mut game = Game::new(4, 30, 18, 7_603, 300, 0);
    for source in 0..4 {
        game.players[source].culture_lifetime = 500.0 + source as f64 * 350.0;
        for target in 0..4 {
            if source != target {
                game.players[source]
                    .tourism_pressure
                    .insert(target, (source * 700 + target * 190) as f64);
            }
        }
    }

    let ai = AdvancedAi::new();
    let bulk = ai.rival_culture_pressures(&game);
    for pid in 0..4 {
        let batched = ai.rival_victory_pressure_with_culture(&game, pid, bulk.get(&pid).copied());
        let individual = ai.rival_victory_pressure(&game, pid);
        assert_eq!(batched.strategy, individual.strategy, "player {pid}");
        assert_eq!(batched.progress, individual.progress, "player {pid}");
        for rival in 0..4 {
            if rival == pid {
                continue;
            }
            assert_eq!(
                ai.rival_value_with_culture(&game, pid, rival, bulk.get(&rival).copied()),
                ai.rival_value(&game, pid, rival),
                "rival score {pid} -> {rival}"
            );
            assert_eq!(
                    ai.campaign_target_value_with_culture(
                        &game,
                        pid,
                        rival,
                        bulk.get(&rival).copied(),
                    ),
                    ai.campaign_target_value(&game, pid, rival),
                    "campaign score {pid} -> {rival}"
                );
        }
    }
    assert_eq!(
        ai.victory_denial_with_culture_pressures(&game, 0, &bulk),
        ai.victory_denial(&game, 0)
    );
}

#[test]
fn disabled_victories_do_not_drive_strategy_or_rival_denial() {
    let mut game = Game::new(2, 24, 16, 7_602, 300, 0);
    game.victory_conditions.religious = false;
    game.victory_conditions.diplomatic = false;
    game.victory_conditions.score = false;
    game.players[0].dvp = 25;
    game.players[0].religion = Some("Test Faith".to_string());
    game.players[0].prophet_pending = true;
    game.players[1].dvp = 25;

    let ai = AdvancedAi::new();
    assert_eq!(
        ai.victory_focus(&game, 0).strategy,
        GrandStrategy::Science,
        "disabled diplomatic and religious progress must not outrank an enabled path"
    );
    assert!(!ai.religious_opening_viable(&game, 0));
    assert!(!ai.religious_offensive_posture(&game, 0, GrandStrategy::Science));

    let mut opening = Game::new(2, 24, 16, 7_604, 300, 0);
    opening.victory_conditions.religious = false;
    opening.players[0].prophet_pending = true;
    assert!(
        ai.religious_opening_viable(&opening, 0),
        "religion remains an economic subsystem when its victory is disabled"
    );
    let mut opening_ai = AdvancedAi::new();
    opening_ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Religion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: opening.turn,
        rush: false,
    });
    assert!(!opening_ai.plan_stale(&opening, 0));
    assert_ne!(
        ai.rival_victory_pressure(&game, 1).strategy,
        GrandStrategy::Diplomacy,
        "a disabled victory is not an imminent rival threat"
    );
    assert_eq!(ai.victory_denial(&game, 0), None);

    let mut targeted = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    assert_eq!(
        targeted.victory_focus(&game, 0).strategy,
        GrandStrategy::Science,
        "an explicit target must yield when the game disables that victory"
    );
    targeted.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    });
    assert!(targeted.plan_stale(&game, 0));

    let mut turn = Game::new(2, 24, 16, 7_603, 300, 0);
    turn.victory_conditions.religious = false;
    turn.victory_conditions.diplomatic = false;
    turn.victory_conditions.score = false;
    targeted.take_turn(&mut turn, 0);
    assert!(
        targeted.base.pursue_religion,
        "a disabled explicit target falls back to adaptive ancillary systems"
    );
    assert_ne!(
        targeted.current_plan().unwrap().strategy,
        GrandStrategy::Diplomacy,
        "the entire turn must fall back from a disabled explicit target"
    );
}

#[test]
fn religious_focus_counts_foreign_conversions_not_the_founder() {
    let mut game = Game::new_full(4, 30, 18, 7_600, 300, 0, false);
    for pid in 0..4 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    game.players[0].religion = Some("Test Faith".to_string());
    let convert = |game: &mut Game, owner: usize| {
        let city = game.player_city_ids(owner)[0];
        game.cities
            .get_mut(&city)
            .unwrap()
            .pressure
            .insert("Test Faith".to_string(), 1_000.0);
    };
    convert(&mut game, 0);

    let ai = AdvancedAi::new();
    let founded = ai.victory_focus(&game, 0);
    assert_eq!(founded.strategy, GrandStrategy::Religion);
    assert_eq!(
        founded.progress, 40,
        "the founder's own majority is not a foreign victory gain"
    );

    convert(&mut game, 1);
    assert_eq!(ai.victory_focus(&game, 0).progress, 60);
    convert(&mut game, 2);
    assert_eq!(ai.victory_focus(&game, 0).progress, 80);
    convert(&mut game, 3);
    assert_eq!(ai.victory_focus(&game, 0).progress, 100);
}

#[test]
fn victory_focus_tracks_technology_before_the_first_space_project() {
    let ai = AdvancedAi::new();
    let mut game = Game::new(2, 24, 16, 761, 300, 0);
    game.turn = 111;
    let opening = ai.victory_focus(&game, 0);
    assert_eq!(opening.strategy, GrandStrategy::Science);
    assert_eq!(opening.progress, 25);

    let researched: Vec<Name> = game
        .rules
        .techs
        .keys()
        .take(game.rules.techs.len() * 2 / 3)
        .cloned()
        .collect();
    game.players[0].techs.extend(researched);
    let developed = ai.victory_focus(&game, 0);
    assert_eq!(developed.strategy, GrandStrategy::Science);
    assert!(developed.progress >= 44, "progress={}", developed.progress);

    game.players[0].civ = "China".to_string();
    game.players[0].techs.clear();
    assert_eq!(ai.victory_focus(&game, 0).progress, 45);
}

#[test]
fn assigned_culture_starts_buildup_before_the_shared_halfway_boundary() {
    let mut game = Game::new(2, 24, 16, 76_007, 250, 0);
    game.game_speed = crate::setup::GameSpeed::Online;
    let culture = AdvancedAi::targeting(VictoryTarget::Culture);
    let science = AdvancedAi::targeting(VictoryTarget::Science);
    game.turn = 83;
    assert!(!culture.phase_specialization_active(&game));
    game.turn = 84;
    assert!(
        culture.phase_specialization_active(&game),
        "Culture needs time to accumulate tourism"
    );
    assert!(!science.phase_specialization_active(&game));
    assert!(!AdvancedAi::new().phase_specialization_active(&game));
    game.max_turns = 650;
    assert!(
        culture.phase_specialization_active(&game),
        "an extended verification cap must not delay buildup"
    );
    game.turn = 125;
    assert!(science.phase_specialization_active(&game));
}

#[test]
fn religious_production_builds_prophet_infrastructure_then_runs_prayers() {
    let mut game = Game::new_full(1, 20, 14, 76_103, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    game.players[0].techs.insert(crate::name!("astrology"));
    install_ai_test_district(&mut game, city, "holy_site");
    game.cities.get_mut(&city).unwrap().queue.clear();

    let ai = AdvancedAi::new();
    ai.religious_production(&mut game, 0);
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::Building { building }) if building == "shrine"
    ));

    game.cities.get_mut(&city).unwrap().queue.clear();
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("shrine"));
    ai.religious_production(&mut game, 0);
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::Project { project }) if project == "holy_site_prayers"
    ));
}

#[test]
fn rival_pressure_uses_living_civilizations_for_active_victory_races() {
    let mut game = Game::new_full(3, 24, 16, 760, 300, 0, false);
    game.players[1].tourism_lifetime = 300_000.0;
    game.players[0].culture_lifetime = 100.0;
    game.players[2].culture_lifetime = 1_000_000.0;
    game.players[2].alive = false;

    let pressure = AdvancedAi::new().rival_victory_pressure(&game, 1);
    assert_eq!(pressure.strategy, GrandStrategy::Culture);
    assert_eq!(pressure.progress, 100);
}

#[test]
fn religious_match_point_spends_the_reserve_only_in_own_faith_cities() {
    let mut game = Game::new_full(4, 42, 24, 7_622, 300, 0, false);
    for pid in 0..4 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    let converted_capital = game.player_city_ids(0)[0];
    let faithful_city = found_test_city(&mut game, 0);
    install_test_holy_site(&mut game, converted_capital);
    install_test_holy_site(&mut game, faithful_city);
    game.current = 0;
    game.turn = 150;
    game.players[0].religion = Some("Home Faith".to_string());
    game.players[0].holy_city = Some(converted_capital);
    game.players[0].techs.insert(crate::name!("astrology"));
    game.players[0].faith = 200.0;
    game.players[3].religion = Some("Runaway Faith".to_string());
    game.cities
        .get_mut(&converted_capital)
        .unwrap()
        .pressure
        .insert("Runaway Faith".to_string(), 1_000.0);
    game.cities
        .get_mut(&faithful_city)
        .unwrap()
        .pressure
        .insert("Home Faith".to_string(), 1_000.0);
    for owner in 1..4 {
        let city = game.player_city_ids(owner)[0];
        game.cities
            .get_mut(&city)
            .unwrap()
            .pressure
            .insert("Runaway Faith".to_string(), 1_000.0);
    }

    let mut ai = AdvancedAi::new();
    let emergency = ai
        .victory_denial(&game, 0)
        .is_some_and(|(_, counter)| counter == GrandStrategy::Religion);
    assert!(emergency);
    ai.religious_spending(&mut game, 0, emergency);

    let missionary = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "missionary")
        .expect("match-point defense should spend the ordinary Faith reserve");
    assert_eq!(missionary.religion.as_deref(), Some("Home Faith"));
    assert_eq!(
        missionary.pos,
        *game.cities[&faithful_city]
            .districts
            .get(crate::name!("holy_site"))
            .unwrap()
    );
    assert!(game.players[0].faith < 1.0);
}

/// ★★★ Four cities claimed the empire-wide first-pad rung at once because
/// it counts finished districts only. Live run civvis-20260817T022159Z:
/// Aquileia t144, Ostia t146, Arretium t146, Brundisium t157 — 119
/// city-turns, 11% of everything that empire built, for one usable pad.
/// See `one_launch_pad`. Two cities with Rocketry and no pad: the treated
/// seat pays 3,000 only in the better producer; the shared launch-site cap
/// vetoes the other until the next science milestone. Once the better
/// producer has the pad in its queue, neither city can start another one.
#[test]
fn the_empire_reserves_one_launch_pad_in_the_city_that_would_run_the_race() {
    let mut game = Game::new(2, 32, 24, 5_414, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let first = game.player_city_ids(0)[0];
    let second = found_test_city(&mut game, 0);
    game.players[0].techs = game.rules.techs.keys().cloned().collect();
    game.turn = 140;
    // Move to the specialization half while keeping every research
    // prerequisite known, so the test isolates the one-pad rung rather than
    // research payback.
    game.max_turns = 100_000;
    // This test exercises the late one-pad specialization, after the shared
    // expansion-and-defense half has handed the plan to the victory lane.
    game.turn = game.max_turns / 2;
    // The measured shape: one city out-produces the other, and the race
    // would be run there.
    install_ai_test_district(&mut game, first, "industrial_zone");
    game.cities.get_mut(&first).unwrap().pop = 14;
    game.cities.get_mut(&second).unwrap().pop = 4;

    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    let pad = Item::District {
        district: crate::name!("spaceport"),
        pos: game.cities[&first].pos,
    };
    let _ = second;

    let mut live = AdvancedAi::targeting(VictoryTarget::Science);
    live.enable_live_bridge_universe();
    assert!(live.one_launch_pad, "the live seat carries the treatment");
    live.refresh_research_weight(&game);
    let counts = live.counts(&game, 0);

    let racer = live
        .launch_pad_site(&game, 0)
        .expect("no pad stands or is queued, so one city holds the rung");
    let passed = *game
        .player_city_ids(0)
        .iter()
        .find(|cid| **cid != racer)
        .expect("the empire has a second city");
    assert!(
        game.city_yields(racer).production >= game.city_yields(passed).production,
        "the rung goes to the better producer"
    );

    let held = live.production_value(&game, 0, racer, &pad, &plan, &counts);
    let passed_over = live.production_value(&game, 0, passed, &pad, &plan, &counts);
    assert!(
        held > 0.0,
        "the better producer retains the first launch-site slot: {held}"
    );
    assert_eq!(
        passed_over, -10_000.0,
        "the slower city cannot begin an unneeded second Spaceport: {passed_over}"
    );

    // A pad already on its way ends the rung for everyone — including the
    // city building it, which does not need to be told twice.
    game.cities.get_mut(&racer).unwrap().queue.push(pad.clone());
    assert_eq!(
        live.launch_pad_site(&game, 0),
        None,
        "a queued pad is the pad the empire is getting"
    );
    let after = live.production_value(&game, 0, racer, &pad, &plan, &counts);
    assert!(
        after < held,
        "no second claim while the first is building: {after} vs {held}"
    );

    // And a pad that stands ends it too.
    game.cities.get_mut(&racer).unwrap().queue.clear();
    install_ai_test_district(&mut game, racer, "spaceport");
    assert_eq!(
        live.launch_pad_site(&game, 0),
        None,
        "a standing pad is enough"
    );

    // Defaults: off for the stock and frozen controllers.
    assert!(!AdvancedAi::new().one_launch_pad);
    assert!(!AdvancedAi::legacy().one_launch_pad);
}

/// See `spaceport_surplus_veto`. With one pad standing and no launch yet,
/// the race stage can use exactly one Spaceport — the treated seat stops
/// paying the flat per-pad bonus for a second, and pays it again the
/// moment the Earth Satellite raises the stage's pad count to two.
#[test]
fn a_pad_beyond_the_race_stage_earns_no_science_bonus() {
    let mut game = Game::new(2, 32, 24, 5_414, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let first = game.player_city_ids(0)[0];
    let second = found_test_city(&mut game, 0);
    // This fixture isolates the late pad-count rule, not the independent
    // research-payback horizon. With the halfway clock below, make the race's
    // project technologies already known so that guard remains out of scope.
    game.players[0].techs = game.rules.techs.keys().cloned().collect();
    game.max_turns = 100_000;
    game.turn = game.max_turns / 2;
    install_ai_test_district(&mut game, first, "spaceport");

    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    let pad = Item::District {
        district: crate::name!("spaceport"),
        pos: game.cities[&second].pos,
    };

    // An unassigned Science plan leaves the explicit-race reservation alone;
    // this opt-in still prices its own adaptive-plan experiment there.
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    live.enable_spaceport_surplus_veto();
    live.refresh_research_weight(&game);
    let mut withheld = AdvancedAi::new();
    withheld.enable_live_bridge_universe();
    withheld.disable_spaceport_surplus_veto();
    withheld.refresh_research_weight(&game);
    let counts = live.counts(&game, 0);

    let vetoed = live.production_value(&game, 0, second, &pad, &plan, &counts);
    let unrestrained = withheld.production_value(&game, 0, second, &pad, &plan, &counts);
    assert!(
        unrestrained > vetoed,
        "one pad serves the whole pre-launch chain, so the second earns \
         no strategic bonus: {unrestrained} withheld vs {vetoed} treated"
    );

    // The Satellite is up: the stage can now use a second pad to prepare
    // the Mars Base while the first launches the Moon Landing, and the
    // veto stands aside.
    game.players[0]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    let staged = live.production_value(&game, 0, second, &pad, &plan, &counts);
    assert_eq!(
        staged,
        withheld.production_value(&game, 0, second, &pad, &plan, &counts),
        "a pad the race stage can use is priced exactly as before"
    );

    // Defaults: off for the stock and frozen controllers.
    assert!(!AdvancedAi::new().spaceport_surplus_veto);
    assert!(!AdvancedAi::legacy().spaceport_surplus_veto);
}

#[test]
fn production_search_uses_incremental_remaining_cost_for_paused_builds() {
    let mut game = Game::new_full(1, 20, 14, 71_002, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.players[0].civ = "Egypt".to_string();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let ai = AdvancedAi::new();
    let counts = ai.counts(&game, 0);
    let fresh = ai.production_value(&game, 0, city, &monument, &plan, &counts);

    game.apply(
        0,
        &Action::Produce {
            city,
            item: monument.clone(),
        },
    )
    .unwrap();
    game.cities.get_mut(&city).unwrap().production = 20.0;
    game.apply(
        0,
        &Action::Produce {
            city,
            item: builder,
        },
    )
    .unwrap();
    let resumed = ai.production_value(&game, 0, city, &monument, &plan, &counts);

    assert_eq!(
        game.item_remaining_cost_for_city(0, city, &monument),
        game.item_cost_for_city(0, city, &monument) - 20.0
    );
    assert!(
        resumed > fresh,
        "incremental evaluation should prefer finishing invested infrastructure"
    );
}

#[test]
fn adaptive_conquest_turn_uses_the_live_plan_for_city_production() {
    let mut game = Game::new_full(1, 20, 14, 71_006, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for unit in game.player_unit_ids(0) {
        if game.rules.units[game.units[&unit].kind].class == "military" {
            game.remove_unit(unit);
        }
    }
    let city = game.player_city_ids(0)[0];
    game.cities.get_mut(&city).unwrap().queue.clear();
    install_ai_test_district(&mut game, city, "campus");
    game.players[0].techs.insert(crate::name!("writing"));
    game.apply(
        0,
        &Action::Produce {
            city,
            item: Item::Project {
                project: crate::name!("campus_research_grants"),
            },
        },
    )
    .unwrap();
    let mut ai = AdvancedAi::new();
    ai.base.book_pos = 4;
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    });

    ai.take_turn(&mut game, 0);

    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::Unit { unit }) if game.rules.units[unit].class == "military"
    ));
}

#[test]
fn peacetime_deterrence_redirects_an_adaptive_science_grant_into_a_defender() {
    // The live run `civvis-20260815T033005Z` reached turn 174 at 85 power
    // against the strongest met rival's 540, then repeatedly queued Campus
    // Research Grants through turn 251. It finished with two Warriors at 40
    // against India's 962. The live bridge was already raising its
    // deterrence target, but adaptive Science bypassed the production
    // governor that could spend it.
    let (mut game, city, home) = empire_with_a_capital(71_108);
    game.at_war.clear();
    game.record_contact(0, 1);
    let idle_city = found_nearby_test_city(&mut game, 0, home);
    for _ in 0..2 {
        game.spawn_test_unit("warrior", 1, anchor_at(&game, home, 15));
    }
    install_ai_test_district(&mut game, city, "campus");
    game.players[0].techs.insert(crate::name!("writing"));
    let grant = Item::Project {
        project: crate::name!("campus_research_grants"),
    };
    assert!(game.can_produce(0, city, &grant));
    game.apply(
        0,
        &Action::Produce {
            city,
            item: grant.clone(),
        },
    )
    .expect("queue Campus Research Grants");
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };

    // The frozen anchor leaves the science project alone: it has no live
    // peacetime deterrence treatment to turn a rival's observed army into
    // a production obligation.
    let anchor = AdvancedAi::legacy();
    anchor.redirect_repeatable_projects_for_force_gap(&mut game, 0, &plan);
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::Project { project }) if *project == crate::name!("campus_research_grants")
    ));

    let mut live = AdvancedAi::new();
    live.enable_peacetime_deterrence();
    assert_eq!(
        live.enemy_weighted_army_target(&game, 0, 2),
        3,
        "the met rival raises the two-city peacetime floor before a declaration"
    );
    live.redirect_repeatable_projects_for_force_gap(&mut game, 0, &plan);
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::Unit { unit }) if game.rules.units[unit].class == "military"
    ));
    assert!(matches!(
        game.cities[&idle_city].queue.first(),
        Some(Item::Unit { unit }) if game.rules.units[unit].class == "military"
    ));
}

#[test]
fn peaceful_city_plan_reclaims_a_research_grant_for_a_settler() {
    // In live run `civvis-20260815T033005Z`, Rome had four cities at
    // turn 121 when its plan rose to five. Its queues stayed on Campus
    // Research Grants until it lost a city at turn 195, then still never
    // started another Settler despite targets of six and seven. The
    // baseline governor had the widened target, but only sees empty
    // queues; this pins the handoff that reaches it.
    let (mut game, city, home) = empire_with_a_capital(71_109);
    game.at_war.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
    }
    game.cities.get_mut(&city).unwrap().pop = 3;
    install_ai_test_district(&mut game, city, "campus");
    game.players[0].techs.insert(crate::name!("writing"));
    let grant = Item::Project {
        project: crate::name!("campus_research_grants"),
    };
    assert!(game.can_produce(0, city, &grant));
    game.apply(
        0,
        &Action::Produce {
            city,
            item: grant.clone(),
        },
    )
    .expect("queue Campus Research Grants");
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };

    // The frozen anchor retains its existing project: this is a live
    // city-plan repair, not a new behavior for historical evaluations.
    let anchor = AdvancedAi::legacy();
    assert!(!anchor.plan_city_target);
    anchor.redirect_repeatable_projects_for_settlement_gap(&mut game, 0, &plan);
    assert_eq!(game.cities[&city].queue.first(), Some(&grant));

    let live = AdvancedAi::new();
    assert!(live.plan_city_target);

    // The handoff keeps the one-settler pipeline intact. A normal
    // in-flight Settler consumes the next planned seat, so its project is
    // not displaced merely because the target is still farther away.
    let mut one_in_flight = game.clone();
    one_in_flight.spawn_test_unit("settler", 0, home);
    let later_plan = StrategicPlan {
        desired_cities: 3,
        ..plan.clone()
    };
    live.redirect_repeatable_projects_for_settlement_gap(&mut one_in_flight, 0, &later_plan);
    assert_eq!(one_in_flight.cities[&city].queue.first(), Some(&grant));

    // A major war remains the force-production path's responsibility.
    let mut wartime = game.clone();
    wartime.at_war.insert((0, 1));
    live.redirect_repeatable_projects_for_settlement_gap(&mut wartime, 0, &plan);
    assert_eq!(wartime.cities[&city].queue.first(), Some(&grant));

    live.redirect_repeatable_projects_for_settlement_gap(&mut game, 0, &plan);
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::Unit { unit }) if unit == "settler"
    ));
}

/// `AdvancedAi::new` promotes the baseline Amenity repair flag, but its
/// strategic production path used to bypass `BasicAi::pick_item` entirely.
/// A severe city could therefore have a legal Entertainment Complex while
/// the targeted governor kept choosing ordinary production. The bridge is
/// deliberately bounded to one idle city per turn and leaves the frozen
/// controller unchanged.
#[test]
fn promoted_amenity_repair_reaches_strategic_production_once_per_turn() {
    let (mut game, capital, home) = empire_with_a_capital(71_116);
    clear_barbarian_fixture(&mut game);
    game.at_war.clear();
    let second = found_nearby_test_city(&mut game, 0, home);
    game.players[0]
        .civics
        .insert(crate::name!("games_recreation"));
    for cid in [capital, second] {
        game.cities.get_mut(&cid).unwrap().pop = 6;
        let site = game.cities[&cid]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| {
                *position != game.cities[&cid].pos
                    && game.map.tiles[position].district.is_none()
                    && game.map.tiles[position].district_foundation.is_none()
                    && game.map.tiles[position].wonder.is_none()
            })
            .expect("the city has an owned district plot");
        let tile = game.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.flooded = false;
        let modeled = game.city_amenity_surplus(&game.cities[&cid]);
        std::sync::Arc::make_mut(&mut game.observed_city_amenity_adjustments)
            .insert(cid, -3 - modeled);
        assert_eq!(game.city_amenity_surplus(&game.cities[&cid]), -3);
        let legal = game.producible_items(0, cid);
        assert!(
            legal.iter().any(|item| matches!(
                item,
                Item::District { district, .. }
                    if game.district_family(*district) == "entertainment_complex"
            )),
            "the severe city has a legal Entertainment Complex; legal: {legal:?}"
        );
    }
    let mut frozen_board = game.clone();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };

    let mut live = AdvancedAi::new();
    assert!(live.base.amenity_districts);
    live.advanced_production(&mut game, 0, &plan, false);
    let repaired: Vec<u32> = [capital, second]
        .into_iter()
        .filter(|cid| {
            matches!(
                game.cities[cid].queue.first(),
                Some(Item::District { district, .. })
                    if game.district_family(*district) == "entertainment_complex"
            )
        })
        .collect();
    assert_eq!(
        repaired.len(),
        1,
        "the strategic handoff reserves one amenity district per turn: {repaired:?}"
    );
    let repaired_city = repaired[0];
    let reserved_repair = game.cities[&repaired_city]
        .queue
        .first()
        .cloned()
        .expect("the first strategic pass leaves the repair queued");
    live.advanced_production(&mut game, 0, &plan, false);
    assert_eq!(
        game.cities[&repaired_city].queue.first(),
        Some(&reserved_repair),
        "a second strategic review must not replace an amenity repair in deficit"
    );

    let mut frozen = AdvancedAi::legacy();
    frozen.advanced_production(&mut frozen_board, 0, &plan, false);
    assert!(
        [capital, second].into_iter().all(|cid| !matches!(
            frozen_board.cities[&cid].queue.first(),
            Some(Item::District { district, .. })
                if frozen_board.district_family(*district) == "entertainment_complex"
        )),
        "the frozen strategic controller keeps the historical queue path"
    );
}

/// A Zoo reaches every own city within six tiles; priced as one city's
/// +1 it never outbid a Library. See `amenity_district_path`.
/// ★★★★ The tally pays three for a civic and two for a tech, and the
/// Science lane priced culture at a quarter of a beaker: eight Campuses
/// and zero Theater Squares in nine cities (civvis-20260816T093036Z), 36–41
/// civics against rivals' 43–53 in every batch-4 game. See
/// `tally_culture`. On the live seat a point of culture is worth no less
/// than a point of science under the Science lane, the Culture lane keeps
/// its own higher culture weight, and the strategic governor's Theater
/// Square carries the Campus's Science-lane weight; stock and frozen
/// controllers are unchanged.
#[test]
fn on_the_tally_seat_a_point_of_culture_is_worth_a_point_of_science() {
    let culture_only = Yields {
        culture: 1.0,
        ..Yields::default()
    };
    let science_only = Yields {
        science: 1.0,
        ..Yields::default()
    };
    let stock = AdvancedAi::new();
    let frozen = AdvancedAi::legacy();
    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    let mut without_tally = AdvancedAi::new();
    without_tally.enable_live_bridge();
    without_tally.disable_tally_culture();
    assert!(live.tally_culture, "the live seat carries the treatment");

    assert!(
        stock.yield_value(culture_only, GrandStrategy::Science)
            < stock.yield_value(science_only, GrandStrategy::Science),
        "the stock Science lane prices culture below science"
    );
    assert_eq!(
        frozen.yield_value(culture_only, GrandStrategy::Science),
        stock.yield_value(culture_only, GrandStrategy::Science),
        "the frozen anchor prices exactly as stock"
    );
    assert_eq!(
        live.yield_value(culture_only, GrandStrategy::Science),
        live.yield_value(science_only, GrandStrategy::Science),
        "the tally seat prices a point of culture at the Science lane's beaker"
    );
    for lane in [
        GrandStrategy::Expansion,
        GrandStrategy::Conquest,
        GrandStrategy::Recovery,
        GrandStrategy::Diplomacy,
        GrandStrategy::Religion,
    ] {
        assert!(
            live.yield_value(culture_only, lane) >= live.yield_value(science_only, lane),
            "under {lane:?} culture is worth no less than science on the tally seat"
        );
        assert!(
            live.yield_value(science_only, lane) == without_tally.yield_value(science_only, lane),
            "the tally treatment itself does not alter a beaker under {lane:?}"
        );
    }
    assert_eq!(
        live.yield_value(culture_only, GrandStrategy::Culture),
        stock.yield_value(culture_only, GrandStrategy::Culture),
        "the Culture lane keeps its own, higher, culture weight"
    );

    // The strategic governor: a Theater Square is priced like a Campus
    // under the Science lane on the tally seat, and below it as stock.
    let (mut game, capital, _home) = empire_with_a_capital(71_113);
    game.players[0].techs.insert(crate::name!("writing"));
    game.players[0].civics.insert(crate::name!("drama_poetry"));
    let campus_site = game
        .district_sites(capital, crate::name!("campus"))
        .into_iter()
        .next()
        .expect("the capital has a plot for a Campus");
    // The same plot serves both, so the comparison is the district alone.
    let theater_site = game
        .district_sites(capital, crate::name!("theater_square"))
        .into_iter()
        .find(|site| *site == campus_site)
        .or_else(|| {
            game.district_sites(capital, crate::name!("theater_square"))
                .into_iter()
                .next()
        })
        .expect("the capital has a plot for a Theater Square");
    let campus = Item::District {
        district: crate::name!("campus"),
        pos: campus_site,
    };
    let theater = Item::District {
        district: crate::name!("theater_square"),
        pos: theater_site,
    };
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let counts = EmpireCounts::default();
    let stock_gap = stock.production_value(&game, 0, capital, &campus, &plan, &counts)
        - stock.production_value(&game, 0, capital, &theater, &plan, &counts);
    let live_gap = live.production_value(&game, 0, capital, &campus, &plan, &counts)
        - live.production_value(&game, 0, capital, &theater, &plan, &counts);
    assert!(
        live_gap < stock_gap,
        "the tally seat closes the Campus-over-Theater gap ({live_gap} vs {stock_gap})"
    );

    assert_eq!(
        without_tally.yield_value(culture_only, GrandStrategy::Science),
        stock.yield_value(culture_only, GrandStrategy::Science)
    );
    assert!(!stock.tally_culture);
    assert!(!frozen.tally_culture);
}

/// `diplomatic_lane_refuses_fossil_power`: the assigned Diplomatic lane never
/// prices a Coal or Oil Power Plant (or a reactor conversion to either); the
/// Nuclear plant, every other lane and a lost lane are untouched.
#[test]
fn a_diplomatic_seat_burns_no_coal_or_oil() {
    let (game, capital, _home) = empire_with_a_capital(71_116);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let counts = EmpireCounts::default();
    let building = |name: &str| Item::Building {
        building: crate::name::Name::new(name),
    };
    let mut diplomatic = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    diplomatic.refresh_research_weight(&game);
    let science = AdvancedAi::targeting(VictoryTarget::Science);

    for fossil in ["coal_power_plant", "oil_power_plant"] {
        assert!(diplomatic.diplomatic_lane_refuses_fossil_power(fossil));
        assert!(!science.diplomatic_lane_refuses_fossil_power(fossil));
        assert!(!AdvancedAi::new().diplomatic_lane_refuses_fossil_power(fossil));
        assert_eq!(
            diplomatic.production_value(&game, 0, capital, &building(fossil), &plan, &counts),
            -10_000.0,
            "{fossil} is refused on the diplomatic seat"
        );
    }
    assert!(!diplomatic.diplomatic_lane_refuses_fossil_power("nuclear_power_plant"));
    assert!(!diplomatic.diplomatic_lane_refuses_fossil_power("factory"));

    let mut lost = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    lost.lane_lost = true;
    assert!(!lost.diplomatic_lane_refuses_fossil_power("coal_power_plant"));

    // The turn hands the refusal to the baseline governor that fills a
    // delegating lane's routine queues (`BasicAi::exclude_fossil_power`).
    let mut game = game;
    let mut seat = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    seat.take_turn(&mut game, 0);
    assert!(
        seat.base.exclude_fossil_power,
        "a diplomatic seat's governor refuses them"
    );
    let mut other = AdvancedAi::targeting(VictoryTarget::Culture);
    other.take_turn(&mut game, 0);
    assert!(!other.base.exclude_fossil_power, "another lane's does not");
}

/// The debt was previously checked only on an untargeted controller, even
/// though the hard veto means it can act in targeted deployment only in the
/// Culture lane. Keep the target equal, enabling just the debt, so the second
/// pre-registered comparison has a reachable mechanism.
#[test]
fn a_culture_target_reaches_the_theater_building_debt() {
    let (mut game, capital, _home) = empire_with_a_capital(71_115);
    game.players[0].civics.insert(crate::name!("drama_poetry"));
    install_ai_test_district(&mut game, capital, "theater_square");
    let plan = StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let amphitheater = Item::Building {
        building: crate::name!("amphitheater"),
    };
    let counts = EmpireCounts::default();
    let mut treated = AdvancedAi::targeting(VictoryTarget::Culture);
    treated.enable_culture_building_debt();
    let mut withheld = AdvancedAi::targeting(VictoryTarget::Culture);
    treated.refresh_research_weight(&game);
    withheld.refresh_research_weight(&game);

    assert_eq!(treated.victory_target(), Some(VictoryTarget::Culture));
    assert!(treated.culture_building_debt);
    assert!(!withheld.culture_building_debt);
    let owed = treated.production_value(&game, 0, capital, &amphitheater, &plan, &counts);
    let unowed = withheld.production_value(&game, 0, capital, &amphitheater, &plan, &counts);
    assert!(
        owed > unowed,
        "a Culture-targeted seat reaches the debt instead of the non-Culture veto: {owed} vs {unowed}"
    );

    // An explicit Culture contract still has to open the district while the
    // war planner is temporarily in Recovery. The normal district value is
    // present in both arms; this larger gap pins the targeted lane's recovery
    // handoff rather than merely proving that Culture is valued when the plan
    // already says Culture.
    let district_city = found_test_city(&mut game, 0);
    let theater_site = game
        .district_sites(district_city, crate::name!("theater_square"))
        .into_iter()
        .next()
        .expect("the second city has a legal Theater Square site");
    let theater = Item::District {
        district: crate::name!("theater_square"),
        pos: theater_site,
    };
    let recovery = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        ..plan
    };
    let targeted_recovery =
        treated.production_value(&game, 0, district_city, &theater, &recovery, &counts);
    let untargeted_recovery =
        AdvancedAi::new().production_value(&game, 0, district_city, &theater, &recovery, &counts);
    assert!(
        targeted_recovery > untargeted_recovery + 10.0,
        "the explicit Culture target preserves a strong Theater district bid during Recovery: {targeted_recovery} vs {untargeted_recovery}"
    );
}

#[test]
fn adaptive_production_reserves_a_real_siege_support_capability() {
    let mut game = Game::new_full(2, 24, 16, 71_007, 160, 0, false);
    for pid in 0..2 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    let home = game.player_city_ids(0)[0];
    let target = game.player_city_ids(1)[0];
    game.cities.get_mut(&home).unwrap().queue.clear();
    game.players[0].techs.insert(crate::name!("flight"));
    let position = game.cities[&home].pos;
    game.spawn_test_unit("catapult", 0, position);
    game.spawn_test_unit("warrior", 0, position);
    game.spawn_test_unit("warrior", 0, position);
    game.at_war.insert((0, 1));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.base.book_pos = 4;

    ai.advanced_support_production(&mut game, 0, &plan);

    assert!(matches!(
        game.cities[&home].queue.first(),
        Some(Item::Unit { unit }) if unit == "observation_balloon"
    ));
}

/// The Builder floor puts charges into a growing empire; this companion
/// contract sends those charges to the new city whose output cannot yet carry
/// the buildings, units, or projects every victory lane needs. The two job
/// tiles are deliberately the only unreserved work, so this proves the
/// production bonus is present in the actual cross-city Builder ranking, not
/// merely in an isolated helper.
#[test]
fn named_victory_builders_raise_a_lagging_citys_production_foundation() {
    let (mut game, capital, home) = empire_with_a_capital(79_124);
    clear_barbarian_fixture(&mut game);
    game.at_war.clear();
    let weak = found_nearby_test_city(&mut game, 0, home);
    game.players[0]
        .techs
        .extend([crate::name!("mining"), crate::name!("construction")]);

    let strong_target = game.cities[&capital]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != home)
        .expect("the capital owns an improvement tile");
    let weak_home = game.cities[&weak].pos;
    let weak_target = game.cities[&weak]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != weak_home)
        .expect("the new city owns an improvement tile");

    // A mature core: its completed Mines put it over the foundation floor,
    // while the remaining Forest can legally become a two-production Lumber
    // Mill. The ordinary ranking should prefer this better local tile.
    for position in game.cities[&capital].owned_tiles.clone() {
        if position == home {
            continue;
        }
        let tile = game.map.tiles.get_mut(&position).expect("capital tile");
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = true;
        tile.resource = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
        tile.improvement = if position == strong_target {
            None
        } else {
            Some(crate::name!("mine"))
        };
    }
    {
        let tile = game
            .map
            .tiles
            .get_mut(&strong_target)
            .expect("strong target");
        tile.feature = Some(crate::name!("forest"));
        tile.hills = false;
    }

    // A just-founded city: leave it below the same floor, with one legal Mine
    // to lift its local output. The other bare plots are reserved below, so
    // they cannot obscure which cross-city job wins.
    for position in game.cities[&weak].owned_tiles.clone() {
        if position == weak_home {
            continue;
        }
        let tile = game.map.tiles.get_mut(&position).expect("weak-city tile");
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.district = None;
        tile.wonder = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    {
        let tile = game.map.tiles.get_mut(&weak_target).expect("weak target");
        tile.terrain = crate::name!("plains");
        tile.hills = true;
    }
    game.cities.get_mut(&capital).unwrap().pop = 10;
    game.cities.get_mut(&weak).unwrap().pop = 1;

    assert!(
        game.city_yields(capital).production >= PRODUCTION_CITY_FOUNDATION_FLOOR,
        "fixture: the mature city is already productive"
    );
    assert!(
        game.city_yields(weak).production < PRODUCTION_CITY_FOUNDATION_FLOOR,
        "fixture: the new city still needs its production foundation"
    );
    assert!(
        game.valid_improvements(0, strong_target)
            .contains(&crate::name!("lumber_mill")),
        "fixture: the core has a legal Lumber Mill"
    );
    assert!(
        game.valid_improvements(0, weak_target)
            .contains(&crate::name!("mine")),
        "fixture: the weak city has a legal Mine"
    );

    let builder = game.spawn_test_unit("builder", 0, home);
    let reserved: HashSet<Pos> = game
        .player_city_ids(0)
        .into_iter()
        .flat_map(|cid| game.cities[&cid].owned_tiles.iter().copied())
        .filter(|position| *position != strong_target && *position != weak_target)
        .collect();
    let adaptive = AdvancedAi::new();
    let named = AdvancedAi::targeting(VictoryTarget::Science);
    let weak_shortfall = named.city_production_foundation_shortfall(&game, 0, weak);
    assert!(weak_shortfall > 0.0, "the target sees the city deficit");
    assert_eq!(
        adaptive.city_production_foundation_shortfall(&game, 0, weak),
        0.0,
        "the evaluated adaptive controller keeps its existing rank"
    );
    assert!(
        named.production_foundation_improvement_value(
            &game,
            0,
            weak_target,
            "mine",
            GrandStrategy::Expansion,
            weak_shortfall,
        ) > named.improvement_value_for(&game, 0, weak_target, "mine", GrandStrategy::Expansion,),
        "the weak city's Mine receives a real production-foundation premium"
    );
    assert_eq!(
        adaptive
            .builder_jobs_ranked(&game, 0, builder, GrandStrategy::Expansion, &reserved)
            .first(),
        Some(&strong_target),
        "without a named target, the core Lumber Mill keeps the ordinary lead"
    );
    assert_eq!(
        named
            .builder_jobs_ranked(&game, 0, builder, GrandStrategy::Expansion, &reserved)
            .first(),
        Some(&weak_target),
        "the named lane sends the next charge to the lagging city's Mine"
    );
}

#[test]
fn never_an_empty_queue_2_uses_a_legal_civilian_below_hard_veto() {
    let (mut game, city, _) = empire_with_a_capital(79_125);
    clear_barbarian_fixture(&mut game);
    game.at_war.clear();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.enable_never_an_empty_queue_2();
    // Make every category-valued candidate unattractive enough to cross the
    // ordinary hard-veto floor. The fixture still has legal civilian menu
    // entries, which is the host state that used to wedge the live turn.
    ai.base.w.p_builder = -20_000.0;
    ai.base.w.p_trader = -20_000.0;
    ai.base.w.p_military = -20_000.0;
    ai.base.w.p_building = -20_000.0;
    ai.base.w.p_district = -20_000.0;
    ai.base.w.p_wonder = -20_000.0;
    ai.base.w.p_project = -20_000.0;

    let counts = ai.counts(&game, 0);
    let items = game.producible_items(0, city);
    let scores = ai.production_values(&game, 0, city, &items, &plan, counts);
    let civilian = |game: &Game, item: &Item| match item {
        Item::Unit { unit } => game
            .rules
            .units
            .get(unit)
            .is_none_or(|spec| spec.class != "military"),
        Item::Formation { .. } | Item::Wonder { .. } => false,
        _ => true,
    };
    assert!(
        items.iter().any(|item| civilian(&game, item)),
        "fixture: Civ VI exposes at least one legal civilian production item"
    );
    assert!(
        items
            .iter()
            .zip(&scores)
            .filter(|(item, _)| civilian(&game, item))
            .all(|(_, score)| *score <= PRODUCTION_VETO_FLOOR),
        "fixture: every civilian candidate must be below the ordinary floor; {items:?} / {scores:?}"
    );

    ai.advanced_production(&mut game, 0, &plan, false);
    let started = game.cities[&city]
        .queue
        .first()
        .cloned()
        .expect("the armed v2 gene must answer a legal empty queue");
    assert!(
        game.can_produce(0, city, &started),
        "the emergency answer must be legal in the same Civvis menu"
    );
    assert!(
        civilian(&game, &started),
        "the v2 escape hatch must stay civilian"
    );
    assert!(
        !matches!(started, Item::Wonder { .. } | Item::Formation { .. }),
        "the emergency answer must not be a wonder or formation"
    );
}

#[test]
fn never_an_empty_queue_versions_are_mutually_exclusive() {
    let mut ai = AdvancedAi::new();

    ai.enable_never_an_empty_queue();
    assert!(ai.never_an_empty_queue);
    assert!(!ai.never_an_empty_queue_2);
    assert!(!ai.never_an_empty_queue_3);

    ai.enable_never_an_empty_queue_2();
    assert!(!ai.never_an_empty_queue);
    assert!(ai.never_an_empty_queue_2);
    assert!(!ai.never_an_empty_queue_3);

    ai.enable_never_an_empty_queue_3();
    assert!(!ai.never_an_empty_queue);
    assert!(!ai.never_an_empty_queue_2);
    assert!(ai.never_an_empty_queue_3);

    ai.enable_never_an_empty_queue();
    assert!(ai.never_an_empty_queue);
    assert!(!ai.never_an_empty_queue_2);
    assert!(!ai.never_an_empty_queue_3);
}

#[test]
fn army_declines_a_captured_settler_when_no_city_site_remains() {
    let mut game = Game::new_full(2, 20, 14, 71_019, 120, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let origin = game.nbrs(home)[0];
    let target = game
        .nbrs(origin)
        .into_iter()
        .find(|position| *position != home && game.wdist(home, *position) <= 2)
        .unwrap();
    let city_centers: BTreeSet<Pos> = game.cities.values().map(|city| city.pos).collect();
    for (position, tile) in &mut game.map.tiles {
        if ![home, origin, target].contains(&position) && !city_centers.contains(&position) {
            tile.terrain = crate::name!("ocean");
            tile.feature = None;
        }
    }
    for position in [origin, target] {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }
    game.at_war.insert((0, 1));
    let warrior = game.spawn_test_unit("warrior", 0, origin);
    let settler = game.spawn_test_unit("settler", 1, target);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: game.player_city_ids(1).first().copied(),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    assert!(!ai.base.has_practical_settle_site(&game, 0));
    let mut direct_capture = game.clone();
    let capture_result = direct_capture.apply(
        0,
        &Action::Move {
            unit: warrior,
            to: target,
        },
    );
    assert!(
        capture_result.is_ok(),
        "staged capture was not legal: {capture_result:?}"
    );
    assert_eq!(direct_capture.units[&settler].owner, 0);

    let _ = ai.advanced_military_step(&mut game, 0, warrior, &plan);

    assert_eq!(
        game.units.get(&settler).map(|unit| unit.owner),
        Some(1),
        "capturing the civilian would create a settler with no legal city site"
    );

    game.remove_unit(warrior);
    let scout = game.spawn_test_unit("scout", 0, origin);
    let _ = ai.advanced_military_step(&mut game, 0, scout, &plan);
    assert_eq!(
        game.units.get(&settler).map(|unit| unit.owner),
        Some(1),
        "a recon fallback must not bypass the unwanted-settler guard"
    );
}

#[test]
fn project_search_maintains_aged_reactors_and_marginal_conversion_avoids_churn() {
    let mut game = Game::new(2, 24, 16, 7_101, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    install_ai_test_district(&mut game, city, "industrial_zone");
    game.cities.get_mut(&city).unwrap().buildings =
        vec![crate::name!("factory"), crate::name!("nuclear_power_plant")];
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let counts = EmpireCounts::default();
    let ai = AdvancedAi::new();
    game.climate_phase = 5;
    game.players[0]
        .strategic_resources
        .insert(crate::name!("coal"), 0.1);
    let original_stock_value = 450.0
        + game.strategic_stockpile(0, crate::name!("coal")).min(50.0) * 18.0
        + game.climate_phase as f64 * -110.0;
    let regrouped_stock_value = 450.0
        + (game.strategic_stockpile(0, crate::name!("coal")).min(50.0) * 18.0
            + game.climate_phase as f64 * -110.0);
    assert_ne!(
        original_stock_value.to_bits(),
        regrouped_stock_value.to_bits(),
        "the regression fixture must distinguish floating-point evaluation order"
    );
    assert_eq!(
        AdvancedAi::stock_power_plant_value(&game, 0, "coal").to_bits(),
        original_stock_value.to_bits(),
        "the default-off path must preserve the stock expression bit for bit"
    );
    game.climate_phase = 0;
    let recommission = Item::Project {
        project: crate::name!("recommission_reactor"),
    };

    assert!(ai.production_value(&game, 0, city, &recommission, &plan, &counts) < -9_000.0);
    game.cities.get_mut(&city).unwrap().reactor_age = 30;
    assert!(ai.production_value(&game, 0, city, &recommission, &plan, &counts) > 0.0);

    game.cities.get_mut(&city).unwrap().buildings =
        vec![crate::name!("factory"), crate::name!("oil_power_plant")];
    game.climate_phase = 6;
    game.players[0]
        .strategic_resources
        .insert(crate::name!("coal"), 10.0);
    game.players[0]
        .strategic_resources
        .insert(crate::name!("uranium"), 10.0);
    let coal = Item::Project {
        project: crate::name!("convert_reactor_to_coal"),
    };
    let oil = Item::Project {
        project: crate::name!("convert_reactor_to_oil"),
    };
    let nuclear = Item::Project {
        project: crate::name!("convert_reactor_to_uranium"),
    };
    assert!(
        ai.production_value(&game, 0, city, &nuclear, &plan, &counts)
            > ai.production_value(&game, 0, city, &coal, &plan, &counts)
    );

    game.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), 10.0);
    let mut marginal = ai.clone();
    marginal.reactor_marginal = true;
    assert!(
        marginal.production_value(&game, 0, city, &coal, &plan, &counts) < -9_000.0,
        "coal is worse than the current oil plant at climate phase six"
    );
    assert!(
        marginal.production_value(&game, 0, city, &nuclear, &plan, &counts) > 0.0,
        "uranium is a strict improvement over the current oil plant"
    );
    assert_eq!(
        ai.production_value(&game, 0, city, &recommission, &plan, &counts),
        marginal.production_value(&game, 0, city, &recommission, &plan, &counts),
        "the treatment must not change non-conversion scoring"
    );

    game.cities.get_mut(&city).unwrap().buildings =
        vec![crate::name!("factory"), crate::name!("nuclear_power_plant")];
    assert!(marginal.production_value(&game, 0, city, &coal, &plan, &counts) < -9_000.0);
    assert!(marginal.production_value(&game, 0, city, &oil, &plan, &counts) < -9_000.0);

    game.climate_phase = 0;
    game.players[0]
        .strategic_resources
        .insert(crate::name!("coal"), 50.0);
    game.players[0]
        .strategic_resources
        .insert(crate::name!("uranium"), 0.0);
    assert!(
        marginal.production_value(&game, 0, city, &coal, &plan, &counts) > 0.0,
        "a real fuel/climate utility reversal must remain eligible"
    );

    game.cities.get_mut(&city).unwrap().buildings = vec![crate::name!("factory")];
    assert!(
        marginal.production_value(&game, 0, city, &nuclear, &plan, &counts) < -9_000.0,
        "a missing current plant fails closed"
    );
    assert!(
        ai.production_value(&game, 0, city, &nuclear, &plan, &counts) > -9_000.0,
        "the default-off controller retains stock absolute valuation"
    );
    game.cities.get_mut(&city).unwrap().buildings.extend([
        crate::name!("oil_power_plant"),
        crate::name!("nuclear_power_plant"),
    ]);
    assert!(
        marginal.production_value(&game, 0, city, &coal, &plan, &counts) < -9_000.0,
        "an impossible multiple-plant state fails closed"
    );
}

#[test]
fn defensive_recovery_force_gap_outranks_rear_city_great_person_project() {
    let mut game = Game::new(2, 24, 16, 7_104, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let threatened_city = game.player_city_ids(0)[0];
    let threatened_position = game.cities[&threatened_city].pos;
    let rear_city = found_nearby_test_city(&mut game, 0, threatened_position);
    install_ai_test_district(&mut game, rear_city, "theater_square");
    game.cities
        .get_mut(&rear_city)
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    game.players[0].civics.insert(crate::name!("drama_poetry"));

    for unit in game.player_unit_ids(0) {
        if game.rules.units[game.units[&unit].kind].class == "military" {
            game.remove_unit(unit);
        }
    }
    for _ in 0..3 {
        game.spawn_test_unit("warrior", 0, game.cities[&rear_city].pos);
    }
    game.record_contact(0, 1);
    game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();

    let project = Item::Project {
        project: crate::name!("theater_square_festival"),
    };
    let award =
        game.project_completion_gpp_awards(0, rear_city, "theater_square_festival")["writer"];
    let cost = game.gp_cost(0, "writer");
    game.players[0]
        .gpp
        .insert("writer".to_string(), cost - award);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: Some(1),
        target_city: None,
        threatened_city: Some(threatened_city),
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let ai = AdvancedAi::new();
    let counts = ai.counts(&game, 0);
    assert_eq!(counts.military, 3, "the two-city defense is one unit short");
    assert!(game.can_produce(0, rear_city, &project));
    assert!(
            ai.production_value(&game, 0, rear_city, &project, &plan, &counts) < -9_000.0,
            "a rear city must reinforce a threatened empire before forcing a cultural Great Person race"
        );
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    assert!(
        ai.production_value(&game, 0, rear_city, &warrior, &plan, &counts) > 0.0,
        "the missing land defender must remain a productive alternative"
    );

    let rear_position = game.cities[&rear_city].pos;
    game.spawn_test_unit("warrior", 0, rear_position);
    let reinforced = ai.counts(&game, 0);
    assert_eq!(reinforced.military, 4);
    assert!(
        ai.production_value(&game, 0, rear_city, &project, &plan, &reinforced) > -9_000.0,
        "Great Person races may resume once the defensive force gap is filled"
    );
}

/// ★★★★ A THOUSAND FAITH THAT NOTHING CAN SPEND. A seat with no religion and
/// a bank of 900 Faith, forty percent of the way to a Scientist: the shipped
/// gate buys only a person 85% earned, so the Faith sits; with
/// `idle_faith_patronage` the Faith buys the Scientist outright. Gold keeps
/// its gate, a founder keeps its Faith, and a bank under 600 is not idle.
#[test]
fn idle_faith_buys_a_great_person_outright_only_for_a_seat_with_no_religion() {
    let mut game = Game::new(2, 24, 16, 7_102, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let campus = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    game.map.tiles.get_mut(&campus).unwrap().district = Some(crate::name!("campus"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), campus);
    let cost = game.gp_cost(0, "scientist");
    game.players[0]
        .gpp
        .insert("scientist".to_string(), cost * 0.4);
    game.players[0].religion = None;
    game.players[0].gold = 0.0;
    game.players[0].faith = 900.0;
    let claimed = |game: &Game| {
        game.players[0]
            .gp_claimed
            .get("scientist")
            .copied()
            .unwrap_or(0)
    };

    let mut stock = game.clone();
    AdvancedAi::new().advanced_great_people(&mut stock, 0, GrandStrategy::Expansion);
    assert_eq!(
        claimed(&stock),
        0,
        "the shipped gate leaves a 40% race alone"
    );
    assert_eq!(stock.players[0].faith, 900.0);

    let mut ai = AdvancedAi::new();
    ai.enable_idle_faith_patronage();
    let mut idle = game.clone();
    ai.advanced_great_people(&mut idle, 0, GrandStrategy::Expansion);
    assert_eq!(claimed(&idle), 1, "idle Faith buys the Scientist");
    assert!(idle.players[0].faith < 900.0, "…with Faith");

    // A founder's Faith is not idle.
    let mut founder = game.clone();
    founder.players[0].religion = Some("Our Faith".to_string());
    ai.advanced_great_people(&mut founder, 0, GrandStrategy::Expansion);
    assert_eq!(claimed(&founder), 0);
    // Under 600 the bank is not idle either.
    let mut thin = game.clone();
    thin.players[0].faith = 500.0;
    ai.advanced_great_people(&mut thin, 0, GrandStrategy::Expansion);
    assert_eq!(claimed(&thin), 0);
    // Gold keeps its gate even with the gene on.
    let mut rich = game.clone();
    rich.players[0].faith = 0.0;
    rich.players[0].gold = 5_000.0;
    ai.advanced_great_people(&mut rich, 0, GrandStrategy::Expansion);
    assert_eq!(claimed(&rich), 0, "gold does not buy a 40% race");
}

#[test]
fn culture_building_debt_is_in_native_universe_and_ledger_decides_deployment() {
    let mut universe = AdvancedAi::new();
    universe.enable_engine_repairs_universe();
    assert!(
        universe.culture_building_debt,
        "the native repair universe must carry every repair gene, or the screen varies nothing"
    );

    let mut deployed = AdvancedAi::new();
    deployed.enable_engine_repairs();
    let tag = "culture-building-debt";
    let row = crate::ai::gene(tag).expect("registered");
    assert!(row.repair(), "{tag} is a native repair");
    assert!(!row.host_only(), "{tag} left the host-only list");
    let selected = crate::ai::ledger_default_on(tag)
        .expect("a registered native repair has a deployment selection");
    assert_eq!(
        deployed.culture_building_debt, selected,
        "{tag}: deployment must follow the ledger"
    );
}

/// The gene is an opt-in: off in every bundle, flippable by name, in
/// `PRODUCTION_OPT_INS`.
#[test]
fn idle_faith_patronage_is_a_native_opt_in() {
    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge_universe();
    assert!(!ai.idle_faith_patronage);
    let enable = GENES
        .iter()
        .find(|gene| gene.tag == "idle-faith-patronage")
        .expect("an opt-in row")
        .enable;
    enable(&mut ai);
    assert!(ai.idle_faith_patronage);
    ai.disable_idle_faith_patronage();
    assert!(!ai.idle_faith_patronage);
}

/// `early-contact-window` is separately screenable and never changes a stock,
/// live, or deployed seat unless the evaluator explicitly selects it.
#[test]
fn early_contact_window_is_an_off_by_default_native_opt_in() {
    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge_universe();
    assert!(!ai.early_contact_window);
    let enable = GENES
        .iter()
        .find(|gene| gene.tag == "early-contact-window")
        .expect("an opt-in row")
        .enable;
    enable(&mut ai);
    assert!(ai.early_contact_window);
    ai.disable_early_contact_window();
    assert!(!ai.early_contact_window);
}

/// ★★★★ Three games ended with 2,302 / 1,341 / 1,283 Faith in the bank
/// (civvis-20260816T101521Z, T123936Z, T132247Z) while every offered Great
/// Person sat at 30–75% of its recruit cost — outside the closeness limit
/// the patronage keeps. See `tally_great_people`. With a scientist offer at
/// 30% of cost and a bank that covers the price, stock buys nothing and
/// the live seat patronizes with Faith; the reserve still holds.
#[test]
fn on_the_tally_seat_banked_faith_patronizes_a_great_person_far_from_recruitment() {
    let mut game = Game::new(2, 24, 16, 7_102, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let campus = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    game.map.tiles.get_mut(&campus).unwrap().district = Some(crate::name!("campus"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), campus);
    let cost = game.gp_cost(0, "scientist");
    // Thirty percent of the way there: outside every closeness limit.
    game.players[0]
        .gpp
        .insert("scientist".to_string(), (cost * 0.30).floor());
    game.players[0].gold = 0.0;
    let price = game
        .great_person_patronage_price(0, "scientist", "faith")
        .expect("a faith price is quoted");
    game.players[0].faith = price + 100.0 + 50.0; // above the reserve

    let stock = AdvancedAi::new();
    assert!(!stock.tally_great_people);
    assert!(!AdvancedAi::legacy().tally_great_people);
    let mut untouched = game.clone();
    stock.advanced_great_people(&mut untouched, 0, GrandStrategy::Science);
    assert_eq!(
        untouched.players[0]
            .gp_claimed
            .get("scientist")
            .copied()
            .unwrap_or(0),
        0,
        "stock leaves a far recruit alone and banks the Faith"
    );

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert!(
        live.tally_great_people,
        "the live seat carries the treatment"
    );
    let mut bought = game.clone();
    live.advanced_great_people(&mut bought, 0, GrandStrategy::Science);
    assert_eq!(
        bought.players[0]
            .gp_claimed
            .get("scientist")
            .copied()
            .unwrap_or(0),
        1,
        "the tally seat patronizes the far recruit with its banked Faith"
    );
    assert!(
        bought.players[0].faith >= 100.0 - f64::EPSILON,
        "the Faith reserve is untouched ({})",
        bought.players[0].faith
    );

    // Below the reserve, the treatment buys nothing either.
    let mut poor = game.clone();
    poor.players[0].faith = price + 50.0;
    live.advanced_great_people(&mut poor, 0, GrandStrategy::Science);
    assert_eq!(
        poor.players[0]
            .gp_claimed
            .get("scientist")
            .copied()
            .unwrap_or(0),
        0
    );

    // The universe, not the deployment genome: the ledger now turns on
    // `idle_faith_patronage`, which also buys this race for a seat with no
    // religion; this test is about the tally gene alone.
    let mut withheld = AdvancedAi::new();
    withheld.enable_live_bridge_universe();
    withheld.disable_tally_great_people();
    let mut kept = game.clone();
    withheld.advanced_great_people(&mut kept, 0, GrandStrategy::Science);
    assert_eq!(
        kept.players[0]
            .gp_claimed
            .get("scientist")
            .copied()
            .unwrap_or(0),
        0
    );
}

/// ★★★★ Run civvis-20260816T233226Z ended with 1,999 Faith banked and 26
/// `gp_class_not_offered` refusals: the seat kept buying out the Artist and
/// Merchant races the host's `great_person_exhausted` listed as gone,
/// while three Musicians it never asked for sat on offer. On a mirrored
/// seat the host's list is the authority: an exhausted class is skipped
/// and the Faith goes to a class still on offer; a native board (no list)
/// keeps CIVVIS's own roster.
#[test]
fn patronage_skips_a_class_the_host_reports_exhausted() {
    let mut game = Game::new(2, 24, 16, 7_102, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    install_ai_test_district(&mut game, city, "campus");
    let cost = game.gp_cost(0, "scientist");
    game.players[0]
        .gpp
        .insert("scientist".to_string(), (cost * 0.30).floor());
    game.players[0].gold = 0.0;
    let price = game
        .great_person_patronage_price(0, "scientist", "faith")
        .expect("a faith price is quoted");
    game.players[0].faith = price + 100.0 + 50.0;

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    // The host says every Scientist is claimed.
    let mut exhausted = game.clone();
    exhausted.players[0].live_great_person_exhausted =
        Some(["scientist".to_string()].into_iter().collect());
    assert!(!exhausted.great_person_class_earnable(0, "scientist"));
    live.advanced_great_people(&mut exhausted, 0, GrandStrategy::Science);
    assert_eq!(
        exhausted.players[0]
            .gp_claimed
            .get("scientist")
            .copied()
            .unwrap_or(0),
        0,
        "a class the host has run out of is not bought"
    );
    assert!(
        exhausted.players[0].faith >= price + 100.0 - f64::EPSILON,
        "and the Faith stays in the bank for a class still on offer"
    );

    // The host says Scientists remain: bought as before.
    let mut offered = game.clone();
    offered.players[0].live_great_person_exhausted = Some(BTreeSet::new());
    live.advanced_great_people(&mut offered, 0, GrandStrategy::Science);
    assert_eq!(
        offered.players[0]
            .gp_claimed
            .get("scientist")
            .copied()
            .unwrap_or(0),
        1
    );

    // No host list at all (a native board): CIVVIS's own roster decides.
    let mut native = game.clone();
    assert!(native.players[0].live_great_person_exhausted.is_none());
    live.advanced_great_people(&mut native, 0, GrandStrategy::Science);
    assert_eq!(
        native.players[0]
            .gp_claimed
            .get("scientist")
            .copied()
            .unwrap_or(0),
        1
    );
}

/// A host class can remain recruitable later in the timeline without
/// being on its Great People screen now. At t241 of
/// `civvis-20260817T030352Z`, Rome had 1,339 Faith and a local Scientist
/// race ready to claim, while Firaxis actually offered Engineer, Merchant,
/// Admiral, and Musician. The wrong-class Scientist order was refused and
/// the treasury stayed idle. Restricting candidate generation to the live
/// offer list sends the spend to an actual offer instead.
#[test]
fn live_offer_list_patronizes_an_offered_class_instead_of_a_native_one() {
    let mut game = Game::new(2, 24, 16, 7_106, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    install_ai_test_district(&mut game, city, "campus");

    let scientist_cost = game.gp_cost(0, "scientist");
    let merchant_cost = game.gp_cost(0, "merchant");
    game.players[0]
        .gpp
        .insert("scientist".to_string(), (scientist_cost * 0.30).floor());
    game.players[0]
        .gpp
        .insert("merchant".to_string(), (merchant_cost * 0.30).floor());
    game.players[0].live_great_person_offers = Some(["merchant".to_string()].into_iter().collect());
    game.players[0].gold = 0.0;
    let merchant_price = game
        .great_person_patronage_price(0, "merchant", "faith")
        .expect("the current Merchant has a Faith quote");
    assert_eq!(
        game.great_person_patronage_price(0, "scientist", "faith"),
        None,
        "a native roster entry outside the host screen must not quote a buyout"
    );
    game.players[0].faith = merchant_price + 100.0 + 50.0;

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    live.advanced_great_people(&mut game, 0, GrandStrategy::Science);

    assert_eq!(
        game.players[0]
            .gp_claimed
            .get("scientist")
            .copied()
            .unwrap_or(0),
        0,
        "the high-affinity native Scientist is not on Firaxis's offer screen"
    );
    assert_eq!(
        game.players[0]
            .gp_claimed
            .get("merchant")
            .copied()
            .unwrap_or(0),
        1,
        "the live controller turns banked Faith into the offered Merchant"
    );
    assert!(
        game.players[0].faith >= 100.0 - f64::EPSILON,
        "the live reserve remains after the productive Faith spend ({})",
        game.players[0].faith
    );
}

/// Industrial Zone Logistics should race for an actual Space Race Engineer,
/// not every live Engineer class. Charles Correa gives appeal; Wernher von
/// Braun directly accelerates Space Race projects. The former stays below the
/// ordinary patronage threshold while the latter is worth the same close
/// buyout.
#[test]
fn named_space_race_engineer_beats_an_appeal_engineer_for_patronage() {
    let mut game = Game::new(2, 24, 16, 7_107, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    install_ai_test_district(&mut game, city, "industrial_zone");
    let cost = game.gp_cost(0, "engineer");
    game.players[0]
        .gpp
        .insert("engineer".to_string(), (cost * 0.70).floor());
    game.players[0].live_great_person_offers = Some(["engineer".to_string()].into_iter().collect());
    game.players[0].gold = 0.0;
    let price = game
        .great_person_patronage_price(0, "engineer", "faith")
        .expect("the offered Engineer has a Faith quote");
    game.players[0].faith = price + 100.0 + 50.0;

    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge();

    let mut appeal = game.clone();
    appeal.players[0]
        .live_great_person_offer_individuals
        .insert("engineer".to_string(), "charles_correa".to_string());
    ai.advanced_great_people(&mut appeal, 0, GrandStrategy::Science);
    assert_eq!(
        appeal.players[0]
            .gp_claimed
            .get("engineer")
            .copied()
            .unwrap_or(0),
        0,
        "an appeal Engineer is not worth buying out for a Science victory"
    );

    game.players[0]
        .live_great_person_offer_individuals
        .insert("engineer".to_string(), "wernher_von_braun".to_string());
    assert_eq!(
        AdvancedAi::science_live_great_person_affinity(&game, 0, "engineer"),
        Some(3.8)
    );
    assert!(
        game.can_activate_current_great_person(0, "engineer"),
        "the fixture's local stand-in for the offered Engineer must be legal: {:?}",
        game.great_person_blocker(0, "engineer")
    );
    ai.advanced_great_people(&mut game, 0, GrandStrategy::Science);
    assert_eq!(
        game.players[0]
            .gp_claimed
            .get("engineer")
            .copied()
            .unwrap_or(0),
        1,
        "a named Space Race Engineer is worth the same close buyout"
    );
}

#[test]
fn culture_patronage_waits_for_compatible_great_work_slots() {
    let mut game = Game::new(1, 20, 14, 7_104, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    game.players[0]
        .counters
        .insert("great_work:writing".to_string(), 1);
    let cost = game.current_great_person("writer").unwrap().1.cost;
    game.players[0].gpp.insert("writer".to_string(), cost - 5.0);
    game.players[0].gold = 500.0;
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    let mut live = game.clone();
    live.players[0].live_great_person_offers = Some(["writer".to_string()].into_iter().collect());
    let individual = live.current_great_person("writer").unwrap().0.to_string();
    live.players[0]
        .live_great_person_offer_individuals
        .insert("writer".to_string(), individual);
    assert!(
        live.can_activate_current_great_person(0, "writer"),
        "native recruitment may legally hold a person for future slots"
    );
    ai.advanced_great_people(&mut live, 0, GrandStrategy::Culture);
    assert_eq!(
        live.players[0]
            .gp_claimed
            .get("writer")
            .copied()
            .unwrap_or(0),
        0,
        "paid Culture patronage must still wait for space on a named live offer"
    );

    ai.advanced_great_people(&mut game, 0, GrandStrategy::Culture);
    assert_eq!(
        game.players[0]
            .gp_claimed
            .get("writer")
            .copied()
            .unwrap_or(0),
        0,
        "the occupied Palace slot cannot host another work"
    );

    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    install_ai_test_district(&mut game, city, "theater_square");
    ai.advanced_great_people(&mut game, 0, GrandStrategy::Culture);
    assert_eq!(game.players[0].gp_claimed["writer"], 1);
    let mut adaptive = live.clone();
    AdvancedAi::new().advanced_great_people(&mut adaptive, 0, GrandStrategy::Culture);
    assert_eq!(
        adaptive.players[0].gp_claimed["writer"], 1,
        "the assigned-Culture guard does not change adaptive patronage"
    );
    live.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    install_ai_test_district(&mut live, city, "theater_square");
    assert!(live.can_house_additional_great_work(0, "writing"));
    live.players[0].live_open_great_work_slots = Some(Default::default());
    ai.advanced_great_people(&mut live, 0, GrandStrategy::Culture);
    assert_eq!(
        live.players[0]
            .gp_claimed
            .get("writer")
            .copied()
            .unwrap_or(0),
        0,
        "observed occupied slots override hypothetical model capacity"
    );
    live.players[0].live_open_great_work_slots = Some(["writing".to_string()].into());
    ai.advanced_great_people(&mut live, 0, GrandStrategy::Culture);
    assert_eq!(
        live.players[0].gp_claimed["writer"], 1,
        "a named live offer can be patronized once compatible space opens"
    );
}

#[test]
fn religious_spending_uses_own_faith_inquisitors_without_funding_a_rival() {
    let mut game = Game::new(2, 24, 16, 7_104, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    install_ai_test_district(&mut game, city, "holy_site");
    game.cities.get_mut(&city).unwrap().buildings =
        vec![crate::name!("shrine"), crate::name!("temple")];
    game.players[0].civics.insert(crate::name!("theology"));
    game.players[0].religion = Some("Our Faith".to_string());
    game.players[0]
        .counters
        .insert("inquisition".to_string(), 1);
    game.players[0].faith = 1_000.0;
    game.cities.get_mut(&city).unwrap().pressure.extend([
        ("Our Faith".to_string(), 1_000.0),
        ("Rival Faith".to_string(), 600.0),
    ]);

    let mut ai = AdvancedAi::new();
    let mut converted = game.clone();
    converted
        .cities
        .get_mut(&city)
        .unwrap()
        .pressure
        .insert("Rival Faith".to_string(), 2_000.0);
    let converted_units = converted.player_unit_ids(0).len();
    ai.religious_spending(&mut converted, 0, true);
    assert_eq!(converted.player_unit_ids(0).len(), converted_units);
    assert_eq!(converted.players[0].faith, 1_000.0);

    let before_units = game.player_unit_ids(0).len();
    ai.religious_spending(&mut game, 0, true);
    assert_eq!(game.player_unit_ids(0).len(), before_units + 1);
    let inquisitor = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "inquisitor")
        .unwrap();
    assert_eq!(inquisitor.religion.as_deref(), Some("Our Faith"));
    assert!(game.players[0].faith < 1_000.0);
}

#[test]
fn religious_spending_stops_at_a_target_scaled_unit_ceiling() {
    let mut game = Game::new_full(2, 30, 18, 7_105, 200, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    let home = game.player_city_ids(0)[0];
    let target = game.player_city_ids(1)[0];
    install_ai_test_district(&mut game, home, "holy_site");
    game.cities.get_mut(&home).unwrap().buildings =
        vec![crate::name!("shrine"), crate::name!("temple")];
    game.players[0].techs.insert(crate::name!("astrology"));
    game.players[0].civics.insert(crate::name!("theology"));
    game.players[0].religion = Some("Our Faith".to_string());
    game.players[0].faith = 10_000.0;
    game.cities
        .get_mut(&home)
        .unwrap()
        .pressure
        .insert("Our Faith".to_string(), 1_000.0);
    game.cities
        .get_mut(&target)
        .unwrap()
        .pressure
        .insert("Rival Faith".to_string(), 1_000.0);

    for kind in [
        "apostle",
        "apostle",
        "guru",
        "missionary",
        "missionary",
        "missionary",
    ] {
        let unit = game.spawn_test_unit(kind, 0, game.cities[&home].pos);
        game.units.get_mut(&unit).unwrap().religion = Some("Our Faith".to_string());
    }
    let before_units = game.player_unit_ids(0).len();
    let before_faith = game.players[0].faith;
    AdvancedAi::new().religious_spending(&mut game, 0, true);
    assert_eq!(game.player_unit_ids(0).len(), before_units);
    assert_eq!(game.players[0].faith, before_faith);
}

/// The religion genes are opt-ins: off in every bundle, flippable by name,
/// and discovered by the screen through `PRODUCTION_OPT_INS`.
#[test]
fn the_religion_genes_are_native_opt_ins() {
    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge_universe();
    assert!(!ai.holy_lane_parity);
    assert!(!ai.founder_temple);
    for tag in ["founder-temple", "holy-lane-parity"] {
        let enable = GENES
            .iter()
            .find(|gene| gene.tag == tag)
            .unwrap_or_else(|| panic!("{tag} is an opt-in row"))
            .enable;
        let mut seat = AdvancedAi::new();
        enable(&mut seat);
        let on = match tag {
            "founder-temple" => seat.founder_temple,
            "holy-lane-parity" => seat.holy_lane_parity,
            _ => unreachable!(),
        };
        assert!(on, "{tag} enables its flag");
    }
    let mut on = AdvancedAi::new();
    on.enable_holy_lane_parity();
    on.disable_holy_lane_parity();
    assert!(!on.holy_lane_parity);
    on.enable_founder_temple();
    on.disable_founder_temple();
    assert!(!on.founder_temple);
}

#[test]
fn nonreligious_strategy_buys_defense_only_when_its_home_is_pressured() {
    let mut game = Game::new_full(2, 30, 18, 7_106, 200, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    let home = game.player_city_ids(0)[0];
    let foreign = game.player_city_ids(1)[0];
    install_ai_test_district(&mut game, home, "holy_site");
    game.cities.get_mut(&home).unwrap().buildings =
        vec![crate::name!("shrine"), crate::name!("temple")];
    game.players[0].techs.insert(crate::name!("astrology"));
    game.players[0].civics.insert(crate::name!("theology"));
    game.players[0].religion = Some("Our Faith".to_string());
    game.players[0].faith = 1_000.0;
    game.cities.get_mut(&home).unwrap().pressure.extend([
        ("Our Faith".to_string(), 1_000.0),
        ("Rival Faith".to_string(), 600.0),
    ]);
    game.cities
        .get_mut(&foreign)
        .unwrap()
        .pressure
        .insert("Rival Faith".to_string(), 1_000.0);

    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    let mut safe = game.clone();
    safe.cities
        .get_mut(&home)
        .unwrap()
        .pressure
        .insert("Rival Faith".to_string(), 100.0);
    let safe_units = safe.player_unit_ids(0).len();
    ai.religious_spending(&mut safe, 0, false);
    assert_eq!(safe.player_unit_ids(0).len(), safe_units);

    let before_units = game.player_unit_ids(0).len();
    ai.religious_spending(&mut game, 0, false);
    assert_eq!(game.player_unit_ids(0).len(), before_units + 1);
    assert!(game
        .units
        .values()
        .any(|unit| unit.owner == 0 && unit.kind == "missionary"));
}

#[test]
fn strategic_gold_purchase_annexes_a_luxury_and_preserves_the_reserve() {
    let mut game = Game::new_full(1, 20, 14, 7_106_002, 160, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let center = game.cities[&city].pos;
    for position in game.wdisk(center, 3) {
        if game.map.tiles[&position].owner_city.is_none() {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.hills = false;
            tile.feature = None;
            tile.resource = None;
        }
    }
    let target = game
        .wdisk(center, 2)
        .into_iter()
        .find(|position| {
            game.wdist(*position, center) == 2
                && game.map.tiles[position].owner_city.is_none()
                && game
                    .nbrs(*position)
                    .into_iter()
                    .any(|neighbor| game.map.tiles[&neighbor].owner_city == Some(city))
        })
        .unwrap();
    game.map.tiles.get_mut(&target).unwrap().resource = Some(crate::name!("diamonds"));
    game.players[0]
        .explored
        .extend(game.map.tiles.keys().copied());
    game.players[0].gold = 550.0;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };

    assert!(
        AdvancedAi::targeting(VictoryTarget::Science).advanced_gold_spending(&mut game, 0, &plan)
    );
    assert_eq!(game.map.tiles[&target].owner_city, Some(city));
    assert_eq!(game.players[0].gold, 500.0);
}

#[test]
fn deep_treasury_buys_useful_infrastructure_in_multiple_cities() {
    let mut game = Game::new_full(1, 30, 18, 7_106_001, 160, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let first = game.player_city_ids(0)[0];
    let anchor = game.cities[&first].pos;
    let second = found_nearby_test_city(&mut game, 0, anchor);
    for city in [first, second] {
        install_ai_test_district(&mut game, city, "campus");
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .extend([crate::name!("monument"), crate::name!("granary")]);
    }
    game.players[0].techs.insert(crate::name!("writing"));
    game.players[0].gold = 5_000.0;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let units_before = game.player_unit_ids(0).len();
    let buildings_before = game
        .cities
        .values()
        .filter(|city| city.owner == 0)
        .map(|city| city.buildings.len())
        .sum::<usize>();

    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    let stock_purchases = game
        .legal_actions_within(0, ActionFamilies::PURCHASES | ActionFamilies::EMPIRE)
        .into_iter()
        .filter(|action| {
            matches!(
                action,
                Action::Buy { .. }
                    | Action::BuyBuilding { .. }
                    | Action::BuyDistrict { .. }
                    | Action::BuyPlot { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(ai.legal_purchase_actions(&game, 0), stock_purchases);
    let mut parallel_ai = ai.clone();
    parallel_ai.work_pool = Some(Arc::new(WorkPool::new(4)));
    assert_eq!(
        parallel_ai.legal_purchase_actions(&game, 0),
        stock_purchases
    );
    let mut parallel_game = game.clone();

    assert!(ai.advanced_gold_spending(&mut game, 0, &plan));
    assert!(parallel_ai.advanced_gold_spending(&mut parallel_game, 0, &plan));
    assert_eq!(
        serde_json::to_value(&parallel_game).unwrap(),
        serde_json::to_value(&game).unwrap()
    );
    let units_after = game.player_unit_ids(0).len();
    let buildings_after = game
        .cities
        .values()
        .filter(|city| city.owner == 0)
        .map(|city| city.buildings.len())
        .sum::<usize>();
    assert!(
        units_after - units_before + buildings_after - buildings_before >= 2,
        "a deep treasury should fund more than one strategic acquisition"
    );
    assert!(units_after - units_before <= 2);
    assert!([first, second].into_iter().any(|city| game.cities[&city]
        .buildings
        .contains(&crate::name!("library"))));
    assert!(game.players[0].gold >= 350.0);
}

#[test]
fn adaptive_turn_uses_its_live_plan_for_gold_purchases() {
    // The fixture needs a capital with somewhere to put a Campus, which is
    // a fact about the map rather than the thing under test. Take the first
    // seed that offers one instead of pinning a seed and trusting that map
    // generation never moves again.
    let (mut game, city) = (7_107..7_160u64)
        .find_map(|seed| {
            let mut game = Game::new_full(1, 20, 14, seed, 160, 0, false);
            let settler = game
                .player_unit_ids(0)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")?;
            game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
            let city = *game.player_city_ids(0).first()?;
            game.turn = 10;
            game.players[0].techs.insert(crate::name!("writing"));
            game.players[0].gold = 10_000.0;
            game.players[0].governor_roster.insert(
                "reyna".to_string(),
                GovernorState {
                    city: Some(city),
                    assigned_turn: 0,
                    disabled_until: 0,
                    promotions: BTreeSet::from(["contractor".to_string()]),
                },
            );
            game.legal_actions(0)
                .iter()
                .any(|action| {
                    matches!(
                        action,
                        Action::BuyDistrict { district, currency, .. }
                            if district == "campus" && currency == "gold"
                    )
                })
                .then_some((game, city))
        })
        .expect("no seed offered a lone capital with room to buy a Campus");

    let mut ai = AdvancedAi::new();
    ai.base.book_pos = 4;
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    });

    ai.take_turn(&mut game, 0);

    assert!(
        game.cities[&city]
            .districts
            .contains_key(crate::name!("campus")),
        "an adaptive Science plan should convert surplus Gold into its Campus immediately"
    );
    assert!(game.players[0].gold >= 300.0);
}

#[test]
fn science_core_modernizes_only_when_reserve_and_upkeep_fit() {
    let (mut g, city, home) = empire_with_a_capital(79_102);
    g.at_war.clear();
    g.players[0].civ = "Egypt".into();
    for tech in g.rules.tech_ancestors["iron_working"].clone() {
        g.players[0].techs.insert(Name::new(&tech));
    }
    g.players[0].techs.insert(crate::name!("iron_working"));
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 400.0);
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), home);
    let uid = g.spawn_test_unit("warrior", 0, home);
    g.players[0].gold = 1000.0;
    let (_, price, _) = g.unit_gold_upgrade_offer(0, uid).expect("legal upgrade");
    g.players[0].gold = price + 125.0;
    g.players[0].gold_per_turn = -1.0;
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.modernize_science_core(&mut g, 0);
    assert_eq!(
        g.units[&uid].kind, "warrior",
        "a cash pile cannot fund recurring debt"
    );
    g.players[0].gold_per_turn = 10.0;
    let mut legacy = g.clone();
    AdvancedAi::new().modernize_science_core(&mut legacy, 0);
    assert_eq!(legacy.units[&uid].kind, "warrior");
    ai.modernize_science_core(&mut g, 0);
    assert_eq!(g.units[&uid].kind, "swordsman");
    assert_eq!(g.players[0].gold, 125.0);
}

#[test]
fn live_war_economy_recovers_before_rearming_a_bankrupt_army() {
    // The live Rome board at t174 had the same shape: a standing army,
    // nine cities, Gold below BasicAi's recovery reserve, and a negative
    // cash flow. Advanced production bypassed that baseline guard, kept
    // adding Field Cannons, and the host disbanded one almost every turn.
    let (mut game, city, home) = empire_with_a_capital(79_100);
    let _second_city = found_nearby_test_city(&mut game, 0, home);
    game.players[0].techs.insert(crate::name!("ballistics"));
    game.players[0].gold = 149.0;
    game.players[0].gold_per_turn = -24.39;
    for _ in 0..2 {
        game.spawn_test_unit("warrior", 0, home);
    }
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    // Make the unguarded choice unmistakable. The recovery branch itself
    // ignores these appetite genes, so the paired assertion proves the
    // solvency handoff rather than a coincidental ordinary re-ranking.
    let arm_forces = |ai: &mut AdvancedAi| {
        ai.base.w.p_military = 100.0;
        ai.base.w.p_builder = 0.01;
        ai.base.w.p_trader = 0.01;
        ai.base.w.p_building = 0.01;
        ai.base.w.p_district = 0.01;
        ai.base.w.p_wonder = 0.01;
        ai.base.w.p_project = 0.01;
    };
    let mut ordinary = AdvancedAi::new();
    arm_forces(&mut ordinary);
    let mut unguarded = game.clone();
    ordinary.advanced_production(&mut unguarded, 0, &plan, false);
    assert!(
        matches!(
            unguarded.cities[&city].queue.first(),
            Some(Item::Unit { unit }) if unguarded.rules.units[unit].class == "military"
        ),
        "without the live war-economy recovery, this board starts another military unit"
    );

    // A second home city supplies a concrete internal route once Foreign
    // Trade is known, so the baseline's first-choice recovery is legal.
    game.players[0].civics.insert(crate::name!("foreign_trade"));
    let mut live = AdvancedAi::new();
    live.enable_war_economy();
    arm_forces(&mut live);
    let recovery = live
        .base
        .economic_recovery_item(&game, 0, city, live.counts(&game, 0).traders)
        .or_else(|| live.base.upkeep_free_recovery_item(&game, 0, city))
        .expect("a productive city has a legal recovery item");
    assert!(
        !matches!(
            &recovery,
            Item::Unit { unit } if game.rules.units[unit].class == "military"
        ),
        "the baseline recovery item cannot deepen the maintenance bill"
    );
    assert!(live.live_war_economy_requires_recovery(&game, 0, &live.counts(&game, 0)));
    live.advanced_production(&mut game, 0, &plan, false);
    assert_eq!(
        game.cities[&city].queue.first(),
        Some(&recovery),
        "the live production route must use the same recovery item as BasicAi"
    );

    // Do not strand a genuinely undefended empire. This is the same
    // emergency exception BasicAi uses before it begins economic recovery.
    let mut emergency = unguarded;
    for cid in emergency.player_city_ids(0) {
        emergency.cities.get_mut(&cid).unwrap().queue.clear();
    }
    for uid in emergency.player_unit_ids(0) {
        if emergency.units[&uid].kind == "warrior" {
            emergency.remove_unit(uid);
        }
    }
    let counts = live.counts(&emergency, 0);
    assert!(counts.military < emergency.player_city_ids(0).len());
    assert!(!live.live_war_economy_requires_recovery(&emergency, 0, &counts));
    live.advanced_production(&mut emergency, 0, &plan, false);
    assert!(
        matches!(
            emergency.cities[&city].queue.first(),
            Some(Item::Unit { unit }) if emergency.rules.units[unit].class == "military"
        ),
        "an undermanned major-war empire retains its emergency defender path"
    );
}

#[test]
fn trader_production_requires_an_open_route_and_respects_idle_supply() {
    let mut game = Game::new_full(1, 30, 18, 79_004, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.players[0].civics.insert(crate::name!("foreign_trade"));
    let city = game.player_city_ids(0)[0];
    let item = Item::Unit {
        unit: crate::name!("trader"),
    };
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let ai = AdvancedAi::new();

    let counts = ai.counts(&game, 0);
    assert!(ai.production_value(&game, 0, city, &item, &plan, &counts) < -9_000.0);

    let city_pos = game.cities[&city].pos;
    found_nearby_test_city(&mut game, 0, city_pos);
    let counts = ai.counts(&game, 0);
    assert!(ai.production_value(&game, 0, city, &item, &plan, &counts) > 0.0);

    game.spawn_test_unit("trader", 0, game.cities[&city].pos);
    let counts = ai.counts(&game, 0);
    assert!(ai.production_value(&game, 0, city, &item, &plan, &counts) < -9_000.0);
}

/// The live failure this gene answers: a raider ring around one frontier
/// city must not leave a safe capital's first route slot empty while the
/// empire runs a deficit. The untreated arm keeps the old empire-wide veto.
#[test]
fn solvency_first_trade_slot_reserves_a_locally_safe_origin() {
    let (mut game, safe, remote) = remote_barbarian_trade_board();
    game.players[0].gold_per_turn = -6.0;
    let trader = Item::Unit {
        unit: crate::name!("trader"),
    };
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };

    assert!(BasicAi::barbarian_local_alarm(&game, 0, remote));
    assert!(!BasicAi::barbarian_local_alarm(&game, 0, safe));
    assert!(BasicAi::barbarian_trade_risk(&game, 0));
    assert!(!BasicAi::should_add_trader_safely(&game, 0, 0));
    assert!(BasicAi::safe_trade_origin(&game, 0, safe));
    assert!(!BasicAi::safe_trade_origin(&game, 0, remote));

    let mut control = AdvancedAi::new();
    control.disable_solvency_first_trade_slot();
    let control_counts = control.counts(&game, 0);
    assert!(
        control.production_value(&game, 0, safe, &trader, &plan, &control_counts) < -9_000.0,
        "the frozen global veto remains the untreated arm"
    );

    let mut treated = AdvancedAi::new();
    treated.enable_solvency_first_trade_slot();
    let counts = treated.counts(&game, 0);
    assert!(
        treated.production_value(&game, 0, safe, &trader, &plan, &counts) > 0.0,
        "the safe capital can service the slot"
    );
    assert!(
        treated.production_value(&game, 0, remote, &trader, &plan, &counts) < -9_000.0,
        "the threatened city cannot build an exposed Trader"
    );

    treated.advanced_production(&mut game, 0, &plan, false);
    assert_eq!(
        game.cities[&safe].queue.first(),
        Some(&trader),
        "the first safe empty slot preempts ordinary production"
    );
    assert_ne!(
        game.cities[&remote].queue.first(),
        Some(&trader),
        "the remote alarm receives no Trader order"
    );

    assert!(AdvancedAi::new().base.solvency_first_trade_slot);
    assert!(!AdvancedAi::legacy().base.solvency_first_trade_slot);
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("solvency-first-trade-slot").is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let gene = crate::ai::gene("solvency-first-trade-slot").expect("registered gene");
    assert!(gene.production());
    assert!(gene.screenable());
}

#[test]
fn naval_threat_triage_keeps_a_ship_that_can_hit_the_city_as_an_emergency() {
    let (mut game, city, old_sea, old_raider) = harmless_naval_raider_board();
    game.remove_unit(old_raider);
    game.map.tiles.get_mut(&old_sea).unwrap().terrain = crate::name!("plains");
    let center = game.cities[&city].pos;
    let coast = game.nbrs(center)[0];
    game.map.tiles.get_mut(&coast).unwrap().terrain = crate::name!("coast");
    let barb = game.barb_pid.unwrap();
    game.spawn_test_unit("galley", barb, coast);

    let mut treated = AdvancedAi::new();
    treated.enable_naval_threat_triage();
    assert!(treated
        .base
        .barbarian_local_alarm_for_controller(&game, 0, city));
    assert_eq!(
        treated
            .base
            .barbarian_threat_pressure_for_controller(&game, 0, city),
        1
    );
    assert_eq!(
        treated.base.barbarian_defense_gap(&game, 0, city),
        1,
        "a ship able to land a serious city blow still buys a defender"
    );
    assert!(treated.base.barbarian_presence_at_home_for_controller(
        &game,
        0,
        crate::ai::HOME_CAMP_RADIUS,
    ));
}

#[test]
fn strategic_governments_use_late_tiers_and_match_the_culture_holdout() {
    let mut culture = Game::new_full(3, 18, 10, 79_002, 200, 0, false);
    culture.players[0]
        .civics
        .extend([crate::name!("class_struggle"), crate::name!("suffrage")]);
    culture.players[1].government = Some("communism".to_string());
    culture.players[1].culture_lifetime = 20_000.0;
    culture.players[2].government = Some("democracy".to_string());
    culture.players[2].culture_lifetime = 10_000.0;
    AdvancedAi::targeting(VictoryTarget::Culture).strategic_government(
        &mut culture,
        0,
        GrandStrategy::Culture,
    );
    assert_eq!(culture.players[0].government.as_deref(), Some("communism"));

    let mut science = Game::new_full(2, 18, 10, 79_003, 200, 0, false);
    science.players[0]
        .civics
        .insert(crate::name!("synthetic_technocracy"));
    AdvancedAi::targeting(VictoryTarget::Science).strategic_government(
        &mut science,
        0,
        GrandStrategy::Science,
    );
    assert_eq!(
        science.players[0].government.as_deref(),
        Some("synthetic_technocracy")
    );

    // An adaptive plan must not fall from its one ideal Tier-2 civic all
    // the way back to Tier 1. The live archive had a Science Rome with
    // Divine Right stuck in Classical Republic and a Religion Egypt with
    // Exploration doing the same, despite their unlocked six-slot choices.
    let mut science_fallback = Game::new_full(1, 18, 10, 79_013, 200, 0, false);
    science_fallback.players[0].government = Some("classical_republic".to_string());
    science_fallback.players[0]
        .civics
        .insert(crate::name!("divine_right"));
    AdvancedAi::new().strategic_government(&mut science_fallback, 0, GrandStrategy::Science);
    assert_eq!(
        science_fallback.players[0].government.as_deref(),
        Some("monarchy")
    );

    let mut religion_fallback = Game::new_full(1, 18, 10, 79_014, 200, 0, false);
    religion_fallback.players[0].government = Some("classical_republic".to_string());
    religion_fallback.players[0]
        .civics
        .insert(crate::name!("exploration"));
    AdvancedAi::new().strategic_government(&mut religion_fallback, 0, GrandStrategy::Religion);
    assert_eq!(
        religion_fallback.players[0].government.as_deref(),
        Some("merchant_republic")
    );
}

#[test]
fn adaptive_government_does_not_repeat_lateral_anarchy() {
    let mut game = Game::new_full(2, 18, 10, 79_015, 200, 0, false);
    game.players[0]
        .civics
        .extend([crate::name!("suffrage"), crate::name!("totalitarianism")]);
    game.players[0].government = Some("fascism".to_string());
    game.players[0]
        .past_governments
        .extend(["fascism".to_string(), "democracy".to_string()]);
    game.players[1].government = Some("democracy".to_string());
    game.players[1].culture_lifetime = 20_000.0;

    AdvancedAi::new().strategic_government(&mut game, 0, GrandStrategy::Culture);

    assert_eq!(game.players[0].government.as_deref(), Some("fascism"));
    assert_eq!(game.players[0].anarchy_turns, 0);
    assert!(game.players[0].pending_government.is_none());

    // Returning to a genuinely larger government remains worthwhile:
    // the two dead turns buy a persistent jump from six to eight slots.
    game.players[0].government = Some("merchant_republic".to_string());
    game.players[0]
        .past_governments
        .insert("merchant_republic".to_string());
    AdvancedAi::new().strategic_government(&mut game, 0, GrandStrategy::Conquest);
    assert!(game.players[0].government.is_none());
    assert_eq!(
        game.players[0].pending_government.as_deref(),
        Some("fascism")
    );
    assert!(game.players[0].anarchy_turns > 0);
}

#[test]
fn advanced_turn_does_not_run_the_baseline_government_selector_first() {
    let mut game = Game::new_full(2, 18, 10, 79_016, 200, 0, false);
    game.players[0].civics.extend([
        crate::name!("code_of_laws"),
        crate::name!("political_philosophy"),
    ]);
    game.players[0].government = Some("chiefdom".to_string());
    game.players[0]
        .past_governments
        .insert("chiefdom".to_string());

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.take_turn(&mut game, 0);

    assert_eq!(game.players[0].government.as_deref(), Some("oligarchy"));
    assert_eq!(game.players[0].anarchy_turns, 0);
    assert!(game.players[0].pending_government.is_none());
    assert!(game.players[0].past_governments.contains("oligarchy"));
    assert!(
        !game.players[0]
            .past_governments
            .contains("classical_republic"),
        "the baseline selector must not install a throwaway government before the strategic one"
    );
}

#[test]
fn adaptive_government_does_not_create_anarchy_by_downgrading_first() {
    let mut game = Game::new_full(2, 18, 10, 79_017, 200, 0, false);
    game.players[0].civics.extend([
        crate::name!("class_struggle"),
        crate::name!("reformed_church"),
    ]);
    game.players[0].government = Some("communism".to_string());
    game.players[0]
        .past_governments
        .insert("communism".to_string());
    game.players[0].faith = 1_000.0;

    AdvancedAi::new().strategic_government(&mut game, 0, GrandStrategy::Conquest);

    assert_eq!(game.players[0].government.as_deref(), Some("communism"));
    assert_eq!(game.players[0].anarchy_turns, 0);
    assert!(game.players[0].pending_government.is_none());
    assert!(
        !game.players[0].past_governments.contains("theocracy"),
        "a free first adoption is still a downgrade when it discards two policy slots"
    );
}

#[test]
fn faith_stockpile_mobilizes_for_an_imminent_threat() {
    let mut game = Game::new_full(2, 24, 16, 79_012, 200, 0, false);
    for pid in 0..2 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    let home = game.player_city_ids(0)[0];
    crate::game::vacate_land_combat_purchase_slot(&mut game, 0, home);
    game.players[0]
        .civics
        .insert(crate::name!("reformed_church"));
    game.players[0].faith = 1_500.0;
    let target = game.player_city_ids(1)[0];
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    let before_units = game.player_unit_ids(0).len();
    let ai = AdvancedAi::new();

    ai.strategic_government(&mut game, 0, plan.strategy);
    assert_eq!(game.players[0].government.as_deref(), Some("theocracy"));
    assert!(ai.military_faith_spending(&mut game, 0, &plan));
    assert_eq!(game.player_unit_ids(0).len(), before_units + 1);
    assert!(game.players[0].faith < 1_500.0);
}

/// The production bridge can withhold the screenable `settler-guard-holds`
/// repair while still carrying the HostOnly capture lessons.  Those lessons
/// must activate the same survival bar: otherwise a live Crossbowman is
/// evaluated at its lower Combat strength and the escort is incorrectly
/// treated as a shield.
#[test]
fn live_capture_lessons_activate_guard_survival_when_the_screen_is_withheld() {
    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert!(live.live_settler_capture_lessons);
    live.disable_settler_guard_holds();
    live.disable_settler_guard_holds_2();
    assert!(!live.settler_guard_holds);
    assert!(live.settler_guard_holds_on());

    let mut unsafe_controller = AdvancedAi::new();
    unsafe_controller.enable_live_bridge();
    unsafe_controller.disable_settler_guard_holds();
    unsafe_controller.disable_settler_guard_holds_2();
    unsafe_controller.settlement_safety = false;
    assert!(!unsafe_controller.settler_guard_holds_on());
}

/// `settler_guard_holds` is the deployed live repair, so its bound guard must
/// reclaim a separated Settler before recovery work.  The old priority only
/// protected an already-stacked pair; a wounded guard one tile away fortified
/// in the city, then the Settler's bounded wait expired and it marched alone.
#[test]
fn a_default_live_bound_guard_rejoins_before_healing() {
    let (mut game, source, _target) = stacked_escort_fixture();
    let field = game
        .nbrs(source)
        .into_iter()
        .find(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        })
        .expect("a passable tile beside the city");
    let settler = game.spawn_test_unit("settler", 0, field);
    let guard = game.spawn_test_unit("warrior", 0, source);
    {
        let guard = game.units.get_mut(&guard).unwrap();
        guard.hp = 30;
        guard.moves_left = 2.0;
        guard.acted = false;
        guard.fortified = false;
    }

    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge_universe();
    assert!(ai.settler_guard_holds, "the deployed repair is active");
    assert!(
        ai.live_formationless_settler_shadow,
        "the live seat uses the shadow"
    );
    assert!(
        !ai.civilian_out_of_reach,
        "the default genome must not need the opt-in to keep this guard"
    );
    ai.settler_guards.insert(settler, guard);

    let plan = stacked_escort_plan(&game);
    assert!(ai.advanced_military_step(&mut game, 0, guard, &plan));
    assert_eq!(
        game.units[&guard].pos, game.units[&settler].pos,
        "the bound guard closes before recovery can fortify it away"
    );
    assert_eq!(
        game.units[&guard].hp, 30,
        "the guard did not spend the turn healing"
    );
}

/// ★★★★ A decider restarted mid-game replays the opening book over the
/// whole empire: `take_turn` hands every city to `BasicAi::cities` while
/// `book_pos < 4`, and a fresh agent has `book_pos = 0` (run
/// civvis-20260816T213447Z, t139: seven cities on Rangers for ten turns).
/// `skip_opening_book` marks the opening as played; a new agent still
/// plays it from turn one.
#[test]
fn a_restarted_agent_skips_the_opening_book_and_a_new_one_plays_it() {
    let mut ai = AdvancedAi::new();
    assert!(
        !ai.opening_book_played(),
        "a new agent still has its opening to play"
    );
    ai.skip_opening_book();
    assert!(ai.opening_book_played());
    assert_eq!(
        ai.base.book_pos, 4,
        "exactly the position the four-build book ends on"
    );
    // Idempotent, and never rewinds a book that has advanced further.
    ai.skip_opening_book();
    assert_eq!(ai.base.book_pos, 4);
    // On the same mid-game board a NEW agent plays the book — its
    // position advances — while the restarted one leaves it alone.
    let build = || {
        let mut game = Game::new_full(2, 24, 16, 8_121, 120, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let pos = game.units[&settler].pos;
        game.remove_unit(settler);
        game.found_city_for(0, pos, None);
        game.turn = 60;
        game.current = 0;
        game
    };
    let mut fresh = AdvancedAi::new();
    let mut game = build();
    fresh.take_turn(&mut game, 0);
    assert!(
        fresh.base.book_pos >= 1,
        "a new agent plays its opening book on an idle capital"
    );
    let mut game = build();
    ai.take_turn(&mut game, 0);
    assert_eq!(
        ai.base.book_pos, 4,
        "the restarted agent never touches the book"
    );
}

#[test]
fn settler_threat_detour_is_a_native_opt_in_governed_by_the_ledger() {
    assert!(!AdvancedAi::new().settler_threat_detour);
    assert!(!AdvancedAi::legacy().settler_threat_detour);
    let mut deployed = AdvancedAi::new();
    deployed.enable_engine_repairs();
    let selected = crate::ai::ledger_default_on("settler-threat-detour")
        .expect("a registered native opt-in has a deployment selection");
    assert_eq!(
        deployed.settler_threat_detour, selected,
        "the deployment must follow the recorded selection"
    );
    let enable = GENES
        .iter()
        .find(|gene| gene.tag == "settler-threat-detour")
        .expect("the gene is published for gene_screen and evaluator arms")
        .enable;
    let mut ai = AdvancedAi::new();
    enable(&mut ai);
    assert!(ai.settler_threat_detour);
    ai.disable_settler_threat_detour();
    assert!(!ai.settler_threat_detour);
}

#[test]
fn settler_detour_stays_near_is_a_native_opt_in_governed_by_the_ledger() {
    assert!(!AdvancedAi::new().settler_detour_stays_near);
    assert!(!AdvancedAi::legacy().settler_detour_stays_near);
    let mut deployed = AdvancedAi::new();
    deployed.enable_engine_repairs();
    let selected = crate::ai::ledger_default_on("settler-detour-stays-near")
        .expect("a registered native opt-in has a deployment selection");
    assert_eq!(
        deployed.settler_detour_stays_near, selected,
        "the deployment must follow the recorded selection"
    );
    let enable = GENES
        .iter()
        .find(|gene| gene.tag == "settler-detour-stays-near")
        .expect("the gene is published for gene_screen and evaluator arms")
        .enable;
    let mut ai = AdvancedAi::new();
    enable(&mut ai);
    assert!(ai.settler_detour_stays_near);
    ai.disable_settler_detour_stays_near();
    assert!(!ai.settler_detour_stays_near);
}

/// `settle_sooner` is a native opt-in: off in both bare controllers, governed
/// by the current deployment genome, and still flippable by name through
/// `PRODUCTION_OPT_INS`.
#[test]
fn settle_sooner_is_a_native_opt_in_governed_by_the_ledger() {
    assert!(!AdvancedAi::new().settle_sooner);
    assert!(!AdvancedAi::legacy().settle_sooner);
    let mut deployed = AdvancedAi::new();
    deployed.enable_engine_repairs();
    let selected = crate::ai::ledger_default_on("settle-sooner")
        .expect("a registered native opt-in has a deployment selection");
    assert_eq!(
        deployed.settle_sooner, selected,
        "the deployment must follow the recorded selection"
    );
    let enable = GENES
        .iter()
        .find(|gene| gene.tag == "settle-sooner")
        .expect("the gene is published for gene_screen and evaluator arms")
        .enable;
    let mut ai = AdvancedAi::new();
    enable(&mut ai);
    assert!(ai.settle_sooner);
    ai.disable_settle_sooner();
    assert!(!ai.settle_sooner);
}

#[test]
fn stacked_escort_keeps_advanced_formations_off_land_settlers() {
    let (mut game, source, target) = stacked_escort_fixture();
    let settler = game.spawn_test_unit("settler", 0, source);
    let guard = game.spawn_test_unit("warrior", 0, source);
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    ai.settler_targets.insert(settler, target);
    // The pre-treatment trigger: a blocked settler standing on its escort
    // would be linked by `advanced_formations`.
    ai.settler_blocked_turns.insert(settler, 3);

    ai.advanced_formations(&mut game, 0);

    assert_eq!(game.units[&settler].linked_to, None);
    assert_eq!(game.units[&guard].linked_to, None);
}

/// The Domination conquest opening captures under Expansion: a revolt-bound
/// city is razed there too. Another seat's Expansion still keeps it.
#[test]
fn a_domination_opening_razes_a_revolt_bound_capture_under_expansion() {
    let (mut game, outpost) = heavy_pressure_capture();
    let mut ai = AdvancedAi::new();
    ai.resolve_city_dispositions(&mut game, 0, GrandStrategy::Expansion);
    assert!(
        game.cities[&outpost].owner == 0,
        "the control: an Expansion seat without the Domination target keeps it"
    );
    let (mut game, outpost) = heavy_pressure_capture();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.resolve_city_dispositions(&mut game, 0, GrandStrategy::Expansion);
    assert!(
        !game.cities.contains_key(&outpost),
        "razed rather than handed back"
    );
}

#[test]
fn adaptive_turn_uses_live_victory_focus_for_mandatory_city_disposition() {
    let mut game = Game::new_full(2, 24, 16, 107_001, 80, 1, false);
    let minor = game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .unwrap()
        .id;
    let city = game.player_city_ids(minor)[0];
    {
        let captured = game.cities.get_mut(&city).unwrap();
        captured.owner = 0;
        captured.captured_from = Some(1);
        captured.occupied_from = Some(1);
        captured.loyalty = 50.0;
    }
    game.players[0].dvp = 19;
    let mut ai = AdvancedAi::new();
    assert_eq!(
        ai.victory_focus(&game, 0).strategy,
        GrandStrategy::Diplomacy
    );

    ai.take_turn(&mut game, 0);

    assert_eq!(game.cities[&city].owner, minor);
    assert_eq!(game.players[0].diplomatic_favor, 100.0);
}

#[test]
fn basil_stages_hippodromes_then_resumes_them_at_divine_right() {
    let mut game = Game::new_full(
        2,
        24,
        16,
        crate::rng::fixture_seed("BASIL", 111),
        200,
        0,
        false,
    );
    game.players[0].civ = "Byzantium".to_string();
    game.players[0].religion = Some("Eastern Orthodoxy".to_string());
    game.players[0]
        .civics
        .insert(crate::name!("games_recreation"));
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    let ai = AdvancedAi::new();
    ai.byzantium_tagma_production(&mut game, 0, &plan);
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::District { district, .. }) if district == "hippodrome"
    ));

    let cost = game.rules.districts["hippodrome"].cost;
    game.cities.get_mut(&city).unwrap().production = cost - 0.5;
    ai.byzantium_tagma_production(&mut game, 0, &plan);
    assert!(
        !matches!(
            game.cities[&city].queue.first(),
            Some(Item::District { district, .. }) if district == "hippodrome"
        ),
        "the one-turn Hippodrome must be banked until the Tagma civic"
    );

    game.players[0].civics.insert(crate::name!("divine_right"));
    ai.byzantium_tagma_production(&mut game, 0, &plan);
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::District { district, .. }) if district == "hippodrome"
    ));
}

/// The barbarian-scout exemption is a native arm: Firaxis' barbarian scouts
/// neither attack nor capture, and the native seat's provably cannot (the
/// minor-seat gate freezes its military units; the engine scout phase moves
/// by `relocate`, which has no capture hook). The deployment ledger chooses
/// its live state, while a focused arm can still price the promotion.
#[test]
fn the_barbarian_scout_exemption_is_native() {
    assert!(AdvancedAi::new().barbarian_scouts_are_scouts);
    assert!(!AdvancedAi::legacy().barbarian_scouts_are_scouts);
    let mut withheld = AdvancedAi::new();
    withheld.disable_barbarian_scouts_are_scouts();
    assert!(!withheld.barbarian_scouts_are_scouts);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert_eq!(
        live.barbarian_scouts_are_scouts,
        crate::ai::ledger_default_on("barbarian-scouts-are-scouts")
            .expect("the native repair is registered in the deployment ledger"),
        "the live bridge must follow the deployment ledger"
    );
    live.disable_barbarian_scouts_are_scouts();
    assert!(
        !live.barbarian_scouts_are_scouts,
        "the focused arm can withhold this screenable repair"
    );
    live.enable_barbarian_scouts_are_scouts();
    assert!(live.barbarian_scouts_are_scouts);
}

/// The errand itself: on the same board, the same two swordsmen clear a
/// warrior-guarded camp six tiles from the capital. The default barbarian
/// response is allowed to clear it too; the bounty treatment must not make
/// the deliberate errand slower than the ordinary response.
#[test]
fn the_peacetime_errand_clears_the_camp_and_collects_the_bounty() {
    let run = |bounty: bool| -> (Option<u32>, i64) {
        let (mut game, home) = camp_bounty_board(90_079);
        let barb = game.barb_pid.unwrap();
        let camp = open_ground_at(&game, home, 6);
        // A spawn tick far in the future keeps THIS camp from raising
        // raiders mid-pin.
        game.barb_camps.insert(camp, game.turn + 1_000);
        game.map.tiles.get_mut(&camp).unwrap().improvement = Some(crate::name!("barbarian_camp"));
        game.spawn_test_unit("warrior", barb, camp);
        game.spawn_test_unit("swordsman", 0, home);
        game.spawn_test_unit("swordsman", 0, home);
        let mut ai = AdvancedAi::new();
        ai.base.camp_bounty = bounty;
        let mut cleared_by = None;
        // Seat-turns: three seats per world turn, thirty world turns of
        // margin for six tiles of walking and a fight.
        for _ in 0..90 {
            let pid = game.current;
            if pid == 0 {
                ai.take_turn(&mut game, 0);
            }
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
            if !game.barb_camps.contains_key(&camp) {
                cleared_by = Some(game.turn);
                break;
            }
        }
        let camps = game.players[0].counters.get("camps").copied().unwrap_or(0);
        (cleared_by, camps)
    };

    let (with_errand, booked) = run(true);
    assert!(
        with_errand.is_some(),
        "thirty turns is six of walking and a fight; the errand must clear \
         the camp"
    );
    assert!(
        booked >= 1,
        "the clear is booked on the counter the boosts read"
    );

    // The default barbarian response now deliberately answers a camp this
    // close, so the errand is a non-regression check rather than a claim that
    // the bounty flag is the only route to the camp.
    let (without_errand, _) = run(false);
    assert!(
        without_errand.is_some(),
        "the default barbarian response must still clear the camp"
    );
    assert!(
        with_errand.unwrap() <= without_errand.unwrap(),
        "the errand (t{:?}) must not clear the camp after the default response \
         (t{:?})",
        with_errand,
        without_errand
    );
}

/// An empty camp is captured by movement, not a melee attack. The barbarian
/// responder used to hand this target to `tactical_step`, which correctly holds
/// an ordinary melee unit one tile from a defender but incorrectly left this
/// camp standing forever. A Scout could instead walk past it to explore. The
/// direct clear must work without the longer-range, default-off camp-bounty
/// treatment.
#[test]
fn an_adjacent_empty_camp_is_cleared_instead_of_being_held_or_explored_past() {
    for (seed, kind) in [(90_082, "warrior"), (90_083, "scout")] {
        let (mut game, home) = camp_bounty_board(seed);
        let camp = open_ground_at(&game, home, 1);
        game.barb_camps.insert(camp, game.turn + 1_000);
        game.map.tiles.get_mut(&camp).unwrap().improvement = Some(crate::name!("barbarian_camp"));
        let unit = game.spawn_test_unit(kind, 0, home);
        let plan = StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::new();
        ai.battlefront_observation = false;

        assert!(
            !ai.base.camp_bounty,
            "the direct clear ships independently of the errand"
        );
        assert!(
            BasicAi::barbarian_presence_at_home_with_camp_radius(
                &game,
                0,
                crate::ai::HOME_CAMP_RADIUS
            ),
            "the local camp takes the barbarian-response path"
        );
        assert!(game.player_can_see(0, camp));
        assert!(ai.advanced_military_step(&mut game, 0, unit, &plan));
        assert_eq!(game.units[&unit].pos, camp, "{kind} must enter the camp");
        assert!(
            !game.barb_camps.contains_key(&camp),
            "the responder must enter the undefended camp, not stop next to it"
        );
        assert_eq!(game.players[0].counters.get("camps"), Some(&1));
    }
}

/// The delegation must carry the plan's target and must not leak it.
///
/// Two properties, both deterministic: with the flag off the baseline
/// governor still sees the gene, and with it on the gene is restored
/// after the turn so no later consumer — gold purchase, the `assess`
/// fallback, a census — reads the substituted value.
#[test]
fn plan_city_target_substitutes_only_inside_the_delegated_call() {
    for flag in [false, true] {
        let mut game = Game::new_full(4, 24, 16, 4_810_001, 120, 1, false);
        game.set_fog_memory(false);
        let mut ais: Vec<AdvancedAi> = (0..game.players.len())
            .map(|_| {
                let mut ai = AdvancedAi::new();
                ai.plan_city_target = flag;
                ai
            })
            .collect();
        let gene = ais[0].base.w.city_target;
        while game.winner.is_none() && game.turn <= 40 {
            let pid = game.current;
            ais[pid].take_turn(&mut game, pid);
            assert_eq!(
                ais[pid].base.w.city_target, gene,
                "the gene must be restored before the turn returns"
            );
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &crate::game::Action::EndTurn);
            }
        }
    }
}

#[test]
fn delegated_wide_expansion_extends_the_window_but_defense_stays_first() {
    let mut game = Game::new_full(2, 30, 18, 4_810_002, 500, 0, false);
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
    }
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    game.cities.get_mut(&city).unwrap().pop = 6;
    game.turn = 200;

    let own_military: Vec<u32> = game
        .player_unit_ids(0)
        .into_iter()
        .filter(|unit| game.rules.units[game.units[unit].kind].class == "military")
        .collect();
    for unit in own_military {
        game.remove_unit(unit);
    }
    game.at_war.insert((0, 1));

    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 9,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.base.book_pos = 4;
    // Prove the delegated plan, not the production constructor's own
    // defaults, opens both the target and the speed-aware time window —
    // and keep the promoted recon fleet out of the queue this fixture
    // reads, since a missing scout would otherwise outrank the Settler.
    ai.base.w.city_target = 1.0;
    ai.base.w.settler_stop_turn = 100.0;
    ai.base.w.builder_per_city = 0.25;
    ai.disable_recon_replacement();
    ai.disable_naval_recon();
    ai.delegated_cities(&mut game, 0, &plan);

    let defensive_item = game.cities[&city].queue.first().unwrap();
    assert!(matches!(
        defensive_item,
        Item::Unit { unit } if game.rules.units[unit].class == "military"
    ));
    assert_eq!(ai.base.w.city_target, 1.0);
    assert_eq!(ai.base.w.settler_stop_turn, 100.0);
    assert_eq!(ai.base.w.builder_per_city, 0.25);

    // Once the one-unit-per-city floor is restored, the same wide plan may
    // produce the Settler even though both historical genes would refuse it.
    game.at_war.clear();
    game.cities.get_mut(&city).unwrap().queue.clear();
    game.spawn_test_unit("warrior", 0, game.cities[&city].pos);
    ai.delegated_cities(&mut game, 0, &plan);
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::Unit { unit }) if unit == "settler"
    ));
    assert_eq!(ai.base.w.city_target, 1.0);
    assert_eq!(ai.base.w.settler_stop_turn, 100.0);
    assert_eq!(ai.base.w.builder_per_city, 0.25);
}

/// ★★★ A STOCK LANE'S PRESSURE ACCELERATES, AND THE BAR READ IT TOO LATE TO
/// VOTE. The first game on the repaired economy (civvis-20260818T231407Z,
/// 16 cities against the best rival's 9) crossed `STOCK_DENIAL_BAR` at ~t221;
/// the game's last Congress sat at t222 with nothing reading urgent when the
/// ballot was priced, and Egypt won Culture at t232. Across the four
/// measurable culture losses the projection moves the first crossing from
/// 3/7/12/37 turns of lead to 12/12/15/66. The projection can only raise a
/// reading — a receding leader clamps to the raw number — and it feeds the
/// stock clause alone.
#[test]
fn a_rising_stock_pressure_reads_urgent_a_congress_earlier() {
    let game = Game::new_full(2, 20, 14, 71_131, 30, 0, false);
    let rising = VictoryFocus {
        strategy: GrandStrategy::Culture,
        progress: 60,
    };

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert!(
        live.projected_stock_denial,
        "the live seat carries the treatment"
    );
    assert!(
        live.stock_denial_lead_time,
        "the projection rides the lead-time bar"
    );

    // No history yet: a projection from a single reading is noise, so the
    // raw bar rules and 60 is not urgent.
    assert!(!live.victory_pressure_is_urgent(&game, 1, rising));

    // Eight turns of the measured Egypt ramp (~2.4/turn): 60 projected
    // fifteen turns forward clears the bar with a Congress still to come.
    live.stock_pressure_history.insert(
        1,
        (0..=8)
            .map(|i| (100 + i, 41 + (i as i32) * 12 / 5))
            .collect(),
    );
    assert!(live.victory_pressure_is_urgent(&game, 1, rising));
    assert_eq!(
        live.denial_response_for_pressure(&game, 0, 100, 1, rising),
        Some(GrandStrategy::Culture),
        "the projected alarm must reach the response, even with an assigned lane's 100 preference"
    );
    let diplomacy = VictoryFocus {
        strategy: GrandStrategy::Diplomacy,
        progress: 60,
    };
    assert_eq!(
        live.denial_response_for_pressure(&game, 0, 100, 1, diplomacy),
        Some(GrandStrategy::Diplomacy)
    );

    let mut targeted = AdvancedAi::targeting(VictoryTarget::Culture);
    targeted.enable_live_bridge();
    targeted.deny_while_targeted = true;
    targeted.stock_pressure_history = live.stock_pressure_history.clone();
    assert_eq!(
        targeted.actionable_victory_denial_with_culture_pressures(
            &game,
            0,
            &BTreeMap::from([(1, 60)]),
        ),
        Some((1, GrandStrategy::Culture)),
        "an assigned lane receives the actionable forecast before the raw bar"
    );

    // A flat or receding leader clamps to the raw reading: not urgent.
    live.stock_pressure_history
        .insert(1, (0..=8).map(|i| (100 + i, 60 - (i as i32))).collect());
    assert!(!live.victory_pressure_is_urgent(&game, 1, rising));

    // The raw bar itself is untouched: 78 stays urgent with any history.
    let at_bar = VictoryFocus {
        strategy: GrandStrategy::Culture,
        progress: STOCK_DENIAL_BAR,
    };
    assert!(live.victory_pressure_is_urgent(&game, 1, at_bar));

    // Non-stock lanes never read the projection: Science at 60 with the
    // steepest ramp on record stays non-urgent.
    live.stock_pressure_history
        .insert(1, (0..=8).map(|i| (100 + i, 40 + (i as i32) * 6)).collect());
    let science = VictoryFocus {
        strategy: GrandStrategy::Science,
        progress: 60,
    };
    assert!(!live.victory_pressure_is_urgent(&game, 1, science));
    assert_eq!(
        live.denial_response_for_pressure(&game, 0, 0, 1, science),
        None
    );

    // The withhold arm restores the raw bar exactly.
    let mut withheld = AdvancedAi::new();
    withheld.enable_live_bridge();
    withheld.disable_projected_stock_denial();
    withheld.stock_pressure_history.insert(
        1,
        (0..=8)
            .map(|i| (100 + i, 41 + (i as i32) * 12 / 5))
            .collect(),
    );
    assert!(!withheld.victory_pressure_is_urgent(&game, 1, rising));
    assert_eq!(
        withheld.denial_response_for_pressure(&game, 0, 0, 1, rising),
        None
    );

    // The published withhold row reaches the same flag.
    let row = GENES
        .iter()
        .find(|gene| gene.field == "projected_stock_denial")
        .expect("the gene is published");
    assert!(row.live());
    assert_eq!(row.tag, "projected-stock-denial");
    let disable = row.disable;
    let mut via_row = AdvancedAi::new();
    via_row.enable_live_bridge();
    disable(&mut via_row);
    assert!(
        !via_row.projected_stock_denial,
        "{} row disables the flag",
        row.field
    );

    // Frozen and ordinary controllers never project.
    assert!(!AdvancedAi::new().projected_stock_denial);
    assert!(!AdvancedAi::legacy().projected_stock_denial);
}

/// A World Congress can award several DVP on one turn, then none for many
/// turns. At nine of twenty points, extrapolating that one jump over the next
/// fifteen turns interrupted the live Munich capital siege. An army already
/// at a required capital keeps its plan until the observed diplomatic clock
/// reaches the early stock bar; an empty front retains the projected warning.
#[test]
fn a_congress_dvp_jump_does_not_retask_an_active_domination_siege() {
    let mut game = Game::new_full(2, 20, 14, 71_132, 300, 0, false);
    let initial_units: Vec<_> = game.units.keys().copied().collect();
    for id in initial_units {
        game.remove_unit(id);
    }
    game.found_city_for(0, (2, 6), None);
    let capital = game.found_city_for(1, (10, 6), None);
    assert!(game.cities[&capital].is_capital);
    game.cities.get_mut(&capital).unwrap().wall_hp = 100;
    game.at_war.insert((0, 1));
    let gun = game.spawn_test_unit("catapult", 0, (8, 6));
    let jump = VictoryFocus {
        strategy: GrandStrategy::Diplomacy,
        progress: 51,
    };
    let history: Vec<_> = (134..=142)
        .map(|turn| (turn, if turn == 142 { 51 } else { 26 }))
        .collect();

    let mut other_lane = AdvancedAi::new();
    other_lane.enable_live_bridge();
    other_lane.stock_pressure_history.insert(1, history.clone());
    assert!(
        other_lane.victory_pressure_is_urgent(&game, 1, jump),
        "the existing general projection remains available"
    );

    let mut domination = AdvancedAi::targeting(VictoryTarget::Domination);
    domination.enable_live_bridge();
    domination.stock_pressure_history.insert(1, history);
    assert!(domination.victory_pressure_is_urgent(&game, 1, jump));
    assert_eq!(
        domination.denial_response_for_pressure(&game, 0, 100, 1, jump),
        None,
        "a one-time DVP jump must not divert an active capital siege"
    );
    game.remove_unit(gun);
    assert_eq!(
        domination.denial_response_for_pressure(&game, 0, 100, 1, jump),
        Some(GrandStrategy::Diplomacy),
        "an empty front retains the projected warning"
    );
    let _gun = game.spawn_test_unit("catapult", 0, (8, 6));
    let actual_alarm = VictoryFocus {
        strategy: GrandStrategy::Diplomacy,
        progress: 80,
    };
    assert!(domination.victory_pressure_is_urgent(&game, 1, actual_alarm));
    assert_eq!(
        domination.denial_response_for_pressure(&game, 0, 100, 1, actual_alarm),
        Some(GrandStrategy::Diplomacy),
        "the real sixteen-point warning still interrupts the siege"
    );
}

/// `A -> B -> C -> A` is a wasted turn the shipped guard cannot see.
///
/// `recorded_tactical_step` remembers the single tile a unit just left, so it
/// refuses `A -> B -> A` and passes a loop one hop longer. Measured over three
/// 6-player 200-turn games, that is not a corner: the live bundle walks 329
/// routes ending where they began and **191 of them are exactly this shape**,
/// against 23 of the two-hop shape the shipped guard already covers.
#[test]
fn the_whole_turn_guard_refuses_a_three_hop_loop_that_the_shipped_one_allows() {
    let mut game = Game::new(2, 24, 16, crate::rng::fixture_seed("AUTO_22", 79), 80, 0);
    let [a, b, c] = hex_triangle(&game).expect("fixture offers three adjacent open land tiles");

    for (guard, expected) in [(false, true), (true, false)] {
        let mut board = game.clone();
        let uid = board.spawn_test_unit("horseman", 0, a);
        // Movement is not what is under test; terrain cost must not decide it.
        board.units.get_mut(&uid).unwrap().moves_left = 12.0;

        let mut ai = AdvancedAi::new();
        ai.enable_recorded_tactical_step();
        if guard {
            ai.enable_whole_turn_backtrack_guard();
        }

        assert!(ai.base.path_move(&mut board, 0, uid, b), "step to B");
        assert!(ai.base.path_move(&mut board, 0, uid, c), "step to C");
        assert_eq!(
            ai.base.path_move(&mut board, 0, uid, a),
            expected,
            "closing the loop back to the start, guard={guard}"
        );
        assert_eq!(
            board.units[&uid].pos == a,
            expected,
            "and the unit is back where it began only when the loop was allowed"
        );
    }
    let _ = &mut game;
}

/// The one-hop reversal stays refused with the wider guard on.
///
/// The wider rule has to *contain* the shipped one — a trail whose last entry
/// is the tile being proposed is still a trail that contains it — or turning
/// it on would trade one pathology for another.
#[test]
fn the_whole_turn_guard_still_refuses_the_immediate_reversal() {
    let mut game = Game::new(2, 24, 16, 79, 80, 0);
    let [a, b, _] = hex_triangle(&game).expect("fixture offers adjacent open land tiles");
    for guard in [false, true] {
        let mut board = game.clone();
        let uid = board.spawn_test_unit("horseman", 0, a);
        board.units.get_mut(&uid).unwrap().moves_left = 12.0;
        let mut ai = AdvancedAi::new();
        ai.enable_recorded_tactical_step();
        if guard {
            ai.enable_whole_turn_backtrack_guard();
        }
        assert!(ai.base.path_move(&mut board, 0, uid, b), "step to B");
        assert!(
            !ai.base.path_move(&mut board, 0, uid, a),
            "stepping straight back is refused either way, guard={guard}"
        );
    }
    let _ = &mut game;
}

/// The treatment is off in the shipped controller and on in the live bundle.
#[test]
fn the_whole_turn_guard_is_a_live_treatment() {
    assert!(!AdvancedAi::new().base.whole_turn_backtrack_guard);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(live.base.whole_turn_backtrack_guard);
    live.disable_whole_turn_backtrack_guard();
    assert!(!live.base.whole_turn_backtrack_guard);
}

/// ★★★★ THE PLAZA STANDS EMPTY IN EVERY RECORDED GAME. See
/// `AdvancedAi::expansion_hall`: a Government Plaza completes at t29–57 in
/// all 35 recorded 2026-08-18/19 runs and no tier-1 plaza building is ever
/// built, because the Ancestral Hall has no yields, its district is not
/// `specialty`, and its two effects — a free Builder in every new city, +50%
/// Settlers — were priced at nothing. With the flag on the land grab pays for
/// them while it is still short of seats, and nothing when it is not; ordinary
/// and frozen controllers keep the zero.
#[test]
fn the_expansion_hall_is_priced_while_the_land_grab_is_short_of_seats() {
    let mut game = Game::new(2, 32, 24, 5_417, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    // A completed Government Plaza in the capital, the way the recorded games
    // hold one by t29–57.
    let plaza = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            *position != game.cities[&city].pos
                && game.map.tiles[position].district.is_none()
                && !game.map.tiles[position].terrain.contains("ocean")
                && !game.map.tiles[position].terrain.contains("coast")
                && !game.map.tiles[position].terrain.contains("mountain")
        })
        .expect("a land tile for the plaza");
    game.map.tiles.get_mut(&plaza).unwrap().district = Some(crate::name!("government_plaza"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("government_plaza"), plaza);
    let hall = Item::Building {
        building: crate::name!("ancestral_hall"),
    };
    let audience = Item::Building {
        building: crate::name!("audience_chamber"),
    };
    let plan = |desired_cities: usize| StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities,
        assessed_turn: game.turn,
        rush: false,
    };

    // Off: nothing prices the hall's effects.
    let control = AdvancedAi::new();
    let counts = control.counts(&game, 0);
    let unpriced = control.production_value(&game, 0, city, &hall, &plan(16), &counts);

    // On, with the land grab short of seats: the hall is owed the Builder in
    // every new city and the faster Settlers, and the Audience Chamber —
    // neither effect — is not.
    let mut live = AdvancedAi::new();
    live.enable_land_grab();
    live.enable_expansion_hall();
    let counts = live.counts(&game, 0);
    let priced = live.production_value(&game, 0, city, &hall, &plan(16), &counts);
    let chamber = live.production_value(&game, 0, city, &audience, &plan(16), &counts);
    // `production_value` normalises by (7 + turns to build), so the raw 1200
    // reads as 1200 / (7 + turns) here; the shape, not the figure, is asserted.
    assert!(
        priced > unpriced + 10.0,
        "fifteen seats short pays the full hall: {priced} against {unpriced}"
    );
    assert!(
        (chamber - unpriced).abs() < 1.0,
        "the Audience Chamber carries neither effect: {chamber} against {unpriced}"
    );
    // On, but the land grab has its seats: back to the unpriced building.
    let satisfied = live.production_value(&game, 0, city, &hall, &plan(1), &counts);
    assert!(
        (satisfied - unpriced).abs() < 1.0,
        "no seats short, nothing owed: {satisfied} against {unpriced}"
    );
    // Half the shortfall, half the price.
    let half = live.production_value(&game, 0, city, &hall, &plan(4), &counts);
    let ratio = (half - unpriced) / (priced - unpriced);
    assert!(
        (0.45..=0.55).contains(&ratio),
        "three seats short pays half of the full hall: {half} against {priced} ({ratio:.2})"
    );

    // Frozen and ordinary controllers never carry the flag.
    assert!(!AdvancedAi::new().expansion_hall);
    assert!(!AdvancedAi::legacy().expansion_hall);
}

/// A due person may pause an ordinary building (its progress is kept); a
/// person still ten turns out may not, and a unit is never paused.
#[test]
fn a_due_person_pauses_a_building_but_never_a_unit() {
    let (mut game, capital) = great_person_housing_fixture(71_303);
    let cost = game.gp_cost(0, "writer");
    let rate = game.great_person_points_per_turn(0)["writer"];
    let plan = great_person_housing_plan(&game, GrandStrategy::Science);
    let mut ai = AdvancedAi::new();
    ai.enable_great_person_housing();
    let granary = Item::Building {
        building: crate::name!("granary"),
    };
    game.players[0].techs.insert(crate::name!("pottery"));
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    assert!(game.can_produce(0, capital, &granary));
    assert!(game.can_produce(0, capital, &warrior));

    let mut due = game.clone();
    due.players[0].gpp.insert("writer".to_string(), cost);
    due.cities.get_mut(&capital).unwrap().queue = vec![granary.clone()];
    assert!(ai.great_person_housing(&mut due, 0, &plan));
    assert!(matches!(
        due.cities[&capital].queue.first(),
        Some(Item::Building { building }) if *building == crate::name!("amphitheater")
    ));

    let mut soon = game.clone();
    soon.players[0]
        .gpp
        .insert("writer".to_string(), cost - rate * 10.0);
    soon.cities.get_mut(&capital).unwrap().queue = vec![granary.clone()];
    assert!(!ai.great_person_housing(&mut soon, 0, &plan));
    assert_eq!(soon.cities[&capital].queue.first(), Some(&granary));

    let mut armed = game.clone();
    armed.players[0].gpp.insert("writer".to_string(), cost);
    armed.cities.get_mut(&capital).unwrap().queue = vec![warrior.clone()];
    assert!(!ai.great_person_housing(&mut armed, 0, &plan));
    assert_eq!(armed.cities[&capital].queue.first(), Some(&warrior));
}

/// The other classes: a due Scientist with no Campus reserves the Campus; a
/// Scientist still on the way does not start a district on speculation; the
/// Musician's chain walks museum before Broadcast Center.
#[test]
fn districts_are_reserved_only_for_a_due_person_and_the_music_chain_walks_its_prerequisites() {
    let (mut game, capital) = great_person_housing_fixture(71_306);
    // Room for a second district beside the Theater Square.
    game.cities.get_mut(&capital).unwrap().pop = 7;
    let mut ai = AdvancedAi::new();
    ai.enable_great_person_housing();
    let scientist = |due: bool| StuckGreatPerson {
        kind: "scientist".to_string(),
        remedy: GreatPersonRemedy::District("campus"),
        due,
        turns_to_due: if due { 0.0 } else { 8.0 },
    };
    assert!(
        ai.great_person_housing_item(&game, 0, capital, &scientist(false))
            .is_none(),
        "a district is a bet for points still eight turns out"
    );
    assert!(
        matches!(
            ai.great_person_housing_item(&game, 0, capital, &scientist(true)),
            Some(Item::District { district, .. }) if game.district_family(district) == "campus"
        ),
        "a due Scientist reserves the Campus"
    );

    let musician = StuckGreatPerson {
        kind: "musician".to_string(),
        remedy: GreatPersonRemedy::Slots("music", 2),
        due: false,
        turns_to_due: 5.0,
    };
    assert!(
        ai.great_person_housing_item(&game, 0, capital, &musician)
            .is_none(),
        "without Radio no museum brings a Broadcast Center nearer"
    );
    game.players[0].techs.insert(crate::name!("radio"));
    assert!(
        matches!(
            ai.great_person_housing_item(&game, 0, capital, &musician),
            Some(Item::Building { building }) if building == crate::name!("amphitheater")
        ),
        "Radio known and no museum yet: the chain starts at the Amphitheater"
    );
}

/// The gene is production-on, still flippable by name, and registered as a
/// production row.
#[test]
fn great_person_housing_is_a_native_production_gene() {
    let ai = AdvancedAi::new();
    assert!(ai.great_person_housing);
    assert!(!AdvancedAi::legacy().great_person_housing);
    let enable = GENES
        .iter()
        .find(|gene| gene.tag == "great-person-housing")
        .expect("a production row")
        .enable;
    let mut ai = AdvancedAi::legacy();
    enable(&mut ai);
    assert!(ai.great_person_housing);
    ai.disable_great_person_housing();
    assert!(!ai.great_person_housing);
    let gene = crate::ai::gene("great-person-housing").expect("registered gene");
    assert!(gene.production());
    assert!(gene.screenable());
}

/// The genes are production-on, still flippable by name, and switching the
/// raid off forgets any open raid.
#[test]
fn opportunistic_war_is_a_native_production_gene() {
    let ai = AdvancedAi::new();
    assert!(ai.opportunistic_war);
    assert!(!ai.raid_pillage_prizes);
    assert!(!AdvancedAi::legacy().opportunistic_war);
    assert!(!AdvancedAi::legacy().raid_pillage_prizes);
    let enable = GENES
        .iter()
        .find(|gene| gene.tag == "opportunistic-war")
        .expect("a production row")
        .enable;
    let mut ai = AdvancedAi::legacy();
    enable(&mut ai);
    assert!(ai.opportunistic_war);
    ai.raid_war = Some(opportunistic_war::RaidWar {
        target: 1,
        declared: 30,
        value: SETTLER_PRIZE,
        settlers: 1,
        builders: 0,
        pillage_tiles: 0,
    });
    ai.disable_opportunistic_war();
    assert!(!ai.opportunistic_war);
    assert!(ai.raid_war.is_none());
    assert!(!ai.raid_pillage_prizes);
    let enable = GENES
        .iter()
        .find(|gene| gene.tag == "raid-pillage-prizes")
        .expect("the pillage half has its own row")
        .enable;
    enable(&mut ai);
    assert!(ai.raid_pillage_prizes);
    ai.disable_raid_pillage_prizes();
    assert!(!ai.raid_pillage_prizes);
    assert!(crate::ai::gene("opportunistic-war")
        .expect("registered gene")
        .production());
    assert!(!crate::ai::gene("raid-pillage-prizes")
        .expect("registered gene")
        .production());
}

/// An unescorted enemy Settler two tiles from our warrior is a prize worth a
/// war: the gene declares by surprise, the warrior walks onto the Settler the
/// same turn, and the Settler is ours.
#[test]
fn an_unescorted_settler_in_reach_opens_a_surprise_war_and_is_taken_that_turn() {
    let (mut game, warrior, settler) = raid_settler_board();
    let mut ai = AdvancedAi::new();
    ai.enable_opportunistic_war();
    let opportunity = ai
        .raid_opportunity(&game, 0)
        .expect("an unescorted Settler in reach is an opportunity");
    assert_eq!(opportunity.target, 1);
    assert!(opportunity.value >= RAID_WAR_MIN_VALUE);
    assert!(matches!(
        opportunity.prizes.as_slice(),
        [opportunistic_war::RaidPrize::Settler { .. }]
    ));
    // Without the gene the same board is not an opportunity at all.
    let mut stock = AdvancedAi::new();
    stock.disable_opportunistic_war();
    assert!(stock.raid_opportunity(&game, 0).is_none());

    assert!(!game.is_at_war(0, 1));
    ai.take_turn(&mut game, 0);
    assert!(game.is_at_war(0, 1), "the raid declares war");
    let raid = ai.raid_war.clone().expect("the raid is recorded");
    assert_eq!((raid.target, raid.declared, raid.settlers), (1, 30, 1));
    assert_eq!(game.players[0].counters.get("raid_wars"), Some(&1));
    assert_eq!(
        game.units[&settler].owner, 0,
        "the warrior walked onto the Settler this turn"
    );
    assert_eq!(game.units[&warrior].pos, game.units[&settler].pos);
    assert_eq!(game.players[0].counters.get("captured:settler"), Some(&1));
}

/// The prize is taken by movement, but the answer to the declaration is an
/// army: a neighbour past `RAID_POWER_RATIO` of our strength is not raided.
#[test]
fn a_stronger_neighbour_is_not_raided_for_a_settler() {
    let (mut game, _, _) = raid_settler_board();
    let (_, _, theirs) = raid_fixture();
    let mut ai = AdvancedAi::new();
    ai.enable_opportunistic_war();
    assert!(ai.raid_opportunity(&game, 0).is_some());
    for _ in 0..3 {
        let post = raid_tile_at(&game, theirs, 1, theirs, 1);
        game.spawn_test_unit("swordsman", 1, post);
    }
    assert!(game.military_power(1) > game.military_power(0) * RAID_POWER_RATIO + RAID_POWER_MARGIN);
    assert!(ai.raid_opportunity(&game, 0).is_none());
}

/// A Settler we could not use — one of our own already walking, or no site
/// left to found — is not a prize; the capture guard would refuse the tile.
#[test]
fn a_settler_we_could_not_use_is_no_raid_prize() {
    let (mut game, _, _) = raid_settler_board();
    let mut ai = AdvancedAi::new();
    ai.enable_opportunistic_war();
    assert!(ai.raid_opportunity(&game, 0).is_some());
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    game.spawn_test_unit("settler", 0, home);
    assert!(ai.raid_opportunity(&game, 0).is_none());
}

/// The gene is off in production, and an inactive surge must be invisible:
/// no appointment, no research goal, and no change to a single production
/// score. This is the property every opt-in row claims and the one a screen
/// cannot recover if it is false.
#[test]
fn an_unopted_air_surge_appoints_nothing_and_prices_nothing() {
    let (mut game, _, _) = air_surge_fixture(941_100);
    let mut ai = AdvancedAi::new();
    assert!(!ai.air_surge, "the gene ships off");

    ai.maintain_air_surge(&game, 0);
    assert!(ai.air_surge_plan.is_none());
    assert!(!ai.air_surge_active());
    assert!(ai.air_surge_research_goal(&game, 0).is_none());
    assert_eq!(
        ai.air_surge_production_value(
            &game,
            0,
            &Item::Unit {
                unit: crate::name!("bomber"),
            },
            1.0,
        ),
        None
    );
    assert!(!ai.air_surge_production(&mut game, 0));

    // And the twin puts an opted-in seat back exactly.
    let mut opted = air_surge_ai();
    assert!(opted.air_surge);
    opted.disable_air_surge();
    assert!(!opted.air_surge);
}

/// One appointed package at a time. Two would bid for the same idle queues and
/// the same declaration, and the cross-gate has to hold in both directions.
#[test]
fn the_two_appointed_packages_never_run_together() {
    let (mut game, _, objective) = air_surge_fixture(941_110);
    let mut ai = air_surge_ai();
    ai.timed_war = true;

    ai.war_plan = Some(timed_plan(
        &game,
        objective,
        crate::name!("stirrups"),
        crate::name!("knight"),
        None,
        None,
        WarPhase::Mobilize,
    ));
    assert!(
        !ai.air_surge_open(&game, 0),
        "a live melee appointment keeps the queues"
    );

    ai.war_plan = None;
    ai.maintain_air_surge(&game, 0);
    assert!(ai.air_surge_active());
    assert!(
        !ai.may_form_war_plan(&game, 0),
        "a live surge keeps the queues"
    );

    // And with the gene off the cross-gate is an exact no-op: `timed_war`
    // sees the board it always saw.
    let mut melee = AdvancedAi::new();
    melee.timed_war = true;
    let mut treated = AdvancedAi::new();
    treated.timed_war = true;
    treated.disable_air_surge();
    assert_eq!(
        melee.may_form_war_plan(&game, 0),
        treated.may_form_war_plan(&game, 0)
    );
    let _ = &mut game;
}

/// The religious-corps genes are registered, discoverable by name, and
/// reversible. The bare controller starts them off; the deployment ledger
/// currently promotes `religious-defence-scales` and
/// `religious-units-heal-first`. See `advanced/religion.rs`.
#[test]
fn the_religious_corps_genes_are_registered_reversible_opt_ins() {
    let mut ai = AdvancedAi::new();
    for (field, tag) in [
        ("religious_defence_scales", "religious-defence-scales"),
        ("guru_heals_the_corps_2", "guru-heals-the-corps-2"),
        ("religious_units_heal_first", "religious-units-heal-first"),
        (
            "holy_site_where_the_threat_is",
            "holy-site-where-the-threat-is",
        ),
        ("enhancer_for_the_corps", "enhancer-for-the-corps"),
    ] {
        assert!(
            GENES
                .iter()
                .any(|gene| gene.opt_in() && gene.field == field && gene.tag == tag),
            "{tag} must be a registered native opt-in"
        );
        assert!(
            crate::ai::advanced::gene_ledger::screenable(tag),
            "{tag} must be screenable, so the ledger can price it"
        );
        assert!(
            crate::ai::advanced::gene_ledger::ledger_default_on(tag).is_some(),
            "{tag}: the batch rule decides its default from its batch columns"
        );
    }
    assert!(
        !ai.religious_defence_scales,
        "the bare controller starts it off"
    );
    assert!(
        !ai.guru_heals_the_corps_2,
        "the bare controller starts it off"
    );
    assert!(
        !ai.religious_units_heal_first,
        "the bare controller starts it off"
    );
    assert!(
        !ai.holy_site_where_the_threat_is,
        "the bare controller starts it off"
    );
    assert!(
        !ai.enhancer_for_the_corps,
        "the bare controller starts it off"
    );
    ai.enable_religious_defence_scales();
    ai.enable_guru_heals_the_corps_2();
    ai.enable_religious_units_heal_first();
    ai.enable_holy_site_where_the_threat_is();
    ai.enable_enhancer_for_the_corps();
    assert!(ai.religious_defence_scales);
    assert!(ai.guru_heals_the_corps_2);
    assert!(ai.religious_units_heal_first);
    assert!(ai.holy_site_where_the_threat_is);
    assert!(ai.enhancer_for_the_corps);
    ai.disable_religious_defence_scales();
    ai.disable_guru_heals_the_corps_2();
    ai.disable_religious_units_heal_first();
    ai.disable_holy_site_where_the_threat_is();
    ai.disable_enhancer_for_the_corps();
    assert!(!ai.religious_defence_scales);
    assert!(!ai.guru_heals_the_corps_2);
    assert!(!ai.religious_units_heal_first);
    assert!(!ai.holy_site_where_the_threat_is);
    assert!(!ai.enhancer_for_the_corps);
}

/// `religious_defence_scales`: the shipped defensive corps is
/// `(1 + targets.div_ceil(2)).min(2)`, which answers three slipping cities
/// exactly as it answers one. The gene answers one spreader per threatened
/// city, and never fewer than the shipped rule, up to its own ceiling of four.
#[test]
fn the_defensive_corps_answers_the_number_of_cities_only_with_the_gene() {
    let off = AdvancedAi::new();
    let mut on = AdvancedAi::new();
    on.enable_religious_defence_scales();
    // (targets, shipped) pairs straight off the shipped expression.
    for (targets, shipped) in [(0_usize, 0_usize), (1, 2), (2, 2), (3, 2), (5, 2), (9, 2)] {
        assert_eq!(
            off.defensive_missionary_cap(targets, shipped),
            shipped,
            "the gene off is the shipped constant, exactly"
        );
    }
    assert_eq!(on.defensive_missionary_cap(1, 2), 2, "one city: unchanged");
    assert_eq!(on.defensive_missionary_cap(2, 2), 3);
    assert_eq!(on.defensive_missionary_cap(3, 2), 4, "three cities, three…");
    assert_eq!(
        on.defensive_missionary_cap(9, 2),
        4,
        "…and never past the ceiling, whatever the pressure"
    );
    assert_eq!(
        on.defensive_missionary_cap(0, 0),
        1,
        "a caller that asked for none still gets the gene's floor of one"
    );
}

/// One version of the family plays: enabling v2 turns v1 off (the
/// `science_victory_drive_2` convention), the surge machinery runs for
/// either version through `air_surge_enabled`, and a plain seat feels
/// nothing.
#[test]
fn the_v2_surge_replaces_v1_and_a_plain_seat_feels_nothing() {
    let (game, _, _) = air_surge_fixture(941_150);
    let mut ai = air_surge_ai();
    ai.enable_air_surge_2();
    assert!(
        !ai.air_surge && ai.air_surge_2,
        "one version of the family plays"
    );
    assert!(ai.air_surge_enabled());

    ai.maintain_air_surge(&game, 0);
    assert!(
        ai.air_surge_active(),
        "the surge machinery appoints under version two"
    );

    // The twin stands the whole family down.
    ai.disable_air_surge_2();
    assert!(!ai.air_surge_enabled());
    ai.maintain_air_surge(&game, 0);
    assert!(!ai.air_surge_active());

    // Defaults: off for the stock and frozen controllers.
    assert!(!AdvancedAi::new().air_surge_2);
    assert!(!AdvancedAi::legacy().air_surge_2);
}

/// The Formal-War clock runs through the buildout. A lane seat assesses no
/// rival, so `advanced_diplomacy` needs the surge to hand it a target while
/// the wing arms — v1 never does, v2 does, and a counter appointed into a
/// running war still borrows no decision it was not given.
#[test]
fn the_buildout_denounce_needs_no_assessed_target() {
    let (game, _, _) = air_surge_fixture(941_152);

    let mut v1 = air_surge_ai();
    v1.maintain_air_surge(&game, 0);
    assert!(v1.air_surge_active());
    assert_eq!(
        v1.air_surge_diplomacy_target(),
        None,
        "version one leaves the diplomacy pass exactly as it was"
    );

    let mut v2 = AdvancedAi::new();
    v2.enable_air_surge_2();
    v2.maintain_air_surge(&game, 0);
    let surge_target = v2.air_surge_plan.as_ref().expect("appointed").target_player;
    assert_eq!(v2.air_surge_diplomacy_target(), Some(surge_target));

    // Appointed into a war already running there is nothing to open.
    v2.air_surge_plan.as_mut().expect("appointed").opened_at_war = true;
    assert_eq!(v2.air_surge_diplomacy_target(), None);

    // And with no appointment at all, nothing is offered.
    v2.air_surge_plan = None;
    assert_eq!(v2.air_surge_diplomacy_target(), None);
}

/// The loop's second surge is nearly free, and only v2 knows it: with the
/// airfield standing and a Bomber alive, v1 re-prices the whole package from
/// scratch and the endgame clock refuses a follow-up that v2 admits.
#[test]
fn a_standing_wing_makes_the_follow_up_surge_affordable() {
    let (mut game, _, _) = air_surge_fixture(941_151);
    air_surge_arm(&mut game);

    let v1 = air_surge_ai();
    let mut v2 = AdvancedAi::new();
    v2.enable_air_surge_2();

    // With nothing standing the two versions price the launch identically.
    assert_eq!(
        v1.air_surge_launch_estimate(&game, 0),
        v2.air_surge_launch_estimate(&game, 0),
        "the first appointment costs the same under either version"
    );

    // The airfield stands and one Bomber flies.
    let first = game.player_city_ids(0)[0];
    install_ai_test_district(&mut game, first, "aerodrome");
    game.spawn_test_unit("bomber", 0, game.cities[&first].pos);
    let (v1_research, v1_production) = v1.air_surge_launch_estimate(&game, 0);
    let (v2_research, v2_production) = v2.air_surge_launch_estimate(&game, 0);
    assert_eq!(v1_research, v2_research, "the research half is untouched");
    assert!(
        v2_production < v1_production,
        "v2 prices only the missing package: {v2_production} vs {v1_production}"
    );

    // Squeeze the clock between the two estimates: the follow-up fits only
    // when the standing package is subtracted.
    let reserve = game.standard_duration(air_surge::AIR_SURGE_ENDGAME_RESERVE);
    game.max_turns = game.turn + v2_research + v2_production + reserve + 1;
    assert!(v2.air_surge_affordable(&game, 0));
    assert!(
        !v1.air_surge_affordable(&game, 0),
        "version one still prices a whole new wing and refuses the follow-up"
    );
}

/// Conquest keeps the labs: under version two a Conquest plan prices the
/// Campus (below the Encampment, so war infrastructure keeps first claim),
/// and under version one it does not.
#[test]
fn conquest_prices_the_campus_only_under_v2() {
    let mut game = Game::new(2, 32, 24, 5_414, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    game.players[0].techs.insert(crate::name!("writing"));

    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    let campus = Item::District {
        district: crate::name!("campus"),
        pos: game.cities[&city].pos,
    };

    let mut looped = AdvancedAi::new();
    looped.enable_air_surge_2();
    looped.refresh_research_weight(&game);
    let mut plain = AdvancedAi::new();
    plain.refresh_research_weight(&game);
    let counts = looped.counts(&game, 0);

    let with_loop = looped.production_value(&game, 0, city, &campus, &plan, &counts);
    let without = plain.production_value(&game, 0, city, &campus, &plan, &counts);
    assert!(
        with_loop > without,
        "the loop's Conquest window still pays for science: {with_loop} vs {without}"
    );
}

/// The gene is registered, discoverable by name, and reversible.
#[test]
fn district_planning_is_a_registered_reversible_opt_in() {
    assert!(GENES.iter().any(|gene| gene.opt_in()
        && gene.field == "district_planning"
        && gene.tag == "district-planning"));
    let mut ai = AdvancedAi::new();
    assert!(!ai.district_planning, "production ships it off");
    ai.enable_district_planning();
    assert!(ai.district_planning);
    ai.disable_district_planning();
    assert!(!ai.district_planning);
}

/// Version 3 is an independent screenable arm. Every district-planning
/// version carries the shared site/menu planner, but only one can own the
/// purchase policy at a time.
#[test]
fn district_planning_3_is_a_native_opt_in_off_and_versions_are_exclusive() {
    assert!(!AdvancedAi::new().district_planning_3);
    assert!(!AdvancedAi::legacy().district_planning_3);
    let gene = GENES
        .iter()
        .find(|gene| gene.tag == "district-planning-3")
        .expect("version 3 is published for gene_screen");
    assert!(gene.opt_in() && gene.screenable() && !gene.live());

    let mut ai = AdvancedAi::new();
    ai.enable_district_planning();
    assert!(ai.district_planning);
    ai.enable_district_planning_3();
    assert!(!ai.district_planning);
    assert!(ai.district_planning_3 && ai.district_planning_on());
    ai.enable_district_planning();
    assert!(ai.district_planning && !ai.district_planning_3);
}

/// `wonder-score-tally` is a native opt-in, off in both controllers, with a
/// published row and two working toggles.
#[test]
fn wonder_score_tally_is_a_native_opt_in_off_in_both_controllers() {
    assert!(!AdvancedAi::new().wonder_score_tally);
    assert!(!AdvancedAi::legacy().wonder_score_tally);
    let gene = GENES
        .iter()
        .find(|gene| gene.tag == "wonder-score-tally")
        .expect("the gene is published for gene_screen");
    assert!(gene.opt_in() && gene.screenable() && !gene.live());
    let mut ai = AdvancedAi::new();
    (gene.enable)(&mut ai);
    assert!(ai.wonder_score_tally);
    (gene.disable)(&mut ai);
    assert!(!ai.wonder_score_tally);
}

/// ★★★★ THE ACTUATION REPAIR, AND THE HALF OF THE SENTINEL IT KEEPS.
///
/// With the gene off, a Roman seat with no victory target and an Expansion
/// plan takes the `-10_000` refusal on a cheap wonder — `lane_opens` is a
/// civilization check it cannot pass. With the gene on, the same wonder is
/// priced and reachable. And the density bar is real: the 1 850-production
/// Sydney Opera House is refused in the same city on the same turn, because
/// fifteen points do not pay for it.
#[test]
fn wonder_score_tally_opens_the_lane_on_merit_and_the_density_bar_still_refuses() {
    let (game, city) = wonder_lane_capital(7_742);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 5,
        assessed_turn: game.turn,
        rush: false,
    };
    let position = game.cities[&city].pos;
    let cheap = Item::Wonder {
        wonder: crate::name!("great_bath"),
        pos: position,
    };
    let dear = Item::Wonder {
        wonder: crate::name!("sydney_opera_house"),
        pos: position,
    };

    let shipped = AdvancedAi::new();
    let counts = shipped.counts(&game, 0);
    let refused = shipped.production_value(&game, 0, city, &cheap, &plan, &counts);
    assert!(
        refused <= -9_999.0,
        "the defect: a Roman Expansion seat is refused every wonder ({refused})"
    );

    let mut treated = AdvancedAi::new();
    treated.enable_wonder_score_tally();
    let counts = treated.counts(&game, 0);
    let opened = treated.production_value(&game, 0, city, &cheap, &plan, &counts);
    assert!(
        opened > 0.0,
        "the repair: the same wonder is reachable on merit ({opened})"
    );

    let over_bar = treated.production_value(&game, 0, city, &dear, &plan, &counts);
    assert!(
        over_bar <= -9_999.0,
        "the density bar must still refuse an 1 850-production wonder ({over_bar})"
    );

    // The bar is arithmetic, not a name: the wonder clears it exactly when its
    // ordinary value plus the tally price covers its cost at the bar's rate.
    let spec = &game.rules.wonders[&crate::name!("great_bath")];
    let ordinary = treated.wonder_ordinary_value(spec, plan.strategy);
    assert!(
        ordinary + AdvancedAi::wonder_tally_value() >= spec.cost * WONDER_TALLY_MIN_DENSITY,
        "the Great Bath clears the bar it is asserted to clear"
    );
    let dear_spec = &game.rules.wonders[&crate::name!("sydney_opera_house")];
    assert!(
        treated.wonder_ordinary_value(dear_spec, plan.strategy) + AdvancedAi::wonder_tally_value()
            < dear_spec.cost * WONDER_TALLY_MIN_DENSITY,
        "and the Opera House fails it"
    );
}

#[test]
fn the_missionary_field_genes_are_registered_reversible_opt_ins() {
    for (field, tag) in [
        (
            "missionary_last_charge_explores",
            "missionary-last-charge-explores",
        ),
        (
            "missionary_last_charge_explores_2",
            "missionary-last-charge-explores-2",
        ),
        ("missionary_evades_raiders", "missionary-evades-raiders"),
    ] {
        assert!(
            GENES
                .iter()
                .any(|gene| gene.opt_in() && gene.field == field && gene.tag == tag),
            "{tag} must be a registered native opt-in"
        );
        assert!(
            crate::ai::advanced::gene_ledger::screenable(tag),
            "{tag} must be screenable, so the ledger can price it"
        );
        assert!(
            crate::ai::advanced::gene_ledger::ledger_default_on(tag).is_some(),
            "{tag}: the batch rule decides its default from its batch columns"
        );
    }
    let mut ai = AdvancedAi::new();
    assert!(
        !ai.missionary_last_charge_explores
            && !ai.missionary_last_charge_explores_2
            && !ai.missionary_evades_raiders
    );
    ai.enable_missionary_last_charge_explores();
    assert!(ai.missionary_last_charge_explores);
    assert!(!ai.missionary_last_charge_explores_2);
    ai.enable_missionary_last_charge_explores_2();
    assert!(!ai.missionary_last_charge_explores);
    assert!(ai.missionary_last_charge_explores_2);
    ai.enable_missionary_evades_raiders();
    assert!(ai.missionary_last_charge_explores_2 && ai.missionary_evades_raiders);
    ai.disable_missionary_last_charge_explores_2();
    ai.disable_missionary_evades_raiders();
    assert!(
        !ai.missionary_last_charge_explores
            && !ai.missionary_last_charge_explores_2
            && !ai.missionary_evades_raiders
    );
    let legacy = AdvancedAi::legacy();
    assert!(
        !legacy.missionary_last_charge_explores
            && !legacy.missionary_last_charge_explores_2
            && !legacy.missionary_evades_raiders,
        "the frozen anchor plays the game it always did"
    );
}

/// The barbarian religious hunt is a controller treatment of the world, not
/// a gene of one seat: on in every current controller, off on the anchor.
#[test]
fn the_barbarian_heretic_hunt_cannot_reach_the_frozen_anchor() {
    assert!(
        !AdvancedAi::legacy().barbarian_heretic_hunt(),
        "the frozen anchor must not hunt heretics"
    );
    assert!(AdvancedAi::new().barbarian_heretic_hunt());
}

#[test]
fn the_religious_veto_defence_gene_is_a_registered_reversible_opt_in() {
    let tag = "religious-veto-defence";
    assert!(
        GENES
            .iter()
            .any(|gene| gene.opt_in() && gene.field == "religious_veto_defence" && gene.tag == tag),
        "{tag} must be a registered native opt-in"
    );
    assert!(crate::ai::advanced::gene_ledger::screenable(tag));
    let deployed = crate::ai::advanced::gene_ledger::ledger_default_on(tag)
        .expect("the registered opt-in has a ledger default");
    let mut deployed_ai = AdvancedAi::new();
    deployed_ai.enable_live_bridge();
    assert_eq!(
        deployed_ai.religious_veto_defence, deployed,
        "the live bridge applies the current latest-three-batch selection"
    );
    let mut ai = AdvancedAi::new();
    assert!(
        !ai.religious_veto_defence,
        "the unconfigured controller keeps opt-ins off until the live bridge applies them"
    );
    ai.enable_religious_veto_defence();
    assert!(ai.religious_veto_defence);
    ai.disable_religious_veto_defence();
    assert!(!ai.religious_veto_defence);
    assert!(
        !AdvancedAi::legacy().religious_veto_defence,
        "the frozen anchor plays the game it always did"
    );
}

#[test]
fn the_trade_deal_genes_are_registered_reversible_opt_ins() {
    for (field, tag) in [
        ("deals_for_our_gain", "deals-for-our-gain"),
        ("deals_at_the_ceiling", "deals-at-the-ceiling"),
        ("no_free_passage", "no-free-passage"),
    ] {
        assert!(
            GENES
                .iter()
                .any(|gene| gene.opt_in() && gene.field == field && gene.tag == tag),
            "{tag} must be a registered native opt-in"
        );
        assert!(crate::ai::advanced::gene_ledger::screenable(tag));
        assert!(
            crate::ai::advanced::gene_ledger::ledger_default_on(tag).is_some(),
            "{tag}: the batch rule decides its default from its batch columns"
        );
    }
    let mut ai = AdvancedAi::new();
    assert!(
        !ai.base.deals_for_our_gain && !ai.base.deals_at_the_ceiling && !ai.base.no_free_passage
    );
    ai.enable_deals_for_our_gain();
    ai.enable_deals_at_the_ceiling();
    ai.enable_no_free_passage();
    assert!(ai.base.deals_for_our_gain && ai.base.deals_at_the_ceiling && ai.base.no_free_passage);
    ai.disable_deals_for_our_gain();
    ai.disable_deals_at_the_ceiling();
    ai.disable_no_free_passage();
    assert!(
        !ai.base.deals_for_our_gain && !ai.base.deals_at_the_ceiling && !ai.base.no_free_passage
    );
    let legacy = AdvancedAi::legacy();
    assert!(
        !legacy.base.deals_for_our_gain
            && !legacy.base.deals_at_the_ceiling
            && !legacy.base.no_free_passage,
        "the frozen anchor plays the game it always did"
    );
}

#[test]
fn a_quote_is_chosen_by_our_gain_only_with_the_gene() {
    use crate::game::{DealItems, QuickDeal};
    let quote = |my_value: f64, partner_value: f64| QuickDeal {
        partner: 1,
        category: "resource".to_string(),
        item: "silk".to_string(),
        direction: "sell".to_string(),
        offer: DealItems::default(),
        request: DealItems::default(),
        my_value,
        partner_value,
    };
    // The lopsided quote is worth more to us; the balanced one is fairer.
    let lopsided = quote(10.0, 3.0);
    let balanced = quote(6.0, 6.0);
    let off = AdvancedAi::new();
    assert!(
        off.base.deal_objective(&balanced) > off.base.deal_objective(&lopsided),
        "shipped: the most balanced exchange wins"
    );
    let mut on = AdvancedAi::new();
    on.enable_deals_for_our_gain();
    assert!(
        on.base.deal_objective(&lopsided) > on.base.deal_objective(&balanced),
        "the gene: the exchange worth most to us wins"
    );
    assert_eq!(on.base.deal_objective(&lopsided), 10.0);
}

#[test]
fn a_sale_is_priced_at_the_counterpartys_ceiling_only_with_the_gene() {
    let game = deal_board();
    let quotes = game.quick_deals(0);
    let sale = quotes
        .iter()
        .find(|deal| deal.direction == "sell" && deal.item == "silk")
        .cloned()
        .expect("fixture: a luxury to sell");
    let purchase = quotes
        .iter()
        .find(|deal| deal.direction == "buy" && deal.item == "wine")
        .cloned()
        .expect("fixture: a luxury to buy");

    let off = AdvancedAi::new();
    assert_eq!(
        off.base.deal_at_the_ceiling(&game, 0, &sale),
        None,
        "the gene off leaves the midpoint quote alone"
    );

    let mut on = AdvancedAi::new();
    on.enable_deals_at_the_ceiling();
    let sharp = on
        .base
        .deal_at_the_ceiling(&game, 0, &sale)
        .expect("a midpoint quote has surplus to move");
    assert!(
        sharp.request.gold > sale.request.gold,
        "the sale asks for more: {} over {}",
        sharp.request.gold,
        sale.request.gold
    );
    assert!(sharp.my_value > sale.my_value);
    let (_, theirs) = game.trade_utilities(0, 1, &sharp.offer, &sharp.request);
    assert!(
        theirs > 0.25 && theirs <= sale.partner_value,
        "the counterparty still gains, by the margin: {theirs}"
    );
    let cheaper = on
        .base
        .deal_at_the_ceiling(&game, 0, &purchase)
        .expect("a purchase has surplus to keep");
    assert!(
        cheaper.offer.gold < purchase.offer.gold,
        "the purchase pays less: {} under {}",
        cheaper.offer.gold,
        purchase.offer.gold
    );

    // And the sharpened sale closes: the engine accepts it and the Gold
    // lands, more of it than the midpoint would have brought.
    let mut closed = game.clone();
    let before = closed.players[0].gold;
    assert!(on.base.close_quick_deal(&mut closed, 0, sale.clone()));
    let banked = closed.players[0].gold - before;
    assert!(
        banked >= sharp.request.gold - 1e-6 && banked > sale.request.gold,
        "banked {banked} against the sharpened {} and the midpoint {}",
        sharp.request.gold,
        sale.request.gold
    );
    // The shipped close banks the midpoint (and the first instalment of any
    // Gold-per-turn rider `do_trade` settles at signing), which is less.
    let mut shipped = game.clone();
    let before = shipped.players[0].gold;
    assert!(off.base.close_quick_deal(&mut shipped, 0, sale.clone()));
    let midpoint = shipped.players[0].gold - before;
    assert!(
        midpoint >= sale.request.gold - 1e-6 && midpoint < banked,
        "the midpoint banks {midpoint}, the ceiling {banked}"
    );
}

#[test]
fn a_friendship_ask_carries_no_passage_only_with_the_gene() {
    let proposal = |gene: bool| -> bool {
        let mut game = deal_board();
        game.turn = 20;
        let mut ai = AdvancedAi::new();
        if gene {
            ai.enable_no_free_passage();
        }
        ai.base.diplomacy(&mut game, 0);
        game.pending_deals
            .iter()
            .find(|deal| deal.from == 0 && deal.to == 1 && deal.friendship)
            .map(|deal| deal.open_borders)
            .expect("fixture: the friendship ask is made at turn 20")
    };
    assert!(
        proposal(false),
        "shipped: passage rides on the friendship ask"
    );
    assert!(!proposal(true), "the gene: the ask carries no passage");
}

#[test]
fn one_war_at_a_time_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "one_war_at_a_time"
            && gene.tag == "one-war-at-a-time"),
        "one-war-at-a-time must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("one-war-at-a-time"),
        "the ledger must be able to price it"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("one-war-at-a-time").is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.one_war_at_a_time && ai.one_war.is_none());
    ai.enable_one_war_at_a_time();
    assert!(ai.one_war_at_a_time);
    ai.disable_one_war_at_a_time();
    assert!(!ai.one_war_at_a_time);
    assert!(
        !AdvancedAi::legacy().one_war_at_a_time,
        "the frozen anchor plays the game it always did"
    );
}

/// `victory_focus` is unchanged by the refactor that exposed the lane table:
/// the civilization preferences stay in the focus and out of the table.
#[test]
fn lane_progress_table_matches_victory_focus() {
    let ai = AdvancedAi::new();
    let mut religion = Game::new(2, 24, 16, 74, 80, 0);
    religion.players[0].religion = Some("Test Faith".to_string());
    let table = ai.lane_progress_table(&religion, 0);
    assert_eq!(
        table[2], 40,
        "a founder with no foreign convert stands at 40"
    );
    let focus = ai.victory_focus(&religion, 0);
    assert_eq!(focus.strategy, GrandStrategy::Religion);
    assert_eq!(focus.progress, table[2]);

    let mut china = Game::new(2, 24, 16, 77, 80, 0);
    china.players[0].civ = "China".to_string();
    let table = ai.lane_progress_table(&china, 0);
    assert!(
        table[0] < 45,
        "the table carries no civilization preference"
    );
    assert_eq!(ai.victory_focus(&china, 0).progress, 45);
}

/// Two advanced seats through a whole short game: quotes are closed, and not
/// one of them was a gift or a demand. `gifts_given` is the engine's own
/// count, so this is a check and not a claim.
#[test]
fn the_ai_never_gives_without_receiving() {
    let mut game = Game::new_full(2, 30, 20, 84_211, 150, 0, false);
    game.record_contact(0, 1);
    // One controller per seat: the dormant Free Cities seat takes turns from
    // the first Loyalty revolt onward.
    let mut ais: Vec<AdvancedAi> = (0..game.players.len()).map(|_| AdvancedAi::new()).collect();
    ais[0].enable_deals_at_the_ceiling();
    ais[0].enable_deals_for_our_gain();
    run_game(&mut game, &mut ais);
    for pid in 0..2 {
        assert_eq!(
            game.players[pid].counters.get("gifts_given"),
            None,
            "seat {pid} gave something for nothing"
        );
    }
    // And the quote lane refuses a gift or a demand on its own, whatever
    // the engine would accept.
    use crate::game::{DealItems, QuickDeal};
    let mut board = deal_board();
    let gift = QuickDeal {
        partner: 1,
        category: "resource".to_string(),
        item: "silk".to_string(),
        direction: "sell".to_string(),
        offer: DealItems {
            gold: 25.0,
            ..DealItems::default()
        },
        request: DealItems::default(),
        my_value: 1.0,
        partner_value: 25.0,
    };
    let ai = AdvancedAi::new();
    assert!(!ai.base.close_quick_deal(&mut board, 0, gift));
    assert_eq!(board.players[0].counters.get("gifts_given"), None);
}

#[test]
fn diplomatic_lane_forecast_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "diplomatic_lane_forecast"
            && gene.tag == "diplomatic-lane-forecast"),
        "diplomatic-lane-forecast must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("diplomatic-lane-forecast"),
        "the ledger must be able to price it"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("diplomatic-lane-forecast").is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.diplomatic_lane_forecast, "off in the stock agent");
    assert!(
        !AdvancedAi::legacy().diplomatic_lane_forecast,
        "off in the legacy agent"
    );
    ai.enable_diplomatic_lane_forecast();
    assert!(ai.diplomatic_lane_forecast);
    ai.disable_diplomatic_lane_forecast();
    assert!(!ai.diplomatic_lane_forecast, "reversible");
}

/// The lock the gene exists to open: with nothing banked the shipped tally
/// reads the Diplomacy lane at zero, so the argmax in `victory_focus` can
/// never land on it however many Congresses are still to sit.
#[test]
fn the_shipped_diplomacy_lane_reads_zero_until_it_has_already_been_played() {
    let game = Game::new(6, 40, 26, 5_101, 250, 9);
    let shipped = AdvancedAi::new();
    let table = shipped.lane_progress_table(&game, 0);
    assert_eq!(table[3], 0, "no points, no suzerainties, no reading");
    assert_ne!(
        shipped.victory_focus(&game, 0).strategy,
        GrandStrategy::Diplomacy,
        "and so the lane cannot be chosen"
    );
    // The one key cut for this lock is unreachable: `diplomatic_opening` has
    // no registry row, so nothing in the tree can set it.
    assert!(
        !GENES.iter().any(|gene| gene.field == "diplomatic_opening"),
        "diplomatic_opening is an orphaned flag, not a gene"
    );
}

/// The forecast reads the Congress calendar: an empire holding the floor is
/// told the lane closes, and the same empire with one Congress left is not.
#[test]
fn diplomatic_lane_forecast_prices_the_calendar_not_the_tally() {
    let mut ai = AdvancedAi::new();
    ai.enable_diplomatic_lane_forecast();

    // Six majors, nine city-states, the ladder's own board. Give seat 0 the
    // Favor floor and leave its point tally empty.
    let mut game = Game::new(6, 40, 26, 5_102, 250, 9);
    game.turn = 60;
    game.players[0].diplomatic_favor = 900.0;
    assert_eq!(game.players[0].dvp, 0, "nothing banked");

    let early = ai.lane_progress_table(&game, 0)[3];
    assert!(
        early > 0,
        "twelve sessions and the floor: the lane has a reading ({early})"
    );
    assert_eq!(
        AdvancedAi::new().lane_progress_table(&game, 0)[3],
        0,
        "and the shipped agent still reads nothing there"
    );

    // Wind the clock to the last session. The same floor, the same empty
    // tally, and now the calendar cannot pay twenty points.
    let mut late = game.clone();
    late.turn = 240;
    let late_reading = ai.lane_progress_table(&late, 0)[3];
    assert!(
        late_reading < early,
        "one session left must read below twelve ({late_reading} vs {early})"
    );

    // Past the clock there is no session at all.
    let mut over = game.clone();
    over.turn = 250;
    assert_eq!(
        ai.lane_progress_table(&over, 0)[3],
        0,
        "no sessions left, no forecast"
    );
}

/// The forecast only ever RAISES the lane's reading, so the gene cannot make
/// the planner abandon a Diplomacy race it is already winning.
#[test]
fn diplomatic_lane_forecast_never_lowers_the_banked_reading() {
    let mut ai = AdvancedAi::new();
    ai.enable_diplomatic_lane_forecast();
    let shipped = AdvancedAi::new();
    for (seed, turn, dvp) in [(6_001u64, 40u32, 0i64), (6_002, 150, 9), (6_003, 240, 18)] {
        let mut game = Game::new(6, 40, 26, seed, 250, 9);
        game.turn = turn;
        game.players[0].dvp = dvp;
        let with = ai.lane_progress_table(&game, 0)[3];
        let without = shipped.lane_progress_table(&game, 0)[3];
        assert!(
            with >= without,
            "seed {seed}: the forecast lowered the reading ({with} < {without})"
        );
        assert!(
            (0..=100).contains(&with),
            "seed {seed}: {with} out of range"
        );
    }
}

/// A seat with no Congress position at all must still read BELOW Religion's
/// standing commitment floor, or the gene is the flat floor it replaces.
#[test]
fn diplomatic_lane_forecast_still_has_to_be_earned() {
    let mut ai = AdvancedAi::new();
    ai.enable_diplomatic_lane_forecast();
    let mut game = Game::new(6, 40, 26, 5_103, 250, 9);
    game.turn = 30;
    let barren = ai.lane_progress_table(&game, 0)[3];
    assert!(
        barren < 46,
        "an empire with nothing must not out-read a viable Religion ({barren})"
    );

    // Give the same seat the whole Favor floor and it does clear that bar.
    game.players[0].diplomatic_favor = 2_000.0;
    let armed = ai.lane_progress_table(&game, 0)[3];
    assert!(
        armed > barren,
        "the floor has to move the reading ({armed} vs {barren})"
    );
}

/// V2 keeps the calendar arithmetic, but a bank of Favor by itself is no
/// longer permission to switch the whole plan to Diplomacy. A point actually
/// earned from Congress or a city-state already under our suzerainty is the
/// concrete foothold that turns the projection back on, and its speculative
/// reading stays below realised lane progress.
#[test]
fn diplomatic_lane_forecast_two_requires_earned_traction_and_caps_a_projection() {
    let mut v1 = AdvancedAi::new();
    v1.enable_diplomatic_lane_forecast();
    let mut v2 = AdvancedAi::new();
    v2.enable_diplomatic_lane_forecast_2();
    let shipped = AdvancedAi::new();
    let mut game = Game::new(6, 40, 26, 5_104, 250, 9);
    game.turn = 60;
    game.players[0].diplomatic_favor = 2_000.0;

    let speculative = v1.lane_progress_table(&game, 0)[3];
    assert!(speculative > 0, "v1 sees the large Favor balance");
    assert_eq!(
        v2.lane_progress_table(&game, 0)[3],
        shipped.lane_progress_table(&game, 0)[3],
        "v2 does not treat Favor alone as a diplomatic opening"
    );

    game.players[0].dvp = 1;
    let earned_v1 = v1.lane_progress_table(&game, 0)[3];
    let earned_v2 = v2.lane_progress_table(&game, 0)[3];
    assert!(
        earned_v1 > DIPLOMATIC_FORECAST_OPENING_CAP,
        "fixture makes v1's projection able to outrank a real opening ({earned_v1})"
    );
    assert!(
        earned_v2 > shipped.lane_progress_table(&game, 0)[3]
            && earned_v2 <= DIPLOMATIC_FORECAST_OPENING_CAP,
        "an earned point restores v2 only as a bounded candidate ({earned_v2})"
    );

    let mut suzerain = game.clone();
    suzerain.players[0].dvp = 0;
    let minor = suzerain
        .players
        .iter()
        .find(|player| player.alive && player.is_minor && !player.is_barbarian)
        .expect("fixture has a living city-state")
        .id;
    suzerain.players[0].envoys = vec![(minor, 3)];
    assert_eq!(suzerain.suzerain_of(minor), Some(0));
    let suzerain_v1 = v1.lane_progress_table(&suzerain, 0)[3];
    let suzerain_v2 = v2.lane_progress_table(&suzerain, 0)[3];
    assert!(
        suzerain_v1 > DIPLOMATIC_FORECAST_OPENING_CAP,
        "fixture makes v1's suzerain projection able to outrank a real opening ({suzerain_v1})"
    );
    assert!(
        suzerain_v2 > shipped.lane_progress_table(&suzerain, 0)[3]
            && suzerain_v2 <= DIPLOMATIC_FORECAST_OPENING_CAP,
        "a current suzerainty restores v2 only as a bounded candidate ({suzerain_v2})"
    );

    let mut progressed = game.clone();
    progressed.players[0].dvp = 10;
    assert_eq!(
        v2.lane_progress_table(&progressed, 0)[3],
        50,
        "ten earned DVP outrank the cap through the lane's actual tally"
    );
}

/// `diplomatic-lane-forecast-2` is a distinct screenable version.  Switching
/// versions is exclusive, so no screen arm can accidentally combine v1's
/// speculative opening with v2's foothold gate.
#[test]
fn diplomatic_lane_forecast_two_is_opt_in_and_turns_version_one_off() {
    let v2 = GENES
        .iter()
        .find(|gene| gene.tag == "diplomatic-lane-forecast-2")
        .expect("version 2 is published");
    assert!(v2.opt_in() && v2.screenable() && !v2.live());

    let mut ai = AdvancedAi::new();
    ai.enable_diplomatic_lane_forecast();
    assert_eq!(
        (ai.diplomatic_lane_forecast, ai.diplomatic_lane_forecast_2),
        (true, false),
        "v1 begins as the sole active version"
    );
    ai.enable_diplomatic_lane_forecast_2();
    assert_eq!(
        (ai.diplomatic_lane_forecast, ai.diplomatic_lane_forecast_2),
        (false, true),
        "v2 turns the speculative version off"
    );
    ai.enable_diplomatic_lane_forecast();
    assert_eq!(
        (ai.diplomatic_lane_forecast, ai.diplomatic_lane_forecast_2),
        (true, false),
        "v1 can likewise replace v2"
    );
    ai.enable_diplomatic_lane_forecast_2();
    ai.disable_diplomatic_lane_forecast_2();
    assert_eq!(
        (ai.diplomatic_lane_forecast, ai.diplomatic_lane_forecast_2),
        (false, false),
        "v2 remains independently reversible"
    );
}

#[test]
fn surprise_mobilization_requires_the_exact_declaration_direction_and_expires() {
    let mut game = surprise_mobilization_board(1);
    let mut ai = AdvancedAi::new();
    ai.enable_surprise_war_mobilization();
    let active = ai
        .surprise_defense_window(&game, 0)
        .expect("a new surprise declaration against us opens the response");
    assert_eq!(active.attacker, 1);
    assert_eq!(active.declared, 40);

    game.wars.values_mut().next().unwrap().casus_belli = None;
    assert_eq!(ai.surprise_defense_window(&game, 0), None);
    game.wars.values_mut().next().unwrap().casus_belli = Some("surprise_war".to_string());
    game.wars.values_mut().next().unwrap().target = 1;
    assert_eq!(
        ai.surprise_defense_window(&game, 0),
        None,
        "a war we declared, or an allied front not declared against us, is inert"
    );
    game.wars.values_mut().next().unwrap().target = 0;
    game.turn = active.ends;
    assert_eq!(
        ai.surprise_defense_window(&game, 0),
        None,
        "the declaration shock has a hard time limit"
    );
    assert_eq!(
        AdvancedAi::new().surprise_defense_window(&game, 0),
        None,
        "the production controller ships the gene off"
    );
}

#[test]
fn surprise_mobilization_keeps_settlers_and_finishing_work_but_raises_a_fast_wave() {
    let mut game = surprise_mobilization_board(4);
    let cities = game.player_city_ids(0);
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    let finishing_builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    let granary = Item::Building {
        building: crate::name!("granary"),
    };
    game.cities.get_mut(&cities[0]).unwrap().queue = vec![settler.clone()];
    game.cities.get_mut(&cities[1]).unwrap().queue = vec![finishing_builder.clone()];
    let builder_cost = game.item_cost_for_city(0, cities[1], &finishing_builder);
    let one_turn = game.city_yields(cities[1]).production.max(0.1);
    game.cities.get_mut(&cities[1]).unwrap().production = (builder_cost - one_turn * 0.5).max(0.0);
    game.cities.get_mut(&cities[2]).unwrap().queue = vec![monument.clone()];
    game.cities.get_mut(&cities[3]).unwrap().queue = vec![granary.clone()];

    let original = game.clone();
    assert_eq!(
        AdvancedAi::new().mobilize_surprise_defense_production(&mut game.clone(), 0),
        0,
        "gene off is an exact no-op"
    );

    let mut ai = AdvancedAi::new();
    ai.enable_surprise_war_mobilization();
    assert_eq!(
        ai.mobilize_surprise_defense_production(&mut game, 0),
        2,
        "four cities may redirect only half their queues in the first wave"
    );
    assert_eq!(game.cities[&cities[0]].queue.first(), Some(&settler));
    assert_eq!(
        game.cities[&cities[1]].queue.first(),
        Some(&finishing_builder),
        "work finishing this turn is cheaper to complete than to interrupt"
    );
    for city in [cities[2], cities[3]] {
        let Item::Unit { unit } = game.cities[&city]
            .queue
            .first()
            .expect("the unfinished economic queue becomes a defender")
        else {
            panic!("the declaration response must queue a unit");
        };
        let spec = &game.rules.units[unit];
        assert_eq!(spec.class, "military");
        assert!(!spec.siege);
        assert!(!matches!(spec.domain.as_deref(), Some("sea" | "air")));
    }

    let mut expired = original;
    expired.turn =
        40 + expired.standard_duration(super::super::surprise_defense::SURPRISE_MOBILIZATION_TURNS);
    assert_eq!(
        ai.mobilize_surprise_defense_production(&mut expired, 0),
        0,
        "an old war cannot keep converting queues"
    );
    assert_eq!(expired.cities[&cities[2]].queue.first(), Some(&monument));
    assert_eq!(expired.cities[&cities[3]].queue.first(), Some(&granary));
}

#[test]
fn frontier_massing_alarm_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "frontier_massing_alarm"
            && gene.tag == "frontier-massing-alarm"),
        "frontier-massing-alarm must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("frontier-massing-alarm"),
        "the ledger must be able to price it"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("frontier-massing-alarm").is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.frontier_massing_alarm, "off in the stock agent");
    assert!(
        !AdvancedAi::legacy().frontier_massing_alarm,
        "off in the legacy agent"
    );
    ai.enable_frontier_massing_alarm();
    assert!(ai.frontier_massing_alarm);
    ai.disable_frontier_massing_alarm();
    assert!(!ai.frontier_massing_alarm, "reversible");
}

#[test]
fn conversion_majority_alarm_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "conversion_majority_alarm"
            && gene.tag == "conversion-majority-alarm"),
        "conversion-majority-alarm must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("conversion-majority-alarm"),
        "the ledger must be able to price it"
    );
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "conversion_majority_alarm_2"
            && gene.tag == "conversion-majority-alarm-2"),
        "conversion-majority-alarm-2 must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("conversion-majority-alarm-2"),
        "the ledger must be able to price v2 independently"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("conversion-majority-alarm").is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.conversion_majority_alarm, "off in the stock agent");
    assert!(!ai.conversion_majority_alarm_2, "v2 is opt-in too");
    assert!(
        !AdvancedAi::legacy().conversion_majority_alarm
            && !AdvancedAi::legacy().conversion_majority_alarm_2,
        "both versions are off in the legacy agent"
    );
    ai.enable_conversion_majority_alarm();
    assert!(ai.conversion_majority_alarm);
    assert!(!ai.conversion_majority_alarm_2);
    ai.enable_conversion_majority_alarm_2();
    assert!(!ai.conversion_majority_alarm);
    assert!(ai.conversion_majority_alarm_2);
    ai.enable_conversion_majority_alarm();
    assert!(ai.conversion_majority_alarm);
    assert!(!ai.conversion_majority_alarm_2, "last family version wins");
    ai.disable_conversion_majority_alarm();
    assert!(!ai.conversion_majority_alarm, "reversible");
    ai.enable_conversion_majority_alarm_2();
    ai.disable_conversion_majority_alarm_2();
    assert!(!ai.conversion_majority_alarm_2, "v2 is reversible");
}

/// A four-city board where each rival is one city short of a majority: the
/// staircase reads zero and the count reads two thirds.
#[test]
fn a_rival_one_city_short_everywhere_reads_zero_on_the_staircase() {
    let mut game = Game::new(4, 40, 26, 8_401, 250, 0);
    let faith = "Test Faith".to_string();
    game.players[0].religion = Some(faith.clone());
    // Three rivals with three cities each. A majority is two.
    let mut spot = (10, 10);
    for rival in 1..4 {
        for _ in 0..3 {
            game.found_city_for(rival, spot, None);
            spot = (spot.0 + 3, spot.1);
        }
        spot = (10, spot.1 + 4);
    }
    // One of each rival's three follows our faith: short of every majority.
    for rival in 1..4 {
        let city = game.player_city_ids(rival)[0];
        game.cities
            .get_mut(&city)
            .unwrap()
            .pressure
            .insert(faith.clone(), 500.0);
    }

    let mut ai = AdvancedAi::new();
    ai.enable_conversion_majority_alarm();
    let mut bottleneck = AdvancedAi::new();
    bottleneck.enable_conversion_majority_alarm_2();
    let shipped = AdvancedAi::new();

    let (converted, _) = shipped.religious_conversion_tally(&game, 0);
    assert_eq!(converted, 0, "no rival has lost a majority yet");
    assert_eq!(
        shipped.conversion_majority_pressure(&game, 0),
        0,
        "the gene is off, so it contributes nothing"
    );
    // Three rivals, majority two each: six cities required, three held.
    assert_eq!(
        ai.conversion_majority_pressure(&game, 0),
        50,
        "half the cities the victory asks for are already converted"
    );
    assert_eq!(
        bottleneck.conversion_majority_pressure(&game, 0),
        50,
        "equal-sized civilizations preserve version one's smooth reading"
    );
    assert!(
        ai.rival_victory_pressure(&game, 0).progress
            > shipped.rival_victory_pressure(&game, 0).progress,
        "and the alarm reads it while the staircase still reads nothing"
    );
}

/// Version one pools every required city. That lets one enormous converted
/// empire outweigh four untouched holdouts even though Religion must take all
/// five civilizations. Version two reads the furthest remaining holdout.
#[test]
fn conversion_majority_alarm_2_does_not_let_one_wide_empire_outvote_four_holdouts() {
    let mut game = Game::new(6, 96, 52, 8_403, 250, 0);
    let faith = "Wide Faith".to_string();
    game.players[0].religion = Some(faith.clone());

    // Seventeen cities make rival 1's majority nine. The other four rivals
    // own one city each, so version one's pooled clock reads 9 / 13 = 69%.
    for index in 0..17 {
        game.found_city_for(1, (4 + 4 * (index % 9), 4 + 5 * (index / 9)), None);
    }
    for rival in 2..6 {
        game.found_city_for(rival, (8 + 12 * (rival as i32 - 2), 30), None);
    }
    for city in game.player_city_ids(1).into_iter().take(9) {
        game.cities
            .get_mut(&city)
            .unwrap()
            .pressure
            .insert(faith.clone(), 500.0);
    }

    let mut pooled = AdvancedAi::new();
    pooled.enable_conversion_majority_alarm();
    let mut bottleneck = AdvancedAi::new();
    bottleneck.enable_conversion_majority_alarm_2();

    assert_eq!(pooled.conversion_majority_pressure(&game, 0), 69);
    assert_eq!(
        bottleneck.conversion_majority_pressure(&game, 0),
        0,
        "an untouched civilization is still the bottleneck"
    );
    let pooled_pressure = pooled.rival_victory_pressure(&game, 0);
    let bottleneck_pressure = bottleneck.rival_victory_pressure(&game, 0);
    assert_eq!(pooled_pressure.strategy, GrandStrategy::Religion);
    assert_eq!(pooled_pressure.progress, 69);
    assert_eq!(bottleneck_pressure.strategy, GrandStrategy::Religion);
    assert_eq!(bottleneck_pressure.progress, 16);
    assert_eq!(
        pooled.denial_response_for_pressure(&game, 2, 0, 0, pooled_pressure),
        Some(GrandStrategy::Conquest),
        "the pooled clock starts a war against one converted empire"
    );
    assert_eq!(
        bottleneck.denial_response_for_pressure(&game, 2, 0, 0, bottleneck_pressure),
        None,
        "four untouched civilizations keep the bottleneck clock below the war bar"
    );
}

/// It can only raise: a founder whose staircase already reads higher keeps its
/// number, and a seat with no religion still reads nothing.
#[test]
fn the_conversion_count_never_lowers_the_staircase() {
    let mut ai = AdvancedAi::new();
    ai.enable_conversion_majority_alarm();
    let shipped = AdvancedAi::new();

    let mut game = Game::new(4, 40, 26, 8_402, 250, 0);
    assert_eq!(
        ai.conversion_majority_pressure(&game, 0),
        0,
        "a seat with no religion has no religious clock"
    );

    let faith = "Test Faith".to_string();
    game.players[0].religion = Some(faith.clone());
    let mut spot = (10, 10);
    for rival in 1..4 {
        for _ in 0..2 {
            game.found_city_for(rival, spot, None);
            spot = (spot.0 + 3, spot.1);
        }
        spot = (10, spot.1 + 4);
    }
    // Convert every city of two rivals and none of the third.
    for rival in 1..3 {
        for city in game.player_city_ids(rival) {
            game.cities
                .get_mut(&city)
                .unwrap()
                .pressure
                .insert(faith.clone(), 500.0);
        }
    }
    let with = ai.rival_victory_pressure(&game, 0).progress;
    let without = shipped.rival_victory_pressure(&game, 0).progress;
    assert!(
        with >= without,
        "the count lowered the reading ({with} < {without})"
    );
    assert!((0..=100).contains(&with), "{with} out of range");
}

#[test]
fn culture_forecast_versions_are_exclusive_and_reversible() {
    let mut ai = AdvancedAi::new();
    assert!(!ai.culture_lane_forecast_2);
    assert!(!AdvancedAi::legacy().culture_lane_forecast_2);
    ai.enable_culture_lane_forecast();
    ai.enable_culture_lane_forecast_2();
    assert!(ai.culture_lane_forecast_2 && !ai.culture_lane_forecast);
    ai.enable_culture_lane_forecast();
    assert!(ai.culture_lane_forecast && !ai.culture_lane_forecast_2);
    ai.enable_culture_lane_forecast_2();
    ai.disable_culture_lane_forecast_2();
    assert!(!ai.culture_lane_forecast && !ai.culture_lane_forecast_2);
}

#[test]
fn culture_forecast_v2_accounts_for_religious_market_penalties() {
    let mut game = culture_forecast_market("relic");
    let mut original = AdvancedAi::new();
    original.enable_culture_lane_forecast();
    let mut revised = AdvancedAi::new();
    revised.enable_culture_lane_forecast_2();
    let full_market = revised.culture_lane_forecast_score(&game, 0);
    assert!(full_market > 0);
    assert_eq!(full_market, original.culture_lane_forecast_score(&game, 0));

    game.players[1].religion = Some("Other Faith".to_string());
    let reduced = revised.culture_lane_forecast_score(&game, 0);
    assert!(reduced > 0 && reduced < full_market);
    assert_eq!(original.culture_lane_forecast_score(&game, 0), full_market);

    game.players[1]
        .civics
        .insert(crate::name!("the_enlightenment"));
    assert_eq!(game.international_tourism_multiplier(0, 1, true), 0.0);
    let religious_market_closed = revised.culture_lane_forecast_score(&game, 0);
    assert!(religious_market_closed < reduced);
    assert!(original.culture_lane_forecast_score(&game, 0) > religious_market_closed);
}

#[test]
fn culture_forecast_v2_keeps_secular_tourism_out_of_religious_penalties() {
    let mut game = culture_forecast_market("writing");
    let mut revised = AdvancedAi::new();
    revised.enable_culture_lane_forecast_2();
    let before = revised.culture_lane_forecast_score(&game, 0);
    assert!(before > 0);
    assert_eq!(game.religious_tourism_per_turn(0), 0.0);
    game.players[1].religion = Some("Other Faith".to_string());
    assert_eq!(revised.culture_lane_forecast_score(&game, 0), before);
}

#[test]
fn culture_forecast_v2_prices_open_borders_until_they_expire() {
    let mut game = culture_forecast_market("writing");
    let mut revised = AdvancedAi::new();
    revised.enable_culture_lane_forecast_2();
    let closed = revised.culture_lane_forecast_score(&game, 0);
    game.players[1]
        .open_borders_until
        .insert(0, game.turn + 100);
    assert!(revised.culture_lane_forecast_score(&game, 0) > closed);
    game.players[1].open_borders_until.insert(0, game.turn);
    assert_eq!(revised.culture_lane_forecast_score(&game, 0), closed);
}

#[test]
fn culture_forecast_v2_does_not_count_teammates_as_markets() {
    let mut game = culture_forecast_market("relic");
    let mut revised = AdvancedAi::new();
    revised.enable_culture_lane_forecast_2();
    assert!(revised.culture_lane_forecast_score(&game, 0) > 0);
    game.players[0].team = Some(7);
    game.players[1].team = Some(7);
    assert_eq!(revised.culture_lane_forecast_score(&game, 0), 0);
}

#[test]
fn culture_lane_forecast_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "culture_lane_forecast"
            && gene.tag == "culture-lane-forecast"),
        "culture-lane-forecast must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("culture-lane-forecast"),
        "the ledger must be able to price it"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("culture-lane-forecast").is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.culture_lane_forecast, "off in the stock agent");
    assert!(
        !AdvancedAi::legacy().culture_lane_forecast,
        "off in the legacy agent"
    );
    ai.enable_culture_lane_forecast();
    assert!(ai.culture_lane_forecast);
    ai.disable_culture_lane_forecast();
    assert!(!ai.culture_lane_forecast, "reversible");
}

/// The lock: an empire generating real Tourism reads the Culture lane at zero
/// on the shipped ratio, because its visitors have not arrived yet.
#[test]
fn the_shipped_culture_lane_reads_a_race_that_has_not_finished_as_nothing() {
    let mut game = Game::new(4, 40, 26, 8_601, 250, 0);
    game.turn = 90;
    game.found_city_for(0, (12, 10), None);
    for rival in 1..4 {
        game.found_city_for(rival, (12 + 4 * rival as i32, 16), None);
        game.players[rival].culture_lifetime = 900.0;
    }
    // Real tourism output, no visitor banked yet.
    game.players[0].tourism_lifetime = 400.0;

    let shipped = AdvancedAi::new();
    assert_eq!(
        g_foreign(&game, 0),
        0,
        "fixture: not one visitor has arrived"
    );
    assert_eq!(
        shipped.lane_progress_table(&game, 0)[1],
        0,
        "and so the shipped lane reads nothing at all"
    );
}

/// The forecast projects BOTH curves: the same board reads lower when the
/// rival's culture is climbing faster, and reads nothing once the clock is
/// gone.
#[test]
fn the_culture_forecast_prices_the_bar_that_is_still_moving() {
    let mut ai = AdvancedAi::new();
    ai.enable_culture_lane_forecast();

    let mut game = Game::new(4, 40, 26, 8_602, 250, 0);
    game.turn = 60;
    game.found_city_for(0, (12, 10), None);
    for rival in 1..4 {
        game.found_city_for(rival, (12 + 4 * rival as i32, 16), None);
    }
    // Bank visitors directly so the reading does not depend on the tourism
    // model: `foreign_tourists` divides accumulated pressure by
    // starting majors * TOURISM_PER_VISITOR.
    let per_visitor = 4.0 * crate::game::TOURISM_PER_VISITOR;
    for rival in 1..4 {
        game.players[0]
            .tourism_pressure
            .insert(rival, per_visitor * 20.0);
    }
    game.players[1].culture_lifetime = 4_000.0;

    let ahead = ai.culture_lane_forecast_score(&game, 0);
    assert!(ahead > 0, "a real tourist stream has a reading ({ahead})");
    assert_eq!(
        AdvancedAi::new().culture_lane_forecast_score(&game, 0),
        0,
        "the gene is off, so it contributes nothing"
    );

    // The same board with a much larger bar must read lower.
    let mut steeper = game.clone();
    steeper.players[1].culture_lifetime = 400_000.0;
    let behind = ai.culture_lane_forecast_score(&steeper, 0);
    assert!(
        behind < ahead,
        "a bigger bar must read lower ({behind} vs {ahead})"
    );

    // Past the clock there is nothing left to project into.
    let mut over = game.clone();
    over.turn = 250;
    let at_the_wire = ai.culture_lane_forecast_score(&over, 0);
    assert!(
        at_the_wire <= ahead,
        "no turns left cannot read higher ({at_the_wire} vs {ahead})"
    );
}

/// It can only raise the shipped ratio, never lower it.
#[test]
fn the_culture_forecast_never_lowers_the_shipped_ratio() {
    let mut ai = AdvancedAi::new();
    ai.enable_culture_lane_forecast();
    let shipped = AdvancedAi::new();
    for (seed, turn) in [(8_701_u64, 40_u32), (8_702, 150), (8_703, 240)] {
        let mut game = Game::new(4, 40, 26, seed, 250, 0);
        game.turn = turn;
        game.found_city_for(0, (12, 10), None);
        for rival in 1..4 {
            game.found_city_for(rival, (12 + 4 * rival as i32, 16), None);
            game.players[rival].culture_lifetime = 500.0 * rival as f64;
        }
        let with = ai.lane_progress_table(&game, 0)[1];
        let without = shipped.lane_progress_table(&game, 0)[1];
        assert!(
            with >= without,
            "seed {seed}: the forecast lowered the reading ({with} < {without})"
        );
        assert!(
            (0..=100).contains(&with),
            "seed {seed}: {with} out of range"
        );
    }
}

#[test]
fn science_chain_alarm_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "science_chain_alarm"
            && gene.tag == "science-chain-alarm"),
        "science-chain-alarm must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("science-chain-alarm"),
        "the ledger must be able to price it"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("science-chain-alarm").is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.science_chain_alarm, "off in the stock agent");
    assert!(
        !AdvancedAi::legacy().science_chain_alarm,
        "off in the legacy agent"
    );
    ai.enable_science_chain_alarm();
    assert!(ai.science_chain_alarm);
    ai.disable_science_chain_alarm();
    assert!(!ai.science_chain_alarm, "reversible");
}

/// The unearned base is dropped, so a rival racing nothing still reads
/// nothing, and the launch ladder still governs above the ramp.
#[test]
fn the_chain_ramp_starts_at_zero_and_yields_to_the_launch_ladder() {
    let mut ai = AdvancedAi::new();
    ai.enable_science_chain_alarm();
    let shipped = AdvancedAi::new();

    // A rival with no techs at all: the ramp must not invent a floor.
    let mut bare = Game::new(3, 30, 20, 9_102, 250, 0);
    bare.found_city_for(1, (12, 10), None);
    bare.players[1].techs.clear();
    assert_eq!(
        ai.rival_pressure(&bare, 1).1,
        shipped.rival_pressure(&bare, 1).1,
        "a rival racing nothing reads exactly what it read before"
    );

    // A rival that has launched reads the ladder, which is above the ramp.
    let mut launched = bare.clone();
    launched.players[1]
        .science_projects
        .insert("launch_moon_landing".to_string());
    launched.players[1]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    let (_, with) = ai.rival_pressure(&launched, 1);
    let (_, without) = shipped.rival_pressure(&launched, 1);
    assert!(
        with >= without,
        "the ramp never lowers the ladder ({with} < {without})"
    );
    assert!(with >= 45, "two launches still read the ladder ({with})");
}

#[test]
fn rival_suzerainty_alarm_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "rival_suzerainty_alarm"
            && gene.tag == "rival-suzerainty-alarm"),
        "rival-suzerainty-alarm must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("rival-suzerainty-alarm"),
        "the ledger must be able to price it"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("rival-suzerainty-alarm").is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.rival_suzerainty_alarm, "off in the stock agent");
    assert!(
        !AdvancedAi::legacy().rival_suzerainty_alarm,
        "off in the legacy agent"
    );
    ai.enable_rival_suzerainty_alarm();
    assert!(ai.rival_suzerainty_alarm);
    ai.disable_rival_suzerainty_alarm();
    assert!(!ai.rival_suzerainty_alarm, "reversible");
}

/// Zero suzerainties reads exactly what it read before, and the gene can only
/// raise.
#[test]
fn the_suzerainty_term_starts_at_zero_and_never_lowers() {
    let mut ai = AdvancedAi::new();
    ai.enable_rival_suzerainty_alarm();
    let shipped = AdvancedAi::new();

    let mut game = Game::new(3, 46, 30, 9_302, 250, 6);
    assert_eq!(
        ai.rival_pressure(&game, 1).1,
        shipped.rival_pressure(&game, 1).1,
        "a rival with no city-states reads what it always read"
    );

    // Points already banked: the term adds to them and stays in range.
    game.players[1].dvp = 12;
    let minors: Vec<usize> = game
        .players
        .iter()
        .filter(|player| player.alive && player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    for minor in &minors {
        game.players[1].envoys.push((*minor, 4));
    }
    let with = ai.rival_pressure(&game, 1).1;
    let without = shipped.rival_pressure(&game, 1).1;
    assert!(
        with >= without,
        "the term lowered the reading ({with} < {without})"
    );
    assert!((0..=100).contains(&with), "{with} out of range");
}

#[test]
fn congress_counter_leader_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "congress_counter_leader"
            && gene.tag == "congress-counter-leader"),
        "congress-counter-leader must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("congress-counter-leader"),
        "the ledger must be able to price it"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("congress-counter-leader").is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.congress_counter_leader, "off in the stock agent");
    assert!(
        !AdvancedAi::legacy().congress_counter_leader,
        "off in the legacy agent"
    );
    ai.enable_congress_counter_leader();
    assert!(ai.congress_counter_leader);
    ai.disable_congress_counter_leader();
    assert!(!ai.congress_counter_leader, "reversible");
}

/// The margin is a margin: level scores read nothing, and the alarm is silent
/// before the game has enough history to mean anything.
#[test]
fn the_score_margin_reads_the_lead_and_not_the_turn() {
    let mut ai = AdvancedAi::new();
    ai.early_score_alarm = true; // unregistered: set the field directly

    let mut game = Game::new(3, 40, 26, 9_502, 250, 0);
    for pid in 0..3 {
        game.found_city_for(pid, (10 + 8 * pid as i32, 12), None);
    }

    // Too early to mean anything: `standard_duration(60)` has not passed.
    game.turn = 10;
    game.players[1].civics.extend(
        game.rules
            .civics
            .keys()
            .take(30)
            .map(|civic| Name::new(civic.as_str())),
    );
    assert_eq!(
        ai.rival_pressure(&game, 1).1,
        0,
        "before the window the alarm is silent"
    );

    // Past the window it reads the LEAD: closing the gap lowers the number,
    // on the same turn, which a clock could not do.
    game.turn = 120;
    let ahead = ai.rival_pressure(&game, 1).1;
    let mut closed = game.clone();
    for pid in [0_usize, 2] {
        closed.players[pid].civics = closed.players[1].civics.clone();
    }
    let level = ai.rival_pressure(&closed, 1).1;
    assert!(
        level < ahead,
        "the same turn with the field caught up must read lower ({level} vs {ahead})"
    );

    // And the shipped clock cannot tell those two boards apart at all.
    let shipped = AdvancedAi::new();
    assert_eq!(
        shipped.rival_pressure(&game, 1).1,
        shipped.rival_pressure(&closed, 1).1,
        "the clock reads the same number for both"
    );
}

#[test]
fn domination_city_count_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "domination_city_count"
            && gene.tag == "domination-city-count"),
        "domination-city-count must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("domination-city-count"),
        "the ledger must be able to price it"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("domination-city-count").is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.domination_city_count, "off in the stock agent");
    assert!(
        !AdvancedAi::legacy().domination_city_count,
        "off in the legacy agent"
    );
    ai.enable_domination_city_count();
    assert!(ai.domination_city_count);
    ai.disable_domination_city_count();
    assert!(!ai.domination_city_count, "reversible");
}

/// The blind spot: an empire that has eaten most of a neighbour, but not the
/// palace, reads exactly what a pacifist reads.
#[test]
fn an_empire_eating_the_board_reads_as_a_pacifist_until_the_palace() {
    let mut game = Game::new(3, 46, 30, 9_701, 250, 0);
    // Player 1 founds a capital and four more cities; player 2 founds one.
    game.found_city_for(1, (12, 10), None);
    // Player 2's FIRST city is its palace and stays with it; the four after it
    // are the ones player 1 eats.
    game.found_city_for(2, (16, 10), None);
    let mut spot = (20, 10);
    let mut taken = Vec::new();
    for _ in 0..4 {
        taken.push(game.found_city_for(2, spot, None));
        spot = (spot.0 + 4, spot.1);
    }
    game.found_city_for(0, (12, 22), None);
    // Player 1 has captured every one of them except the palace.
    for city in &taken {
        game.cities.get_mut(city).unwrap().owner = 1;
    }
    assert!(
        !game
            .cities
            .values()
            .any(|city| city.is_capital && city.original_owner != 1 && city.owner == 1),
        "fixture: no foreign capital has fallen"
    );

    let shipped = AdvancedAi::new();
    let mut ai = AdvancedAi::new();
    ai.enable_domination_city_count();

    // The shipped reading counts capitals only, so the conqueror scores zero
    // on this lane and the lane is not what names it.
    assert_ne!(
        shipped.rival_pressure(&game, 1).0,
        GrandStrategy::Conquest,
        "the shipped model does not see the conquest at all"
    );
    let (lane, progress) = ai.rival_pressure(&game, 1);
    assert_eq!(lane, GrandStrategy::Conquest, "the gene does");
    assert!(
        progress >= 50,
        "four of the board's six foreign cities is most of it ({progress})"
    );
}

/// Zero conquests reads zero, a fallen capital still reads through the shipped
/// term, and the fold can only raise.
#[test]
fn the_city_count_starts_at_zero_and_never_lowers_the_capital_count() {
    let mut ai = AdvancedAi::new();
    ai.enable_domination_city_count();
    let shipped = AdvancedAi::new();

    // Nobody has taken anything.
    let mut game = Game::new(3, 46, 30, 9_702, 250, 0);
    for pid in 0..3 {
        game.found_city_for(pid, (12 + 8 * pid as i32, 12), None);
    }
    assert_eq!(
        ai.rival_pressure(&game, 1).1,
        shipped.rival_pressure(&game, 1).1,
        "an empire that has conquered nothing reads what it always read"
    );

    // One foreign capital taken: the shipped term reads 50 of two rivals, and
    // the city term cannot drag that down.
    let capital = game.player_city_ids(2)[0];
    game.cities.get_mut(&capital).unwrap().owner = 1;
    let with = ai.rival_pressure(&game, 1).1;
    let without = shipped.rival_pressure(&game, 1).1;
    assert!(
        with >= without,
        "the city count lowered the capital count ({with} < {without})"
    );
    assert!((0..=100).contains(&with), "{with} out of range");
}

#[test]
fn unchosen_war_keeps_the_lane_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "unchosen_war_keeps_the_lane"
            && gene.tag == "unchosen-war-keeps-the-lane"),
        "unchosen-war-keeps-the-lane must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("unchosen-war-keeps-the-lane"),
        "the ledger must be able to price it"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("unchosen-war-keeps-the-lane")
            .is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.unchosen_war_keeps_the_lane, "off in the stock agent");
    assert!(
        !AdvancedAi::legacy().unchosen_war_keeps_the_lane,
        "off in the legacy agent"
    );
    ai.enable_unchosen_war_keeps_the_lane();
    assert!(ai.unchosen_war_keeps_the_lane);
    ai.disable_unchosen_war_keeps_the_lane();
    assert!(!ai.unchosen_war_keeps_the_lane, "reversible");
}

#[test]
fn elective_war_in_reach_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "elective_war_in_reach"
            && gene.tag == "elective-war-in-reach"),
        "elective-war-in-reach must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("elective-war-in-reach"),
        "the ledger must be able to price it"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("elective-war-in-reach").is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.elective_war_in_reach, "off in the stock agent");
    assert!(
        !AdvancedAi::legacy().elective_war_in_reach,
        "off in the legacy agent"
    );
    ai.enable_elective_war_in_reach();
    assert!(ai.elective_war_in_reach);
    ai.disable_elective_war_in_reach();
    assert!(!ai.elective_war_in_reach, "reversible");
}

#[test]
fn expansion_pays_back_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "expansion_pays_back"
            && gene.tag == "expansion-pays-back"),
        "expansion-pays-back must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("expansion-pays-back"),
        "the ledger must be able to price it"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("expansion-pays-back").is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.expansion_pays_back, "off in the stock agent");
    assert!(
        !AdvancedAi::legacy().expansion_pays_back,
        "off in the legacy agent"
    );
    ai.enable_expansion_pays_back();
    assert!(ai.expansion_pays_back);
    ai.disable_expansion_pays_back();
    assert!(!ai.expansion_pays_back, "reversible");
}

/// ⚠⚠ Stock native boards still shut their Settler window on a clock; the
/// explicit Science contract now uses the payback test as well.
///
/// `settler_expansion_window_open` takes the payback branch under `land_grab`,
/// `expansion_pays_back`, or an explicit Science target. Stock and the other
/// assigned lanes retain their historical deadline.
#[test]
fn the_payback_branch_is_reserved_for_expansion_and_science() {
    assert!(
        GENES
            .iter()
            .any(|gene| gene.field == "land_grab" && !gene.screenable()),
        "land_grab is host-only, so a native board cannot take that branch"
    );
    let stock = AdvancedAi::new();
    assert!(!stock.land_grab && !stock.expansion_pays_back);

    let mut game = Game::new(2, 40, 26, 5_501, 250, 0);
    let city = game.found_city_for(0, (12, 12), None);

    // Late enough that the deadline has shut, but with turns to spare: the
    // stock rule refuses on the clock, and the payback rule asks whether the
    // city would repay the settler and answers for itself.
    game.turn = 200;
    let mut paid = AdvancedAi::new();
    paid.enable_expansion_pays_back();
    assert_eq!(
        paid.settler_expansion_window_open(&game, 0, city),
        paid.expansion_pays_back_for(&game, 0, city),
        "the gene routes the window through the payback test"
    );
    assert_eq!(
        stock.settler_expansion_window_open(&game, 0, city),
        stock.adaptive_expansion_window_open(&game),
        "and without it the window is the deadline, whatever the city can do"
    );
}

/// The payback test is a clock about THIS city: build time at its own
/// production, plus the walk and the repayment interval, against the turns
/// that are actually left.
#[test]
fn the_payback_test_prices_the_city_that_would_build_the_settler() {
    let mut ai = AdvancedAi::new();
    ai.enable_expansion_pays_back();

    let mut game = Game::new(2, 40, 26, 5_502, 250, 0);
    let city = game.found_city_for(0, (12, 12), None);

    // Plenty of game left: a settler can be built, walked and repaid.
    game.turn = 20;
    assert!(ai.expansion_pays_back_for(&game, 0, city));

    // At the wire it cannot, and the test says so without consulting any
    // deadline: the same city, the same production, only the clock moved.
    game.turn = game.max_turns.saturating_sub(2);
    assert!(!ai.expansion_pays_back_for(&game, 0, city));

    // And a game with no turn limit always pays back, because nothing can run
    // out: `remaining` is the whole of `max_turns` minus the turn.
    let mut endless = game.clone();
    endless.max_turns = 0;
    endless.turn = 0;
    assert!(!ai.expansion_pays_back_for(&endless, 0, city));
}

#[test]
fn elective_war_yields_to_a_lane_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "elective_war_yields_to_a_lane"
            && gene.tag == "elective-war-yields-to-a-lane"),
        "elective-war-yields-to-a-lane must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("elective-war-yields-to-a-lane"),
        "the ledger must be able to price it"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("elective-war-yields-to-a-lane")
            .is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.elective_war_yields_to_a_lane, "off in the stock agent");
    assert!(
        !AdvancedAi::legacy().elective_war_yields_to_a_lane,
        "off in the legacy agent"
    );
    ai.enable_elective_war_yields_to_a_lane();
    assert!(ai.elective_war_yields_to_a_lane);
    ai.disable_elective_war_yields_to_a_lane();
    assert!(!ai.elective_war_yields_to_a_lane, "reversible");
}

#[test]
fn defensible_sites_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "defensible_sites"
            && gene.tag == "defensible-sites"),
        "defensible-sites must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("defensible-sites"),
        "the ledger must be able to price it"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("defensible-sites").is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.defensible_sites, "off in the stock agent");
    assert!(
        !AdvancedAi::legacy().defensible_sites,
        "off in the legacy agent"
    );
    ai.enable_defensible_sites();
    assert!(ai.defensible_sites);
    ai.disable_defensible_sites();
    assert!(!ai.defensible_sites, "reversible");
}

/// The other half: a site with no friendly city within reach. The shipped
/// score treats an isolated site exactly like one inside the empire.
#[test]
fn an_isolated_site_is_penalised_and_the_penalty_is_bounded() {
    let mut ai = AdvancedAi::new();
    ai.enable_defensible_sites();
    let mut game = Game::new(2, 60, 40, 6_102, 250, 0);
    game.found_city_for(0, (10, 20), None);

    // Inside the empire: no isolation penalty at all.
    assert_eq!(ai.defensibility(&game, 0, (14, 20)), 0.0);

    // Just outside the six-tile support radius, and further still.
    let near = ai.defensibility(&game, 0, (18, 20));
    let far = ai.defensibility(&game, 0, (24, 20));
    assert!(near < 0.0, "eight tiles out is isolated ({near})");
    assert!(far < near, "fourteen tiles out is worse ({far} vs {near})");

    // ⚠ And the penalty is capped, so a site across the map is not scored as
    // infinitely bad — the term is a tiebreak on holdability, not a leash.
    let further = ai.defensibility(&game, 0, (40, 20));
    assert_eq!(
        further, far,
        "the isolation term stops growing past its cap ({further} vs {far})"
    );
}

/// ⚠⚠ A BUILDER DOES NOT STOP WORKING BECAUSE IT RAN OUT OF JOBS. It stops
/// because the one job it scored best cannot be walked to — and then it picks
/// that same job again next turn, and the turn after that, for the rest of the
/// game. `advanced_builder_step` prices distance with `wdist`, a straight line
/// across the map, so a walled-in tile scores exactly as if the unit could
/// stroll there; the refused step ends the turn and the pin in
/// `builder_targets` survives to re-select it. See
/// `builder_step_to_the_first_reachable_job`.
#[test]
fn a_builder_whose_best_job_is_walled_off_walks_to_the_next_one() {
    let setup = || {
        let (mut game, city, home) = empire_with_a_capital(71_141);
        for uid in game.units.keys().copied().collect::<Vec<_>>() {
            if game.units[&uid].owner != 0 {
                game.remove_unit(uid);
            }
        }
        game.barb_camps.clear();
        game.barb_naval_camps.clear();
        for tile in game.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = Some(crate::name!("farm"));
            tile.pillaged = false;
        }
        game.players[0].techs.extend([
            crate::name!("mining"),
            crate::name!("bronze_working"),
            crate::name!("irrigation"),
        ]);
        game.players[0]
            .explored
            .extend(game.map.tiles.keys().copied());
        // Two jobs the capital owns, in opposite directions: the near one is
        // ringed by mountains, the far one is open ground.
        let walled = (home.0 + 2, home.1);
        let open = (home.0 - 4, home.1);
        for pos in [walled, open] {
            assert!(game.map.tiles.contains_key(&pos), "{pos:?} is on the map");
            let tile = game.map.tiles.get_mut(&pos).unwrap();
            tile.improvement = None;
            tile.owner_city = Some(city);
            game.cities.get_mut(&city).unwrap().owned_tiles.push(pos);
        }
        for pos in game.nbrs(walled) {
            assert_ne!(pos, home, "the wall must not be the Builder's own tile");
            game.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
        }
        let wall = game.nbrs(walled)[0];
        assert!(
            !game.rules.is_passable(&game.map.tiles[&wall]),
            "the ring around the near job is impassable"
        );
        assert!(
            game.rules.is_passable(&game.map.tiles[&open]),
            "the far job is open ground"
        );
        (game, home, walled, open)
    };

    // Untreated: the Builder scores the walled tile best because it is nearer,
    // cannot take a step toward it, and gives the turn up — with the pin left
    // in place, so the next turn produces the same nothing.
    let (mut stock_game, home, walled, _open) = setup();
    let builder = stock_game.spawn_test_unit("builder", 0, home);
    let mut stock = AdvancedAi::new();
    assert!(!stock.advanced_builder_step(&mut stock_game, 0, builder, GrandStrategy::Expansion));
    assert_eq!(
        stock.builder_targets.get(&builder),
        Some(&walled),
        "the untreated Builder pins the job it cannot reach"
    );
    assert_eq!(
        stock_game.units[&builder].pos, home,
        "and does not move at all"
    );
    stock_game.units.get_mut(&builder).unwrap().moved = false;
    assert!(!stock.advanced_builder_step(&mut stock_game, 0, builder, GrandStrategy::Expansion));
    assert_eq!(
        stock_game.units[&builder].pos, home,
        "the pin re-selects the same unreachable tile next turn: this is the stall"
    );

    // Treated: the same ranking, the same first choice — and when the step is
    // refused it unpins that tile and starts toward the job it can reach.
    let (mut game, home, walled, open) = setup();
    let builder = game.spawn_test_unit("builder", 0, home);
    let mut treated = AdvancedAi::new();
    treated.enable_builder_tries_the_next_tile();
    assert!(treated.advanced_builder_step(&mut game, 0, builder, GrandStrategy::Expansion));
    assert_ne!(game.units[&builder].pos, home, "the Builder is working");
    assert!(
        game.wdist(game.units[&builder].pos, open) < game.wdist(home, open),
        "and it is walking toward the reachable job, not the walled one"
    );
    assert_eq!(
        treated.builder_targets.get(&builder),
        Some(&open),
        "the pin follows the job it can actually reach"
    );
    assert_ne!(treated.builder_targets.get(&builder), Some(&walled));
}

/// The half of the treatment that is *not* the fix: a Builder that refused
/// because it had nothing left to spend is mid-journey, not stuck, and must
/// keep the job it is walking to. Dropping the pin there would re-rank the
/// whole empire every time a Builder ran its movement out.
#[test]
fn a_builder_out_of_movement_keeps_the_job_it_is_walking_to() {
    let (mut game, city, home) = empire_with_a_capital(71_153);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        if game.units[&uid].owner != 0 {
            game.remove_unit(uid);
        }
    }
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = Some(crate::name!("farm"));
        tile.pillaged = false;
    }
    game.players[0].techs.extend([
        crate::name!("mining"),
        crate::name!("bronze_working"),
        crate::name!("irrigation"),
    ]);
    game.players[0]
        .explored
        .extend(game.map.tiles.keys().copied());
    let job = (home.0 - 4, home.1);
    let tile = game.map.tiles.get_mut(&job).unwrap();
    tile.improvement = None;
    tile.owner_city = Some(city);
    game.cities.get_mut(&city).unwrap().owned_tiles.push(job);

    let builder = game.spawn_test_unit("builder", 0, home);
    let mut ai = AdvancedAi::new();
    ai.enable_builder_tries_the_next_tile();
    assert!(ai.advanced_builder_step(&mut game, 0, builder, GrandStrategy::Expansion));
    assert_eq!(ai.builder_targets.get(&builder), Some(&job));

    // Spent: every tile refuses now, including the one it is walking to.
    game.units.get_mut(&builder).unwrap().moves_left = 0.0;
    let waypoint = game.units[&builder].pos;
    assert!(!ai.advanced_builder_step(&mut game, 0, builder, GrandStrategy::Expansion));
    assert_eq!(game.units[&builder].pos, waypoint);
    assert_eq!(
        ai.builder_targets.get(&builder),
        Some(&job),
        "an empty movement bar is not a reason to forget the job"
    );
}

/// The gene is off in the stock and legacy agents and follows the current
/// deployment selection when repairs are enabled.
#[test]
fn builder_tries_the_next_tile_follows_the_ledger() {
    assert!(!AdvancedAi::new().base.builder_tries_the_next_tile);
    assert!(!AdvancedAi::legacy().base.builder_tries_the_next_tile);
    let mut deployment = AdvancedAi::new();
    deployment.enable_engine_repairs();
    assert_eq!(
        Some(deployment.base.builder_tries_the_next_tile),
        crate::ai::advanced::gene_ledger::ledger_default_on("builder-tries-the-next-tile"),
        "the deployment controller must follow the ledger"
    );
}

/// The same defect one layer down, where the city-states and the basic ladder
/// live: `BasicAi::builder_step` ranks every owned job by `wdist`, asks
/// `step_toward` for the nearest one alone, and returns `false` when that is
/// refused. See `BasicAi::builder_tries_the_next_tile`.
#[test]
fn a_basic_builder_walled_off_from_the_nearest_job_takes_the_next_one() {
    let setup = || {
        let (mut game, city, home) = empire_with_a_capital(71_147);
        for uid in game.units.keys().copied().collect::<Vec<_>>() {
            if game.units[&uid].owner != 0 {
                game.remove_unit(uid);
            }
        }
        game.barb_camps.clear();
        game.barb_naval_camps.clear();
        for tile in game.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = Some(crate::name!("farm"));
            tile.pillaged = false;
        }
        game.players[0].techs.extend([
            crate::name!("mining"),
            crate::name!("bronze_working"),
            crate::name!("irrigation"),
        ]);
        game.players[0]
            .explored
            .extend(game.map.tiles.keys().copied());
        let walled = (home.0 + 2, home.1);
        let open = (home.0 - 4, home.1);
        for pos in [walled, open] {
            let tile = game.map.tiles.get_mut(&pos).unwrap();
            tile.improvement = None;
            tile.owner_city = Some(city);
            game.cities.get_mut(&city).unwrap().owned_tiles.push(pos);
        }
        for pos in game.nbrs(walled) {
            game.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
        }
        (game, home, open)
    };

    let (mut stock_game, home, _open) = setup();
    let builder = stock_game.spawn_test_unit("builder", 0, home);
    let stock = AdvancedAi::new();
    assert!(
        !stock.base.builder_step(&mut stock_game, 0, builder),
        "the untreated Builder gives its turn up at the first refusal"
    );
    assert_eq!(stock_game.units[&builder].pos, home);

    let (mut game, home, open) = setup();
    let builder = game.spawn_test_unit("builder", 0, home);
    let mut treated = AdvancedAi::new();
    treated.enable_builder_tries_the_next_tile();
    assert!(treated.base.builder_step(&mut game, 0, builder));
    assert!(
        game.wdist(game.units[&builder].pos, open) < game.wdist(home, open),
        "it starts toward the job it can reach"
    );
}

/// The gene is registered, discoverable by name, and reversible.
#[test]
fn civilian_out_of_reach_is_a_registered_reversible_opt_in() {
    assert!(GENES.iter().any(|gene| gene.opt_in()
        && gene.field == "civilian_out_of_reach"
        && gene.tag == "civilian-out-of-reach"));
    let mut ai = AdvancedAi::new();
    assert!(!ai.civilian_out_of_reach, "production ships it off");
    ai.enable_civilian_out_of_reach();
    assert!(ai.civilian_out_of_reach);
    ai.disable_civilian_out_of_reach();
    assert!(!ai.civilian_out_of_reach);
}

/// Native version-2 families are opt-in, off in both controllers, published
/// for `gene_screen` as `<base>-2`, and their enables turn version 1 off so a
/// seat plays one version of a family.
#[test]
fn version_two_genes_are_opt_in_and_turn_version_one_off() {
    let families = [
        "amenity-project-preemption",
        "settler-guard-holds",
        "campus-adjacency-threshold",
        "holy-site-where-the-threat-is",
        "naval-recon",
        "settler-target-hysteresis",
        "district-coverage",
        "power-the-laboratory",
        "science-victory-drive",
        "amphibious-siege-train",
    ];
    fn reads(ai: &AdvancedAi) -> Vec<(bool, bool)> {
        vec![
            (
                ai.amenity_project_preemption,
                ai.amenity_project_preemption_2,
            ),
            (ai.settler_guard_holds, ai.settler_guard_holds_2),
            (
                ai.campus_adjacency_threshold,
                ai.campus_adjacency_threshold_2,
            ),
            (
                ai.holy_site_where_the_threat_is,
                ai.holy_site_where_the_threat_is_2,
            ),
            (ai.base.naval_recon, ai.base.naval_recon_2),
            (ai.settler_target_hysteresis, ai.settler_target_hysteresis_2),
            (ai.base.district_coverage, ai.base.district_coverage_2),
            (ai.power_the_laboratory, ai.power_the_laboratory_2),
            (ai.science_victory_drive, ai.science_victory_drive_2),
            (ai.amphibious_siege_train, ai.amphibious_siege_train_2),
        ]
    }
    for (i, base) in families.iter().enumerate() {
        let tag = format!("{base}-2");
        let v1 = GENES
            .iter()
            .find(|gene| gene.tag == *base)
            .expect("version 1 is published");
        let v2 = GENES
            .iter()
            .find(|gene| gene.tag == tag)
            .expect("version 2 is published");
        assert!(v2.opt_in() && v2.screenable() && !v2.live(), "{tag}");
        // Version 1 may be a Repair gene the native bundle turns on; only
        // version 2 must be off in both constructors.
        let mut ai = AdvancedAi::new();
        assert!(!reads(&ai)[i].1, "{tag} off in new()");
        assert!(!reads(&AdvancedAi::legacy())[i].1, "{tag} off in legacy()");
        (v1.enable)(&mut ai);
        assert_eq!(reads(&ai)[i], (true, false), "{tag}: version 1 alone");
        (v2.enable)(&mut ai);
        assert_eq!(
            reads(&ai)[i],
            (false, true),
            "{tag}: version 2 turns version 1 off"
        );
        (v2.disable)(&mut ai);
        assert_eq!(reads(&ai)[i], (false, false), "{tag}: disabled");
    }
}

/// `naval-recon-3` is a separate, screenable land-first treatment. It
/// replaces both earlier versions, so its one peacetime Galley cannot inherit
/// v2's unconditional second-hull opening or preempt a missing land scout.
#[test]
fn naval_recon_three_is_opt_in_and_turns_prior_versions_off() {
    let v3 = GENES
        .iter()
        .find(|gene| gene.tag == "naval-recon-3")
        .expect("version 3 is published");
    assert!(v3.opt_in() && v3.screenable() && !v3.live());

    let mut ai = AdvancedAi::new();
    assert!(
        (
            ai.base.naval_recon,
            ai.base.naval_recon_2,
            ai.base.naval_recon_3
        ) == (true, false, false)
    );
    ai.enable_naval_recon_2();
    assert_eq!(
        (
            ai.base.naval_recon,
            ai.base.naval_recon_2,
            ai.base.naval_recon_3
        ),
        (false, true, false),
        "v2 has one version active"
    );
    ai.enable_naval_recon_3();
    assert_eq!(
        (
            ai.base.naval_recon,
            ai.base.naval_recon_2,
            ai.base.naval_recon_3
        ),
        (false, false, true),
        "v3 replaces both prior versions"
    );
    ai.disable_naval_recon_3();
    assert_eq!(
        (
            ai.base.naval_recon,
            ai.base.naval_recon_2,
            ai.base.naval_recon_3
        ),
        (false, false, false),
        "v3 remains independently reversible"
    );
}

/// The chain debts are priced through a payback window rather than a game
/// fraction, and the difference is largest exactly where the science race is
/// decided. See `AdvancedAi::chain_payback_window`.
#[test]
fn the_chain_payback_window_stops_the_science_debt_decaying_to_nothing() {
    let (mut game, capital, _home) = empire_with_a_capital(71_402);
    game.players[0].techs.insert(crate::name!("writing"));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let counts = EmpireCounts::default();
    let library = Item::Building {
        building: crate::name!("library"),
    };
    install_ai_test_district(&mut game, capital, "campus");

    let mut priced = AdvancedAi::new();
    priced.enable_live_bridge_universe();
    priced.enable_chain_payback_window();
    priced.refresh_research_weight(&game);
    let mut shipped = AdvancedAi::new();
    shipped.enable_live_bridge_universe();
    shipped.disable_chain_payback_window();
    shipped.refresh_research_weight(&game);

    // The two horizons are the whole gene, so pin them where the difference
    // decides the race. Turn 193 of 250 is the median turn a standard Emperor
    // game ends on a science victory: the game fraction has fallen to 0.23,
    // while the payback window still has more than its 40-turn span to run and
    // therefore pays in full.
    game.max_turns = 250;
    game.turn = 193;
    let fraction = AdvancedAi::research_horizon(&game);
    assert!(
        (fraction - 0.228).abs() < 0.005,
        "the shipped horizon is a game fraction and is nearly spent: {fraction}"
    );
    assert_eq!(
        AdvancedAi::campus_payback_horizon(&game),
        1.0,
        "a Library begun with 57 turns left repays many times over"
    );

    // And the debt that reaches the build order follows it. Priced at a turn
    // where the capital can still finish a Library inside the clock, so the
    // comparison is the horizon rather than `production_value`'s
    // cannot-finish-in-time sentinel.
    game.turn = 120;
    let with_window = priced.production_value(&game, 0, capital, &library, &plan, &counts);
    let with_fraction = shipped.production_value(&game, 0, capital, &library, &plan, &counts);
    assert!(
        with_window > with_fraction,
        "the Campus owes its Library at full price, not at the clock's: \
         {with_window} vs {with_fraction}"
    );

    // The reason the original horizon existed is preserved: at the very end
    // the debt is gone either way, so a Campus building begun at turn 249
    // still does not outbid a defender.
    game.turn = game.max_turns;
    assert_eq!(
        AdvancedAi::campus_payback_horizon(&game),
        0.0,
        "the payback window still closes at the turn limit"
    );
}

/// The gene is registered, discoverable by name, and reversible.
#[test]
fn camp_tile_buyout_is_a_registered_reversible_opt_in() {
    assert!(GENES.iter().any(|gene| gene.opt_in()
        && gene.field == "camp_tile_buyout"
        && gene.tag == "camp-tile-buyout"));
    let mut ai = AdvancedAi::new();
    assert!(!ai.camp_tile_buyout, "production ships it off");
    ai.enable_camp_tile_buyout();
    assert!(ai.camp_tile_buyout);
    ai.disable_camp_tile_buyout();
    assert!(!ai.camp_tile_buyout);
}

/// Ground under an outpost no soldier of ours can walk into is a hole in the
/// border with a deed attached. Civilization VI disperses an outpost on entry
/// (`Improvements.xml`: `RemoveOnEntry="true"`), so reach is the whole
/// question.
#[test]
fn an_outpost_no_soldier_can_reach_is_not_bought() {
    let (mut game, city, target, _) = capital_beside_an_outpost();
    disband_the_field_army(&mut game, 0);
    let center = game.cities[&city].pos;
    let ai = AdvancedAi::new();
    assert_eq!(
        ai.camp_buyout_clearance(&game, 0, city, target),
        None,
        "an empire with no soldier at all buys no outposts"
    );
    // A Scout is not an answer to a camp: the errand excludes recon for the
    // same reason.
    game.spawn_test_unit("scout", 0, center);
    assert_eq!(
        ai.camp_buyout_clearance(&game, 0, city, target),
        None,
        "recon does not count as the soldier that walks in"
    );
    game.spawn_test_unit("warrior", 0, center);
    assert!(
        ai.camp_buyout_clearance(&game, 0, city, target).is_some(),
        "a Warrior at home does"
    );
}

/// A raid party forming on the outpost is worth more than a quiet one, and
/// only the barbarians we can actually see count.
#[test]
fn a_party_forming_on_the_outpost_raises_its_price() {
    let (mut game, city, target, _) = capital_beside_an_outpost();
    let ai = AdvancedAi::new();
    let quiet = ai.camp_buyout_clearance(&game, 0, city, target).unwrap();
    // The fixture's world is seated without barbarians, so the second seat
    // stands in for them, as the belief and espionage suites do.
    game.barb_pid = Some(1);
    game.spawn_test_unit("warrior", 1, target);
    let raided = ai
        .camp_buyout_clearance(&game, 0, city, target)
        .expect("the outpost is still priced");
    assert!(
        raided > quiet,
        "a raider on the outpost raises the price: {raided} over {quiet}"
    );
}

/// The operator's rule is a comparison, and a late quote wins it:
/// `tile_purchase_cost` charges up to five times the base price as the game
/// runs on, and an outpost is not worth any price.
#[test]
fn a_quote_that_outgrows_the_outpost_is_refused() {
    let (mut game, city, target, early) = capital_beside_an_outpost();
    let ai = AdvancedAi::new();
    assert!(
        ai.camp_buyout_clearance(&game, 0, city, target)
            .is_some_and(|clearance| clearance >= early),
        "the opening quote is one the outpost is worth"
    );
    let techs: Vec<Name> = game.rules.techs.keys().copied().collect();
    for tech in techs.iter().take(techs.len() * 3 / 4) {
        game.players[0].techs.insert(*tech);
    }
    let late = game
        .plot_purchase_cost(0, city, target)
        .expect("the engine still sells the hex");
    assert!(
        late > CAMP_STANDING_COST * CAMP_RING_SHARE[1],
        "the fixture reaches a quote above what this outpost is worth: {late}"
    );
    let mut on = AdvancedAi::new();
    on.enable_camp_tile_buyout();
    assert!(
        !on.camp_tile_buyout_purchase(&mut game, 0, 0.0),
        "we care about clearing it, not at any price"
    );
    assert_eq!(game.map.tiles[&target].owner_city, None);
}

/// The reserve the ordinary spender protects is protected here too.
#[test]
fn a_buyout_never_reaches_into_the_reserve() {
    let (mut game, city, target, cost) = capital_beside_an_outpost();
    let mut on = AdvancedAi::new();
    on.enable_camp_tile_buyout();
    game.players[0].gold = cost;
    assert!(
        !on.camp_tile_buyout_purchase(&mut game, 0, 300.0),
        "affording the quote is not enough; the reserve comes first"
    );
    game.players[0].gold = 300.0 + 200.0 + cost;
    assert!(
        on.camp_tile_buyout_purchase(&mut game, 0, 300.0),
        "with the reserve and the spender's headroom clear, the hex is bought"
    );
    assert_eq!(game.map.tiles[&target].owner_city, Some(city));
}

/// End to end through the spender: the gene turns a hex the shipped
/// appraisal will not pay for into one it buys, and off, the same board
/// leaves the outpost's ground neutral.
#[test]
fn the_spender_buys_the_outposts_ground_only_with_the_gene_on() {
    let plan = district_planning_lane(1);

    let (mut off_game, city, target, _) = capital_beside_an_outpost();
    let mut on_game = off_game.clone();

    let off = AdvancedAi::new();
    off.advanced_gold_spending(&mut off_game, 0, &plan);
    assert_eq!(
        off_game.map.tiles[&target].owner_city, None,
        "bare snow under an outpost is not ground the shipped appraisal buys"
    );

    let mut on = AdvancedAi::new();
    on.enable_camp_tile_buyout();
    assert!(
        on.advanced_gold_spending(&mut on_game, 0, &plan),
        "the gene finds something to spend Gold on"
    );
    assert_eq!(
        on_game.map.tiles[&target].owner_city,
        Some(city),
        "the outpost's hex is annexed"
    );
    assert!(
        on_game.barb_camps.contains_key(&target),
        "and the outpost is still standing on it: Civilization VI disperses \
         a camp on entry, never because the deed changed hands"
    );
}

/// `contested-land-first` is a registered opt-in gene, off in both agents
/// and reversible; its flag lives on the base controller, which owns the
/// peacetime garrison.
#[test]
fn contested_land_first_is_a_registered_reversible_opt_in() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "contested_land_first"
            && gene.tag == "contested-land-first"),
        "contested-land-first must be a registered native opt-in"
    );
    assert!(
        crate::ai::advanced::gene_ledger::screenable("contested-land-first"),
        "the ledger must be able to price it"
    );
    assert!(
        crate::ai::advanced::gene_ledger::ledger_default_on("contested-land-first").is_some(),
        "the batch rule decides its default from its batch columns"
    );
    let mut ai = AdvancedAi::new();
    assert!(!ai.contested_land_first(), "off in the stock agent");
    assert!(
        !AdvancedAi::legacy().contested_land_first(),
        "off in the legacy agent"
    );
    ai.enable_contested_land_first();
    assert!(ai.contested_land_first());
    ai.disable_contested_land_first();
    assert!(!ai.contested_land_first(), "reversible");
}

/// "So long as we have a decent military": outmatched, or with a single
/// body, the gene is silent and the shipped terms keep the Settler home.
#[test]
fn contested_land_credit_needs_a_decent_army() {
    use super::super::contested_land::{CONTESTED_LAND_MIN_DEFENDERS, CONTESTED_LAND_POWER_RATIO};
    let mut game = a_neighbour_twelve_tiles_east(true);
    let between = (16, 10);
    let mut ai = AdvancedAi::new();
    ai.enable_contested_land_first();
    assert!(
        ai.contested_land_credit(&game, 0, between) > 0.0,
        "two on one is decent"
    );

    // The neighbour raises four more Warriors: 40 against 100.
    for step in 0..4 {
        game.spawn_unit("warrior", 1, (23, 11 + step));
    }
    assert!(game.military_power(0) < CONTESTED_LAND_POWER_RATIO * game.military_power(1));
    assert_eq!(
        ai.contested_land_credit(&game, 0, between),
        0.0,
        "outmatched, the Settler stays home"
    );
    // Parity restored.
    for step in 0..4 {
        game.spawn_unit("warrior", 0, (9, 11 + step));
    }
    assert!(game.military_power(0) >= CONTESTED_LAND_POWER_RATIO * game.military_power(1));
    assert!(
        ai.contested_land_credit(&game, 0, between) > 0.0,
        "a decent army claims"
    );

    // One body is not an army, whatever the ratio says.
    let mut lone = a_neighbour_twelve_tiles_east(true);
    let ours: Vec<u32> = lone
        .units
        .values()
        .filter(|u| u.owner == 0)
        .map(|u| u.id)
        .collect();
    for uid in &ours[1..] {
        lone.remove_unit(*uid);
    }
    let theirs: Vec<u32> = lone
        .units
        .values()
        .filter(|u| u.owner == 1)
        .map(|u| u.id)
        .collect();
    for uid in theirs {
        lone.remove_unit(uid);
    }
    assert!(lone.military_power(0) >= CONTESTED_LAND_POWER_RATIO * lone.military_power(1));
    let mut ai = AdvancedAi::new();
    ai.enable_contested_land_first();
    assert_eq!(
        ai.contested_land_credit(&lone, 0, between),
        0.0,
        "fewer than {CONTESTED_LAND_MIN_DEFENDERS} defenders is no posture"
    );
}

/// Fog stays honest and the war planner keeps its ground: an unmet major
/// and a major we are at war with are no front.
#[test]
fn contested_land_reads_only_met_neighbours_at_peace() {
    let between = (16, 10);
    let unmet = a_neighbour_twelve_tiles_east(false);
    let mut ai = AdvancedAi::new();
    ai.enable_contested_land_first();
    assert_eq!(
        ai.contested_land_credit(&unmet, 0, between),
        0.0,
        "a neighbour we have never met cannot be settled toward"
    );

    let mut war = a_neighbour_twelve_tiles_east(true);
    war.apply(0, &Action::DeclareWar { player: 1 })
        .expect("fixture: the declaration is legal");
    assert!(war.is_at_war(0, 1), "fixture");
    let mut ai = AdvancedAi::new();
    ai.enable_contested_land_first();
    assert_eq!(
        ai.contested_land_credit(&war, 0, between),
        0.0,
        "the ground between us and an enemy is the war planner's, not the Settler's"
    );
}

/// The provocation is waived where the ground is contested, and stands
/// everywhere else.
#[test]
fn contested_land_first_waives_the_provocation_only_on_contested_ground() {
    let mut game = a_neighbour_twelve_tiles_east(true);
    let theirs = game.player_city_ids(1)[0];
    let between = (16, 10);
    let behind = (4, 10);
    for site in [between, behind] {
        for near in game.wdisk(site, 1) {
            if near != site {
                if let Some(tile) = game.map.tiles.get_mut(&near) {
                    tile.owner_city = Some(theirs);
                }
            }
        }
    }
    let shipped = AdvancedAi::new();
    let mut ai = AdvancedAi::new();
    ai.enable_contested_land_first();
    let visible = game.player_vision_now(0);
    let pressure = shipped.foreign_border_pressure(&game, 0, between);
    assert!(pressure > 0.0, "fixture: their ground rings the site");
    let waived = shipped.settlement_safety_penalty(&game, 0, between, &visible)
        - ai.settlement_safety_penalty(&game, 0, between, &visible);
    assert!(
        (waived - pressure).abs() < 1e-9,
        "the provocation ({pressure}) is waived where the ground is contested, got {waived}"
    );
    assert_eq!(
        shipped.settlement_safety_penalty(&game, 0, behind, &visible),
        ai.settlement_safety_penalty(&game, 0, behind, &visible),
        "and stands everywhere else"
    );
}

/// `buy-what-cards-cannot-boost`: the same Library in the same city scores
/// bit-equal with the gene on while no card touches it, and strictly lower
/// the moment the engine prices a boost on it — here the Urban Development
/// Treaty's +100% on Campus buildings, which `Game::item_prod_mult` carries
/// exactly as it carries Agoge or Colonization. The gene-off score never
/// moves, which is the defect: the shipped scorer cannot see the boost.
#[test]
fn a_card_boosted_item_loses_gold_purchase_priority_only_with_the_gene_on() {
    let (mut game, city, _home) = empire_with_a_capital(71_131);
    game.players[0].techs.insert(crate::name!("writing"));
    let campus = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    game.map.tiles.get_mut(&campus).unwrap().district = Some(crate::name!("campus"));
    let state = game.cities.get_mut(&city).unwrap();
    state.districts.insert(crate::name!("campus"), campus);
    state
        .buildings
        .extend([crate::name!("monument"), crate::name!("granary")]);
    game.turn = 30;
    game.players[0].gold = 5_000.0;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    let off = AdvancedAi::targeting(VictoryTarget::Science);
    let mut on = AdvancedAi::targeting(VictoryTarget::Science);
    on.enable_buy_what_cards_cannot_boost();
    let library = Item::Building {
        building: crate::name!("library"),
    };
    let action = off
        .legal_purchase_actions(&game, 0)
        .into_iter()
        .find(|action| {
            matches!(action, Action::BuyBuilding { city: where_, building, currency }
                if *where_ == city && building == "library" && currency == "gold")
        })
        .expect("the Library has a Gold purchase action");
    let score = |ai: &AdvancedAi, game: &Game| {
        let counts = ai.counts(game, 0);
        let context = PurchaseScoreContext {
            g: game,
            pid: 0,
            plan: &plan,
            counts: &counts,
            bank: game.players[0].gold,
            reserve: 0.0,
        };
        ai.gold_purchase_score(context, &action, city, &library)
    };

    assert_eq!(game.item_prod_mult(0, city, Some(&library)), 1.0);
    let plain = score(&off, &game).expect("a Science empire buys its Library");
    assert_eq!(
        score(&on, &game),
        Some(plain),
        "no card, no change: the gene on is bit-equal to the gene off"
    );

    game.active_congress_effects
        .push(crate::game::CongressEffect {
            resolution: "urban_development_treaty".to_string(),
            outcome: "A".to_string(),
            target: "campus".to_string(),
            expires: game.turn + 10,
        });
    assert!(
        (game.item_prod_mult(0, city, Some(&library)) - 2.0).abs() < 1e-9,
        "the engine prices the treaty at +100%"
    );
    assert_eq!(
        score(&off, &game),
        Some(plain),
        "correcting build time must not inflate willingness to buy the same item"
    );
    let boosted = score(&on, &game);
    assert!(
        boosted.is_none_or(|boosted| boosted < plain),
        "a boosted build loses purchase priority: {plain:.1} -> {boosted:?}"
    );
}

/// ★★★★ THE LANE LIST IS A PREFERENCE ORDER, NOT A CAPACITY CAP. Live King
/// seat `civvis-20260826T112920Z` held Corporate Libertarianism -- ten policy
/// slots -- from turn 222 and played Classical Republic's four to turn 248,
/// because no lane's priority list in `strategic_government` names it. See
/// `AdvancedAi::government_capacity_fallback`.
#[test]
fn an_unlocked_government_the_lane_list_never_names_still_takes_the_seat() {
    let (mut game, _capital, _home) = empire_with_a_capital(71_501);
    game.players[0].civics.insert(crate::name!("code_of_laws"));
    game.players[0]
        .civics
        .insert(crate::name!("political_philosophy"));
    game.players[0].government = Some("classical_republic".to_string());
    // The exact live shape: the tier-4 gate is owned, the tier-2 and tier-3
    // gates the Diplomacy list names are not.
    game.players[0]
        .civics
        .insert(crate::name!("corporate_libertarianism"));

    let mut shipped = AdvancedAi::new();
    shipped.victory_target = Some(VictoryTarget::Diplomacy);
    let mut off = game.clone();
    shipped.strategic_government(&mut off, 0, GrandStrategy::Diplomacy);
    assert_eq!(
        off.players[0].government.as_deref(),
        Some("classical_republic"),
        "the shipped lane list cannot see a government it does not name"
    );

    let mut ai = AdvancedAi::new();
    ai.victory_target = Some(VictoryTarget::Diplomacy);
    ai.enable_government_capacity_fallback();
    let mut on = game.clone();
    ai.strategic_government(&mut on, 0, GrandStrategy::Diplomacy);
    assert_eq!(
        on.players[0].government.as_deref(),
        Some("corporate_libertarianism"),
        "the unlocked ten-slot government wins the seat"
    );
}

/// The fallback only ever raises policy capacity: an unlocked government with
/// fewer slots than the listed pick is still ignored, and the Anarchy guard
/// keeps its veto. See `AdvancedAi::government_capacity_fallback`.
#[test]
fn the_capacity_fallback_never_trades_policy_slots_away() {
    let (mut game, _capital, _home) = empire_with_a_capital(71_502);
    game.players[0].civics.insert(crate::name!("code_of_laws"));
    game.players[0]
        .civics
        .insert(crate::name!("political_philosophy"));
    // Only the tier-1 governments are unlocked, and the seat already plays
    // the richest of them.
    game.players[0].government = Some("classical_republic".to_string());

    let mut ai = AdvancedAi::new();
    ai.victory_target = Some(VictoryTarget::Diplomacy);
    ai.enable_government_capacity_fallback();
    ai.strategic_government(&mut game, 0, GrandStrategy::Diplomacy);
    assert_eq!(
        game.players[0].government.as_deref(),
        Some("classical_republic"),
        "nothing unlocked beats four slots, so the seat stands"
    );
}

#[test]
fn a_seat_outside_its_listed_pick_still_reaches() {
    let (mut game, _capital, _home) = empire_with_a_capital(71_504);
    for civic in [
        "code_of_laws",
        "political_philosophy",
        "corporate_libertarianism",
    ] {
        game.players[0].civics.insert(Name::new(civic));
    }
    game.players[0].government = Some("classical_republic".to_string());
    let mut ai = AdvancedAi::new();
    ai.victory_target = Some(VictoryTarget::Diplomacy);
    ai.enable_government_capacity_fallback();
    ai.enable_government_reach_pays_the_switch();
    ai.strategic_government(&mut game, 0, GrandStrategy::Diplomacy);
    assert_eq!(
        game.players[0].government.as_deref(),
        Some("corporate_libertarianism"),
        "six slots pay for the switch"
    );
}

/// ★★★★ THE ASSIGNED LANE VETOES THE SCOREBOARD. A `victory_target` refuses
/// every Great Work building off the Culture lane and every space-race
/// project off the Science lane. Live King seat `civvis-20260826T112920Z`
/// played the diplomatic lane to turn 248 at 2 diplomatic points against 19
/// and built no Museum in twelve cities. See
/// `AdvancedAi::lane_release_when_hopeless`.
#[test]
fn a_lost_lane_stops_vetoing_the_buildings_that_score() {
    let (mut game, _capital, _home) = empire_with_a_capital(71_504);
    game.max_turns = 250;
    game.turn = 200;

    let mut ai = AdvancedAi::new();
    ai.victory_target = Some(VictoryTarget::Diplomacy);
    ai.enable_lane_release_when_hopeless();

    // Level on the lane, there is no verdict and the vetoes stand.
    assert!(
        !ai.assigned_lane_is_lost(&game, 0),
        "nobody is winning the diplomatic race, so nothing is lost"
    );

    // The live shape: the rival is most of the way through the race and the
    // seat has barely started.
    game.players[0].dvp = 2;
    game.players[1].dvp = 19;
    assert!(
        ai.assigned_lane_is_lost(&game, 0),
        "two points against nineteen with fifty turns left is a lost race"
    );

    // Before the verdict turn the same board is not yet a verdict.
    game.turn = 100;
    assert!(
        !ai.assigned_lane_is_lost(&game, 0),
        "inside the first half the race is still open"
    );

    // And the gene is an exact no-op when off.
    game.turn = 200;
    ai.disable_lane_release_when_hopeless();
    assert!(
        !ai.assigned_lane_is_lost(&game, 0),
        "off, there is never a verdict"
    );
}

/// The verdict is never reached for a Score seat, which is the lane the
/// release falls back to. See `AdvancedAi::lane_release_when_hopeless`.
#[test]
fn the_score_lane_is_never_released() {
    let (mut game, _capital, _home) = empire_with_a_capital(71_505);
    game.max_turns = 250;
    game.turn = 240;
    game.players[0].dvp = 0;
    game.players[1].dvp = 19;

    let mut ai = AdvancedAi::new();
    ai.victory_target = Some(VictoryTarget::Score);
    ai.enable_lane_release_when_hopeless();
    assert!(
        !ai.assigned_lane_is_lost(&game, 0),
        "the score lane runs to the last turn"
    );
}

/// ★★★★ THE SEAT DECLARED AT ZERO GOLD. Live King seat
/// `civvis-20260826T112920Z` declared war on the Maori at turn 110 holding a
/// treasury that had been empty for fifty turns. See
/// `AdvancedAi::war_needs_a_treasury`.
#[test]
fn a_war_is_not_declared_out_of_an_empty_treasury() {
    let (mut game, _capital, _home) = empire_with_a_capital(71_506);
    let mut ai = AdvancedAi::new();
    ai.enable_war_needs_a_treasury();

    // Solvent: the veto is silent.
    game.players[0].gold = 400.0;
    game.players[0].gold_per_turn = 6.0;
    assert!(
        ai.war_is_affordable(&game, 0),
        "a solvent empire may still declare"
    );

    // The live shape at turn 110: nothing in the bank and a deficit.
    game.players[0].gold = 17.0;
    game.players[0].gold_per_turn = -7.0;
    assert!(
        !ai.war_is_affordable(&game, 0),
        "an empire inside the engine's own recovery condition may not"
    );

    // A full treasury that the deficit will drain before the campaign lands.
    game.players[0].gold = 150.0;
    game.players[0].gold_per_turn = -30.0;
    assert!(
        !ai.war_is_affordable(&game, 0),
        "a treasury that runs dry inside the solvency horizon may not either"
    );

    // Off, it is an exact no-op.
    ai.disable_war_needs_a_treasury();
    assert!(
        ai.war_is_affordable(&game, 0),
        "off, every war is affordable again"
    );
}

/// ★★★★ THE RACE IS OVER AND THE SEAT IS STILL PAYING THE ENTRY FEE. Once
/// `religions_founded()` reaches `max_religions()` with none of them ours, no
/// prophet this empire ever recruits can found anything. Live King seat
/// `civvis-20260826T112920Z`: four religions on a Small map capped at four,
/// none of them Rome's, and the seat finished holding a Holy Site, 228
/// Prophet points, two Missionaries and 6,329 unspendable Faith. See
/// `AdvancedAi::religion_race_is_closed`.
#[test]
fn a_religion_race_the_world_has_closed_stops_charging_the_empire() {
    // The live shape: six majors on a map whose cap the rivals fill.
    let mut game = Game::new_full(6, 74, 46, 91_302, 250, 0, false);
    game.current = 0;
    let cap = game.max_religions();
    assert!(
        cap > 0 && cap < game.players.len(),
        "the fixture needs a cap the rivals can fill without us: {cap}"
    );

    let mut ai = AdvancedAi::new();
    ai.victory_target = Some(VictoryTarget::Diplomacy);
    ai.enable_religion_race_is_closed();
    assert!(
        !ai.religion_race_closed_for(&game, 0),
        "an open race is not closed"
    );

    // `religions_founded` counts the players holding one, so filling the cap
    // means the rivals took every seat at the table.
    for rival in 1..=cap {
        game.players[rival].religion = Some(format!("faith_{rival}"));
    }
    assert!(
        game.religions_founded() >= cap,
        "the cap is reached: {} of {cap}",
        game.religions_founded()
    );
    assert!(
        ai.religion_race_closed_for(&game, 0),
        "every religion founded and none of them ours is a closed race"
    );

    // An empire that DID found one is untouched — it has a prize to defend.
    game.players[0].religion = Some("faith_ours".to_string());
    assert!(
        !ai.religion_race_closed_for(&game, 0),
        "our own religion is not a closed race"
    );
    game.players[0].religion = None;

    // Off, it is an exact no-op.
    ai.disable_religion_race_is_closed();
    assert!(
        !ai.religion_race_closed_for(&game, 0),
        "off, the race is never read as closed"
    );
}

#[test]
fn live_bridge_and_each_new_withhold_arm_change_the_host_controls() {
    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert!(live.parallel_settlers);
    assert!(live.base.parallel_settlers);
    assert!(live.base.host_settler_pop);
    assert!(live.base.explore_dead_targets);
    assert!(live.base.explore_commit);
    assert!(live.bank_envoys);
    assert!(live.base.bank_envoys);

    live.disable_parallel_settlers();
    live.disable_host_settler_pop();
    live.disable_explore_dead_targets();
    live.disable_explore_commit();
    live.disable_bank_envoys();
    assert!(!live.parallel_settlers);
    assert!(!live.base.parallel_settlers);
    assert!(!live.base.host_settler_pop);
    assert!(!live.base.explore_dead_targets);
    assert!(!live.base.explore_commit);
    assert!(!live.bank_envoys);
    assert!(!live.base.bank_envoys);
}

/// `first-luxury-first`: 46% of King city-turns are Amenity-short (11 games,
/// 7,071 city-turns, 2026-08-29) and 93.4% of that deficit sits on a luxury
/// the empire already OWNS and never improved — 47% of owned luxury tiles
/// were never improved at all, and the flat +14 `improvement_value_with_appeal`
/// pays for a worked luxury cannot tell the empire's FIRST Silk from its
/// fifth Wine. Off, the Builder walks to the Iron; under the gene, with three
/// Amenities missing, it opens the Silk first; and once a copy of that Silk is
/// held the premium is gone and the Iron wins again.
#[test]
fn the_builder_opens_the_empires_first_luxury_before_the_iron() {
    let (mut game, capital, home) = empire_with_a_capital(crate::rng::fixture_seed("AUTO_21", 43_000_001));
    for tech in ["irrigation", "mining", "bronze_working"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    let ring: Vec<Pos> = game.cities[&capital]
        .owned_tiles
        .iter()
        .copied()
        .filter(|pos| *pos != home)
        .collect();
    assert!(ring.len() >= 6, "fixture: a capital with a ring to work");
    let (silk, farmland, iron, spare) = (ring[0], ring[1], ring[2], ring[5]);
    // An owned, UNIMPROVED luxury: the tile the census says waits a median 21
    // turns for a charge, and 47% of the time never gets one.
    let plant_silk = |game: &mut Game, pos: Pos, improvement: Option<Name>| {
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = Some(crate::name!("forest"));
        tile.hills = false;
        tile.resource = Some(crate::name!("silk"));
        tile.improvement = improvement;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
    };
    plant_silk(&mut game, silk, None);
    {
        // Plain ground a Farm fits: the order the census counted 147 of
        // against 15 Plantations.
        let tile = game.map.tiles.get_mut(&farmland).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
    }
    {
        // An unopened strategic deposit, which the shipped scorer pays 30 for
        // — comfortably above a first luxury's flat 14.
        let tile = game.map.tiles.get_mut(&iron).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = true;
        tile.resource = Some(crate::name!("iron"));
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
    }
    // Displeased: `city_amenities_required` is ceil(pop / 2) against the
    // Palace's two.
    game.cities.get_mut(&capital).unwrap().pop = 10;
    let builder = game.spawn_unit("builder", 0, home);

    let mut ai = AdvancedAi::new();
    assert!(!ai.first_luxury_first, "the gene ships off");
    assert!(!AdvancedAi::legacy().first_luxury_first);
    assert_eq!(
        ai.empire_amenity_deficit(&game, 0),
        3.0,
        "fixture: a capital three Amenities short"
    );
    assert_eq!(
        ai.worthwhile_improvements(&game, 0, silk, GrandStrategy::Expansion)
            .first()
            .map(|name| name.to_string()),
        Some("plantation".to_string()),
        "fixture: the Plantation is the job the Silk tile is waiting for"
    );
    let plantation = ai.improvement_value(&game, silk, "plantation", GrandStrategy::Expansion);
    let farm = ai.improvement_value(&game, farmland, "farm", GrandStrategy::Expansion);
    assert_eq!(
        ai.improvement_value_for(&game, 0, silk, "plantation", GrandStrategy::Expansion),
        plantation,
        "off, the premium is not reached at all"
    );
    let ranked_off =
        ai.builder_jobs_ranked(&game, 0, builder, GrandStrategy::Expansion, &HashSet::new());
    assert_eq!(
        ranked_off.first(),
        Some(&iron),
        "off, the strategic deposit outranks an unopened luxury: \
         plantation {plantation:.1}, farm {farm:.1}"
    );

    ai.enable_first_luxury_first();
    assert!(ai.first_luxury_first);
    assert_eq!(
        ai.first_luxury_premium(&game, 0, silk, "plantation"),
        first_luxury::FIRST_COPY_BASE + first_luxury::FIRST_COPY_PER_AMENITY * 3.0,
        "the first copy is priced by the Amenities the empire is short"
    );
    assert_eq!(
        ai.first_luxury_premium(&game, 0, farmland, "farm"),
        0.0,
        "ordinary ground carries no premium"
    );
    assert_eq!(
        ai.first_luxury_premium(&game, 0, iron, "mine"),
        0.0,
        "a strategic deposit is not a luxury"
    );
    assert_eq!(
        ai.builder_jobs_ranked(&game, 0, builder, GrandStrategy::Expansion, &HashSet::new())
            .first(),
        Some(&silk),
        "under the gene the empire's first Silk is the Builder's first job"
    );

    // A second copy of the SAME luxury is worth what it always was: the gene
    // says "open the ones nobody has opened", not "improve more luxuries".
    plant_silk(&mut game, spare, Some(crate::name!("plantation")));
    assert_eq!(
        game.resource_access_count(0, "silk"),
        1,
        "fixture: the empire now holds one improved Silk"
    );
    assert_eq!(
        ai.first_luxury_premium(&game, 0, silk, "plantation"),
        0.0,
        "a duplicate copy keeps the shipped flat premium and nothing more"
    );
    assert_eq!(
        ai.builder_jobs_ranked(&game, 0, builder, GrandStrategy::Expansion, &HashSet::new())
            .first(),
        Some(&iron),
        "with the luxury held, the Builder is back on the shipped ranking"
    );

    ai.disable_first_luxury_first();
    assert!(!ai.first_luxury_first);
}

/// `first-luxury-first-2`: version 1 pays a first copy +26 even with every city
/// content; version 2 prices it only while the empire is actually Amenity-short,
/// and then exactly as version 1 does.
#[test]
fn the_second_version_prices_the_first_luxury_only_while_a_city_is_short() {
    let (mut game, capital, home) = empire_with_a_capital(43_000_002);
    game.players[0].techs.insert(Name::new("irrigation"));
    let silk = game.cities[&capital]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != home)
        .expect("fixture: a ring tile");
    {
        let tile = game.map.tiles.get_mut(&silk).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = Some(crate::name!("forest"));
        tile.hills = false;
        tile.resource = Some(crate::name!("silk"));
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
    }
    let armed = |second: bool| {
        let mut ai = AdvancedAi::new();
        if second {
            ai.enable_first_luxury_first_2();
        } else {
            ai.enable_first_luxury_first();
        }
        ai
    };
    assert!(
        !AdvancedAi::new().first_luxury_first_2,
        "the gene ships off"
    );
    assert_eq!(
        AdvancedAi::new().first_luxury_premium(&game, 0, silk, "plantation"),
        0.0,
        "off, no premium"
    );

    // Content: the Palace's two Amenities against ceil(2 / 2).
    game.cities.get_mut(&capital).unwrap().pop = 2;
    assert_eq!(
        armed(true).empire_amenity_deficit(&game, 0),
        0.0,
        "fixture: content"
    );
    assert_eq!(
        armed(false).first_luxury_premium(&game, 0, silk, "plantation"),
        first_luxury::FIRST_COPY_BASE,
        "version 1 pays the base to a content empire"
    );
    assert_eq!(
        armed(true).first_luxury_premium(&game, 0, silk, "plantation"),
        0.0,
        "version 2 leaves a content empire on the shipped flat +14"
    );

    // Three Amenities short: both versions price the first copy the same.
    game.cities.get_mut(&capital).unwrap().pop = 10;
    let short = armed(true);
    assert_eq!(
        short.empire_amenity_deficit(&game, 0),
        3.0,
        "fixture: three short"
    );
    assert_eq!(
        short.first_luxury_premium(&game, 0, silk, "plantation"),
        first_luxury::FIRST_COPY_BASE + first_luxury::FIRST_COPY_PER_AMENITY * 3.0,
    );
    assert_eq!(
        short.first_luxury_premium(&game, 0, silk, "plantation"),
        armed(false).first_luxury_premium(&game, 0, silk, "plantation"),
        "short, version 2 matches version 1"
    );
}

/// `live-move-refusal-break`: an issued step seen untaken on two judged turns
/// is barred, the pathing gate refuses it, any other step stays open, and the
/// bar expires. The proof needs no host channel — the step is ours and the
/// board shows the unit never took it.
#[test]
fn a_twice_refused_step_is_barred_and_the_route_must_change() {
    let mut g = Game::new_full(2, 24, 16, 7_925, 250, 1, false);
    g.current = 0;
    let mut ai = AdvancedAi::new();
    ai.enable_live_move_refusal_break();
    let uid = g.player_unit_ids(0)[0];
    let from = g.units[&uid].pos;
    let mut steps = g.wdisk(from, 1).into_iter().filter(|pos| *pos != from);
    let refused = steps.next().expect("a neighboring tile");
    let elsewhere = steps.next().expect("a second neighboring tile");

    ai.base.record_move_refusal_watch(&g, uid, from, refused);
    g.turn += 1;
    ai.base.judge_move_refusals(&g, 0);
    assert!(
        !ai.base.move_refusal_blocked(&g, uid),
        "one refused turn is a report, not a verdict"
    );

    ai.base.record_move_refusal_watch(&g, uid, from, refused);
    g.turn += 1;
    ai.base.judge_move_refusals(&g, 0);
    assert!(ai.base.move_refusal_blocked(&g, uid));
    assert!(
        !ai.base.path_step_allowed(&g, uid, from, refused, false),
        "the proved-refused step must not be proposed while barred"
    );
    assert!(
        ai.base.path_step_allowed(&g, uid, from, elsewhere, false),
        "only the refused step is barred; the route may bend anywhere else"
    );

    g.turn += g.standard_duration(crate::ai::MOVE_REFUSAL_BLOCK_TURNS) + 1;
    assert!(
        !ai.base.move_refusal_blocked(&g, uid),
        "a barred step is a lesson with an expiry, not a scar"
    );
}

/// A unit that actually moved wipes the slate: whatever held it is gone, and
/// the next refusal must build its own two-turn proof from scratch.
#[test]
fn a_unit_that_moved_wipes_the_refusal_slate() {
    let mut g = Game::new_full(2, 24, 16, 7_925, 250, 1, false);
    g.current = 0;
    let mut ai = AdvancedAi::new();
    ai.enable_live_move_refusal_break();
    let uid = g.player_unit_ids(0)[0];
    let here = g.units[&uid].pos;
    let step = g
        .wdisk(here, 1)
        .into_iter()
        .find(|pos| *pos != here)
        .expect("a neighboring tile");

    // Strike one on the true tile, then a watch entry recorded from a tile
    // the unit is no longer on — the judged turn sees it moved.
    ai.base.record_move_refusal_watch(&g, uid, here, step);
    g.turn += 1;
    ai.base.judge_move_refusals(&g, 0);
    let elsewhere = g
        .wdisk(here, 2)
        .into_iter()
        .find(|pos| *pos != here && *pos != step)
        .expect("a distinct origin");
    ai.base.record_move_refusal_watch(&g, uid, elsewhere, step);
    g.turn += 1;
    ai.base.judge_move_refusals(&g, 0);

    // The old strike must not survive the observed movement: one more
    // identical ask is a fresh first strike, not the second of two.
    ai.base.record_move_refusal_watch(&g, uid, here, step);
    g.turn += 1;
    ai.base.judge_move_refusals(&g, 0);
    assert!(
        !ai.base.move_refusal_blocked(&g, uid),
        "movement clears the count; a stale strike must not complete a bar"
    );
}

/// The recovery evacuates on the MEAN blow; a unit in the band between the
/// mean and the top of the roll stands, and dies to the roll. With the gene
/// on it leaves the raider's reach; off, the same board leaves it standing.
#[test]
fn a_unit_the_roll_could_kill_leaves_the_raiders_reach() {
    let Some((mut g, front, refuge, barbarian)) = wounded_out_of_reach_board(91_611) else {
        assert!(fixture_holds(false, "a board with a barbarian seat"));
        return;
    };
    let Some(lair) = far_side_of(&g, front, refuge) else {
        assert!(fixture_holds(false, "a far-side tile for the raider"));
        return;
    };
    let ours = g.spawn_test_unit("warrior", 0, front);
    let raider = g.spawn_test_unit("horseman", barbarian, lair);
    if !fixture_holds(
        g.attack_reach(raider).contains(&front) && g.unit_visible_to(raider, 0),
        "the raider is in reach of the front tile and in sight",
    ) {
        return;
    }
    let ai_off = AdvancedAi::new();
    let envelopes = ai_off.base.enemy_attack_envelopes(&g, 0);
    // The mean blow depends on the defender's hit points (a damaged unit
    // defends weaker), so walk the hit points down until they sit in the
    // band between the mean and the top of the roll.
    let mut band = None;
    for hp in (46..=100).rev() {
        g.units.get_mut(&ours).unwrap().hp = hp;
        let mean = BasicAi::incoming_damage(&g, 0, ours, front, &envelopes).total;
        if f64::from(hp) > mean && f64::from(hp) <= mean * crate::ai::COMBAT_ROLL_MAX {
            band = Some((hp, mean));
            break;
        }
    }
    let Some((hp, mean)) = band else {
        if !fixture_holds(false, "an hp in the roll band above the withdrawal line") {
            return;
        }
        unreachable!();
    };
    g.units.get_mut(&ours).unwrap().hp = hp;
    assert!(
        f64::from(hp) > mean,
        "the mean blow leaves it alive: {hp} vs {mean:.1}"
    );
    assert!(
        f64::from(hp) <= mean * crate::ai::COMBAT_ROLL_MAX,
        "the top of the roll does not: {hp} vs {:.1}",
        mean * crate::ai::COMBAT_ROLL_MAX
    );

    // Off: the recovery reads the mean and leaves it standing.
    let mut untouched = g.clone();
    assert_eq!(
        ai_off.wounded_out_of_reach_step(&mut untouched, 0, ours),
        None
    );
    assert_eq!(ai_off.base.retreat_step(&mut untouched, 0, ours), None);
    assert_eq!(
        untouched.units[&ours].pos, front,
        "the shipped recovery left it in reach"
    );

    // On: it leaves the reach.
    let mut ai_on = AdvancedAi::new();
    ai_on.enable_wounded_out_of_reach();
    ai_on.attach_journal(Journal::recording());
    assert_eq!(ai_on.wounded_out_of_reach_step(&mut g, 0, ours), Some(true));
    let after = g.units[&ours].pos;
    assert_ne!(after, front, "it moved");
    // Four movement points of cavalry cover most of an open plain; the
    // refuge a two-move warrior can reach is the city, where the blow lands
    // on the district, or failing that a tile outside the envelope.
    assert!(
        after == refuge || !g.attack_reach(raider).contains(&after),
        "it stands in the city or where the horseman cannot strike it: {after:?} (city {refuge:?})"
    );
}

/// A shooter with a melee raider two tiles away and nothing beside it steps
/// out of the raider's reach; with a warrior standing between them it holds.
#[test]
fn an_unscreened_shooter_steps_out_of_a_raiders_reach_and_a_screened_one_holds() {
    let Some((mut g, front, refuge, barbarian)) = wounded_out_of_reach_board(91_613) else {
        assert!(fixture_holds(false, "a board with a barbarian seat"));
        return;
    };
    let Some(lair) = far_side_of(&g, front, refuge) else {
        assert!(fixture_holds(false, "a far-side tile for the raider"));
        return;
    };
    let archer = g.spawn_test_unit("archer", 0, front);
    let raider = g.spawn_test_unit("warrior", barbarian, lair);
    if !fixture_holds(
        g.attack_reach(raider).contains(&front) && g.unit_visible_to(raider, 0),
        "the raider is in reach of the front tile and in sight",
    ) {
        return;
    }
    let ai_off = AdvancedAi::new();
    let mut untouched = g.clone();
    assert_eq!(
        ai_off.wounded_out_of_reach_step(&mut untouched, 0, archer),
        None
    );
    assert_eq!(
        ai_off.base.retreat_step(&mut untouched, 0, archer),
        None,
        "a full-health archer is nothing the recovery evacuates"
    );

    let mut ai_on = AdvancedAi::new();
    ai_on.enable_wounded_out_of_reach();
    ai_on.attach_journal(Journal::recording());

    // Screened: a warrior on the tile between them, no farther from the
    // raider than the archer is.
    let Some(between) = g.nbrs(front).into_iter().find(|position| {
        g.wdist(*position, lair) <= g.wdist(front, lair) && g.can_stop(archer, *position)
    }) else {
        assert!(fixture_holds(
            false,
            "a tile between the archer and the raider"
        ));
        return;
    };
    let mut screened = g.clone();
    let guard = screened.spawn_test_unit("warrior", 0, between);
    assert_eq!(screened.units[&guard].pos, between);
    assert_eq!(
        ai_on.wounded_out_of_reach_step(&mut screened, 0, archer),
        None,
        "with a melee unit beside it the archer keeps its ground"
    );

    // Unscreened: it leaves the reach.
    assert_eq!(
        ai_on.wounded_out_of_reach_step(&mut g, 0, archer),
        Some(true)
    );
    let after = g.units[&archer].pos;
    assert_ne!(after, front);
    assert!(
        !g.attack_reach(raider).contains(&after),
        "the archer ended its turn outside the warrior's reach: {after:?}"
    );
}

/// One attack this turn that kills the last thing in reach is the attack
/// scan's decision: the step stands down rather than walking away from a
/// kill.
#[test]
fn a_wounded_unit_that_can_finish_its_only_threat_is_left_to_the_attack_scan() {
    let Some((mut g, front, refuge, barbarian)) = wounded_out_of_reach_board(91_617) else {
        assert!(fixture_holds(false, "a board with a barbarian seat"));
        return;
    };
    let Some(adjacent) = g
        .nbrs(front)
        .into_iter()
        .find(|position| g.wdist(*position, refuge) >= 2)
    else {
        assert!(fixture_holds(
            false,
            "a neighbour of the front tile away from the city"
        ));
        return;
    };
    let ours = g.spawn_test_unit("warrior", 0, front);
    g.units.get_mut(&ours).unwrap().hp = 40;
    let raider = g.spawn_test_unit("warrior", barbarian, adjacent);
    g.units.get_mut(&raider).unwrap().hp = 5;
    if !fixture_holds(
        g.attack_reach(raider).contains(&front) && g.unit_visible_to(raider, 0),
        "the raider is in reach of the front tile and in sight",
    ) {
        return;
    }
    let can_attack = g
        .legal_actions_within(0, crate::game::ActionFamilies::UNITS)
        .into_iter()
        .any(|action| matches!(action, Action::Attack { unit, .. } if unit == ours));
    if !fixture_holds(can_attack, "an attack on the adjacent raider is legal") {
        return;
    }
    let mut ai_on = AdvancedAi::new();
    ai_on.enable_wounded_out_of_reach();
    assert_eq!(
        ai_on.wounded_out_of_reach_step(&mut g, 0, ours),
        None,
        "a 5 HP raider beside a 40 HP warrior is a kill, not a reason to withdraw"
    );
    assert_eq!(g.units[&ours].pos, front, "and nothing moved");
}

/// See `campus_through_expansion`. A Science seat whose plan still reads
/// Expansion priced its Campus as a generic Expansion district — no
/// strategic arm, a beaker at 1.2 instead of 4.2 — and lost every queue to
/// the Plaza, walls and an Arena-credited Entertainment Complex (live
/// Emperor game 20260901T175154Z: four cities, zero districts at turn 64).
/// Treated, the Campus and its Library carry the Science lane's price while
/// the plan reads Expansion, an Entertainment Complex may not be a city's
/// first specialty district, and a seat on any other lane feels nothing.
#[test]
fn the_campus_keeps_its_science_price_through_the_expansion_phase() {
    let mut game = Game::new(2, 32, 24, 5_414, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    game.players[0].techs.insert(crate::name!("writing"));
    game.cities.get_mut(&city).unwrap().pop = 3;
    game.turn = 40;
    game.max_turns = 250;

    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 15,
        assessed_turn: game.turn,
        rush: false,
    };
    let campus = Item::District {
        district: crate::name!("campus"),
        pos: game.cities[&city].pos,
    };
    let complex = Item::District {
        district: crate::name!("entertainment_complex"),
        pos: game.cities[&city].pos,
    };
    let library = Item::Building {
        building: crate::name!("library"),
    };

    let mut treated = AdvancedAi::new();
    treated.enable_live_bridge_universe();
    treated.enable_campus_through_expansion();
    treated.victory_target = Some(VictoryTarget::Science);
    treated.refresh_research_weight(&game);
    let mut withheld = AdvancedAi::new();
    withheld.enable_live_bridge_universe();
    withheld.disable_campus_through_expansion();
    withheld.victory_target = Some(VictoryTarget::Science);
    withheld.refresh_research_weight(&game);
    let counts = treated.counts(&game, 0);

    let lifted = treated.production_value(&game, 0, city, &campus, &plan, &counts);
    let stock = withheld.production_value(&game, 0, city, &campus, &plan, &counts);
    assert!(
        lifted > stock,
        "the Campus carries the Science lane's price through the Expansion plan: \
         {lifted} treated vs {stock} withheld"
    );
    assert_eq!(
        treated.production_value(&game, 0, city, &complex, &plan, &counts),
        -10_000.0,
        "an Entertainment Complex may not be a city's first specialty district"
    );
    assert!(
        withheld.production_value(&game, 0, city, &complex, &plan, &counts) > -10_000.0,
        "withheld, the Entertainment Complex is priced as before"
    );

    // Every other lane keeps the plan's price exactly.
    let mut culture = AdvancedAi::new();
    culture.enable_live_bridge_universe();
    culture.enable_campus_through_expansion();
    culture.victory_target = Some(VictoryTarget::Culture);
    culture.refresh_research_weight(&game);
    let mut culture_off = AdvancedAi::new();
    culture_off.enable_live_bridge_universe();
    culture_off.disable_campus_through_expansion();
    culture_off.victory_target = Some(VictoryTarget::Culture);
    culture_off.refresh_research_weight(&game);
    assert_eq!(
        culture.production_value(&game, 0, city, &campus, &plan, &counts),
        culture_off.production_value(&game, 0, city, &campus, &plan, &counts),
        "a Culture seat's Campus is priced exactly as before"
    );

    // Once the Campus stands, its Library keeps the Science beaker weight and
    // the Entertainment Complex is a legal second district again.
    install_ai_test_district(&mut game, city, "campus");
    let counts = treated.counts(&game, 0);
    assert!(
        treated.production_value(&game, 0, city, &library, &plan, &counts)
            > withheld.production_value(&game, 0, city, &library, &plan, &counts),
        "the Library carries the Science lane's beaker weight through the Expansion plan"
    );
    assert!(
        treated.production_value(&game, 0, city, &complex, &plan, &counts) > -10_000.0,
        "with a specialty district standing the Entertainment Complex is priced again"
    );

    // Defaults: off for the stock and frozen controllers.
    assert!(!AdvancedAi::new().campus_through_expansion);
    assert!(!AdvancedAi::legacy().campus_through_expansion);
}

/// See `trade_route_network`. The Science contract priced every Commercial
/// Hub at −10,000 and the Science district table had no Hub arm, so 22 live
/// runs built none and ran the whole game on one Trade Route. Treated, a Hub
/// beside a standing Campus is priced as trade capacity (and still refused
/// in a city without one), a Market is worth the route it adds unless a
/// Lighthouse already adds it, and the Conquest plan pays for a Hub.
#[test]
fn a_hub_beside_a_campus_joins_the_science_lanes_trade_network() {
    let mut game = Game::new(2, 32, 24, 5_414, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    game.players[0].techs.insert(crate::name!("currency"));
    game.cities.get_mut(&city).unwrap().pop = 5;
    game.max_turns = 250;
    game.turn = game.max_turns / 2;

    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 8,
        assessed_turn: game.turn,
        rush: false,
    };
    let hub = Item::District {
        district: crate::name!("commercial_hub"),
        pos: game.cities[&city].pos,
    };
    let market = Item::Building {
        building: crate::name!("market"),
    };

    let mut treated = AdvancedAi::new();
    treated.enable_live_bridge_universe();
    treated.enable_trade_route_network();
    treated.victory_target = Some(VictoryTarget::Science);
    treated.refresh_research_weight(&game);
    let mut withheld = AdvancedAi::new();
    withheld.enable_live_bridge_universe();
    withheld.disable_trade_route_network();
    withheld.victory_target = Some(VictoryTarget::Science);
    withheld.refresh_research_weight(&game);
    let counts = treated.counts(&game, 0);

    assert_eq!(
        treated.production_value(&game, 0, city, &hub, &plan, &counts),
        -10_000.0,
        "the Campus keeps first claim: no Hub in a city without one"
    );

    install_ai_test_district(&mut game, city, "campus");
    let counts = treated.counts(&game, 0);
    let priced = treated.production_value(&game, 0, city, &hub, &plan, &counts);
    assert_eq!(
        withheld.production_value(&game, 0, city, &hub, &plan, &counts),
        -10_000.0,
        "withheld, the Science contract still refuses every Hub"
    );
    assert!(
        priced > 0.0,
        "beside a standing Campus the Hub is priced as trade capacity: {priced}"
    );

    let route = treated.production_value(&game, 0, city, &market, &plan, &counts)
        - withheld.production_value(&game, 0, city, &market, &plan, &counts);
    // The building score is normalised by its build time after the sum, so
    // the route shows as a strictly positive uplift, not the raw constant.
    assert!(
        route > 0.0,
        "a Market is worth the Trade Route it adds: {route}"
    );
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("lighthouse"));
    assert_eq!(
        treated.production_value(&game, 0, city, &market, &plan, &counts),
        withheld.production_value(&game, 0, city, &market, &plan, &counts),
        "a Lighthouse already adds this city's route, so the Market's is void"
    );

    let conquest = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        ..plan
    };
    let mut war = AdvancedAi::new();
    war.enable_live_bridge_universe();
    war.enable_trade_route_network();
    war.refresh_research_weight(&game);
    let mut war_off = AdvancedAi::new();
    war_off.enable_live_bridge_universe();
    war_off.disable_trade_route_network();
    war_off.refresh_research_weight(&game);
    let war_hub = war.production_value(&game, 0, city, &hub, &conquest, &counts)
        - war_off.production_value(&game, 0, city, &hub, &conquest, &counts);
    assert!(
        war_hub > 0.0,
        "the Conquest plan pays for the Hub: {war_hub}"
    );

    // Defaults: off for the stock and frozen controllers.
    assert!(!AdvancedAi::new().trade_route_network);
    assert!(!AdvancedAi::legacy().trade_route_network);
}

/// See `industrial_chain_debt`. Nine Industrial Zones stood with one
/// Factory in the best Emperor game because the production chain, unlike
/// the research chain, was owed nothing. This keeps the independently
/// screenable adaptive arm honest: treated, a Workshop in a city that holds
/// an Industrial Zone is owed the flat debt, a Factory is additionally worth
/// the own cities within its range, and a city without the district prices
/// both exactly as before.
#[test]
fn an_industrial_zone_owes_its_workshop_and_a_factory_reaches_its_neighbours() {
    let mut game = Game::new(2, 32, 24, 5_414, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let first = game.player_city_ids(0)[0];
    let second = found_test_city(&mut game, 0);
    game.players[0]
        .techs
        .insert(crate::name!("industrialization"));
    game.cities.get_mut(&first).unwrap().pop = 6;
    game.turn = 120;
    game.max_turns = 250;

    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 8,
        assessed_turn: game.turn,
        rush: false,
    };
    let workshop = Item::Building {
        building: crate::name!("workshop"),
    };
    let factory = Item::Building {
        building: crate::name!("factory"),
    };

    let mut treated = AdvancedAi::new();
    treated.enable_live_bridge_universe();
    treated.enable_industrial_chain_debt();
    treated.refresh_research_weight(&game);
    let mut withheld = AdvancedAi::new();
    withheld.enable_live_bridge_universe();
    withheld.disable_industrial_chain_debt();
    withheld.refresh_research_weight(&game);
    let counts = treated.counts(&game, 0);

    assert_eq!(
        treated.production_value(&game, 0, first, &workshop, &plan, &counts),
        withheld.production_value(&game, 0, first, &workshop, &plan, &counts),
        "without an Industrial Zone nothing is owed"
    );

    install_ai_test_district(&mut game, first, "industrial_zone");
    let counts = treated.counts(&game, 0);
    let owed = treated.production_value(&game, 0, first, &workshop, &plan, &counts)
        - withheld.production_value(&game, 0, first, &workshop, &plan, &counts);
    // The building score is normalised by its build time after the sum, so
    // the debt shows as a strictly positive uplift, not the raw constant.
    assert!(
        owed > 0.0,
        "a standing Industrial Zone owes its Workshop the flat debt: {owed}"
    );

    game.cities
        .get_mut(&first)
        .unwrap()
        .buildings
        .push(crate::name!("workshop"));
    let factory_uplift = treated.production_value(&game, 0, first, &factory, &plan, &counts)
        - withheld.production_value(&game, 0, first, &factory, &plan, &counts);
    assert!(
        factory_uplift > 0.0,
        "a Factory in a city holding the district is owed too: {factory_uplift}"
    );
    let factory_name = crate::name!("factory");
    let spec = &game.rules.buildings[&factory_name];
    let neighbours =
        game.wdist(game.cities[&first].pos, game.cities[&second].pos) <= spec.regional_range;
    let reach = treated.regional_production_reach(
        &game,
        0,
        &game.cities[&first],
        &factory_name,
        spec,
        plan.strategy,
    );
    if neighbours {
        assert!(
            reach > 0.0,
            "a Factory is worth the own city inside its range: {reach}"
        );
        // A standing copy that already reaches the neighbour makes a second
        // Factory's reach void.
        game.cities
            .get_mut(&second)
            .unwrap()
            .buildings
            .push(factory_name);
        assert_eq!(
            treated.regional_production_reach(
                &game,
                0,
                &game.cities[&first],
                &factory_name,
                spec,
                plan.strategy,
            ),
            0.0,
            "a neighbour a standing Factory already reaches gains nothing from a second"
        );
    } else {
        assert_eq!(
            reach, 0.0,
            "with no city in range there is nothing to reach"
        );
    }

    // Defaults: off for the stock and frozen controllers.
    assert!(!AdvancedAi::new().industrial_chain_debt);
    assert!(!AdvancedAi::legacy().industrial_chain_debt);
}

#[test]
fn settler_target_floor_charges_the_walk_and_holds_a_floor() {
    use super::super::settler_target_floor::{
        SETTLER_TARGET_FREE_TILES, SETTLER_TARGET_WORTH_FLOOR,
    };
    let g = Game::new_full(2, 40, 20, 11, 50, 1, false);
    let from = (5, 10);
    let near = (7, 10);
    let far = from.0 + SETTLER_TARGET_FREE_TILES + 12;
    let far = (far, 10);
    assert!(
        g.wdist(from, far) > SETTLER_TARGET_FREE_TILES,
        "the fixture's far site is far"
    );
    // Inside the free ring a site is judged on its worth alone.
    assert_eq!(
        AdvancedAi::settler_target_worth_after_travel(&g, from, near, 13.8),
        13.8
    );
    // Beyond it every tile costs the extra-travel price.
    let extra = (g.wdist(from, far) - SETTLER_TARGET_FREE_TILES) as f64;
    assert_eq!(
        AdvancedAi::settler_target_worth_after_travel(&g, from, far, 17.7),
        17.7 - extra * SETTLER_EXTRA_TRAVEL_PRICE
    );

    let mut ai = AdvancedAi::new();
    assert!(!ai.settler_target_floor, "an OptIn gene ships off");
    // Off: every site clears, the run 182050Z marches included.
    assert!(ai.settler_target_clears_floor(&g, from, far, -35.0));
    ai.enable_settler_target_floor();
    assert!(ai.settler_target_floor);
    // On: nothing is worth marching to a site worth nothing …
    assert!(!ai.settler_target_clears_floor(&g, from, near, -4.1));
    assert!(!ai.settler_target_clears_floor(&g, from, far, -35.0));
    // … a far marginal site is not worth the walk …
    assert!(!ai.settler_target_clears_floor(&g, from, far, 17.7));
    // … a near marginal one still is (a city beats no city) …
    assert!(ai.settler_target_clears_floor(&g, from, near, 13.8));
    // … and a rich far site clears with the walk charged.
    assert!(ai.settler_target_clears_floor(&g, from, far, 96.4));
    // A site worth exactly the floor, inside the ring, is the least a target may be.
    assert!(ai.settler_target_clears_floor(&g, from, near, SETTLER_TARGET_WORTH_FLOOR));
    ai.disable_settler_target_floor();
    assert!(ai.settler_target_clears_floor(&g, from, far, -35.0));
}

#[test]
fn settler_walk_deadline_is_off_by_default_and_toggles() {
    assert!(
        !AdvancedAi::new().settler_walk_deadline,
        "an opt-in ships off"
    );
    assert!(!AdvancedAi::legacy().settler_walk_deadline);
    let mut ai = AdvancedAi::new();
    ai.enable_settler_walk_deadline();
    assert!(ai.settler_walk_deadline);
    ai.disable_settler_walk_deadline();
    assert!(!ai.settler_walk_deadline);
    assert!(
        GENES
            .iter()
            .any(|gene| gene.tag == "settler-walk-deadline" && gene.opt_in()),
        "registered as an opt-in"
    );
}

#[test]
fn a_settler_inside_its_deadline_is_left_to_the_ordinary_step() {
    let (mut game, settler) = walk_deadline_board(93_201);
    let mut ai = AdvancedAi::new();
    ai.enable_settler_walk_deadline();
    let cities = game.cities.len();
    // No clock yet: the first step starts it at zero turns out.
    assert_eq!(ai.settler_walk_deadline_step(&mut game, 0, settler), None);
    assert_eq!(ai.settler_turns_out(settler), 0);
    // A clock one turn short of the deadline is still inside it.
    let deadline = AdvancedAi::settler_walk_deadline_turns(&game);
    assert!(
        deadline >= 2,
        "the deadline is turns, not a moment: {deadline}"
    );
    game.turn += 1;
    walk_out(&mut ai, &game, settler, deadline - 1);
    assert_eq!(ai.settler_walk_deadline_step(&mut game, 0, settler), None);
    assert_eq!(
        game.cities.len(),
        cities,
        "nothing founded inside the deadline"
    );
    assert!(game.units.contains_key(&settler));
    // A walk that began after the band turn is the ordinary search's, however
    // long it runs: the gene is about the opening.
    game.turn += 1;
    ai.settler_walk_clock.insert(
        settler,
        (
            AdvancedAi::expansion_band_turn(&game) + 1,
            game.turn,
            deadline + 5,
        ),
    );
    assert_eq!(ai.settler_walk_deadline_step(&mut game, 0, settler), None);
    assert_eq!(game.cities.len(), cities);
}

/// See `campus_before_halfway`. Since #3124 gated specialization to the
/// second half of the clock, a Science seat whose plan already reads Science
/// (its sites taken, expansion done) priced its own Campus at zero to the
/// lane for 125 of 250 turns. Treated, the Campus carries the lane's 170
/// before halfway exactly as it does after; after halfway nothing changes;
/// a Culture seat feels nothing.
#[test]
fn the_campus_keeps_its_lane_arm_before_the_halfway_clock() {
    let mut game = Game::new(2, 32, 24, 5_414, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    game.players[0].techs.insert(crate::name!("writing"));
    game.cities.get_mut(&city).unwrap().pop = 3;
    game.max_turns = 250;

    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: 40,
        rush: false,
    };
    let campus = Item::District {
        district: crate::name!("campus"),
        pos: game.cities[&city].pos,
    };

    let seat = |on: bool, target: VictoryTarget, game: &Game| {
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge_universe();
        if on {
            ai.enable_campus_before_halfway();
        } else {
            ai.disable_campus_before_halfway();
        }
        ai.victory_target = Some(target);
        ai.refresh_research_weight(game);
        ai
    };

    game.turn = 40;
    let treated = seat(true, VictoryTarget::Science, &game);
    let withheld = seat(false, VictoryTarget::Science, &game);
    let counts = treated.counts(&game, 0);
    let lifted = treated.production_value(&game, 0, city, &campus, &plan, &counts);
    let stock = withheld.production_value(&game, 0, city, &campus, &plan, &counts);
    assert!(
        lifted > stock,
        "before halfway the Science seat's Campus carries the lane's arm: \
         {lifted} treated vs {stock} withheld"
    );

    // After halfway the shipped arm already pays 170; the gene adds nothing.
    game.turn = 130;
    let treated_late = seat(true, VictoryTarget::Science, &game);
    let withheld_late = seat(false, VictoryTarget::Science, &game);
    let counts_late = treated_late.counts(&game, 0);
    assert_eq!(
        treated_late.production_value(&game, 0, city, &campus, &plan, &counts_late),
        withheld_late.production_value(&game, 0, city, &campus, &plan, &counts_late),
        "after halfway the Campus is priced exactly as before"
    );

    // The arm follows the plan, as the shipped post-halfway arm does: a
    // Culture plan's Campus is priced exactly as before.
    game.turn = 40;
    let culture_plan = StrategicPlan {
        strategy: GrandStrategy::Culture,
        ..plan
    };
    let culture = seat(true, VictoryTarget::Culture, &game);
    let culture_off = seat(false, VictoryTarget::Culture, &game);
    let counts_culture = culture.counts(&game, 0);
    assert_eq!(
        culture.production_value(&game, 0, city, &campus, &culture_plan, &counts_culture),
        culture_off.production_value(&game, 0, city, &campus, &culture_plan, &counts_culture),
        "a Culture plan's Campus is priced exactly as before"
    );
}

#[test]
fn envoy_dividends_remains_a_drawable_opt_in_gene() {
    let gene = crate::ai::gene("envoy-building-dividends").unwrap();
    assert!(gene.opt_in() && gene.screenable());
    let mut ai = AdvancedAi::new();
    assert!(!ai.envoy_building_dividends);
    assert!(!AdvancedAi::legacy().envoy_building_dividends);
    (gene.enable)(&mut ai);
    assert!(ai.envoy_building_dividends);
    (gene.disable)(&mut ai);
    assert!(!ai.envoy_building_dividends);
}

#[test]
fn hostile_memory_versions_are_exclusive_and_opt_in() {
    let mut ai = AdvancedAi::new();
    assert!(!ai.hostile_memory && !ai.hostile_memory_2);
    ai.enable_hostile_memory();
    ai.enable_hostile_memory_2();
    assert!(!ai.hostile_memory && ai.hostile_memory_2);
    ai.enable_hostile_memory();
    assert!(ai.hostile_memory && !ai.hostile_memory_2);
    ai.disable_hostile_memory();
    assert!(!ai.hostile_memory && !ai.hostile_memory_2);
    assert!(GENES
        .iter()
        .any(|g| g.tag == "hostile-memory-2" && g.opt_in()));
}

#[test]
fn scout_first_opening_registry_toggles_both_governors() {
    let gene = crate::ai::GENES
        .iter()
        .find(|gene| gene.tag == "scout-first-opening")
        .unwrap();
    assert!(gene.opt_in());
    let mut ai = AdvancedAi::new();
    assert!(!ai.scout_first_opening);
    assert!(!ai.base.scout_first_opening);
    assert!(!AdvancedAi::legacy().scout_first_opening);
    ai.disable_recon_replacement();
    (gene.enable)(&mut ai);
    assert!(ai.scout_first_opening);
    assert!(ai.base.scout_first_opening);
    assert!(!ai.base.recon_replacement);
    (gene.disable)(&mut ai);
    assert!(!ai.scout_first_opening);
    assert!(!ai.base.scout_first_opening);
}

#[test]
fn battle_recovery_follows_surviving_units_across_a_live_id_rebuild() {
    let mut ai = AdvancedAi::new();
    // The rebuilt board reuses the old wounded unit's id for a healthy
    // survivor, while another recovering unit has died. Recovery belongs
    // to the surviving host identity, not either reused native number.
    ai.battle_planner_recovering.extend([7, 9]);
    ai.remap_unit_memory(&BTreeMap::from([(7, 8), (8, 7)]));
    assert_eq!(ai.battle_planner_recovering, BTreeSet::from([8]));
    ai.remap_unit_memory(&BTreeMap::from([(8, 12), (7, 13)]));
    assert_eq!(ai.battle_planner_recovering, BTreeSet::from([12]));
}

/// ★★★★ THE PLAZA IS BUILT AT TURN 112 AND THE OPENING NEEDED IT AT 40.
///
/// Over the 38 recorded live runs of 2026-09-10/11 a `DISTRICT_GOVERNMENT`
/// stands in 25 at a median turn 112 and the Ancestral Hall in 8 at a median
/// turn 119, while the fourth city the opening band needs by turn 60 arrives at
/// a median turn 77. `expansion_hall` prices the Hall and says in its own note
/// that the plaza's placement keeps its price, so the plot the Hall stands on
/// scores like an empty one. See `expansion_hall_district`.
#[test]
fn an_expansion_district_is_worth_the_hall_it_hosts() {
    let (game, capital, _home) = empire_with_a_capital(71_117);
    let hall = &game.rules.buildings["ancestral_hall"];
    let expected = (hall.effects["free_builder_new_city"] * EXPANSION_HALL_BUILDER_VALUE
        + (hall.effects["settler_production_pct"] / 50.0) * EXPANSION_HALL_SETTLER_VALUE)
        * HOSTED_EXPANSION_DISCOUNT;

    assert_eq!(
        AdvancedAi::hosted_expansion_path(&game, 0, &crate::name!("government_plaza")),
        expected,
        "the Ancestral Hall is the land-grab building the plaza hosts"
    );
    assert_eq!(
        AdvancedAi::hosted_expansion_path(&game, 0, &crate::name!("campus")),
        0.0,
        "a Campus hosts nothing that pays the land grab"
    );

    let site = game
        .district_sites(capital, crate::name!("government_plaza"))
        .into_iter()
        .next()
        .expect("the capital has a plot for a Government Plaza");
    let plaza = Item::District {
        district: crate::name!("government_plaza"),
        pos: site,
    };
    let short = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 9,
        assessed_turn: game.turn,
        rush: false,
    };
    let counts = EmpireCounts::default();

    let ordinary = AdvancedAi::new();
    assert!(!ordinary.expansion_hall_district, "the gene ships off");
    let frozen = AdvancedAi::legacy();
    assert!(!frozen.expansion_hall_district);
    let stock = ordinary.production_value(&game, 0, capital, &plaza, &short, &counts);
    assert_eq!(
        frozen.production_value(&game, 0, capital, &plaza, &short, &counts),
        stock,
        "the frozen anchor prices the plaza exactly as the ordinary controller"
    );

    let mut treated = AdvancedAi::new();
    treated.enable_expansion_hall_district();
    let paid = treated.production_value(&game, 0, capital, &plaza, &short, &counts);
    assert!(
        paid > stock,
        "a plaza worth nothing on its own is worth the Hall it will host: \
         {paid} against {stock}"
    );

    // ⭐ AND THE CREDIT FADES WITH THE SHORTFALL, on the building arm's own
    // scale, so the district's term reaches zero on the turn the Hall's does.
    let met = StrategicPlan {
        desired_cities: 0,
        ..short.clone()
    };
    assert_eq!(
        treated.production_value(&game, 0, capital, &plaza, &met, &counts),
        ordinary.production_value(&game, 0, capital, &plaza, &met, &counts),
        "an empire no longer short of seats prices the plaza as stock does"
    );

    // And nothing else moves: a district that hosts no land-grab building is
    // priced identically with the gene on.
    if let Some(campus_site) = game
        .district_sites(capital, crate::name!("campus"))
        .into_iter()
        .next()
    {
        let campus = Item::District {
            district: crate::name!("campus"),
            pos: campus_site,
        };
        assert_eq!(
            treated.production_value(&game, 0, capital, &campus, &short, &counts),
            ordinary.production_value(&game, 0, capital, &campus, &short, &counts),
            "a Campus is priced the same with the gene on"
        );
    }
}

/// See `AdvancedAi::breaker_commitment_holds`: a queued siege gun with
/// production in it keeps its city's queue against the strategic scorer,
/// unless the city is threatened or nothing is invested yet.
#[test]
fn a_reserved_gun_keeps_its_queue_against_the_scorer() {
    let mut game = Game::new(2, 24, 16, 61_501, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let gun = Item::Unit {
        unit: crate::name!("catapult"),
    };
    assert!(game.rules.units[&crate::name!("catapult")].siege);
    {
        let c = game.cities.get_mut(&city).unwrap();
        c.queue = vec![gun.clone()];
        c.production = 20.0;
    }
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    assert!(
        !ai.breaker_commitment_holds(&game, city, &gun, &plan),
        "off by default"
    );
    ai.enable_breaker_keeps_its_queue();
    assert!(ai.breaker_commitment_holds(&game, city, &gun, &plan));
    let threatened = StrategicPlan {
        threatened_city: Some(city),
        ..plan.clone()
    };
    assert!(
        !ai.breaker_commitment_holds(&game, city, &gun, &threatened),
        "a threatened city gives it up"
    );
    game.cities.get_mut(&city).unwrap().production = 0.0;
    assert!(
        !ai.breaker_commitment_holds(&game, city, &gun, &plan),
        "nothing invested yet"
    );
    let horse = Item::Unit {
        unit: crate::name!("horseman"),
    };
    assert!(
        !ai.breaker_commitment_holds(&game, city, &horse, &plan),
        "not a gun"
    );
}

#[test]
fn stalled_settler_takes_a_safe_site_is_off_by_default_and_toggles() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "stalled-settler-takes-a-safe-site",
        |ai| ai.stalled_settler_takes_a_safe_site,
    );
    let mut ai = AdvancedAi::new();
    ai.enable_stalled_settler_takes_a_safe_site();
    assert!(ai.stalled_settler_takes_a_safe_site);
    ai.disable_stalled_settler_takes_a_safe_site();
    assert!(!ai.stalled_settler_takes_a_safe_site);
}

#[test]
fn a_settler_inside_its_walk_allowance_or_with_the_gene_off_is_left_alone() {
    let (mut game, settler) = walk_deadline_board(93_401);
    let cities = game.cities.len();
    let allowance = stalled_allowance(&game);
    assert!(allowance >= 2, "the allowance is turns: {allowance}");
    game.turn += 1;
    let mut ai = AdvancedAi::new();
    ai.enable_stalled_settler_takes_a_safe_site();
    walk_out(&mut ai, &game, settler, allowance - 1);
    assert_eq!(ai.stalled_settler_step(&mut game, 0, settler), None);
    // Off: never acts, however long the walk.
    let mut off = AdvancedAi::new();
    walk_out(&mut off, &game, settler, allowance + 20);
    assert_eq!(off.stalled_settler_step(&mut game, 0, settler), None);
    assert_eq!(game.cities.len(), cities);
    assert!(!off.settler_targets.contains_key(&settler));
}

/// Game 355's shape: a Settler hovering beside its capital, where every tile
/// within two is too near a city for `settler-walk-deadline` to found, still
/// finds a legal site a few tiles out and walks to it.
#[test]
fn a_stalled_settler_beside_its_city_takes_a_site_the_deadline_cannot_reach() {
    let (mut game, home) = camp_bounty_board(93_402);
    let near = open_ground_at(&game, home, 1);
    let settler = game.spawn_test_unit("settler", 0, near);
    let mut ai = AdvancedAi::new();
    ai.enable_settler_walk_deadline();
    ai.enable_stalled_settler_takes_a_safe_site();
    let journal = crate::reasoning::Journal::recording();
    ai.attach_journal(journal.handle());
    game.turn += 1;
    let allowance = stalled_allowance(&game);
    walk_out(&mut ai, &game, settler, allowance + 1);
    assert_eq!(
        ai.settler_walk_deadline_site(&game, 0, settler),
        None,
        "every tile within two of a Settler beside its capital is too near the city"
    );
    let (site, _) = ai
        .stalled_settler_site(&game, 0, settler)
        .expect("a legal, safe site within four tiles of the capital's doorstep");
    assert!(game.wdist(site, near) <= STALLED_SETTLER_RADIUS);
    assert!(
        ai.base.valid_settle_site(&game, 0, site),
        "the pick is a legal city site"
    );
    let cities = game.cities.len();
    let acted = ai.stalled_settler_step(&mut game, 0, settler);
    assert_eq!(
        acted,
        Some(true),
        "the stalled Settler walks to its new site"
    );
    assert_eq!(game.cities.len(), cities, "nothing founded on the doorstep");
    assert_eq!(ai.settler_targets.get(&settler), Some(&site));
    assert!(
        game.wdist(game.units[&settler].pos, site) < game.wdist(near, site),
        "the step makes progress"
    );
    let lines: Vec<String> = journal
        .since(0)
        .thoughts
        .iter()
        .map(|thought| format!("{} | {}", thought.headline, thought.detail))
        .collect();
    assert!(
        lines
            .iter()
            .any(|line| line.contains("Stalled settler takes the safe site")),
        "the pick is journaled; journal:\n{}",
        lines.join("\n")
    );
}

/// `staging-reaches-the-border`: a target whose closed territory runs five
/// tiles out from its city leaves the 3-5 staging band no stand at peace.
/// With the gene the band widens until it holds six stands our army can
/// reach (the sixth ring, all outside the border and open from our city); a
/// tile inside the territory still never stages.
#[test]
fn the_staging_band_reaches_past_a_deep_closed_border_only_under_the_gene() {
    let mut g = Game::new_full(2, 40, 30, 388_111_334, 300, 0, false);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let settler = g
        .player_unit_ids(1)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let city = g.found_city_for(1, g.units[&settler].pos, None);
    let objective = g.cities[&city].pos;
    for tile in g.map.tiles.values_mut() {
        let (dq, dr) = (tile.pos.0 - objective.0, tile.pos.1 - objective.1);
        if (dq.abs() + dr.abs() + (dq + dr).abs()) / 2 <= 5 {
            tile.owner_city = Some(city);
        }
    }
    g.players[1].borders_enforced = Some(true);
    let ours = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let home = g.units[&ours].pos;
    assert!(
        g.wdist(home, objective) > 9,
        "fixture: our city stands clear of the band"
    );
    g.found_city_for(0, home, None);
    let soldier = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind != "settler")
        .unwrap_or_else(|| g.spawn_test_unit("warrior", 0, (0, 0)));
    let at = |d: i32| {
        g.wring(objective, d)
            .into_iter()
            .find(|pos| g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile)))
            .expect("a land tile at that distance")
    };
    let (inside, six, seven) = (at(4), at(6), at(7));
    let mut ai = AdvancedAi::new();
    assert_eq!(ai.campaign_staging_reach(&g, 0, 1, objective), 5);
    assert!(
        !ai.campaign_staging_position(&g, 0, 1, soldier, objective, six),
        "off: six out is no stand"
    );
    ai.enable_staging_reaches_the_border();
    assert_eq!(
        ai.campaign_staging_reach(&g, 0, 1, objective),
        6,
        "the sixth ring holds the stands"
    );
    assert!(
        ai.campaign_staging_position(&g, 0, 1, soldier, objective, six),
        "on: six out stages"
    );
    assert!(
        !ai.campaign_staging_position(&g, 0, 1, soldier, objective, seven),
        "on: no further than it needs"
    );
    assert!(
        !ai.campaign_staging_position(&g, 0, 1, soldier, objective, inside),
        "never inside the closed territory"
    );
}

#[test]
fn districts_price_the_improvement_is_a_registered_reversible_opt_in() {
    assert!(GENES.iter().any(|gene| gene.opt_in()
        && gene.field == "districts_price_the_improvement"
        && gene.tag == "districts-price-the-improvement"));
    let mut ai = AdvancedAi::new();
    assert!(
        !ai.districts_price_the_improvement,
        "production ships it off"
    );
    ai.enable_districts_price_the_improvement();
    assert!(ai.districts_price_the_improvement);
    ai.disable_districts_price_the_improvement();
    assert!(!ai.districts_price_the_improvement);
}
