use super::*;
use crate::game::{Action, Game};

fn board() -> (Game, u32) {
    let mut g = Game::new(2, 24, 16, 72, 250, 0);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    g.current = 0;
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    (g, city)
}

fn settler() -> Item {
    Item::Unit {
        unit: crate::name!("settler"),
    }
}

fn monument() -> Item {
    Item::Building {
        building: crate::name!("monument"),
    }
}

#[test]
fn off_by_default_and_toggles() {
    let ai = AdvancedAi::new();
    assert!(!ai.buy_what_cards_cannot_boost, "an opt-in ships off");
    assert!(!ai.build_what_cards_boost, "an opt-in ships off");
    assert!(!ai.native_emergency_purchase, "an opt-in ships off");
    assert!(!ai.native_emergency_purchase_2, "a successor ships off");
    let legacy = AdvancedAi::legacy();
    assert!(!legacy.buy_what_cards_cannot_boost);
    assert!(!legacy.build_what_cards_boost);
    assert!(!legacy.native_emergency_purchase);
    assert!(!legacy.native_emergency_purchase_2);

    let mut ai = AdvancedAi::new();
    ai.enable_buy_what_cards_cannot_boost();
    ai.enable_build_what_cards_boost();
    ai.enable_native_emergency_purchase();
    assert!(ai.buy_what_cards_cannot_boost);
    assert!(ai.build_what_cards_boost);
    assert!(ai.native_emergency_purchase);
    assert!(!ai.native_emergency_purchase_2);
    ai.enable_native_emergency_purchase_2();
    assert!(!ai.native_emergency_purchase, "one emergency version plays");
    assert!(ai.native_emergency_purchase_2);
    ai.enable_native_emergency_purchase();
    assert!(ai.native_emergency_purchase);
    assert!(
        !ai.native_emergency_purchase_2,
        "version one also selects its family"
    );
    ai.disable_buy_what_cards_cannot_boost();
    ai.disable_build_what_cards_boost();
    ai.disable_native_emergency_purchase();
    ai.disable_native_emergency_purchase_2();
    assert!(!ai.buy_what_cards_cannot_boost);
    assert!(!ai.build_what_cards_boost);
    assert!(!ai.native_emergency_purchase);
    assert!(!ai.native_emergency_purchase_2);
}

/// `threatened-city-reserve`: the same run bought a Water Mill at t160
/// with Aquileia named under threat and 399 Gold in hand (a Line
/// Infantry cost 360), and a Market in Aquileia itself at t162 with the
/// walls half down. The floor is one emergency defender while a city is
/// threatened or bleeding, and exactly zero otherwise.
#[test]
fn the_treasury_keeps_one_defender_while_a_city_is_threatened() {
    use super::super::GrandStrategy;
    let (mut g, city) = board();
    let threatened = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: Some(city),
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    let calm = StrategicPlan {
        threatened_city: None,
        ..threatened.clone()
    };

    let off = AdvancedAi::new();
    assert!(!off.threatened_city_reserve, "a repair gene ships off");
    assert!(!AdvancedAi::legacy().threatened_city_reserve);
    assert_eq!(off.threatened_city_gold_floor(&g, 0, &threatened), 0.0);
    assert_eq!(
        off.reserve_for_the_threatened_city(&g, 0, &threatened, 125.0),
        125.0
    );

    let mut on = AdvancedAi::new();
    on.enable_threatened_city_reserve();
    assert!(on.threatened_city_reserve);
    assert_eq!(
        on.threatened_city_gold_floor(&g, 0, &calm),
        0.0,
        "no threat, no floor"
    );
    let floor = on.threatened_city_gold_floor(&g, 0, &threatened);
    let defender = on
        .emergency_defender_price(&g, 0)
        .unwrap_or_else(|| g.game_speed.scale(FALLBACK_DEFENDER_PRICE));
    assert!(
        floor > 0.0,
        "a threatened city holds a defender's price back"
    );
    assert_eq!(floor, defender);
    assert_eq!(
        on.reserve_for_the_threatened_city(&g, 0, &threatened, 1.0),
        floor
    );
    assert_eq!(
        on.reserve_for_the_threatened_city(&g, 0, &threatened, floor + 1_000.0),
        floor + 1_000.0,
        "a stock reserve already above the floor stands"
    );

    // A bleeding city with no named threat is the other trigger, through
    // `native-emergency-purchase`'s own confirmed-damage signal.
    g.turn = 10;
    {
        let bleeding = g.cities.get_mut(&city).unwrap();
        bleeding.hp = 150;
        bleeding.last_attacked = 9;
    }
    assert_eq!(
        on.threatened_city_gold_floor(&g, 0, &calm),
        0.0,
        "unseen without the signal"
    );
    on.enable_native_emergency_purchase();
    assert_eq!(on.threatened_city_gold_floor(&g, 0, &calm), defender);
    on.disable_threatened_city_reserve();
    assert_eq!(on.threatened_city_gold_floor(&g, 0, &threatened), 0.0);
}

#[test]
fn the_engine_prices_colonization_and_the_controller_reads_it() {
    let (mut g, city) = board();
    assert_eq!(g.item_prod_mult(0, city, Some(&settler())), 1.0);
    g.players[0].policies.insert(crate::name!("colonization"));
    assert!(
        (g.item_prod_mult(0, city, Some(&settler())) - 1.5).abs() < 1e-9,
        "Colonization is +50% toward Settlers"
    );
    assert_eq!(g.item_prod_mult(0, city, Some(&monument())), 1.0);

    let off = AdvancedAi::new();
    assert_eq!(off.purchase_card_multiplier(&g, 0, city, &settler()), 1.0);
    let mut on = AdvancedAi::new();
    on.enable_buy_what_cards_cannot_boost();
    assert!((on.purchase_card_multiplier(&g, 0, city, &settler()) - 1.5).abs() < 1e-9);
    assert_eq!(on.purchase_card_multiplier(&g, 0, city, &monument()), 1.0);
}

#[test]
fn the_governor_leans_toward_the_boosted_item_and_only_upward() {
    let (mut g, city) = board();
    g.players[0].policies.insert(crate::name!("colonization"));
    let off = AdvancedAi::new();
    assert_eq!(
        off.card_boosted_value(&g, 0, city, &settler(), 100.0),
        100.0
    );
    let mut on = AdvancedAi::new();
    on.enable_build_what_cards_boost();
    assert!(
        (on.card_boosted_value(&g, 0, city, &settler(), 100.0) - 125.0).abs() < 1e-9,
        "half of a +50% card"
    );
    assert_eq!(
        on.card_boosted_value(&g, 0, city, &monument(), 100.0),
        100.0
    );
    assert_eq!(
        on.card_boosted_value(&g, 0, city, &settler(), -100.0),
        -100.0
    );
    assert_eq!(on.card_boosted_value(&g, 0, city, &settler(), 0.0), 0.0);
}

#[test]
fn the_native_emergency_needs_damage_and_a_recent_strike() {
    let (mut g, city) = board();
    g.turn = 30;

    let mut on = AdvancedAi::new();
    on.enable_native_emergency_purchase();
    assert!(
        !on.native_city_emergency(&g, 0, city),
        "an unhurt city is not an emergency"
    );

    let state = g.cities.get_mut(&city).unwrap();
    state.hp = 120;
    state.last_attacked = 29;
    assert!(on.native_city_emergency(&g, 0, city));
    let item = on
        .native_emergency_item(&g, 0, city)
        .expect("a recently damaged city has an answer after its attacker leaves sight");
    assert!(
        matches!(item, Item::Building { .. } | Item::Unit { .. }),
        "walls or a land defender: {item:?}"
    );

    let off = AdvancedAi::new();
    assert!(!off.native_city_emergency(&g, 0, city), "off is off");
    assert!(off.native_emergency_item(&g, 0, city).is_none());

    g.cities.get_mut(&city).unwrap().last_attacked = 20;
    assert!(
        !on.native_city_emergency(&g, 0, city),
        "the strike is stale"
    );
    g.cities.get_mut(&city).unwrap().last_attacked = 29;
    assert!(
        on.native_city_emergency(&g, 0, city),
        "a freshly struck city stays an emergency even without a visible attacker"
    );
}

#[test]
fn native_emergency_v2_accepts_a_same_frame_host_hit_or_visible_legal_attacker() {
    let (mut g, city) = board();
    g.turn = 30;
    let city_pos = g.cities[&city].pos;
    {
        let state = g.cities.get_mut(&city).unwrap();
        state.buildings.push(crate::name!("walls"));
        state.wall_hp = 100;
        state.hp = 120;
        state.last_attacked = 29;
    }

    let mut on = AdvancedAi::new();
    on.enable_native_emergency_purchase_2();
    assert!(on.native_emergency_purchase_on());
    assert!(
        !on.native_city_emergency_2(&g, 0, city),
        "a recent scar without a current attacker is not an emergency"
    );
    assert!(on.native_emergency_item(&g, 0, city).is_none());

    g.cities.get_mut(&city).unwrap().last_attacked = g.turn;
    assert!(
        on.native_city_emergency_2(&g, 0, city),
        "a host-confirmed same-frame hit remains an emergency after the attacker leaves sight"
    );
    assert!(
        on.native_emergency_item(&g, 0, city).is_some(),
        "the direct host signal reaches the existing walls-or-defender choice"
    );
    g.cities.get_mut(&city).unwrap().last_attacked = 29;
    for unit in g.player_unit_ids(0) {
        g.remove_unit(unit);
    }

    let attack_tile = g
        .nbrs(city_pos)
        .into_iter()
        .find(|position| {
            g.city_at(*position).is_none()
                && g.unit_ids_at(*position).is_empty()
                && g.map
                    .get(*position)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        })
        .expect("the city needs an accessible attack tile");
    g.at_war.insert((0, 1));
    let attacker = g.spawn_test_unit("warrior", 1, attack_tile);
    assert!(g.player_can_see(0, attack_tile), "the attacker is observed");
    assert!(
        g.attack_reach(attacker).contains(&city_pos),
        "the attacker can legally strike the City Center"
    );
    assert!(on.native_city_emergency_2(&g, 0, city));
    assert!(
        on.native_emergency_item(&g, 0, city).is_some(),
        "the corroborated signal reaches the existing walls-or-defender choice"
    );
    let plan = StrategicPlan {
        strategy: super::super::GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: Some(city),
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut purchase = g.clone();
    purchase.players[0].gold = 1_000.0;
    let before_purchase = purchase.players[0].gold;
    assert!(
        on.emergency_city_defense_purchase(&mut purchase, 0, &plan),
        "version two opens the existing emergency purchase path"
    );
    assert!(
        purchase.players[0].gold < before_purchase,
        "the corroborated emergency spends Gold on its local answer"
    );

    g.remove_unit(attacker);
    assert!(
        !on.native_city_emergency_2(&g, 0, city),
        "a departing attacker clears version two immediately"
    );

    let attacker = g.spawn_test_unit("warrior", 1, attack_tile);
    g.cities.get_mut(&city).unwrap().last_attacked = 28;
    assert!(
        !on.native_city_emergency_2(&g, 0, city),
        "even a current attacker cannot revive a two-turn-old scar"
    );
    assert!(g.attack_reach(attacker).contains(&city_pos));
}

#[test]
fn the_treasury_gene_ships_off_and_toggles() {
    let ai = AdvancedAi::new();
    assert!(!ai.treasury_at_work_2, "an opt-in ships off");
    assert!(!AdvancedAi::legacy().treasury_at_work_2);
    let mut ai = AdvancedAi::new();
    ai.enable_treasury_at_work_2();
    assert!(ai.treasury_at_work_2);
    ai.disable_treasury_at_work_2();
    assert!(!ai.treasury_at_work_2);
}

#[test]
fn the_working_reserve_is_a_defender_plus_ten_turns_of_deficit() {
    // Solvent: the defender alone, whatever the surplus.
    assert_eq!(working_treasury_reserve_from(120.0, 7.0, 0.0), 120.0);
    assert_eq!(working_treasury_reserve_from(120.0, 0.0, 0.0), 120.0);
    // Insolvent: ten turns of the deficit on top.
    assert_eq!(working_treasury_reserve_from(120.0, -8.0, 0.0), 200.0);
    // An appointed war's bill is a floor, never an addend.
    assert_eq!(working_treasury_reserve_from(120.0, -8.0, 500.0), 500.0);
    assert_eq!(working_treasury_reserve_from(120.0, 7.0, 119.0), 120.0);
}

#[test]
fn a_unit_purchase_must_keep_the_recurring_budget_at_or_above_zero() {
    assert!(unit_purchase_keeps_solvent(7.0, 1.0));
    assert!(unit_purchase_keeps_solvent(1.0, 1.0));
    assert!(!unit_purchase_keeps_solvent(0.0, 1.0));
    assert!(!unit_purchase_keeps_solvent(-3.0, 1.0));
    // Free upkeep is always solvent, however deep the deficit.
    assert!(unit_purchase_keeps_solvent(-30.0, 0.0));

    let (mut g, _city) = board();
    let archer = Item::Unit {
        unit: crate::name!("archer"),
    };
    let upkeep = g.rules.units["archer"].maintenance;
    assert!(upkeep > 0.0, "an Archer costs upkeep in the shipped rules");
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    // Off: never consulted.
    g.players[0].gold_per_turn = -30.0;
    assert!(AdvancedAi::new().treasury_purchase_stays_solvent(&g, 0, &archer));
    // On: the unit is declined at a deficit and allowed with the income
    // to carry it; a building is never a unit.
    let mut ai = AdvancedAi::new();
    ai.enable_treasury_at_work_2();
    assert!(!ai.treasury_purchase_stays_solvent(&g, 0, &archer));
    assert!(ai.treasury_purchase_stays_solvent(&g, 0, &monument));
    g.players[0].gold_per_turn = upkeep;
    assert!(ai.treasury_purchase_stays_solvent(&g, 0, &archer));
    g.players[0].gold_per_turn = upkeep - 0.5;
    assert!(!ai.treasury_purchase_stays_solvent(&g, 0, &archer));
}

#[test]
fn off_the_reserve_is_the_stock_value_and_on_it_is_a_defender() {
    let (g, _city) = board();
    let ai = AdvancedAi::new();
    assert_eq!(
        ai.working_treasury_reserve(&g, 0, 325.0),
        325.0,
        "off: untouched"
    );

    let mut ai = AdvancedAi::new();
    ai.enable_treasury_at_work_2();
    let reserve = ai.working_treasury_reserve(&g, 0, 325.0);
    // A fresh capital can build a Slinger, the ancient ranged unit, and
    // the reserve is exactly its Gold price: no city count anywhere.
    let slinger = Item::Unit {
        unit: crate::name!("slinger"),
    };
    assert!(g.can_produce(0, g.player_city_ids(0)[0], &slinger));
    let expected = g.item_cost_for(0, &slinger) * GOLD_PER_PRODUCTION;
    assert!(expected > 0.0);
    assert_eq!(
        reserve, expected,
        "one ranged defender, {reserve} vs {expected}"
    );
    assert!(reserve < 325.0, "far below the plan's 250 + 75 per city");
}

#[test]
fn a_deficit_raises_the_working_reserve_by_ten_turns_of_it() {
    let (mut g, _city) = board();
    let mut ai = AdvancedAi::new();
    ai.enable_treasury_at_work_2();
    let solvent = ai.working_treasury_reserve(&g, 0, 325.0);
    g.players[0].gold_per_turn = -6.0;
    let insolvent = ai.working_treasury_reserve(&g, 0, 325.0);
    assert_eq!(insolvent, solvent + DEFICIT_COVER_TURNS * 6.0);
}

#[test]
fn without_a_city_the_reserve_is_the_scaled_fallback_defender() {
    let g = Game::new(2, 24, 16, 71, 250, 0);
    assert!(g.player_city_ids(0).is_empty());
    let mut ai = AdvancedAi::new();
    ai.enable_treasury_at_work_2();
    assert_eq!(
        ai.working_treasury_reserve(&g, 0, 325.0),
        g.game_speed.scale(FALLBACK_DEFENDER_PRICE)
    );
}

#[test]
fn version_two_buys_the_first_builder_then_a_monument_and_never_the_reserve() {
    let (mut g, city) = board();
    let mut ai = AdvancedAi::new();
    ai.enable_treasury_at_work_2();
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    // A building already in the queue is not for sale, and the fresh
    // capital is seeded with its Monument: clear both so the fixture
    // has one to want.
    let capital = g.cities.get_mut(&city).unwrap();
    capital.queue.clear();
    capital.buildings.retain(|building| building != "monument");
    let builder_price = g
        .unit_purchase_cost(0, city, "builder", "gold")
        .expect("a builder is buyable");
    let monument_price = g
        .building_purchase_cost(0, city, "monument", "gold")
        .expect("a monument is buyable");
    let reserve = 100.0;

    // Short of the reserve by one Gold: nothing is bought.
    g.players[0].gold = reserve + builder_price - 1.0;
    assert!(!ai.young_empire_purchase(&mut g, 0, reserve));
    assert_eq!(g.players[0].gold, reserve + builder_price - 1.0);

    // At the reserve: the empire's first Builder, one purchase a turn.
    g.players[0].gold = reserve + builder_price + monument_price;
    assert!(ai.young_empire_purchase(&mut g, 0, reserve));
    let builders = g
        .player_unit_ids(0)
        .into_iter()
        .filter(|uid| g.units[uid].kind == "builder")
        .count();
    assert_eq!(builders, 1, "one Builder bought");
    assert!(g.players[0].gold >= reserve, "the reserve is never spent");
    assert!(
        !g.cities[&city].buildings.iter().any(|b| b == "monument"),
        "one a turn"
    );

    // Next turn, with a Builder on the map, the Monument.
    assert!(ai.young_empire_purchase(&mut g, 0, reserve));
    assert!(g.cities[&city].buildings.iter().any(|b| b == "monument"));
    assert!(g.players[0].gold >= reserve);
    // Nothing under-bought remains: the ladder stands down.
    g.players[0].gold = 10_000.0;
    assert!(!ai.young_empire_purchase(&mut g, 0, reserve));
    let _ = builder;
}

#[test]
fn the_upgrade_floor_keeps_the_armys_bill_only_while_on() {
    assert_eq!(upgrade_floor_from(30.0, 0.0), 30.0);
    assert_eq!(upgrade_floor_from(30.0, 140.0), 210.0);
    assert_eq!(
        upgrade_floor_from(300.0, 140.0),
        300.0,
        "a larger floor stands"
    );
    let (mut g, _) = board();
    std::sync::Arc::make_mut(&mut g.host_maintenance).insert(
        0,
        crate::game::HostMaintenance {
            units: Some(140.0),
            buildings: None,
            districts: None,
        },
    );
    let mut ai = AdvancedAi::new();
    assert_eq!(
        ai.upgrade_treasury_floor(&g, 0, 30.0),
        30.0,
        "off by default"
    );
    ai.enable_upkeep_reserve();
    assert_eq!(ai.upgrade_treasury_floor(&g, 0, 30.0), 210.0);
    ai.disable_upkeep_reserve();
    assert_eq!(ai.upgrade_treasury_floor(&g, 0, 30.0), 30.0);
}
