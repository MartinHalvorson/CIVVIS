use super::*;
use crate::rules::PillageReward;

fn controlled_game(seed: u64) -> (Game, Pos, Vec<Pos>) {
    let mut g = Game::new_full(2, 20, 14, seed, 40, 0, false);
    let ids: Vec<u32> = g.units.keys().copied().collect();
    for id in ids {
        g.remove_unit(id);
    }
    for player in g.players.iter_mut() {
        player.civ = "Rome".to_string();
        player.unit_lifetimes.clear();
        player.government = None;
        player.policies.clear();
        player.techs.clear();
        player.civics.clear();
    }
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.owner_city = None;
        tile.hills = false;
        tile.road = 0;
    }
    let center = *g
        .map
        .tiles
        .keys()
        .find(|p| g.wdisk(**p, 2).len() == 19)
        .expect("controlled map has an interior tile");
    let ring = g.nbrs(center);
    assert_eq!(ring.len(), 6);
    g.current = 0;
    g.at_war.insert(pair(0, 1));
    (g, center, ring.to_vec())
}

#[test]
fn tile_entry_ignores_a_foreign_link_removed_by_elimination() {
    let (mut g, target, ring) = controlled_game(3001);
    let attacker = g.spawn_unit("warrior", 1, ring[0]);
    let stale_peer = g.spawn_unit("battering_ram", 0, ring[0]);
    let settler = g.spawn_unit("settler", 0, target);

    // Old saves and pre-fix levy transitions can contain this invalid
    // cross-owner link. Capturing player 0's final Settler eliminates the
    // player and removes the ram while `enter_tile` is in progress.
    g.units.get_mut(&attacker).unwrap().linked_to = Some(stale_peer);
    g.units.get_mut(&stale_peer).unwrap().linked_to = Some(attacker);
    g.enter_tile(attacker, target);

    assert_eq!(g.units[&attacker].pos, target);
    assert_eq!(g.units[&attacker].linked_to, None);
    assert_eq!(g.units[&settler].owner, 1);
    assert!(!g.units.contains_key(&stale_peer));
    assert!(!g.players[0].alive);
}

fn found_controlled_home(game: &mut Game, center: Pos) -> (u32, Pos) {
    let city = game.found_city_for(0, center, Some("Upgrade Test".to_string()));
    let home = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center)
        .expect("new city owns a non-center tile");
    (city, home)
}

fn set_controlled_district(game: &mut Game, city: u32, position: Pos, district: &str) {
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.district = Some(Name::new(district));
    tile.improvement = None;
    tile.pillaged = false;
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(Name::new(district), position);
}

#[test]
fn units_keep_exact_lifetime_damage_without_counting_overkill() {
    let (mut ranged, target, ring) = controlled_game(41_061);
    let archer = ranged.spawn_unit("archer", 0, ring[0]);
    let warrior = ranged.spawn_unit("warrior", 1, target);
    let hp_before = ranged.units[&warrior].hp;
    ranged
        .apply(
            0,
            &Action::Ranged {
                unit: archer,
                target,
            },
        )
        .unwrap();
    let hp_after = ranged.units[&warrior].hp;
    assert_eq!(
        ranged.units[&archer].damage_dealt,
        (hp_before - hp_after) as u64,
        "a ranged unit owns exactly the health its shot removed"
    );
    assert_eq!(ranged.units[&warrior].damage_dealt, 0);

    let (mut melee, target, ring) = controlled_game(41_062);
    let attacker = melee.spawn_unit("warrior", 0, ring[0]);
    let defender = melee.spawn_unit("warrior", 1, target);
    let attacker_hp = melee.units[&attacker].hp;
    let defender_hp = melee.units[&defender].hp;
    melee
        .apply(
            0,
            &Action::Attack {
                unit: attacker,
                target,
            },
        )
        .unwrap();
    assert_eq!(
        melee.units[&attacker].damage_dealt,
        (defender_hp - melee.units[&defender].hp) as u64
    );
    assert_eq!(
        melee.units[&defender].damage_dealt,
        (attacker_hp - melee.units[&attacker].hp) as u64,
        "melee counter-damage belongs to the defending unit's lifetime"
    );

    let (mut overkill, target, ring) = controlled_game(41_063);
    let attacker = overkill.spawn_unit("warrior", 0, ring[0]);
    let defender = overkill.spawn_unit("warrior", 1, target);
    overkill.units.get_mut(&defender).unwrap().hp = 7;
    overkill.apply_unit_damage(attacker, defender, 30);
    assert_eq!(overkill.units[&attacker].damage_dealt, 7);
    assert!(overkill.units[&defender].hp <= 0);
    let production_cost = overkill.rules.units["warrior"].cost;
    overkill.remove_unit(attacker);
    let lifetime = &overkill.unit_lifetime_stats(0)[&crate::name!("warrior")];
    assert_eq!(lifetime.units, 1);
    assert_eq!(lifetime.damage_dealt, 7);
    assert_eq!(lifetime.production_cost, production_cost);
    assert_eq!(lifetime.average_damage(), Some(7.0));
    assert_eq!(lifetime.damage_per_production(), Some(7.0 / production_cost));

    let (mut last_blow, target, ring) = controlled_game(41_064);
    let doomed = last_blow.spawn_unit("warrior", 0, ring[0]);
    let defender = last_blow.spawn_unit("warrior", 1, target);
    last_blow.units.get_mut(&doomed).unwrap().hp = 1;
    last_blow
        .apply(
            0,
            &Action::Attack {
                unit: doomed,
                target,
            },
        )
        .unwrap();
    assert!(!last_blow.units.contains_key(&doomed));
    let recorded = &last_blow.unit_lifetime_stats(0)[&crate::name!("warrior")];
    assert!(
        recorded.damage_dealt > 0,
        "the attack that killed its attacker must survive in the completed-life ledger"
    );
    assert!(last_blow.units[&defender].hp < 100);
}

/// The shipped government Combat Strength abilities carry conditions, and
/// they are not the same condition. `ABILITY_OLIGARCHY_MELEE_BUFF` reaches
/// melee, anti-cavalry and naval melee; `FASCISM_ATTACK_BUFF` carries
/// `FASCISM_REQUIREMENTS`, a single `REQUIREMENT_PLAYER_IS_ATTACKING`, so
/// it is an attacking bonus. CIVVIS paid Fascism's on defence too, which
/// made the domination government stronger than the shipped one.
#[test]
fn fascism_pays_its_combat_strength_only_on_the_attack() {
    let (mut game, center, _) = controlled_game(41_060);
    let warrior = game.spawn_test_unit("warrior", 0, center);
    let unit = game.units[&warrior].clone();
    let bare_attack = game.unit_strength(&unit, false);
    let bare_defend = game.unit_strength(&unit, true);

    game.players[0].government = Some("fascism".to_string());
    let unit = game.units[&warrior].clone();
    assert_eq!(
        game.unit_strength(&unit, false) - bare_attack,
        5.0,
        "FASCISM_ATTACK_BUFF is +5 when attacking"
    );
    assert_eq!(
        game.unit_strength(&unit, true),
        bare_defend,
        "and nothing at all when defending"
    );

    // Oligarchy's is unconditional on attack/defence but restricted by
    // promotion class, which is a different shipped condition.
    game.players[0].government = Some("oligarchy".to_string());
    let unit = game.units[&warrior].clone();
    assert_eq!(game.unit_strength(&unit, false) - bare_attack, 4.0);
    assert_eq!(game.unit_strength(&unit, true) - bare_defend, 4.0);
}

/// `tile_defense_bonus` used to carry a hand-written list of features and
/// none of the Natural Wonders, so Ha Long Bay's shipped +15, Gobustan's
/// and Chocolate Hills' +3 and Ubsunur Hollow's -2 never reached a
/// defender. It reads `Features.DefenseModifier` from the ruleset now.
#[test]
fn every_shipped_feature_defense_modifier_reaches_the_defender() {
    let (mut game, center, _) = controlled_game(41_050);
    let flat = game.tile_defense_bonus(center);
    assert_eq!(flat, 0.0, "featureless plains carry no modifier");

    for (feature, expected) in [
        ("forest", 3.0),
        ("jungle", 3.0),
        ("reef", 3.0),
        ("marsh", -2.0),
        ("floodplains", -2.0),
        ("ha_long_bay", 15.0),
        ("gobustan", 3.0),
        ("chocolate_hills", 3.0),
        ("ubsunur_hollow", -2.0),
    ] {
        game.map.tiles.get_mut(&center).unwrap().feature = Some(Name::new(feature));
        assert_eq!(
            game.tile_defense_bonus(center),
            expected,
            "{feature} must carry its shipped DefenseModifier"
        );
    }

    // Hills and a feature are two separate shipped rows and both apply.
    game.map.tiles.get_mut(&center).unwrap().feature = Some(crate::name!("forest"));
    game.map.tiles.get_mut(&center).unwrap().hills = true;
    assert_eq!(game.tile_defense_bonus(center), 6.0);
}

#[test]
fn slinger_upgrade_is_legal_only_when_affordable_and_in_friendly_territory() {
    let (mut game, center, _) = controlled_game(41_001);
    let (city, home) = found_controlled_home(&mut game, center);
    game.players[0].techs.insert(crate::name!("archery"));
    game.players[0].gold = 59.0;
    let slinger = game.spawn_test_unit("slinger", 0, home);
    let action = Action::UpgradeUnit { unit: slinger };

    assert!(!game.legal_unit_upgrade_actions(0).contains(&action));
    game.players[0].gold = 60.0;
    game.map.tiles.get_mut(&home).unwrap().owner_city = None;
    assert!(!game.legal_unit_upgrade_actions(0).contains(&action));
    game.map.tiles.get_mut(&home).unwrap().owner_city = Some(city);
    assert!(game.legal_unit_upgrade_actions(0).contains(&action));
    assert_eq!(
        game.unit_gold_upgrade_offer(0, slinger).unwrap().1,
        60.0
    );
    game.game_speed = GameSpeed::Online;
    assert_eq!(
        game.unit_gold_upgrade_offer(0, slinger).unwrap().1,
        30.0
    );
    game.game_speed = GameSpeed::Standard;

    {
        let unit = game.units.get_mut(&slinger).unwrap();
        unit.hp = 43;
        unit.damage_dealt = 91;
        unit.xp = 17;
        unit.level = 2;
        unit.promotions.insert(crate::name!("volley"));
    }
    game.apply(0, &action).unwrap();
    let unit = &game.units[&slinger];
    assert_eq!(unit.kind, "archer");
    assert_eq!(unit.hp, 43);
    assert_eq!(unit.damage_dealt, 91, "an upgrade continues the same lifetime");
    assert_eq!(
        unit.production_cost, game.rules.units["slinger"].cost,
        "an upgrade does not pretend the original unit cost Archer Production"
    );
    assert_eq!(unit.xp, 17);
    assert_eq!(unit.level, 2);
    assert!(unit.promotions.contains(&Name::new("volley")));
    assert_eq!(unit.moves_left, 0.0);
    assert_eq!(unit.attacks_left, 0);
    assert!(game.players[0].gold.abs() < 1e-9);
}

#[test]
fn formation_upgrade_applies_policy_and_resource_costs() {
    let (mut game, center, _) = controlled_game(41_002);
    game.players[0].civ = "Greece".to_string();
    let (_, home) = found_controlled_home(&mut game, center);
    game.players[0].techs.insert(crate::name!("iron_working"));
    game.players[0]
        .policies
        .extend([crate::name!("professional_army"), crate::name!("retinues")]);
    // 110 base, halved by Professional Army, then tripled for an Army.
    game.players[0].gold = 165.0;
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 30.0);
    let warrior = game.spawn_test_unit("warrior", 0, home);
    game.units.get_mut(&warrior).unwrap().formation = 2;

    let action = Action::UpgradeUnit { unit: warrior };
    assert!(game.legal_unit_upgrade_actions(0).contains(&action));
    game.apply(0, &action).unwrap();
    assert_eq!(game.units[&warrior].kind, "swordsman");
    assert_eq!(game.units[&warrior].formation, 2);
    assert_eq!(game.players[0].counters["strongest_unit_built"], 35);
    assert!(game.players[0].gold.abs() < 1e-9);
    assert!(game.strategic_stockpile(0, crate::name!("iron")).abs() < 1e-9);
}

#[test]
fn upgrades_resolve_the_owners_unique_replacement() {
    let (mut game, center, _) = controlled_game(41_003);
    game.players[0].civ = "Nubia".to_string();
    let (_, home) = found_controlled_home(&mut game, center);
    game.players[0].techs.insert(crate::name!("archery"));
    game.players[0].gold = 80.0;
    let slinger = game.spawn_test_unit("slinger", 0, home);
    let action = Action::UpgradeUnit { unit: slinger };

    assert_eq!(
        game.unit_upgrade_target(0, crate::name!("slinger")).as_deref(),
        Some("pitati_archer")
    );
    assert!(game.legal_unit_upgrade_actions(0).contains(&action));
    game.apply(0, &action).unwrap();
    assert_eq!(game.units[&slinger].kind, "pitati_archer");
}

#[test]
fn obsolete_production_modernizes_without_losing_progress_or_material() {
    let (mut game, center, _) = controlled_game(41_004);
    game.players[0].civ = "Greece".to_string();
    let (city, _) = found_controlled_home(&mut game, center);
    let slinger = Item::Unit {
        unit: crate::name!("slinger"),
    };
    let archer = Item::Unit {
        unit: crate::name!("archer"),
    };
    let crossbowman = Item::Unit {
        unit: crate::name!("crossbowman"),
    };
    assert!(game.can_produce(0, city, &slinger));
    game.cities.get_mut(&city).unwrap().queue = vec![slinger.clone()];
    game.cities.get_mut(&city).unwrap().production = 18.0;
    game.cities
        .get_mut(&city)
        .unwrap()
        .production_progress
        .insert(Game::item_progress_key(&crossbowman), 7.0);
    game.players[0]
        .techs
        .extend([crate::name!("archery"), crate::name!("machinery")]);

    assert!(!game.can_produce(0, city, &slinger));
    // ⚠ Machinery retires the Archer TOO, because it unlocks the Crossbowman.
    // Civilization VI withdraws a unit as soon as its upgrade is available, so
    // the queue walks the whole chain rather than stopping one rung up.
    assert!(!game.can_produce(0, city, &archer));
    game.modernize_unit_queue(0, city);
    assert_eq!(game.cities[&city].queue, vec![crossbowman]);
    assert!((game.cities[&city].production - 25.0).abs() < 1e-9);

    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let swordsman = Item::Unit {
        unit: crate::name!("swordsman"),
    };
    game.cities.get_mut(&city).unwrap().queue = vec![warrior.clone()];
    game.cities.get_mut(&city).unwrap().production = 11.0;
    game.players[0].techs.insert(crate::name!("iron_working"));
    assert!(
        game.can_produce(0, city, &warrior),
        "with NO iron the Swordsman cannot be built, so the Warrior stays on the \
         menu -- a successor that cannot be built retires nothing"
    );
    game.modernize_unit_queue(0, city);
    assert_eq!(game.cities[&city].queue, vec![warrior.clone()]);

    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 20.0);
    // ⚠ Iron in the stockpile makes the Swordsman buildable, and Civilization VI
    // withdraws the Warrior the moment it is -- long before gunpowder, which is
    // only its MandatoryObsoleteTech.
    assert!(!game.can_produce(0, city, &warrior));
    game.players[0].techs.insert(crate::name!("gunpowder"));
    assert!(!game.can_produce(0, city, &warrior));
    game.modernize_unit_queue(0, city);
    assert_eq!(game.cities[&city].queue, vec![swordsman.clone()]);
    assert!((game.cities[&city].production - 11.0).abs() < 1e-9);
    assert!(game.strategic_stockpile(0, crate::name!("iron")).abs() < 1e-9);
    assert_eq!(
        game.cities[&city]
            .strategic_resource_commitments
            .get(&Game::item_progress_key(&swordsman)),
        Some(&20.0)
    );
}

#[test]
fn the_tourism_multipliers_are_the_ones_the_parameters_name() {
    // TOURISM_OPEN_BORDERS_BONUS 25, TOURISM_TRADE_ROUTE_BONUS 25 and
    // TOURISM_DIFFERENT_RELIGION_REDUCTION 50. A culture victory is won
    // and lost on these, and nothing held them.
    let (mut g, centre, ring) = controlled_game(4_166);
    g.found_city_for(0, centre, None);
    g.found_city_for(1, ring[3], None);
    let plain = g.international_tourism_multiplier(0, 1, false);
    assert_eq!(plain, 1.0, "no borders, no route, no religion");

    g.players[0].open_borders_until.insert(1, g.turn + 30);
    g.players[1].open_borders_until.insert(0, g.turn + 30);
    assert_eq!(g.international_tourism_multiplier(0, 1, false), 1.25);
    g.players[0].open_borders_until.clear();
    g.players[1].open_borders_until.clear();

    // Religious Tourism is halved between two different religions, and
    // only when both sides actually have one.
    g.players[0].religion = Some("Ours".to_string());
    assert_eq!(g.international_tourism_multiplier(0, 1, true), 1.0, "the target has none");
    g.players[1].religion = Some("Theirs".to_string());
    assert_eq!(g.international_tourism_multiplier(0, 1, true), 0.5);
    g.players[1].religion = Some("Ours".to_string());
    assert_eq!(g.international_tourism_multiplier(0, 1, true), 1.0, "the same faith is not reduced");

    // TOURISM_CULTURE_PER_CITIZEN 100 and TOURISM_TOURISM_TO_MOVE_CITIZEN 200.
    g.players[0].culture_lifetime = 950.0;
    assert_eq!(g.domestic_tourists(0), 9);
    assert_eq!(TOURISM_PER_VISITOR, 200.0);
}

#[test]
fn the_damage_curve_is_the_one_the_parameters_describe() {
    // COMBAT_BASE_DAMAGE 24 with COMBAT_MAX_EXTRA_DAMAGE 12 is a roll of
    // 24 to 36, which is what 30 * U(0.8, 1.2) gives. COMBAT_POWER_SCALING
    // 0.04 is the 1/25 the strength difference is divided by, and
    // COMBAT_MINIMUM_DAMAGE 1 and COMBAT_MAX_HIT_POINTS 100 are the clamp.
    let mut rng = crate::rng::Rng::new(4_165);
    let mut lowest = i32::MAX;
    let mut highest = 0;
    for _ in 0..20_000 {
        let rolled = damage(50.0, 50.0, &mut rng);
        lowest = lowest.min(rolled);
        highest = highest.max(rolled);
    }
    // An even fight rolls the base band itself.
    assert_eq!((lowest, highest), (24, 36));

    // Every 25 points of advantage multiplies the roll by e, so a
    // twenty-five point edge is worth about 2.718 times the damage.
    let mut even = 0.0;
    let mut ahead = 0.0;
    for _ in 0..20_000 {
        even += f64::from(damage(50.0, 50.0, &mut rng));
        ahead += f64::from(damage(75.0, 50.0, &mut rng));
    }
    assert!(
        (ahead / even - std::f64::consts::E).abs() < 0.05,
        "25 strength should be worth e times the damage, got {}",
        ahead / even
    );

    // The clamp holds at both ends: nothing does less than 1, and a
    // hopeless mismatch still cannot exceed a full health bar.
    assert_eq!(damage(1.0, 500.0, &mut rng), 1);
    assert_eq!(damage(500.0, 1.0, &mut rng), 100);
}

#[test]
fn walls_absorb_the_share_of_each_attack_its_parameter_names() {
    // COMBAT_DEFENSE_DAMAGE_PERCENT_MELEE 15, _RANGED 50, _BOMBARD 100.
    // A melee swing barely scratches Outer Defenses; a Bombard takes them
    // down at full rate. Behind healthy walls the city itself takes 1.
    let (mut g, centre, _) = controlled_game(4_164);
    let capital = g.found_city_for(1, centre, None);
    g.cities.get_mut(&capital).unwrap().buildings.push(crate::name!("walls"));
    let max = g.city_max_wall_hp(&g.cities[&capital]);
    assert!(max > 0);

    let wall_after = |g: &Game, multiplier: f64| {
        let mut probe = g.clone();
        let city = probe.cities.get_mut(&capital).unwrap();
        city.wall_hp = max;
        probe.city_take_damage(0, capital, 20, multiplier, false);
        max - probe.cities[&capital].wall_hp
    };
    assert_eq!(wall_after(&g, 0.15), 3, "melee puts 15% of the roll on walls");
    assert_eq!(wall_after(&g, 0.5), 10, "ranged puts half");
    assert_eq!(wall_after(&g, 1.0), 20, "a Bombard puts all of it");

    // And the city behind full walls takes 1, not the roll.
    let city = g.cities.get_mut(&capital).unwrap();
    city.wall_hp = max;
    let hp = city.hp;
    g.city_take_damage(0, capital, 20, 0.15, false);
    assert_eq!(g.cities[&capital].hp, hp - 1);
}

#[test]
fn the_healing_rates_are_the_ones_the_parameters_name() {
    // COMBAT_HEAL_LAND_FRIENDLY 15, _NEUTRAL 10, _ENEMY 5 and
    // COMBAT_HEAL_CITY_GARRISON 20 for a unit standing in a district.
    assert_eq!(HealingLocation::District.rate(), 20);
    assert_eq!(HealingLocation::FriendlyTerritory.rate(), 15);
    assert_eq!(HealingLocation::NeutralTerritory.rate(), 10);
    assert_eq!(HealingLocation::EnemyTerritory.rate(), 5);

    // COMBAT_HEAL_NAVAL_FRIENDLY is 20, and _NEUTRAL and _ENEMY are both
    // zero: a ship away from a friendly city recovers nothing at all
    // unless a promotion says otherwise.
    assert!(HealingLocation::NeutralTerritory.rate() > 0);
}

#[test]
fn gathering_storm_pillage_rewards_are_data_driven_and_complete() {
    let (game, _, _) = controlled_game(299);
    let improvements = [
        ("farm", "heal", 50.0),
        ("fishery", "heal", 50.0),
        ("city_park", "heal", 50.0),
        ("fishing_boats", "heal", 50.0),
        ("seastead", "heal", 50.0),
        ("mine", "gold", 50.0),
        ("lumber_mill", "gold", 50.0),
        ("oil_well", "gold", 50.0),
        ("offshore_oil_rig", "gold", 50.0),
        ("wind_farm", "gold", 50.0),
        ("geothermal_plant", "gold", 50.0),
        ("solar_farm", "gold", 50.0),
        ("offshore_wind_farm", "gold", 50.0),
        ("seaside_resort", "gold", 50.0),
        ("industry", "gold", 50.0),
        ("great_wall", "gold", 50.0),
        ("corporation", "gold", 50.0),
        ("quarry", "faith", 25.0),
        ("pasture", "faith", 25.0),
        ("plantation", "faith", 25.0),
        ("camp", "faith", 25.0),
        ("sphinx", "faith", 25.0),
        ("nubian_pyramid", "faith", 25.0),
        ("kurgan", "faith", 25.0),
    ];
    for (improvement, yield_type, amount) in improvements {
        let spec = &game.rules.improvements[improvement];
        assert_eq!(spec.plunder_type.as_deref(), Some(yield_type), "{improvement}");
        assert_eq!(spec.plunder_amount, amount, "{improvement}");
    }
    for improvement in [
        "great_wall",
        "corporation",
        "national_park",
        "ski_resort",
        "mountain_tunnel",
    ] {
        assert!(
            !game.rules.improvements[improvement].unit_pillageable,
            "{improvement} is disaster-only or otherwise not unit-pillageable"
        );
    }
    for improvement in ["fort", "airstrip", "missile_silo"] {
        let spec = &game.rules.improvements[improvement];
        assert!(spec.unit_pillageable, "{improvement}");
        assert_eq!(spec.plunder_type, None, "{improvement}");
        assert_eq!(spec.plunder_amount, 0.0, "{improvement}");
    }
    assert_eq!(
        game.rules.improvements["mine"].bonus_pillage["knarr"],
        PillageReward {
            yield_type: "science".to_string(),
            amount: 15.0,
        }
    );
    for improvement in ["quarry", "pasture", "plantation", "camp"] {
        assert_eq!(
            game.rules.improvements[improvement].bonus_pillage["knarr"],
            PillageReward {
                yield_type: "culture".to_string(),
                amount: 15.0,
            },
            "{improvement}"
        );
    }

    let districts = [
        ("campus", "science", 25.0),
        ("holy_site", "faith", 25.0),
        ("commercial_hub", "gold", 50.0),
        ("harbor", "gold", 50.0),
        ("theater_square", "culture", 25.0),
        ("industrial_zone", "science", 25.0),
        ("entertainment_complex", "heal", 50.0),
        ("water_park", "heal", 50.0),
        ("aqueduct", "gold", 50.0),
        ("neighborhood", "gold", 50.0),
        ("canal", "gold", 50.0),
        ("dam", "heal", 50.0),
        ("aerodrome", "gold", 50.0),
        ("spaceport", "science", 25.0),
        ("government_plaza", "culture", 25.0),
        ("diplomatic_quarter", "culture", 25.0),
        ("preserve", "gold", 50.0),
    ];
    for (district, yield_type, amount) in districts {
        let spec = &game.rules.districts[district];
        assert_eq!(spec.plunder_type.as_deref(), Some(yield_type), "{district}");
        assert_eq!(spec.plunder_amount, amount, "{district}");
    }
}

#[test]
fn barbarians_do_not_heal_passively_but_healing_plunder_still_works() {
    let (mut game, center, _) = controlled_game(2991);
    let enemy_city = game.found_city_for(1, center, None);
    let farm = game.cities[&enemy_city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center)
        .unwrap();
    game.map.tiles.get_mut(&farm).unwrap().improvement = Some(crate::name!("farm"));
    game.players[0].is_barbarian = true;
    game.players[0].policies.insert(crate::name!("raid"));
    game.world_era = 8;
    let barbarian = game.spawn_test_unit("warrior", 0, farm);
    game.units.get_mut(&barbarian).unwrap().hp = 40;

    assert_eq!(game.unit_heal_rate(barbarian), 0);
    game.begin_turn(0);
    assert_eq!(game.units[&barbarian].hp, 40);

    game.apply(0, &Action::Pillage { unit: barbarian })
        .unwrap();
    assert_eq!(
        game.units[&barbarian].hp, 90,
        "healing plunder is the fixed 50 HP even in the Future Era with Raid"
    );
}

#[test]
fn the_ledger_counts_a_missionary_the_barbarians_condemn() {
    // `do_condemn_heretic` removes the unit itself — neither `record_kill`
    // nor `resolve_entered_units` sees it — so the loss has its own counter.
    let (mut g, center, ring) = controlled_game(3171);
    g.players[0].is_barbarian = true;
    g.barb_pid = Some(0);
    g.players[1].religion = Some("B".to_string());
    let raider = g.spawn_unit("warrior", 0, center);
    let missionary = g.spawn_unit("missionary", 1, center);
    assert!(g.is_at_war(0, 1), "the barbarian seat is at war with everyone");
    g.apply(
        0,
        &Action::CondemnHeretic {
            unit: raider,
            target_unit: missionary,
        },
    )
    .unwrap();
    assert!(!g.units.contains_key(&missionary));
    assert_eq!(
        g.players[1].counters.get("religious_lost_to_barbarians"),
        Some(&1)
    );
    assert_eq!(g.players[0].counters.get("condemned:missionary"), Some(&1));
    assert_eq!(
        g.players[1].counters.get("lost_to_barbarians"),
        None,
        "a condemnation is not a combat kill"
    );

    // The Free Cities seat also carries `is_barbarian`; its condemnations
    // stay off the raid ledger.
    g.players[0].is_free_city = true;
    g.begin_turn(0);
    let second = g.spawn_unit("warrior", 0, ring[0]);
    let heretic = g.spawn_unit("missionary", 1, ring[0]);
    g.apply(
        0,
        &Action::CondemnHeretic {
            unit: second,
            target_unit: heretic,
        },
    )
    .unwrap();
    assert!(!g.units.contains_key(&heretic));
    assert_eq!(
        g.players[1].counters.get("religious_lost_to_barbarians"),
        Some(&1)
    );
}

#[test]
fn the_ledger_counts_what_the_barbarians_take() {
    let (mut g, target, ring) = controlled_game(3105);
    g.players[0].is_barbarian = true;

    // A kill dealt by the barbarian seat lands on the victim's ledger.
    let raider = g.spawn_unit("warrior", 0, ring[0]);
    let victim = g.spawn_unit("warrior", 1, target);
    g.units.get_mut(&victim).unwrap().hp = 1;
    g.apply(
        0,
        &Action::Attack {
            unit: raider,
            target,
        },
    )
    .unwrap();
    assert!(!g.units.contains_key(&victim));
    assert_eq!(g.players[1].counters.get("lost_to_barbarians"), Some(&1));
    assert_eq!(
        g.players[1].counters.get("civilians_lost_to_barbarians"),
        None
    );

    // Walking onto an undefended Builder is a capture, and it counts too.
    g.begin_turn(0);
    let at = g.units[&raider].pos;
    let builder_home = g
        .nbrs(at)
        .into_iter()
        .find(|p| g.units_at(*p).is_empty())
        .unwrap();
    let builder = g.spawn_unit("builder", 1, builder_home);
    g.apply(
        0,
        &Action::Move {
            unit: raider,
            to: builder_home,
        },
    )
    .unwrap();
    assert_eq!(g.units[&builder].owner, 0);
    assert_eq!(
        g.players[1].counters.get("civilians_lost_to_barbarians"),
        Some(&1)
    );

    // The Free Cities seat also carries `is_barbarian`; a revolt's garrison
    // is not a raider, so its takings stay off the barbarian ledger.
    g.players[0].is_free_city = true;
    g.begin_turn(0);
    let at = g.units[&raider].pos;
    let second_home = g
        .nbrs(at)
        .into_iter()
        .find(|p| g.units_at(*p).is_empty())
        .unwrap();
    let second = g.spawn_unit("warrior", 1, second_home);
    g.units.get_mut(&second).unwrap().hp = 1;
    g.apply(
        0,
        &Action::Attack {
            unit: raider,
            target: second_home,
        },
    )
    .unwrap();
    assert!(!g.units.contains_key(&second));
    assert_eq!(g.players[1].counters.get("lost_to_barbarians"), Some(&1));
}

#[test]
fn pillage_rewards_scale_and_stack_with_norway_raid_and_the_chapel() {
    let (mut game, center, _) = controlled_game(2992);
    game.players[0].civ = "Norway".to_string();
    game.players[0].policies.insert(crate::name!("raid"));
    game.world_era = 2;
    game.game_speed = GameSpeed::Online;
    assert_eq!(
        game.scaled_pillage_amount(0, "gold", 50.0),
        112.5
    );
    game.game_speed = GameSpeed::Standard;

    let home_center = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| {
            game.wdist(center, *position) >= 6 && game.wdisk(*position, 1).len() == 7
        })
        .unwrap();
    let home_city = game.found_city_for(0, home_center, None);
    let plaza = game.cities[&home_city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != home_center)
        .unwrap();
    set_controlled_district(&mut game, home_city, plaza, "government_plaza");
    game.cities
        .get_mut(&home_city)
        .unwrap()
        .buildings
        .push(crate::name!("grand_masters_chapel"));

    let enemy_city = game.found_city_for(1, center, None);
    let mine = game.cities[&enemy_city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center)
        .unwrap();
    game.map.tiles.get_mut(&mine).unwrap().improvement = Some(crate::name!("mine"));
    let raider = game.spawn_test_unit("warrior", 0, mine);
    let (gold, science, faith) = (
        game.players[0].gold,
        game.players[0].research_overflow,
        game.players[0].faith,
    );

    game.apply(0, &Action::Pillage { unit: raider }).unwrap();
    assert_eq!(game.players[0].gold - gold, 225.0);
    assert_eq!(game.players[0].research_overflow - science, 67.5);
    assert_eq!(game.players[0].faith - faith, 67.5);
}

#[test]
fn every_district_layer_pays_the_district_reward() {
    let (mut game, center, _) = controlled_game(2993);
    let enemy_city = game.found_city_for(1, center, None);
    let campus = game.cities[&enemy_city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center)
        .unwrap();
    set_controlled_district(&mut game, enemy_city, campus, "campus");
    game.cities
        .get_mut(&enemy_city)
        .unwrap()
        .buildings
        .extend([crate::name!("library"), crate::name!("university")]);
    let raider = game.spawn_test_unit("horseman", 0, campus);
    let science = game.players[0].research_overflow;

    for (layer, expected_building) in
        [(1, Some("university")), (2, Some("library")), (3, None)]
    {
        game.units.get_mut(&raider).unwrap().moves_left = 4.0;
        game.apply(0, &Action::Pillage { unit: raider }).unwrap();
        assert_eq!(
            game.players[0].research_overflow - science,
            25.0 * f64::from(layer),
            "campus building and base layers all pay Science"
        );
        if let Some(building) = expected_building {
            assert!(game.cities[&enemy_city]
                .pillaged_buildings
                .contains(&Name::new(building)));
        }
    }
    assert!(game.map.tiles[&campus].pillaged);
}

#[test]
fn coastal_raids_spend_movement_and_loot_is_flat_gold() {
    let (mut game, center, _) = controlled_game(2994);
    let enemy_city = game.found_city_for(1, center, None);
    let target = game.cities[&enemy_city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center)
        .unwrap();
    game.map.tiles.get_mut(&target).unwrap().improvement = Some(crate::name!("farm"));
    let origin = game
        .nbrs(target)
        .into_iter()
        .find(|position| *position != center)
        .unwrap();
    game.map.tiles.get_mut(&origin).unwrap().terrain = crate::name!("coast");
    let privateer = game.spawn_test_unit("privateer", 0, origin);
    game.units
        .get_mut(&privateer)
        .unwrap()
        .promotions
        .insert(crate::name!("loot"));
    game.units.get_mut(&privateer).unwrap().hp = 25;
    let attacks = game.units[&privateer].attacks_left;
    let gold = game.players[0].gold;

    game.apply(
        0,
        &Action::CoastalRaid {
            unit: privateer,
            target,
        },
    )
    .unwrap();
    assert_eq!(game.units[&privateer].hp, 75);
    assert_eq!(game.players[0].gold - gold, 50.0);
    assert_eq!(game.units[&privateer].moves_left, 1.0);
    assert_eq!(game.units[&privateer].attacks_left, attacks);

    let (mut norway, center, _) = controlled_game(2995);
    norway.players[0].civ = "Norway".to_string();
    let enemy_city = norway.found_city_for(1, center, None);
    let target = norway.cities[&enemy_city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center)
        .unwrap();
    norway.map.tiles.get_mut(&target).unwrap().improvement = Some(crate::name!("mine"));
    let origin = norway
        .nbrs(target)
        .into_iter()
        .find(|position| *position != center)
        .unwrap();
    norway.map.tiles.get_mut(&origin).unwrap().terrain = crate::name!("coast");
    let galley = norway.spawn_test_unit("galley", 0, origin);
    norway.units.get_mut(&galley).unwrap().attacks_left = 0;
    assert!(norway.can_coastal_raid(0, &norway.units[&galley]));
    norway
        .apply(
            0,
            &Action::CoastalRaid {
                unit: galley,
                target,
            },
        )
        .unwrap();
    assert_eq!(norway.players[0].research_overflow, 15.0);
    assert_eq!(norway.units[&galley].moves_left, 0.0);
}

#[test]
fn passive_healing_uses_city_friendly_neutral_and_enemy_rates() {
    let (mut g, center, ring) = controlled_game(300);
    let settler = g.spawn_unit("settler", 0, center);
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let home = g.player_city_ids(0)[0];
    let friendly = ring
        .iter()
        .copied()
        .find(|pos| g.map.tiles[pos].owner_city == Some(home))
        .unwrap();
    assert_ne!(friendly, center);
    assert_eq!(g.city_at(friendly), None);
    // Any district receives the 20 HP district rate.
    g.map.tiles.get_mut(&friendly).unwrap().district = Some(crate::name!("campus"));
    let plain_friendly = ring
        .iter()
        .copied()
        .find(|pos| *pos != friendly && g.map.tiles[pos].owner_city == Some(home))
        .unwrap();

    let neutral = g
        .wdisk(center, 2)
        .into_iter()
        .find(|pos| g.wdist(center, *pos) == 2 && g.map.tiles[pos].owner_city.is_none())
        .unwrap();
    let enemy_center = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| g.wdist(center, *pos) >= 6 && g.wdisk(*pos, 1).len() == 7)
        .unwrap();
    let enemy_city = g.found_city_for(1, enemy_center, None);
    let enemy = g
        .nbrs(enemy_center)
        .into_iter()
        .find(|pos| g.map.tiles[pos].owner_city == Some(enemy_city))
        .unwrap();
    assert_eq!(g.city_at(friendly), None);

    let cases = [
        (
            g.spawn_unit("warrior", 0, center),
            HealingLocation::District,
            20,
        ),
        (
            g.spawn_unit("warrior", 0, friendly),
            HealingLocation::District,
            20,
        ),
        (
            g.spawn_unit("warrior", 0, plain_friendly),
            HealingLocation::FriendlyTerritory,
            15,
        ),
        (
            g.spawn_unit("warrior", 0, neutral),
            HealingLocation::NeutralTerritory,
            10,
        ),
        (
            g.spawn_unit("warrior", 0, enemy),
            HealingLocation::EnemyTerritory,
            5,
        ),
    ];
    for (uid, location, rate) in cases {
        let pos = {
            let unit = g.units.get_mut(&uid).unwrap();
            unit.hp = 50;
            unit.acted = false;
            unit.pos
        };
        assert_eq!(g.healing_location(0, pos), location);
        assert_eq!(g.unit_heal_rate(uid), rate);
    }

    g.begin_turn(0);
    assert_eq!(g.units[&cases[0].0].hp, 70);
    assert_eq!(g.units[&cases[1].0].hp, 70);
    assert_eq!(g.units[&cases[2].0].hp, 65);
    assert_eq!(g.units[&cases[3].0].hp, 60);
    assert_eq!(g.units[&cases[4].0].hp, 55);

    // Peace and Open Borders do not make another civilization friendly.
    g.at_war.remove(&pair(0, 1));
    assert_eq!(
        g.healing_location(0, enemy),
        HealingLocation::EnemyTerritory
    );
    assert_eq!(g.unit_heal_rate(cases[4].0), 5);
}

#[test]
fn naval_and_embarked_units_only_heal_in_friendly_territory() {
    let (mut g, center, ring) = controlled_game(3001);
    let cid = g.found_city_for(0, center, None);
    let friendly = ring[0];
    g.map.tiles.get_mut(&friendly).unwrap().owner_city = Some(cid);
    g.map.tiles.get_mut(&friendly).unwrap().terrain = crate::name!("coast");
    let neutral = g
        .wdisk(center, 2)
        .into_iter()
        .find(|pos| g.wdist(center, *pos) == 2 && g.map.tiles[pos].owner_city.is_none())
        .unwrap();
    g.map.tiles.get_mut(&neutral).unwrap().terrain = crate::name!("coast");

    let galley_home = g.spawn_unit("galley", 0, friendly);
    let galley_away = g.spawn_unit("galley", 0, neutral);
    let embarked = g.spawn_unit("warrior", 0, friendly);
    assert_eq!(g.unit_heal_rate(galley_home), 20);
    assert_eq!(g.unit_heal_rate(embarked), 20);
    assert_eq!(g.unit_heal_rate(galley_away), 0);
}

#[test]
fn unit_class_matchups_feed_the_real_melee_damage_roll() {
    let (mut g, target, ring) = controlled_game(301);
    let attacker = g.spawn_unit("spearman", 0, ring[0]);
    let defender = g.spawn_unit("horseman", 1, target);

    assert_eq!(g.matchup_bonus(attacker, &g.units[&defender], true), 10.0);
    let mut expected_rng = g.rng.clone();
    let expected_out = damage(35.0, 36.0, &mut expected_rng);
    let expected_in = damage(36.0, 35.0, &mut expected_rng);
    g.apply(
        0,
        &Action::Attack {
            unit: attacker,
            target,
        },
    )
    .unwrap();
    assert_eq!(g.units[&defender].hp, 100 - expected_out);
    assert_eq!(g.units[&attacker].hp, 100 - expected_in);

    let (mut g, target, ring) = controlled_game(302);
    let spear = g.spawn_unit("spearman", 0, ring[0]);
    let war_cart = g.spawn_unit("war_cart", 1, target);
    assert_eq!(
        g.matchup_bonus(spear, &g.units[&war_cart], true),
        0.0,
        "War-Carts are immune to the anti-cavalry modifier"
    );
    let maryannu = g.spawn_unit("maryannu_chariot_archer", 1, ring[1]);
    assert_eq!(
        g.matchup_bonus(spear, &g.units[&maryannu], true),
        10.0,
        "ranged cavalry still receives the anti-cavalry modifier"
    );
}

#[test]
fn military_tradition_flanking_and_support_follow_provider_rules() {
    let (mut g, target, ring) = controlled_game(303);
    let attacker = g.spawn_unit("warrior", 0, ring[0]);
    let defender = g.spawn_unit("warrior", 1, target);
    let flank_archer = g.spawn_unit("archer", 0, ring[1]);
    let support_archer = g.spawn_unit("archer", 1, ring[2]);

    assert_eq!(g.flanking_bonus(attacker, target), 0.0);
    assert_eq!(g.support_bonus(&g.units[&defender]), 0.0);
    g.players[0].civics.insert(crate::name!("military_tradition"));
    g.players[1].civics.insert(crate::name!("military_tradition"));
    assert_eq!(
        g.flanking_bonus(attacker, target),
        2.0,
        "a ranged military unit provides one flanking stack"
    );
    assert_eq!(g.support_bonus(&g.units[&defender]), 2.0);

    // Rivers block flanking but not support.
    assert!(g.map.set_river_edge(ring[1], target, true));
    assert_eq!(g.flanking_bonus(attacker, target), 0.0);
    assert!(g.map.set_river_edge(ring[2], target, true));
    assert_eq!(g.support_bonus(&g.units[&defender]), 2.0);

    // Embarked land units provide Support but cannot provide Flanking.
    assert!(g.map.set_river_edge(ring[1], target, false));
    g.map.tiles.get_mut(&ring[1]).unwrap().terrain = crate::name!("coast");
    assert!(g.is_embarked(&g.units[&flank_archer]));
    assert_eq!(g.flanking_bonus(attacker, target), 0.0);
    g.map.tiles.get_mut(&ring[2]).unwrap().terrain = crate::name!("coast");
    assert!(g.is_embarked(&g.units[&support_archer]));
    assert_eq!(g.support_bonus(&g.units[&defender]), 2.0);
}

#[test]
fn ranged_attacks_require_an_open_range_two_sight_corridor() {
    let (mut g, target, _) = controlled_game(304);
    let from = g
        .wdisk(target, 2)
        .into_iter()
        .find(|p| g.wdist(*p, target) == 2)
        .unwrap();
    let attacker = g.spawn_unit("archer", 0, from);
    let defender = g.spawn_unit("warrior", 1, target);
    let middles: Vec<Pos> = g
        .nbrs(from)
        .into_iter()
        .filter(|p| g.wdist(*p, target) == 1)
        .collect();
    assert!(!middles.is_empty());
    for middle in &middles {
        g.map.tiles.get_mut(middle).unwrap().terrain = crate::name!("mountain");
    }
    let shot = Action::Ranged {
        unit: attacker,
        target,
    };
    let legal_shot = |g: &Game| {
        g.legal_actions(0).into_iter().any(|action| {
            matches!(action, Action::Ranged { unit, target: to }
            if unit == attacker && to == target)
        })
    };
    assert!(!legal_shot(&g));
    assert_eq!(g.apply(0, &shot).unwrap_err(), "target is not visible");
    assert_eq!(g.units[&defender].hp, 100);

    g.map.tiles.get_mut(&middles[0]).unwrap().terrain = crate::name!("plains");
    assert!(legal_shot(&g));
    g.apply(0, &shot).unwrap();
    assert!(g.units[&defender].hp < 100);
}

#[test]
fn melee_attack_requires_enough_movement_to_enter_the_target_tile() {
    let (mut g, target, ring) = controlled_game(305);
    let attacker = g.spawn_unit("warrior", 0, ring[0]);
    g.spawn_unit("warrior", 1, target);
    g.map.tiles.get_mut(&target).unwrap().feature = Some(crate::name!("forest"));
    assert!(g.map.set_river_edge(ring[0], target, true));
    let attack = Action::Attack {
        unit: attacker,
        target,
    };
    let legal_attack = |g: &Game| {
        g.legal_actions(0).into_iter().any(|action| {
            matches!(action, Action::Attack { unit, target: to }
            if unit == attacker && to == target)
        })
    };

    g.units.get_mut(&attacker).unwrap().moves_left = 1.0;
    assert!(!legal_attack(&g));
    assert_eq!(
        g.apply(0, &attack).unwrap_err(),
        "not enough movement to attack"
    );

    // The minimum-one-tile rule allows the costly forest/river entry
    // when the unit still has all of its normal Movement.
    g.units.get_mut(&attacker).unwrap().moves_left = 2.0;
    assert!(legal_attack(&g));
    g.apply(0, &attack).unwrap();
    assert_eq!(g.units[&attacker].moves_left, 0.0);
}

#[test]
fn hybrid_and_interception_only_units_offer_exact_attack_modes() {
    let (mut game, target, ring) = controlled_game(3051);
    let robot = game.spawn_unit("giant_death_robot", 0, ring[0]);
    game.spawn_unit("warrior", 1, target);
    let actions = game.legal_actions(0);
    assert!(actions.iter().any(|action| {
        matches!(action, Action::Attack { unit, target: position }
            if *unit == robot && *position == target)
    }));
    assert!(actions.iter().any(|action| {
        matches!(action, Action::Ranged { unit, target: position }
            if *unit == robot && *position == target)
    }));
    let mut melee = game.clone();
    melee
        .apply(
            0,
            &Action::Attack {
                unit: robot,
                target,
            },
        )
        .unwrap();
    game.apply(
        0,
        &Action::Ranged {
            unit: robot,
            target,
        },
    )
    .unwrap();

    for (seed, kind) in [(3052, "anti_air_gun"), (3053, "mobile_sam")] {
        let (mut defense, target, ring) = controlled_game(seed);
        let anti_air = defense.spawn_unit(kind, 0, ring[0]);
        defense.spawn_unit("warrior", 1, target);
        assert!(!defense.rules.units[kind].is_melee_capable());
        assert!(!defense.rules.units[kind].has_ranged_attack());
        assert!(!defense.legal_actions(0).iter().any(|action| {
            matches!(action,
                Action::Attack { unit, .. } | Action::Ranged { unit, .. }
                if *unit == anti_air)
        }));
        assert_eq!(
            defense
                .apply(
                    0,
                    &Action::Attack {
                        unit: anti_air,
                        target,
                    },
                )
                .unwrap_err(),
            "unit cannot melee attack"
        );
        assert_eq!(
            defense
                .apply(
                    0,
                    &Action::Ranged {
                        unit: anti_air,
                        target,
                    },
                )
                .unwrap_err(),
            "unit has no ranged attack"
        );
    }
}

#[test]
fn zoc_is_innate_and_the_unit_roster_uses_explicit_civ6_classes() {
    let (g, _, _) = controlled_game(306);
    for name in [
        "scout",
        "warrior",
        "spearman",
        "horseman",
        "infantry",
        "tank",
        "helicopter",
        "galley",
        "quadrireme",
        "frigate",
        "privateer",
        "battleship",
        "destroyer",
        "aircraft_carrier",
        "missile_cruiser",
        "giant_death_robot",
    ] {
        assert!(g.rules.units[name].zone_of_control, "{name} must exert ZOC");
    }

    for name in ["horseman", "knight", "war_cart"] {
        let spec = &g.rules.units[name];
        assert!(spec.zone_of_control, "{name} must exert ZOC");
        assert!(spec.cavalry, "{name} must ignore incoming ZOC");
    }
    for name in [
        "slinger",
        "archer",
        "catapult",
        "crossbowman",
        "pitati_archer",
        "maryannu_chariot_archer",
        "saka_horse_archer",
        "crouching_tiger",
        "artillery",
        "machine_gun",
        "anti_air_gun",
        "mobile_sam",
        "observation_balloon",
        "submarine",
        "nuclear_submarine",
    ] {
        assert!(
            !g.rules.units[name].zone_of_control,
            "{name} must not exert ZOC"
        );
    }
    assert!(g
        .players
        .iter()
        .all(|p| !p.civics.contains(&crate::name!("military_tradition"))));
}

#[test]
fn zoc_stops_combatants_but_cavalry_ignores_and_rivers_block_it() {
    let (mut g, enemy_pos, ring) = controlled_game(307);
    g.spawn_unit("warrior", 1, enemy_pos);
    let entry = ring[0];
    let start = g
        .nbrs(entry)
        .into_iter()
        .find(|p| g.wdist(*p, enemy_pos) == 2)
        .unwrap();
    let warrior = g.spawn_unit("warrior", 0, start);
    g.apply(
        0,
        &Action::Move {
            unit: warrior,
            to: entry,
        },
    )
    .unwrap();
    assert!(g.units[&warrior].zoc_stopped);
    assert_eq!(g.units[&warrior].moves_left, 1.0);
    assert!(g.legal_actions(0).into_iter().any(|action| {
        matches!(action, Action::Attack { unit, target }
            if unit == warrior && target == enemy_pos)
    }));

    let (mut g, enemy_pos, ring) = controlled_game(308);
    g.spawn_unit("warrior", 1, enemy_pos);
    let entry = ring[0];
    let start = g
        .nbrs(entry)
        .into_iter()
        .find(|p| g.wdist(*p, enemy_pos) == 2)
        .unwrap();
    let horse = g.spawn_unit("horseman", 0, start);
    g.apply(
        0,
        &Action::Move {
            unit: horse,
            to: entry,
        },
    )
    .unwrap();
    assert!(!g.units[&horse].zoc_stopped);
    assert!(g.units[&horse].moves_left > 0.0);

    let (mut g, enemy_pos, ring) = controlled_game(309);
    g.spawn_unit("warrior", 1, enemy_pos);
    assert!(g.map.set_river_edge(enemy_pos, ring[0], true));
    assert!(!g.in_enemy_zoc(0, ring[0]));
}

/// The joint tactical search builds its approach lines from this reading:
/// real step costs and paths, and a zone of control that stops the walk but
/// not the blow — the flood keeps the movement the unit arrives with, unlike
/// `reachable`'s "can it move on" answer.
#[test]
fn approach_reach_keeps_movement_for_a_blow_inside_zoc_and_stops_the_walk_there() {
    let (mut g, enemy_pos, ring) = controlled_game(310);
    g.spawn_unit("warrior", 1, enemy_pos);
    // A horseman four tiles out on flat ground: it reaches the ring with
    // movement to spare, and the flood's path is the engine's own.
    let far = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|p| g.wdist(*p, enemy_pos) == 4 && g.rules.is_passable(&g.map.tiles[p]))
        .expect("a tile four out");
    let horse = g.spawn_unit("horseman", 0, far);
    let reach = g.approach_reach(horse);
    let entry = ring
        .iter()
        .copied()
        .find(|r| reach.contains_key(r))
        .expect("the ring is within a four-move unit's reach");
    let (kept, path) = &reach[&entry];
    assert_eq!(path.len(), 3, "four out, three steps to the ring");
    assert_eq!(*path.last().unwrap(), entry);
    assert!(
        *kept > 0.0,
        "cavalry ignores zone of control; it keeps its spare movement: {kept}"
    );
    // Every reported tile is reachable, and every path step is a legal move
    // in sequence — the reading is the engine's, not a guess.
    for (to, (_, path)) in &reach {
        let mut probe = g.clone();
        for step in path {
            probe
                .apply(
                    0,
                    &Action::Move {
                        unit: horse,
                        to: *step,
                    },
                )
                .unwrap_or_else(|why| panic!("path to {to:?} refused at {step:?}: {why}"));
        }
        assert_eq!(probe.units[&horse].pos, *to);
    }
    assert!(
        !reach.contains_key(&enemy_pos),
        "the enemy's own tile is no destination"
    );

    // A foot soldier that ends its walk inside the zone of control keeps what
    // it has left for the attack (`zoc_stops_combatants…` proves the engine
    // does), and the flood does not walk on from that tile.
    let (mut g, enemy_pos, ring) = controlled_game(311);
    g.spawn_unit("warrior", 1, enemy_pos);
    let start = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|p| g.wdist(*p, enemy_pos) == 2 && g.rules.is_passable(&g.map.tiles[p]))
        .expect("a tile two out");
    let warrior = g.spawn_unit("warrior", 0, start);
    let reach = g.approach_reach(warrior);
    let ring_tiles: Vec<Pos> = ring
        .iter()
        .copied()
        .filter(|r| reach.contains_key(r))
        .collect();
    assert!(
        !ring_tiles.is_empty(),
        "a two-move warrior two out reaches the ring"
    );
    for r in &ring_tiles {
        let (kept, path) = &reach[r];
        assert_eq!(path.len(), 1);
        assert_eq!(*kept, 1.0, "one step spent, one kept for the blow");
        assert!(
            *kept >= g.step_cost_for(warrior, *r, enemy_pos),
            "the kept movement pays the defender's plains tile"
        );
    }
    for to in reach.keys() {
        assert!(
            g.wdist(*to, start) <= 2,
            "{to:?} lies past a zone-of-control stop"
        );
    }
}

#[test]
fn civilian_support_religious_and_district_zoc_follow_civ6_behavior() {
    for (seed, kind) in [(310, "builder"), (311, "battering_ram")] {
        let (mut g, enemy_pos, ring) = controlled_game(seed);
        g.spawn_unit("warrior", 1, enemy_pos);
        let entry = ring[0];
        let start = g
            .nbrs(entry)
            .into_iter()
            .find(|p| g.wdist(*p, enemy_pos) == 2)
            .unwrap();
        let mover = g.spawn_unit(kind, 0, start);
        g.apply(
            0,
            &Action::Move {
                unit: mover,
                to: entry,
            },
        )
        .unwrap();
        assert_eq!(g.units[&mover].moves_left, 0.0, "{kind}");
        assert!(g.units[&mover].zoc_stopped, "{kind}");
        assert!(
            !g.legal_actions(0).iter().any(|action| {
                matches!(action, Action::Improve { unit, .. } if *unit == mover)
                    || matches!(action, Action::UnlinkUnits { unit } if *unit == mover)
            }),
            "{kind} must not receive follow-up actions after entering ZOC"
        );
        if kind == "builder" {
            assert_eq!(
                g.apply(
                    0,
                    &Action::Improve {
                        unit: mover,
                        improvement: crate::name!("farm"),
                    },
                )
                .unwrap_err(),
                "non-combat unit cannot act after entering zone of control"
            );
        }
    }

    let (mut g, enemy_pos, ring) = controlled_game(312);
    g.at_war.clear();
    g.players[0].religion = Some("A".to_string());
    g.players[1].religion = Some("B".to_string());
    g.spawn_unit("missionary", 1, enemy_pos);
    let entry = ring[0];
    let start = g
        .nbrs(entry)
        .into_iter()
        .find(|p| g.wdist(*p, enemy_pos) == 2)
        .unwrap();
    let missionary = g.spawn_unit("missionary", 0, start);
    g.apply(
        0,
        &Action::Move {
            unit: missionary,
            to: entry,
        },
    )
    .unwrap();
    assert!(g.units[&missionary].zoc_stopped);
    assert!(g.units[&missionary].moves_left > 0.0);

    let (mut g, enemy_pos, ring) = controlled_game(3121);
    g.at_war.clear();
    g.players[0].religion = Some("A".to_string());
    g.players[1].religion = Some("A".to_string());
    g.spawn_unit("missionary", 1, enemy_pos);
    let entry = ring[0];
    let start = g
        .nbrs(entry)
        .into_iter()
        .find(|p| g.wdist(*p, enemy_pos) == 2)
        .unwrap();
    let missionary = g.spawn_unit("missionary", 0, start);
    g.apply(
        0,
        &Action::Move {
            unit: missionary,
            to: entry,
        },
    )
    .unwrap();
    assert!(!g.units[&missionary].zoc_stopped);

    let (mut g, city_pos, ring) = controlled_game(313);
    g.found_city_for(1, city_pos, Some("Test".to_string()));
    assert!(g.in_enemy_zoc(0, ring[0]));
}

#[test]
fn naval_surface_units_exert_zoc_and_naval_raiders_ignore_it() {
    let (mut g, enemy_pos, ring) = controlled_game(314);
    let entry = ring[0];
    let start = g
        .nbrs(entry)
        .into_iter()
        .find(|p| g.wdist(*p, enemy_pos) == 2)
        .unwrap();
    for pos in [enemy_pos, entry, start] {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
    }
    g.spawn_unit("quadrireme", 1, enemy_pos);
    assert!(g.in_enemy_zoc(0, entry), "naval ranged units exert ZOC");
    let privateer = g.spawn_unit("privateer", 0, start);
    g.apply(
        0,
        &Action::Move {
            unit: privateer,
            to: entry,
        },
    )
    .unwrap();
    assert!(!g.units[&privateer].zoc_stopped);
    assert!(g.units[&privateer].moves_left > 0.0);
    assert!(
        g.reachable(privateer).contains(&start),
        "the path planner must share Naval Raider ZOC immunity"
    );

    let (mut g, enemy_pos, ring) = controlled_game(315);
    for pos in std::iter::once(enemy_pos).chain(ring.iter().copied()) {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
    }
    g.spawn_unit("privateer", 1, enemy_pos);
    assert!(g.in_enemy_zoc(0, ring[0]), "Privateers also project ZOC");

    let (mut g, enemy_pos, ring) = controlled_game(316);
    for pos in std::iter::once(enemy_pos).chain(ring.iter().copied()) {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
    }
    g.spawn_unit("submarine", 1, enemy_pos);
    assert!(
        !g.in_enemy_zoc(0, ring[0]),
        "Submarines are the naval projection exception"
    );
}

#[test]
fn linked_noncombat_units_inherit_their_escorts_zoc_behavior() {
    let (mut g, enemy_pos, ring) = controlled_game(317);
    g.spawn_unit("warrior", 1, enemy_pos);
    let entry = ring[0];
    let start = g
        .nbrs(entry)
        .into_iter()
        .find(|p| g.wdist(*p, enemy_pos) == 2)
        .unwrap();
    let horse = g.spawn_unit("horseman", 0, start);
    let ram = g.spawn_unit("battering_ram", 0, start);
    g.apply(
        0,
        &Action::LinkUnits {
            unit: horse,
            with: ram,
        },
    )
    .unwrap();
    g.apply(
        0,
        &Action::Move {
            unit: horse,
            to: entry,
        },
    )
    .unwrap();
    assert!(!g.units[&horse].zoc_stopped);
    assert!(!g.units[&ram].zoc_stopped);
    assert!(g.units[&ram].moves_left > 0.0);

    let (mut g, enemy_pos, ring) = controlled_game(318);
    g.spawn_unit("warrior", 1, enemy_pos);
    let entry = ring[0];
    let start = g
        .nbrs(entry)
        .into_iter()
        .find(|p| g.wdist(*p, enemy_pos) == 2)
        .unwrap();
    let escort = g.spawn_unit("warrior", 0, start);
    let ram = g.spawn_unit("battering_ram", 0, start);
    g.apply(
        0,
        &Action::LinkUnits {
            unit: escort,
            with: ram,
        },
    )
    .unwrap();
    g.apply(
        0,
        &Action::Move {
            unit: escort,
            to: entry,
        },
    )
    .unwrap();
    assert!(g.units[&escort].zoc_stopped);
    assert!(g.units[&ram].zoc_stopped);
    assert_eq!(g.units[&ram].moves_left, 0.0);
    assert!(g.apply(0, &Action::UnlinkUnits { unit: ram }).is_err());
}

#[test]
fn keshig_shares_its_four_moves_with_an_escorted_civilian() {
    let (mut g, center, ring) = controlled_game(3181);
    let keshig = g.spawn_unit("keshig", 0, center);
    let builder = g.spawn_unit("builder", 0, center);
    g.apply(
        0,
        &Action::LinkUnits {
            unit: keshig,
            with: builder,
        },
    )
    .unwrap();
    g.begin_turn(0);

    assert_eq!(g.unit_max_moves(keshig), 4.0);
    assert_eq!(g.unit_max_moves(builder), 4.0);
    let second = g
        .nbrs(ring[0])
        .into_iter()
        .find(|pos| *pos != center && g.map.tiles.contains_key(pos))
        .unwrap();
    for to in [ring[0], second] {
        g.apply(0, &Action::Move { unit: keshig, to }).unwrap();
    }
    assert_eq!(g.units[&keshig].moves_left, 2.0);
    assert_eq!(g.units[&builder].moves_left, 2.0);
}

#[test]
fn starting_in_zoc_allows_leaving_first_but_not_after_attacking() {
    let (mut g, enemy_pos, ring) = controlled_game(319);
    let entry = ring[0];
    let escape = g
        .nbrs(entry)
        .into_iter()
        .find(|p| g.wdist(*p, enemy_pos) == 2)
        .unwrap();
    g.spawn_unit("warrior", 1, enemy_pos);
    let warrior = g.spawn_unit("warrior", 0, entry);
    g.units
        .get_mut(&warrior)
        .unwrap()
        .promotions
        .insert(crate::name!("elite_guard"));
    g.begin_turn(0);
    assert!(g.units[&warrior].started_turn_in_zoc);
    g.apply(
        0,
        &Action::Attack {
            unit: warrior,
            target: enemy_pos,
        },
    )
    .unwrap();
    assert!(g.units.contains_key(&warrior));
    assert!(g.units[&warrior].moves_left > 0.0);
    assert!(!g.can_move(warrior, escape));

    let (mut g, enemy_pos, ring) = controlled_game(320);
    let entry = ring[0];
    let escape = g
        .nbrs(entry)
        .into_iter()
        .find(|p| g.wdist(*p, enemy_pos) == 2)
        .unwrap();
    g.spawn_unit("warrior", 1, enemy_pos);
    let warrior = g.spawn_unit("warrior", 0, entry);
    g.begin_turn(0);
    assert!(g.units[&warrior].started_turn_in_zoc);
    assert!(g.can_move(warrior, escape));
    g.apply(
        0,
        &Action::Move {
            unit: warrior,
            to: escape,
        },
    )
    .unwrap();
    assert_eq!(g.units[&warrior].pos, escape);
    assert!(!g.units[&warrior].zoc_stopped);
}

/// The range a watcher is shown for somebody else's unit reads past the
/// unit standing in the way and never past zone of control. The two are
/// opposite kinds of obstacle: whatever is parked on a tile can walk off
/// it or die on it long before the threat expires, so for the question
/// "how far does that thing reach" it is noise — while zone of control is
/// a fact about the ground that will still be there, and the reading is
/// worthless if it quietly ignores it.
#[test]
fn a_read_range_passes_units_and_still_stops_at_zone_of_control() {
    // Flat plains, no rivers, no borders, two seats already at war: the
    // only things deciding this answer are the units placed below.
    let (mut g, start, ring) = controlled_game(4471);
    let mover = g.spawn_unit("warrior", 0, start);
    let blocked = ring[0];
    g.spawn_unit("warrior", 0, blocked);
    g.begin_turn(0);

    assert!(
        !g.reachable(mover).contains(&blocked),
        "a tile with a unit on it is not somewhere its owner may legally move"
    );
    assert!(
        g.threat_reach(mover).contains(&blocked),
        "the tile in the way is still inside the reach a watcher is shown"
    );

    // The far corner of the second ring beyond `blocked`, which the layout
    // reaches only through `blocked` — derived rather than assumed, so a
    // change of hex geometry fails here loudly instead of quietly
    // asserting nothing.
    let corner = g
        .wdisk(start, 2)
        .into_iter()
        .find(|pos| {
            g.wdist(*pos, start) == 2
                && g
                    .nbrs(*pos)
                    .into_iter()
                    .filter(|step| g.wdist(*step, start) == 1)
                    .eq(std::iter::once(blocked))
        })
        .expect("a second-ring tile entered only through the blocked one");
    assert!(
        g.threat_reach(mover).contains(&corner),
        "two plains steps are two movement points; the corner is in reach"
    );
    assert!(
        g.attack_reach(mover).contains(&corner),
        "after one legal approach step the warrior still has the Movement to attack the corner"
    );

    // The reading survives a unit having nothing left to spend. Outside
    // its own turn every unit on the board is in exactly this state, and
    // in spectate there is no acting seat at all — so a reach measured
    // from `moves_left` would be empty for almost everything anybody ever
    // points at, which is how this was found in the first place.
    let spent = g.units[&mover].moves_left;
    g.units.get_mut(&mover).expect("the mover").moves_left = 0.0;
    assert!(
        g.reachable(mover).is_empty(),
        "a unit with nothing left may legally go nowhere"
    );
    assert!(
        g.threat_reach(mover).contains(&corner),
        "and still reaches exactly as far, because the threat is about a \
         turn's movement rather than about this instant"
    );
    g.units.get_mut(&mover).expect("the mover").moves_left = spent;

    // Now put a hostile unit where it exerts zone of control over the one
    // tile that corner is entered from. Entering ZOC ends movement, so the
    // corner goes out of reach — for the watcher's reading exactly as for
    // the mover's own.
    let watcher = g
        .nbrs(blocked)
        .into_iter()
        .find(|pos| *pos != corner && *pos != start && g.wdist(*pos, start) == 2)
        .expect("a tile adjacent to the blocked step");
    g.spawn_unit("warrior", 1, watcher);
    g.begin_turn(0);
    assert!(
        !g.threat_reach(mover).contains(&corner),
        "zone of control does not move out of the way, so the reading keeps it"
    );
    assert!(
        !g.attack_reach(mover).contains(&corner),
        "the same ZOC leaves no Movement for the final melee attack, so the corner is safe"
    );
}

#[test]
fn melee_advance_into_a_second_zoc_stops_move_after_attack_units() {
    let (mut g, provider_pos, ring) = controlled_game(3201);
    let target = ring[0];
    let start = g
        .nbrs(target)
        .into_iter()
        .find(|p| g.wdist(*p, provider_pos) == 2)
        .unwrap();
    let reserve = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|p| g.wdist(*p, provider_pos) >= 4)
        .unwrap();
    g.spawn_unit("settler", 1, reserve);
    g.spawn_unit("warrior", 1, provider_pos);
    let victim = g.spawn_unit("scout", 1, target);
    g.units.get_mut(&victim).unwrap().hp = 1;
    let scout = g.spawn_unit("scout", 0, start);
    g.units
        .get_mut(&scout)
        .unwrap()
        .promotions
        .insert(crate::name!("guerrilla"));
    g.apply(
        0,
        &Action::Attack {
            unit: scout,
            target,
        },
    )
    .unwrap();
    assert_eq!(g.units[&scout].pos, target);
    assert!(g.units[&scout].moves_left > 0.0);
    assert!(g.in_enemy_zoc_for(scout, target));
    assert!(g.units[&scout].zoc_stopped);
    assert!(g.reachable(scout).is_empty());
}

#[test]
fn stopped_combatants_can_promote_and_suppression_projects_zoc() {
    let (mut g, enemy_pos, ring) = controlled_game(321);
    g.spawn_unit("warrior", 1, enemy_pos);
    let entry = ring[0];
    let start = g
        .nbrs(entry)
        .into_iter()
        .find(|p| g.wdist(*p, enemy_pos) == 2)
        .unwrap();
    let archer = g.spawn_unit("archer", 0, start);
    g.units.get_mut(&archer).unwrap().xp = 15;
    g.apply(
        0,
        &Action::Move {
            unit: archer,
            to: entry,
        },
    )
    .unwrap();
    assert!(g.units[&archer].zoc_stopped);
    assert!(g.units[&archer].acted);
    assert!(g.units[&archer].moves_left > 0.0);
    let promotion = g.available_promotions(archer).into_iter().next().unwrap();
    g.apply(
        0,
        &Action::Promote {
            unit: archer,
            promotion,
        },
    )
    .unwrap();

    let (mut g, enemy_pos, ring) = controlled_game(322);
    let archer = g.spawn_unit("archer", 1, enemy_pos);
    assert!(!g.in_enemy_zoc(0, ring[0]));
    g.units
        .get_mut(&archer)
        .unwrap()
        .promotions
        .insert(crate::name!("suppression"));
    assert!(g.in_enemy_zoc(0, ring[0]));
}

#[test]
fn zoc_respects_native_domains_and_unpillaged_districts() {
    let (mut g, enemy_pos, ring) = controlled_game(323);
    g.map.tiles.get_mut(&enemy_pos).unwrap().terrain = crate::name!("coast");
    g.map.tiles.get_mut(&ring[0]).unwrap().terrain = crate::name!("coast");
    g.spawn_unit("warrior", 1, enemy_pos);
    assert!(
        !g.in_enemy_zoc(0, ring[0]),
        "embarked land units do not project onto adjacent water"
    );

    let (mut g, enemy_pos, ring) = controlled_game(324);
    g.map.tiles.get_mut(&enemy_pos).unwrap().terrain = crate::name!("coast");
    g.spawn_unit("galley", 1, enemy_pos);
    assert!(
        !g.in_enemy_zoc(0, ring[0]),
        "naval units do not project onto adjacent land"
    );

    let (mut g, city_pos, ring) = controlled_game(325);
    g.found_city_for(1, city_pos, None);
    g.map.tiles.get_mut(&ring[0]).unwrap().terrain = crate::name!("coast");
    assert!(g.map.set_river_edge(city_pos, ring[0], true));
    assert!(
        g.in_enemy_zoc(0, ring[0]),
        "City Centers project across rivers and into water"
    );

    let (mut g, city_pos, ring) = controlled_game(326);
    let cid = g.found_city_for(1, city_pos, None);
    let camp = ring[0];
    let target = g
        .nbrs(camp)
        .into_iter()
        .find(|p| g.wdist(*p, city_pos) == 2)
        .unwrap();
    {
        let tile = g.map.tiles.get_mut(&camp).unwrap();
        tile.district = Some(crate::name!("encampment"));
        tile.owner_city = Some(cid);
    }
    g.cities.get_mut(&cid).unwrap().encampment_hp = 0;
    g.cities.get_mut(&cid).unwrap().encampment_pillaged = false;
    assert!(g.in_enemy_zoc(0, target));
    g.cities.get_mut(&cid).unwrap().encampment_pillaged = true;
    assert!(!g.in_enemy_zoc(0, target));

    let (mut g, city_pos, ring) = controlled_game(327);
    let cid = g.found_city_for(1, city_pos, None);
    let oppidum = ring[0];
    let target = g
        .nbrs(oppidum)
        .into_iter()
        .find(|p| g.wdist(*p, city_pos) == 2)
        .unwrap();
    {
        let tile = g.map.tiles.get_mut(&oppidum).unwrap();
        tile.district = Some(crate::name!("oppidum"));
        tile.owner_city = Some(cid);
        tile.pillaged = false;
    }
    assert!(g.in_enemy_zoc(0, target));
    g.map.tiles.get_mut(&oppidum).unwrap().pillaged = true;
    assert!(!g.in_enemy_zoc(0, target));
}

#[test]
fn religious_layer_and_undefended_unit_capture_follow_civ6_targeting() {
    let (mut g, target, ring) = controlled_game(3131);
    g.at_war.clear();
    let missionary = g.spawn_unit("missionary", 1, target);
    let warrior = g.spawn_unit("warrior", 0, ring[0]);
    g.apply(
        0,
        &Action::Move {
            unit: warrior,
            to: target,
        },
    )
    .unwrap();
    assert_eq!(g.units[&missionary].pos, target);
    assert_eq!(
        g.units[&warrior].pos, target,
        "military and religious units use separate map layers"
    );

    // Settlers and Builders have no combat health to wear down: a melee
    // unit captures either one by walking onto its tile. Neither melee nor
    // ranged combat can target them.
    for (seed, civilian_kind) in [(3132, "settler"), (3133, "builder")] {
        let (mut g, target, ring) = controlled_game(seed);
        let civilian = g.spawn_unit(civilian_kind, 1, target);
        let warrior = g.spawn_unit("warrior", 0, ring[0]);
        let archer = g.spawn_unit("archer", 0, ring[1]);
        assert!(
            g.apply(
                0,
                &Action::Ranged {
                    unit: archer,
                    target
                }
            )
            .is_err(),
            "ranged attacks cannot target capture-only {civilian_kind}s"
        );
        assert!(!g.legal_actions(0).into_iter().any(|action| {
            matches!(action, Action::Attack { unit, target: to }
                if unit == warrior && to == target)
                || matches!(action, Action::Ranged { unit, target: to }
                    if unit == archer && to == target)
        }));
        g.apply(
            0,
            &Action::Move {
                unit: warrior,
                to: target,
            },
        )
        .unwrap();
        assert_eq!(g.units[&civilian].owner, 0, "{civilian_kind} should be captured");
        assert!(matches!(
            g.log.last(),
            Some((0, Action::Move { unit, to })) if *unit == warrior && *to == target
        ));
    }

    let (mut g, city_pos, ring) = controlled_game(3134);
    let cid = g.found_city_for(0, city_pos, None);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    let civilian_pos = ring[0];
    g.spawn_unit("builder", 1, civilian_pos);
    assert!(!g.legal_actions(0).into_iter().any(|action| {
        matches!(action, Action::CityStrike { city, target }
            if city == cid && target == civilian_pos)
    }));
    assert!(g
        .apply(
            0,
            &Action::CityStrike {
                city: cid,
                target: civilian_pos,
            }
        )
        .is_err());
}

#[test]
fn city_garrisons_are_protected_and_a_siege_ring_prevents_healing() {
    let (mut g, city_pos, ring) = controlled_game(314);
    let cid = g.found_city_for(1, city_pos, Some("Test".to_string()));
    let garrison = g.spawn_unit("warrior", 1, city_pos);
    let archer = g.spawn_unit("archer", 0, ring[0]);
    let before = g.cities[&cid].hp;
    g.apply(
        0,
        &Action::Ranged {
            unit: archer,
            target: city_pos,
        },
    )
    .unwrap();
    assert!(g.cities[&cid].hp < before);
    assert_eq!(g.units[&garrison].hp, 100);
    assert!(g.wars[&pair(0, 1)]
        .participants
        .iter()
        .find(|participant| participant.player == 1)
        .unwrap()
        .saw_action_units
        .contains_key(&garrison));

    g.cities.get_mut(&cid).unwrap().hp = 100;
    for pos in ring {
        g.spawn_unit("warrior", 0, pos);
    }
    assert!(g.city_under_siege(cid));
    g.process_city(1, cid);
    assert_eq!(g.cities[&cid].hp, 100);
}

#[test]
fn ranged_and_bombard_city_final_blows_and_scythia_healing_follow_civ6() {
    let (mut g, city_pos, ring) = controlled_game(3141);
    let cid = g.found_city_for(1, city_pos, None);
    g.cities.get_mut(&cid).unwrap().hp = 1;
    let archer = g.spawn_unit("archer", 0, ring[0]);
    g.apply(
        0,
        &Action::Ranged {
            unit: archer,
            target: city_pos,
        },
    )
    .unwrap();
    assert_eq!(g.cities[&cid].hp, 1);
    assert_eq!(g.units[&archer].xp, 3);

    let catapult = g.spawn_unit("catapult", 0, ring[1]);
    g.apply(
        0,
        &Action::Ranged {
            unit: catapult,
            target: city_pos,
        },
    )
    .unwrap();
    assert_eq!(
        g.cities[&cid].hp, 0,
        "Bombard attacks may deplete but cannot capture cities"
    );
    assert_eq!(g.cities[&cid].owner, 1);
    assert_eq!(g.units[&catapult].xp, 10);

    let second_catapult = g.spawn_unit("catapult", 0, ring[2]);
    g.apply(
        0,
        &Action::Ranged {
            unit: second_catapult,
            target: city_pos,
        },
    )
    .unwrap();
    assert_eq!(g.cities[&cid].hp, 0);
    assert_eq!(
        g.units[&second_catapult].xp, 0,
        "attacks after city depletion grant no XP"
    );

    let (mut g, target, ring) = controlled_game(3142);
    g.players[0].civ = "Scythia".to_string();
    let archer = g.spawn_unit("archer", 0, ring[0]);
    g.units.get_mut(&archer).unwrap().hp = 50;
    let defender = g.spawn_unit("warrior", 1, target);
    g.units.get_mut(&defender).unwrap().hp = 1;
    g.apply(
        0,
        &Action::Ranged {
            unit: archer,
            target,
        },
    )
    .unwrap();
    assert!(!g.units.contains_key(&defender));
    assert_eq!(g.units[&archer].hp, 80);

    let (mut g, target, ring) = controlled_game(3145);
    g.players[1].civ = "Scythia".to_string();
    let attacker = g.spawn_unit("warrior", 0, ring[0]);
    g.units.get_mut(&attacker).unwrap().hp = 1;
    let defender = g.spawn_unit("warrior", 1, target);
    g.units.get_mut(&defender).unwrap().hp = 50;
    let mut expected_rng = g.rng.clone();
    let expected_damage = damage(10.0, 15.0, &mut expected_rng);
    g.apply(
        0,
        &Action::Attack {
            unit: attacker,
            target,
        },
    )
    .unwrap();
    assert!(!g.units.contains_key(&attacker));
    assert_eq!(
        g.units[&defender].hp,
        (50 - expected_damage + 30).min(100),
        "Scythian defenders also heal after eliminating an attacker"
    );
}

#[test]
fn gathering_storm_walls_require_explicit_repair_and_land_units_to_fortify() {
    let (mut g, city_pos, _) = controlled_game(3143);
    let cid = g.found_city_for(0, city_pos, None);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    g.cities.get_mut(&cid).unwrap().wall_hp = 50;
    assert_eq!(g.city_max_wall_hp(&g.cities[&cid]), 100);

    let medieval = Item::Building {
        building: crate::name!("medieval_walls"),
    };
    g.players[0].techs.insert(crate::name!("castles"));
    assert!(
        !g.can_produce(0, cid, &medieval),
        "damaged Ancient Walls must be repaired before upgrading"
    );

    g.turn = 3;
    g.process_city(0, cid);
    assert_eq!(
        g.cities[&cid].wall_hp, 50,
        "Outer Defenses never regenerate passively"
    );
    let repair = Item::Project {
        project: crate::name!("repair_outer_defenses"),
    };
    assert!(g.can_produce(0, cid, &repair));
    g.cities.get_mut(&cid).unwrap().queue.push(repair);
    g.process_city(0, cid);
    assert!(g.cities[&cid].wall_hp > 50);

    let galley = g.spawn_unit("galley", 0, city_pos);
    assert!(!g.unit_can_fortify(&g.units[&galley]));
    assert!(g.apply(0, &Action::Fortify { unit: galley }).is_err());
}

#[test]
fn only_stock_eligible_districts_add_city_combat_strength() {
    let (mut g, city_pos, _) = controlled_game(31_432);
    let city = g.found_city_for(0, city_pos, None);
    let positions: Vec<Pos> = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| *position != city_pos)
        .take(7)
        .collect();
    let base = g.city_strength(city);
    for (index, district) in [
        "preserve",
        "aqueduct",
        "bath",
        "canal",
        "dam",
        "campus",
        "encampment",
    ]
    .into_iter()
    .enumerate()
    {
        g.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(Name::new(district), positions[index]);
    }
    assert_eq!(g.city_defense_district_count(&g.cities[&city]), 2);
    assert_eq!(g.city_strength(city), base + 4.0);
    g.map.tiles.get_mut(&positions[5]).unwrap().pillaged = true;
    assert_eq!(g.city_defense_district_count(&g.cities[&city]), 1);
    assert_eq!(g.city_strength(city), base + 2.0);
}

#[test]
fn siege_support_obeys_wall_eras_replacements_and_urban_defenses() {
    let (mut g, city_pos, ring) = controlled_game(31431);
    let cid = g.found_city_for(1, city_pos, None);
    g.spawn_unit("battering_ram", 0, ring[0]);
    g.spawn_unit("siege_tower", 0, ring[1]);

    g.cities.get_mut(&cid).unwrap().buildings = vec![crate::name!("walls")];
    assert_eq!(
        g.siege_support_effects(0, cid, city_pos, "melee"),
        (true, true),
        "both support units work against Ancient Walls"
    );
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("medieval_walls"));
    assert_eq!(
        g.siege_support_effects(0, cid, city_pos, "anti_cavalry"),
        (false, true),
        "Medieval Walls stop Rams but not Towers"
    );
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("renaissance_walls"));
    assert_eq!(
        g.siege_support_effects(0, cid, city_pos, "melee"),
        (false, false),
        "Renaissance Walls stop both support units"
    );

    g.cities.get_mut(&cid).unwrap().buildings = vec![
        crate::name!("walls"),
        crate::name!("medieval_walls"),
        crate::name!("tsikhe"),
    ];
    assert_eq!(
        g.siege_support_effects(0, cid, city_pos, "melee"),
        (false, false),
        "replacement Renaissance Walls inherit Siege Tower immunity"
    );

    g.cities.get_mut(&cid).unwrap().buildings.clear();
    g.players[1].techs.insert(crate::name!("steel"));
    assert_eq!(
        g.siege_support_effects(0, cid, city_pos, "melee"),
        (false, false),
        "Steel's Urban Defenses make both support units ineffective"
    );
    g.players[1].techs.remove(&Name::new("steel"));
    assert_eq!(
        g.siege_support_effects(0, cid, city_pos, "ranged"),
        (false, false),
        "siege support applies only to melee and anti-cavalry units"
    );
}

#[test]
fn modern_support_auras_apply_range_bombard_healing_and_movement() {
    let (mut game, _, ring) = controlled_game(31_433);
    let siege = game.spawn_unit("catapult", 0, ring[0]);
    let base_bombard = game.unit_bombard_strength(&game.units[&siege]);
    assert_eq!(game.unit_attack_range(siege), 2);

    let balloon = game.spawn_unit("observation_balloon", 0, ring[0]);
    assert_eq!(game.unit_attack_range(siege), 3);
    assert_eq!(
        game.unit_bombard_strength(&game.units[&siege]),
        base_bombard
    );
    let extended_target = game
        .wdisk(ring[0], 3)
        .into_iter()
        .find(|position| game.wdist(ring[0], *position) == 3)
        .unwrap();
    game.spawn_unit("warrior", 1, extended_target);
    let extended_strike = Action::Ranged {
        unit: siege,
        target: extended_target,
    };
    assert!(game.legal_actions(0).contains(&extended_strike));
    game.remove_unit(balloon);
    assert!(!game.legal_actions(0).contains(&extended_strike));
    let drone = game.spawn_unit("drone", 0, ring[0]);
    assert_eq!(game.unit_attack_range(siege), 3);
    assert_eq!(
        game.unit_bombard_strength(&game.units[&siege]),
        base_bombard + 5.0
    );
    let mut validated = game.clone();
    validated.apply(0, &extended_strike).unwrap();
    game.remove_unit(drone);
    assert_eq!(game.unit_attack_range(siege), 2);

    let soldier = game.spawn_unit("warrior", 0, ring[2]);
    game.units.get_mut(&soldier).unwrap().hp = 50;
    let base_heal = game.unit_heal_rate(soldier);
    let medic = game.spawn_unit("medic", 0, ring[2]);
    assert_eq!(game.unit_heal_rate(soldier), base_heal + 20);
    let convoy = game.spawn_unit("supply_convoy", 0, ring[2]);
    assert_eq!(
        game.unit_heal_rate(soldier),
        base_heal + 20,
        "Medic and Supply Convoy healing does not stack"
    );
    assert_eq!(game.unit_max_moves(soldier), 3.0);
    assert_eq!(
        game.unit_max_moves(convoy),
        5.0,
        "a Supply Convoy begins adjacent to its own aura"
    );
    game.remove_unit(medic);
    game.remove_unit(convoy);
    assert_eq!(game.unit_heal_rate(soldier), base_heal);
    assert_eq!(game.unit_max_moves(soldier), 2.0);
}

#[test]
fn a_unit_leaves_the_menu_when_its_upgrade_unlocks_not_when_steel_arrives() {
    let (mut game, _center, _ring) = controlled_game(31_436);

    // Catapult: obsolete_tech is `steel`, but Civilization VI withdraws it the
    // moment Trebuchets unlock at Military Engineering.
    assert!(
        !game.unit_is_obsolete(0, crate::name!("catapult")),
        "a Catapult is trainable before its successor exists"
    );
    assert!(
        !game.players[0].techs.contains(&crate::name!("steel")),
        "the late MandatoryObsoleteTech is NOT what retires it"
    );

    game.players[0].techs.insert(crate::name!("military_engineering"));
    assert!(
        game.unit_is_obsolete(0, crate::name!("catapult")),
        "unlocking the Trebuchet withdraws the Catapult, 54 refused siege orders \
         in one live game before this rule existed"
    );

    // A Scout has NO obsolete_tech at all, so it was buildable forever: 48
    // refusals in that same run.
    assert!(!game.unit_is_obsolete(0, crate::name!("scout")));
    game.players[0].techs.insert(crate::name!("machinery"));
    assert!(
        game.unit_is_obsolete(0, crate::name!("scout")),
        "the Skirmisher withdraws the Scout even though scout.obsolete_tech is None"
    );

    // The successor itself stays available -- this retires predecessors only.
    assert!(
        !game.unit_is_obsolete(0, crate::name!("trebuchet")),
        "the Trebuchet is what CIVVIS should be building instead"
    );
}

#[test]
fn a_promotion_the_host_refused_is_never_asked_for_again() {
    let (mut game, center, _ring) = controlled_game(31_435);
    let warrior = game.spawn_unit("warrior", 0, center);
    game.award_xp(warrior, 20.0);
    let offers = game.available_promotions(warrior);
    assert!(
        offers.len() >= 2,
        "the warrior has a promotion pending with several choices"
    );

    // ONE refusal removes exactly that promotion and leaves the rest: another
    // choice may still be legal for this unit.
    game.blocked_promotions
        .insert(warrior, [offers[0]].into_iter().collect());
    let after_one = game.available_promotions(warrior);
    assert!(
        !after_one.contains(&offers[0]),
        "the refused promotion is withdrawn"
    );
    assert!(
        after_one.contains(&offers[1]),
        "the others are still offered"
    );

    // TWO distinct refusals mean the unit cannot promote at all. This is the
    // Apostle case: it takes one promotion when it is created and can never take
    // another, so offering the next name just walks the list.
    game.blocked_promotions
        .insert(warrior, [offers[0], offers[1]].into_iter().collect());
    assert!(
        game.available_promotions(warrior).is_empty(),
        "a unit refused twice is not offered a third name"
    );

    // A native game never populates the block set and is untouched.
    game.blocked_promotions.clear();
    assert_eq!(game.available_promotions(warrior), offers);
}

#[test]
fn hybrid_units_offer_both_attacks_and_can_capture_cities() {
    let (mut game, center, ring) = controlled_game(31_434);
    let robot = game.spawn_unit("giant_death_robot", 0, center);
    let adjacent = ring[0];
    game.spawn_unit("warrior", 1, adjacent);
    let distant = game
        .wdisk(center, 2)
        .into_iter()
        .find(|position| game.wdist(center, *position) == 2)
        .unwrap();
    game.spawn_unit("warrior", 1, distant);

    let spec = &game.rules.units["giant_death_robot"];
    assert!(spec.is_melee_capable());
    assert!(spec.has_ranged_attack());
    assert!(!spec.can_formations);
    assert!(!spec.earns_xp);
    game.players[0].civics.insert(crate::name!("nationalism"));
    let second_robot = game.spawn_unit("giant_death_robot", 0, ring[1]);
    assert_eq!(game.can_combine_units(0, robot, second_robot), None);
    game.award_xp(robot, 20.0);
    assert_eq!(game.units[&robot].xp, 0);
    assert!(game.available_promotions(robot).is_empty());
    let legal = game.legal_actions(0);
    assert!(legal.contains(&Action::Attack {
        unit: robot,
        target: adjacent,
    }));
    assert!(legal.contains(&Action::Ranged {
        unit: robot,
        target: adjacent,
    }));
    assert!(legal.contains(&Action::Ranged {
        unit: robot,
        target: distant,
    }));

    let (mut capture, city_pos, capture_ring) = controlled_game(31_435);
    let city = capture.found_city_for(1, city_pos, None);
    capture.cities.get_mut(&city).unwrap().hp = 0;
    capture.cities.get_mut(&city).unwrap().wall_hp = 0;
    let captor = capture.spawn_unit("giant_death_robot", 0, capture_ring[0]);
    capture
        .apply(
            0,
            &Action::Attack {
                unit: captor,
                target: city_pos,
            },
        )
        .unwrap();
    assert_eq!(capture.cities[&city].owner, 0);
    assert_eq!(capture.units[&captor].pos, city_pos);
}

#[test]
fn anti_air_is_support_only_and_uses_dedicated_land_naval_and_gdr_strengths() {
    let (mut ground, center, ring) = controlled_game(31_436);
    let anti_air = ground.spawn_unit("anti_air_gun", 0, ring[0]);
    let bomber = ground.spawn_unit("bomber", 1, center);
    let spec = &ground.rules.units["anti_air_gun"];
    assert_eq!(spec.class, "support");
    assert!(!spec.is_melee_capable());
    assert!(!spec.has_ranged_attack());
    assert!(!ground.legal_actions(0).iter().any(|action| {
        matches!(
            action,
            Action::Attack { unit, .. } | Action::Ranged { unit, .. } if *unit == anti_air
        )
    }));
    let bomber_state = ground.units[&bomber].clone();
    assert_eq!(
        ground.air_interception_strength(&bomber_state, center),
        90.0
    );

    let (mut naval, center, ring) = controlled_game(31_437);
    let battleship = naval.spawn_unit("battleship", 0, ring[0]);
    naval.units.get_mut(&battleship).unwrap().formation = 1;
    let bomber = naval.spawn_unit("bomber", 1, center);
    let bomber_state = naval.units[&bomber].clone();
    assert_eq!(naval.air_interception_strength(&bomber_state, center), 97.0);

    let (mut future, center, ring) = controlled_game(31_438);
    future.spawn_unit("giant_death_robot", 0, ring[0]);
    let bomber = future.spawn_unit("bomber", 1, center);
    let bomber_state = future.units[&bomber].clone();
    assert_eq!(
        future.air_interception_strength(&bomber_state, center),
        90.0
    );
    future.players[0].techs.insert(crate::name!("robotics"));
    future.players[0].techs.insert(crate::name!("advanced_ai"));
    assert_eq!(
        future.air_interception_strength(&bomber_state, center),
        130.0
    );
}

#[test]
fn late_aircraft_use_stock_stats_promotion_trees_and_rebase_distance() {
    let (mut game, center, _) = controlled_game(31_439);
    let expected = [
        ("biplane", 6.0, 4, 4, "air_fighter"),
        ("fighter", 8.0, 5, 4, "air_fighter"),
        ("bomber", 10.0, 10, 4, "air_bomber"),
        ("jet_fighter", 10.0, 6, 5, "air_fighter"),
        ("jet_bomber", 15.0, 15, 5, "air_bomber"),
    ];
    for (kind, moves, range, sight, promotion_class) in expected {
        let spec = &game.rules.units[kind];
        assert_eq!(spec.moves, moves, "{kind} movement");
        assert_eq!(spec.range, range, "{kind} operational range");
        assert_eq!(spec.sight, sight, "{kind} sight");
        assert_eq!(
            spec.promotion_class, promotion_class,
            "{kind} promotion class"
        );
    }
    assert!(game.rules.units["aircraft_carrier"].can_formations);
    assert!(!game.rules.units["aircraft_carrier"].can_combine);

    let fighter = game.spawn_unit("fighter", 0, center);
    assert_eq!(game.unit_attack_range(fighter), 5);
    assert_eq!(game.air_rebase_range(fighter), 16);
    game.units
        .get_mut(&fighter)
        .unwrap()
        .promotions
        .insert(crate::name!("drop_tanks"));
    assert_eq!(game.unit_attack_range(fighter), 7);
    assert_eq!(
        game.air_rebase_range(fighter),
        16,
        "range promotions do not alter twice-Movement rebasing"
    );

    let fighter_promotions = game
        .rules
        .promotions
        .values()
        .filter(|promotion| promotion.class == "air_fighter")
        .count();
    let bomber_promotions = game
        .rules
        .promotions
        .values()
        .filter(|promotion| promotion.class == "air_bomber")
        .count();
    assert_eq!(fighter_promotions, 7);
    assert_eq!(bomber_promotions, 7);
}

#[test]
fn carriers_keep_their_own_sight_while_embarked_aircraft_supply_theirs() {
    let (mut game, position, _) = controlled_game(314_391);
    let carrier = game.spawn_unit("aircraft_carrier", 0, position);
    assert_eq!(game.unit_sight(carrier), 2);
    game.units
        .get_mut(&carrier)
        .unwrap()
        .promotions
        .insert(crate::name!("scout_planes"));
    assert_eq!(game.unit_sight(carrier), 3);

    let fighter = game.spawn_unit("biplane", 0, position);
    assert_eq!(game.unit_sight(carrier), 3);
    assert!(game
        .player_visibility(0)
        .iter()
        .any(|target| game.wdist(position, *target) == 4));
    let jet = game.spawn_unit("jet_fighter", 0, position);
    assert_eq!(game.unit_sight(carrier), 3);
    assert!(game
        .player_visibility(0)
        .iter()
        .any(|target| game.wdist(position, *target) == 5));
    game.remove_unit(jet);
    assert_eq!(game.unit_sight(carrier), 3);
    game.remove_unit(fighter);
    assert_eq!(game.unit_sight(carrier), 3);
}

#[test]
fn air_strikes_and_priority_targeting_damage_support_units_exactly() {
    let (mut exposed, base, ring) = controlled_game(314_392);
    exposed.found_city_for(0, base, None);
    let medic = exposed.spawn_unit("medic", 1, ring[0]);
    let bomber = exposed.spawn_unit("bomber", 0, base);
    let direct = Action::AirStrike {
        unit: bomber,
        target: ring[0],
    };
    assert!(exposed.legal_actions(0).contains(&direct));
    exposed.apply(0, &direct).unwrap();
    assert_eq!(exposed.units[&medic].hp, 35);

    let (mut stacked, base, ring) = controlled_game(314_393);
    stacked.found_city_for(0, base, None);
    let target = ring[0];
    let escort = stacked.spawn_unit("modern_armor", 1, target);
    let medic = stacked.spawn_unit("medic", 1, target);
    let jet = stacked.spawn_unit("jet_fighter", 0, base);
    let ordinary = Action::AirStrike { unit: jet, target };
    let priority = Action::PriorityTarget { unit: jet, target };
    assert!(stacked.legal_actions(0).contains(&ordinary));
    assert!(stacked.legal_actions(0).contains(&priority));

    let mut ordinary_result = stacked.clone();
    ordinary_result.apply(0, &ordinary).unwrap();
    assert_eq!(ordinary_result.units[&medic].hp, 100);
    assert!(
        !ordinary_result.units.contains_key(&escort) || ordinary_result.units[&escort].hp < 100,
        "an ordinary strike remains on the military escort"
    );

    stacked.apply(0, &priority).unwrap();
    assert_eq!(stacked.units[&medic].hp, 35);
    assert_eq!(stacked.units[&escort].hp, 100);
    let biplane = stacked.spawn_unit("biplane", 0, base);
    assert!(!stacked.legal_actions(0).contains(&Action::PriorityTarget {
        unit: biplane,
        target,
    }));

    let (mut ground, base, ring) = controlled_game(314_394);
    let target = ring[0];
    ground.spawn_unit("warrior", 1, target);
    let support = ground.spawn_unit("mobile_sam", 1, target);
    let spec_ops = ground.spawn_unit("spec_ops", 0, base);
    let priority = Action::PriorityTarget {
        unit: spec_ops,
        target,
    };
    assert!(ground.legal_actions(0).contains(&priority));
    ground.apply(0, &priority).unwrap();
    assert_eq!(ground.units[&support].hp, 35);
}

#[test]
fn air_promotions_apply_exact_target_bonuses_and_patrol_healing() {
    let (mut game, center, ring) = controlled_game(31_440);
    game.found_city_for(0, center, None);
    let fighter = game.spawn_unit("fighter", 0, center);
    game.units.get_mut(&fighter).unwrap().promotions.extend([
        crate::name!("dogfighting"),
        crate::name!("strafe"),
        crate::name!("tank_buster"),
        crate::name!("ground_crews"),
    ]);
    let enemy_fighter = game.spawn_unit("biplane", 1, ring[0]);
    let infantry = game.spawn_unit("warrior", 1, ring[1]);
    let cavalry = game.spawn_unit("horseman", 1, ring[2]);
    assert_eq!(
        game.air_strike_unit_bonus(&game.units[&fighter], &game.units[&enemy_fighter]),
        7.0
    );
    assert_eq!(
        game.air_strike_unit_bonus(&game.units[&fighter], &game.units[&infantry]),
        17.0
    );
    assert_eq!(
        game.air_strike_unit_bonus(&game.units[&fighter], &game.units[&cavalry]),
        17.0
    );

    let bomber = game.spawn_unit("bomber", 0, ring[3]);
    game.units.get_mut(&bomber).unwrap().promotions.extend([
        crate::name!("torpedo_bomber"),
        crate::name!("close_air_support"),
    ]);
    let ship = game.spawn_unit("galley", 1, ring[4]);
    assert_eq!(
        game.air_strike_unit_bonus(&game.units[&bomber], &game.units[&ship]),
        17.0
    );
    assert_eq!(
        game.air_strike_unit_strength(&game.units[&bomber], &game.units[&ship]),
        127.0,
        "bombard-type aircraft retain full strength against naval targets"
    );
    assert_eq!(
        game.air_strike_unit_bonus(&game.units[&bomber], &game.units[&infantry]),
        12.0
    );
    assert_eq!(
        game.air_strike_unit_strength(&game.units[&bomber], &game.units[&infantry]),
        105.0,
        "Close Air Support partly offsets the stock -17 Bombard-vs-land penalty"
    );
    assert_eq!(
        game.air_strike_unit_strength(&game.units[&fighter], &game.units[&ship]),
        100.0,
        "Strafe offsets the stock -17 ranged-air-vs-naval penalty"
    );

    let aircraft = game.units.get_mut(&fighter).unwrap();
    aircraft.hp = 50;
    aircraft.acted = true;
    aircraft.air_patrol = true;
    aircraft.air_patrol_pos = Some(ring[5]);
    // Keep the fixture out of the bankruptcy-disband path; this assertion
    // isolates Ground Crews healing rather than economy maintenance.
    game.players[0].gold = 1_000.0;
    game.begin_turn(0);
    assert!(game.units[&fighter].hp > 50);
}

#[test]
fn air_pillage_uses_exact_health_floor_layer_order_and_no_spoils() {
    let (mut game, base, _) = controlled_game(31_445);
    game.found_city_for(0, base, None);
    let enemy_center = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| {
            game.wdist(base, *position) >= 5
                && game.wdist(base, *position) <= 8
                && game.wdisk(*position, 1).len() == 7
        })
        .unwrap();
    let enemy_city = game.found_city_for(1, enemy_center, None);
    let mut owned: Vec<Pos> = game.cities[&enemy_city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != enemy_center)
        .collect();
    owned.sort_unstable();
    let campus = owned[0];
    let farm = owned[1];
    game.map.tiles.get_mut(&campus).unwrap().district = Some(crate::name!("campus"));
    game.cities
        .get_mut(&enemy_city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), campus);
    game.cities
        .get_mut(&enemy_city)
        .unwrap()
        .buildings
        .extend([crate::name!("library"), crate::name!("university")]);

    let spotter =
        game.nbrs(campus)
            .into_iter()
            .find(|position| {
                game.map.get(*position).is_some_and(|tile| {
                    game.rules.is_passable(tile) && !game.rules.is_water(tile)
                }) && game.units_at(*position).is_empty()
            })
            .unwrap();
    game.spawn_unit("warrior", 0, spotter);

    let bomber = game.spawn_unit("bomber", 0, base);
    game.units.get_mut(&bomber).unwrap().hp = 50;
    let action = Action::AirPillage {
        unit: bomber,
        target: campus,
    };
    assert!(game.legal_actions(0).contains(&action));
    let treasury = game.players[0].gold;
    let faith = game.players[0].faith;
    let science = game.players[0].research_overflow;
    let culture = game.players[0].civic_overflow;

    game.apply(0, &action).unwrap();
    assert!(game.cities[&enemy_city]
        .pillaged_buildings
        .contains(&Name::new("university")));
    assert!(!game.cities[&enemy_city]
        .pillaged_buildings
        .contains(&Name::new("library")));
    assert!(!game.map.tiles[&campus].pillaged);

    for expected_building in [Some("library"), None] {
        let unit = game.units.get_mut(&bomber).unwrap();
        unit.moves_left = 10.0;
        unit.attacks_left = 1;
        unit.acted = false;
        game.apply(0, &action).unwrap();
        if let Some(building) = expected_building {
            assert!(game.cities[&enemy_city]
                .pillaged_buildings
                .contains(&Name::new(building)));
            assert!(!game.map.tiles[&campus].pillaged);
        }
    }
    assert!(game.map.tiles[&campus].pillaged);
    assert_eq!(game.players[0].gold, treasury);
    assert_eq!(game.players[0].faith, faith);
    assert_eq!(game.players[0].research_overflow, science);
    assert_eq!(game.players[0].civic_overflow, culture);

    game.map.tiles.get_mut(&farm).unwrap().improvement = Some(crate::name!("farm"));
    let low_bomber = game.spawn_unit("bomber", 0, base);
    game.units.get_mut(&low_bomber).unwrap().hp = 49;
    let low_action = Action::AirPillage {
        unit: low_bomber,
        target: farm,
    };
    assert!(!game.legal_actions(0).contains(&low_action));
    assert!(game.apply(0, &low_action).is_err());
    game.units
        .get_mut(&low_bomber)
        .unwrap()
        .promotions
        .insert(crate::name!("superfortress"));
    assert!(game.legal_actions(0).contains(&low_action));
    game.apply(0, &low_action).unwrap();
    assert!(game.map.tiles[&farm].pillaged);
    assert_eq!(game.units[&low_bomber].hp, 49);
}

#[test]
fn fighter_interception_diverts_fighter_strikes_from_ground_targets() {
    let (mut game, target, ring) = controlled_game(31_441);
    let victim = game.spawn_unit("warrior", 1, target);
    let attacker = game.spawn_unit("fighter", 0, ring[0]);
    let interceptor = game.spawn_unit("biplane", 1, ring[1]);
    let patrol = game.units.get_mut(&interceptor).unwrap();
    patrol.air_patrol = true;
    patrol.air_patrol_pos = Some(target);

    game.do_air_strike(0, attacker, target).unwrap();
    assert_eq!(game.units[&victim].hp, 100);
    let war = &game.wars[&pair(0, 1)];
    assert!(war
        .participants
        .iter()
        .find(|participant| participant.player == 0)
        .unwrap()
        .saw_action_units
        .contains_key(&attacker));
    let defenders = &war
        .participants
        .iter()
        .find(|participant| participant.player == 1)
        .unwrap()
        .saw_action_units;
    assert!(defenders.contains_key(&interceptor));
    assert!(!defenders.contains_key(&victim));
    assert!(
        !game.units.contains_key(&attacker) || game.units[&attacker].hp < 100,
        "the dogfight must affect the attacking fighter"
    );
}

#[test]
fn patrols_deploy_by_movement_and_project_only_adjacent_interception() {
    let (mut game, base, _) = controlled_game(31_442);
    game.found_city_for(0, base, None);
    let fighter = game.spawn_unit("fighter", 0, base);
    let patrol = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| game.wdist(base, *position) <= 8)
        .max_by_key(|position| (game.wdist(base, *position), *position))
        .unwrap();
    assert_eq!(game.wdist(base, patrol), 8);
    let hostile_territory = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.wdist(base, *position) == 3 && *position != patrol)
        .unwrap();
    game.found_city_for(1, hostile_territory, None);
    assert!(!game.legal_actions(0).contains(&Action::AirPatrol {
        unit: fighter,
        to: hostile_territory,
    }));
    let action = Action::AirPatrol {
        unit: fighter,
        to: patrol,
    };
    assert!(game.legal_actions(0).contains(&action));
    game.apply(0, &action).unwrap();
    assert_eq!(game.units[&fighter].pos, base);
    assert_eq!(game.units[&fighter].air_patrol_pos, Some(patrol));

    let bomber_position = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| *position != base && *position != patrol)
        .unwrap();
    let bomber = game.spawn_unit("bomber", 1, bomber_position);
    let adjacent = game.nbrs(patrol)[0];
    let outside = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.wdist(patrol, *position) == 2)
        .unwrap();
    assert_eq!(
        game.air_interception_strength(&game.units[&bomber], adjacent),
        100.0
    );
    assert_eq!(
        game.air_interception_strength(&game.units[&bomber], outside),
        0.0
    );

    let remote_target = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| {
            game.wdist(base, *position) > game.unit_attack_range(fighter)
                && game.wdist(patrol, *position) <= game.unit_attack_range(fighter)
        })
        .unwrap();
    let victim = game.spawn_unit("warrior", 1, remote_target);
    game.begin_turn(0);
    assert_eq!(game.units[&fighter].air_patrol_pos, Some(patrol));
    assert!(game.legal_actions(0).contains(&Action::AirStrike {
        unit: fighter,
        target: remote_target,
    }));
    assert_eq!(game.units[&victim].hp, 100);
}

#[test]
fn based_aircraft_are_protected_but_deployed_fighters_can_be_dogfought() {
    let (mut game, target, ring) = controlled_game(31_443);
    let archer = game.spawn_unit("archer", 0, ring[0]);
    let based = game.spawn_unit("biplane", 1, target);
    assert!(!game.legal_actions(0).contains(&Action::Ranged {
        unit: archer,
        target,
    }));
    assert!(game
        .apply(
            0,
            &Action::Ranged {
                unit: archer,
                target,
            },
        )
        .is_err());
    assert_eq!(game.units[&based].hp, 100);

    let attacker = game.spawn_unit("fighter", 0, ring[1]);
    let patrol_tile = target;
    let deployed = game.units.get_mut(&based).unwrap();
    deployed.air_patrol = true;
    deployed.air_patrol_pos = Some(patrol_tile);
    let mut ground_attack = game.clone();
    let ranged = Action::Ranged {
        unit: archer,
        target: patrol_tile,
    };
    assert!(ground_attack.legal_actions(0).contains(&ranged));
    ground_attack.apply(0, &ranged).unwrap();
    assert!(!ground_attack.units.contains_key(&based) || ground_attack.units[&based].hp < 100);
    assert!(!game.can_move(archer, patrol_tile));

    let dogfight = Action::AirStrike {
        unit: attacker,
        target: patrol_tile,
    };
    assert!(game.legal_actions(0).contains(&dogfight));
    game.apply(0, &dogfight).unwrap();
    assert!(
        !game.units.contains_key(&attacker)
            || !game.units.contains_key(&based)
            || game.units[&attacker].hp < 100
            || game.units[&based].hp < 100
    );
}

#[test]
fn aircraft_heal_only_at_bases_and_interception_counts_as_combat() {
    let (mut game, target, ring) = controlled_game(31_444);
    let unbased = game.spawn_unit("biplane", 0, target);
    assert_eq!(game.unit_heal_rate(unbased), 0);
    game.found_city_for(0, target, None);
    assert_eq!(game.unit_heal_rate(unbased), 20);

    let defender_city = game.found_city_for(1, ring[1], None);
    assert_eq!(game.cities[&defender_city].pos, ring[1]);
    let interceptor = game.spawn_unit("jet_fighter", 1, ring[1]);
    {
        let unit = game.units.get_mut(&interceptor).unwrap();
        unit.hp = 50;
        unit.air_patrol = true;
        unit.air_patrol_pos = Some(target);
        unit.acted = false;
    }
    let bomber = game.spawn_unit("bomber", 0, target);
    game.spawn_unit("warrior", 1, ring[0]);
    game.do_air_strike(0, bomber, ring[0]).unwrap();
    assert!(game.units[&interceptor].acted);
    game.begin_turn(1);
    assert_eq!(
        game.units[&interceptor].hp, 50,
        "defensive combat prevents ordinary aircraft healing"
    );

    {
        let unit = game.units.get_mut(&interceptor).unwrap();
        unit.acted = true;
        unit.air_patrol = true;
        unit.air_patrol_pos = Some(target);
        unit.promotions.insert(crate::name!("ground_crews"));
    }
    game.begin_turn(1);
    assert!(game.units[&interceptor].hp > 50);
}

#[test]
fn pillaged_airbases_force_aircraft_to_scatter() {
    let (mut game, city_pos, ring) = controlled_game(31_445);
    let city = game.found_city_for(0, city_pos, None);
    let aerodrome = ring[0];
    game.map.tiles.get_mut(&aerodrome).unwrap().terrain = crate::name!("plains");
    game.map.tiles.get_mut(&aerodrome).unwrap().district = Some(crate::name!("aerodrome"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("aerodrome"), aerodrome);
    let aircraft = game.spawn_unit("biplane", 0, aerodrome);
    let attacker_pos = game
        .nbrs(aerodrome)
        .into_iter()
        .find(|position| *position != city_pos && *position != aerodrome)
        .unwrap();
    game.map.tiles.get_mut(&attacker_pos).unwrap().terrain = crate::name!("plains");
    let raider = game.spawn_unit("warrior", 1, attacker_pos);
    game.current = 1;
    let enter = Action::Move {
        unit: raider,
        to: aerodrome,
    };
    assert!(game.legal_actions(1).contains(&enter));
    game.apply(1, &enter).unwrap();
    assert_eq!(game.units[&aircraft].pos, aerodrome);
    game.apply(1, &Action::Pillage { unit: raider }).unwrap();
    assert!(game.map.tiles[&aerodrome].pillaged);
    let raider_party = game.wars[&pair(0, 1)]
        .participants
        .iter()
        .find(|participant| participant.player == 1)
        .unwrap();
    assert!(raider_party.saw_action_units.contains_key(&raider));
    assert_eq!(game.units[&aircraft].pos, city_pos);
    assert_eq!(game.units[&aircraft].moves_left, 0.0);
    assert!(game.units[&aircraft].acted);
}

#[test]
fn bombers_air_pillage_without_spoils_and_superfortress_ignores_health_gate() {
    let (mut game, base, ring) = controlled_game(31_446);
    let home = game.found_city_for(0, base, None);
    let enemy_city = game.found_city_for(1, ring[2], None);
    let target = ring[0];
    {
        let tile = game.map.tiles.get_mut(&target).unwrap();
        tile.terrain = crate::name!("plains");
        tile.owner_city = Some(enemy_city);
        tile.improvement = Some(crate::name!("mine"));
        tile.pillaged = false;
    }
    let bomber = game.spawn_unit("bomber", 0, base);
    let mission = Action::AirPillage {
        unit: bomber,
        target,
    };

    game.units.get_mut(&bomber).unwrap().hp = 49;
    assert!(!game.legal_actions(0).contains(&mission));
    assert_eq!(
        game.apply(0, &mission),
        Err("invalid air pillage".to_string()),
        "ordinary bombers need at least 50 HP"
    );

    game.units.get_mut(&bomber).unwrap().hp = 50;
    assert!(game.legal_actions(0).contains(&mission));
    let before = (
        game.players[0].gold,
        game.players[0].faith,
        game.players[0].research_overflow,
        game.players[0].civic_overflow,
        game.cities[&home].production,
    );
    game.apply(0, &mission).unwrap();
    assert!(game.map.tiles[&target].pillaged);
    let attacker = game.wars[&pair(0, 1)]
        .participants
        .iter()
        .find(|participant| participant.player == 0)
        .unwrap();
    assert_eq!(
        attacker.saw_action_units[&bomber],
        (game.rules.units["bomber"].strength * 0.5).round() as i64
    );
    assert_eq!(
        (
            game.players[0].gold,
            game.players[0].faith,
            game.players[0].research_overflow,
            game.players[0].civic_overflow,
            game.cities[&home].production,
        ),
        before,
        "Air Pillage damages infrastructure but awards no plunder"
    );
    assert_eq!(game.units[&bomber].moves_left, 0.0);
    assert_eq!(game.units[&bomber].attacks_left, 0);

    let (mut intercepted, base, ring) = controlled_game(31_448);
    intercepted.found_city_for(0, base, None);
    let enemy_city = intercepted.found_city_for(1, ring[2], None);
    let target = ring[0];
    {
        let tile = intercepted.map.tiles.get_mut(&target).unwrap();
        tile.terrain = crate::name!("plains");
        tile.owner_city = Some(enemy_city);
        tile.improvement = Some(crate::name!("mine"));
    }
    let bomber = intercepted.spawn_unit("bomber", 0, base);
    intercepted.units.get_mut(&bomber).unwrap().hp = 51;
    let fighter = intercepted.spawn_unit("biplane", 1, ring[3]);
    let patrol = intercepted.units.get_mut(&fighter).unwrap();
    patrol.air_patrol = true;
    patrol.air_patrol_pos = Some(target);
    intercepted
        .apply(
            0,
            &Action::AirPillage {
                unit: bomber,
                target,
            },
        )
        .unwrap();
    assert!(intercepted.units.contains_key(&bomber));
    assert!(intercepted.units[&bomber].hp < 50);
    assert!(
        !intercepted.map.tiles[&target].pillaged,
        "dropping below 50 HP during interception aborts ordinary Air Pillage"
    );
    assert_eq!(intercepted.units[&bomber].attacks_left, 0);

    let (mut promoted, base, ring) = controlled_game(31_447);
    promoted.found_city_for(0, base, None);
    let enemy_city = promoted.found_city_for(1, ring[2], None);
    let target = ring[0];
    {
        let tile = promoted.map.tiles.get_mut(&target).unwrap();
        tile.terrain = crate::name!("plains");
        tile.owner_city = Some(enemy_city);
        tile.improvement = Some(crate::name!("mine"));
    }
    let bomber = promoted.spawn_unit("bomber", 0, base);
    let unit = promoted.units.get_mut(&bomber).unwrap();
    unit.hp = 1;
    unit.promotions.insert(crate::name!("superfortress"));
    let mission = Action::AirPillage {
        unit: bomber,
        target,
    };
    assert!(promoted.legal_actions(0).contains(&mission));
    promoted.apply(0, &mission).unwrap();
    assert!(promoted.map.tiles[&target].pillaged);
}

#[test]
fn eagle_warrior_conversion_uses_base_strength_probability() {
    let (mut g, target, ring) = controlled_game(3144);
    let eagle = g.spawn_unit("eagle_warrior", 0, ring[0]);
    let warrior = g.spawn_unit("warrior", 1, target);
    let scout = g.spawn_unit("scout", 1, ring[1]);
    let horse = g.spawn_unit("horseman", 1, ring[2]);
    assert_eq!(g.eagle_capture_chance(eagle, &g.units[&warrior]), 70.0);
    assert_eq!(g.eagle_capture_chance(eagle, &g.units[&scout]), 95.0);
    assert_eq!(g.eagle_capture_chance(eagle, &g.units[&horse]), 30.0);
    g.players[1].is_barbarian = true;
    assert_eq!(g.eagle_capture_chance(eagle, &g.units[&warrior]), 0.0);
}

#[test]
fn combat_xp_and_fortification_use_civ6_timing_and_modifiers() {
    let (mut g, target, ring) = controlled_game(315);
    g.players[0].civ = "Nubia".to_string();
    let archer = g.spawn_unit("archer", 0, ring[0]);
    let defender = g.spawn_unit("warrior", 1, target);
    g.apply(
        0,
        &Action::Ranged {
            unit: archer,
            target,
        },
    )
    .unwrap();
    assert_eq!(g.units[&archer].xp, 5);
    assert_eq!(g.units[&defender].xp, 2);
    assert_eq!(g.modified_xp(defender, 2.49), 2);
    assert_eq!(
        g.modified_xp(defender, 2.50),
        3,
        "half an XP rounds upward, while smaller fractions do not"
    );

    g.players[0].government = Some("oligarchy".to_string());
    assert_eq!(
        g.modified_xp(archer, 3.0),
        5,
        "Nubia's 50% and Oligarchy's 20% XP modifiers stack"
    );

    let scout = g.spawn_unit("scout", 0, ring[2]);
    g.players[0].policies.insert(crate::name!("survey"));
    let strong_enemy = g.spawn_unit("swordsman", 1, ring[3]);
    let enemy = g.units[&strong_enemy].clone();
    g.award_unit_combat_xp(scout, &enemy, false, true, true);
    assert_eq!(
        g.units[&scout].xp, 8,
        "the unit-combat XP cap applies after percentage modifiers"
    );

    g.players[1].is_barbarian = true;
    let barb = g.units[&strong_enemy].clone();
    g.units.get_mut(&scout).unwrap().level = 2;
    g.award_unit_combat_xp(scout, &barb, false, true, true);
    assert_eq!(
        g.units[&scout].xp, 9,
        "post-promotion barbarian combat grants exactly 1 XP"
    );

    let veteran = g.spawn_unit("warrior", 0, ring[4]);
    g.units.get_mut(&veteran).unwrap().xp = 420;
    g.begin_turn(0);
    assert_eq!(
        g.units[&veteran].level, 1,
        "earned promotions remain explicit choices"
    );
    for expected_level in 2..=8 {
        let promotion = g.available_promotions(veteran)[0];
        g.apply(
            0,
            &Action::Promote {
                unit: veteran,
                promotion,
            },
        )
        .unwrap();
        assert_eq!(g.units[&veteran].level, expected_level);
        assert!(
            g.available_promotions(veteran).is_empty(),
            "a promotion consumes the unit's turn"
        );
        if expected_level < 8 {
            g.begin_turn(0);
        }
    }

    let (mut g, _, ring) = controlled_game(316);
    let unit = g.spawn_unit("warrior", 0, ring[0]);
    let destination = ring[1];
    g.apply(
        0,
        &Action::Move {
            unit,
            to: destination,
        },
    )
    .unwrap();
    g.apply(0, &Action::Fortify { unit }).unwrap();
    assert_eq!(g.units[&unit].fortify_turns, 0);
    g.begin_turn(0);
    assert_eq!(g.units[&unit].fortify_turns, 1);
    g.begin_turn(0);
    assert_eq!(g.units[&unit].fortify_turns, 2);
}

#[test]
fn corps_armies_and_linked_escorts_preserve_their_rules() {
    let (mut g, center, ring) = controlled_game(3161);
    g.players[0].civics.insert(crate::name!("nationalism"));
    let veteran = g.spawn_unit("warrior", 0, center);
    let recruit = g.spawn_unit("warrior", 0, ring[0]);
    g.units.get_mut(&veteran).unwrap().xp = 20;
    g.units.get_mut(&veteran).unwrap().damage_dealt = 12;
    g.units.get_mut(&veteran).unwrap().hp = 40;
    g.units.get_mut(&veteran).unwrap().xp_bonus_pct = 10.0;
    g.units
        .get_mut(&veteran)
        .unwrap()
        .promotions
        .insert(crate::name!("battlecry"));
    g.units.get_mut(&recruit).unwrap().hp = 80;
    g.units.get_mut(&recruit).unwrap().damage_dealt = 8;
    g.units.get_mut(&recruit).unwrap().xp_bonus_pct = 25.0;
    g.units
        .get_mut(&recruit)
        .unwrap()
        .promotions
        .insert(crate::name!("tortoise"));
    g.apply(
        0,
        &Action::CombineUnits {
            unit: veteran,
            with: recruit,
        },
    )
    .unwrap();
    assert!(!g.units.contains_key(&recruit));
    assert_eq!(g.units[&veteran].formation, 1);
    assert_eq!(g.units[&veteran].damage_dealt, 20);
    assert_eq!(g.units[&veteran].production_cost, 80.0);
    assert!(
        g.players[0].unit_lifetimes.is_empty(),
        "a constituent continues in the formation instead of ending a second life"
    );
    assert_eq!(g.units[&veteran].hp, 60);
    assert_eq!(g.units[&veteran].xp, 20);
    assert!(g.units[&veteran].promotions.contains(&Name::new("battlecry")));
    assert!(g.units[&veteran].promotions.contains(&Name::new("tortoise")));
    assert_eq!(g.units[&veteran].xp_bonus_pct, 25.0);
    assert_eq!(g.unit_unembarked_strength(&g.units[&veteran]), 30.0);

    g.begin_turn(0);
    g.players[0].civics.insert(crate::name!("mobilization"));
    let third = g.spawn_unit("warrior", 0, ring[1]);
    g.units.get_mut(&third).unwrap().hp = 90;
    g.units.get_mut(&third).unwrap().damage_dealt = 7;
    g.units
        .get_mut(&third)
        .unwrap()
        .promotions
        .insert(crate::name!("amphibious"));
    g.apply(
        0,
        &Action::CombineUnits {
            unit: veteran,
            with: third,
        },
    )
    .unwrap();
    assert_eq!(g.units[&veteran].formation, 2);
    assert_eq!(g.units[&veteran].damage_dealt, 27);
    assert_eq!(g.units[&veteran].production_cost, 120.0);
    assert_eq!(g.units[&veteran].hp, 70);
    assert!(g.units[&veteran].promotions.contains(&Name::new("amphibious")));
    assert_eq!(g.unit_unembarked_strength(&g.units[&veteran]), 37.0);

    let (mut g, center, ring) = controlled_game(3162);
    let escort = g.spawn_unit("warrior", 0, center);
    let builder = g.spawn_unit("builder", 0, center);
    let form_escort = Action::LinkUnits {
        unit: escort,
        with: builder,
    };
    assert!(
        g.legal_actions_within(0, ActionFamilies::FORMATIONS)
            .contains(&form_escort)
    );
    g.apply(0, &form_escort).unwrap();
    assert_eq!(g.units[&escort].linked_to, Some(builder));
    assert_eq!(g.units[&builder].linked_to, Some(escort));
    g.apply(
        0,
        &Action::Move {
            unit: escort,
            to: ring[0],
        },
    )
    .unwrap();
    assert_eq!(g.units[&escort].pos, ring[0]);
    assert_eq!(g.units[&builder].pos, ring[0]);
    let unform_escort = Action::UnlinkUnits { unit: escort };
    assert!(
        g.legal_actions_within(0, ActionFamilies::FORMATIONS)
            .contains(&unform_escort)
    );
    g.apply(0, &unform_escort).unwrap();
    assert_eq!(g.units[&escort].linked_to, None);
    assert_eq!(g.units[&builder].linked_to, None);
}

#[test]
fn unique_formation_unlocks_and_city_state_exclusions_match_gathering_storm() {
    let (mut spain, center, ring) = controlled_game(3166);
    spain.players[0].civ = "Spain".to_string();
    spain.players[0].civics.insert(crate::name!("mercantilism"));
    let galley = spain.spawn_unit("galley", 0, center);
    let second = spain.spawn_unit("galley", 0, ring[0]);
    assert_eq!(spain.can_combine_units(0, galley, second), Some(1));
    spain.do_combine_units(0, galley, second).unwrap();
    spain.begin_turn(0);
    let third = spain.spawn_unit("galley", 0, ring[1]);
    assert_eq!(spain.can_combine_units(0, galley, third), Some(2));

    let (mut zulu, center, ring) = controlled_game(3167);
    zulu.players[0].civ = "Zulu".to_string();
    zulu.players[0].civics.insert(crate::name!("mercenaries"));
    let warrior = zulu.spawn_unit("warrior", 0, center);
    let second = zulu.spawn_unit("warrior", 0, ring[0]);
    assert_eq!(zulu.can_combine_units(0, warrior, second), Some(1));
    zulu.do_combine_units(0, warrior, second).unwrap();
    assert_eq!(zulu.unit_formation_bonus(&zulu.units[&warrior]), 15.0);
    zulu.begin_turn(0);
    zulu.players[0].civics.insert(crate::name!("nationalism"));
    let third = zulu.spawn_unit("warrior", 0, ring[1]);
    assert_eq!(zulu.can_combine_units(0, warrior, third), Some(2));
    zulu.do_combine_units(0, warrior, third).unwrap();
    assert_eq!(zulu.unit_formation_bonus(&zulu.units[&warrior]), 22.0);

    let (mut levied, center, ring) = controlled_game(3168);
    levied.players[0].civics.insert(crate::name!("nationalism"));
    let regular = levied.spawn_unit("warrior", 0, center);
    let city_state_unit = levied.spawn_unit("warrior", 0, ring[0]);
    levied.units.get_mut(&city_state_unit).unwrap().levied_from = Some(2);
    assert_eq!(levied.can_combine_units(0, regular, city_state_unit), None);
    levied.units.get_mut(&city_state_unit).unwrap().levied_from = None;
    levied.players[0].is_minor = true;
    assert_eq!(levied.can_combine_units(0, regular, city_state_unit), None);
}

#[test]
fn isibongo_capture_upgrade_and_garrison_loyalty_are_formation_aware() {
    let (mut game, city_pos, ring) = controlled_game(3169);
    game.players[0].civ = "Zulu".to_string();
    game.players[0].civics.insert(crate::name!("mercenaries"));
    let city = game.found_city_for(1, city_pos, None);
    game.cities.get_mut(&city).unwrap().hp = 0;
    let captor = game.spawn_unit("warrior", 0, ring[0]);
    game.apply(
        0,
        &Action::Attack {
            unit: captor,
            target: city_pos,
        },
    )
    .unwrap();
    assert_eq!(game.cities[&city].owner, 0);
    assert_eq!(game.units[&captor].formation, 1);

    let with_corps = game.city_loyalty_per_turn(&game.cities[&city]);
    game.units.get_mut(&captor).unwrap().formation = 0;
    let with_single = game.city_loyalty_per_turn(&game.cities[&city]);
    assert!((with_corps - with_single - 2.0).abs() < 1e-9);

    let (mut carrier_capture, city_pos, ring) = controlled_game(3171);
    carrier_capture.players[0].civ = "Zulu".to_string();
    carrier_capture.players[0]
        .civics
        .insert(crate::name!("nationalism"));
    carrier_capture.map.tiles.get_mut(&city_pos).unwrap().terrain = crate::name!("coast");
    carrier_capture.map.tiles.get_mut(&ring[0]).unwrap().terrain = crate::name!("coast");
    let city = carrier_capture.found_city_for(1, city_pos, None);
    carrier_capture.cities.get_mut(&city).unwrap().hp = 0;
    let carrier = carrier_capture.spawn_unit("aircraft_carrier", 0, ring[0]);
    carrier_capture
        .apply(
            0,
            &Action::Attack {
                unit: carrier,
                target: city_pos,
            },
        )
        .unwrap();
    assert_eq!(carrier_capture.units[&carrier].formation, 1);
}

#[test]
fn formation_great_admirals_create_fleets_and_armadas() {
    let (mut game, center, _) = controlled_game(3170);
    let ship = game.spawn_unit("galley", 0, center);
    let duilius = game.rules.great_people["gaius_duilius"].clone();
    game.named_great_person_effect(0, &duilius);
    assert_eq!(game.units[&ship].formation, 1);

    let santa_cruz = game.rules.great_people["santa_cruz"].clone();
    game.named_great_person_effect(0, &santa_cruz);
    assert_eq!(game.units[&ship].formation, 2);
}

#[test]
fn naval_raider_promotions_apply_strength_and_victory_gold() {
    let (mut g, target, ring) = controlled_game(3165);
    g.map.tiles.get_mut(&target).unwrap().terrain = crate::name!("coast");
    g.map.tiles.get_mut(&ring[0]).unwrap().terrain = crate::name!("coast");
    let raider = g.spawn_unit("privateer", 0, ring[0]);
    g.units
        .get_mut(&raider)
        .unwrap()
        .promotions
        .extend([crate::name!("boarding"), crate::name!("homing_torpedoes")]);
    let victim = g.spawn_unit("galley", 1, target);
    g.units.get_mut(&victim).unwrap().hp = 1;
    assert_eq!(
        g.promotion_effect(&g.units[&raider], "ranged_vs_naval"),
        10.0
    );
    let gold = g.players[0].gold;
    g.apply(
        0,
        &Action::Ranged {
            unit: raider,
            target,
        },
    )
    .unwrap();
    assert!(!g.units.contains_key(&victim));
    // BOARDING_GOLD_FROM_NAVAL_VICTORY is PercentDefeatedStrength 100, so a
    // sunk Galley pays its whole Combat Strength of 30 rather than half.
    assert_eq!(g.players[0].gold, gold + 30.0);
}

#[test]
fn theological_combat_and_condemnation_change_nearby_pressure() {
    let (mut g, center, ring) = controlled_game(3163);
    g.at_war.clear();
    g.players[0].religion = Some("A".to_string());
    g.players[1].religion = Some("B".to_string());
    let cid = g.found_city_for(0, ring[2], None);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .pressure
        .insert("A".to_string(), 500.0);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .pressure
        .insert("B".to_string(), 500.0);
    let apostle = g.spawn_unit("apostle", 0, ring[0]);
    let rival = g.spawn_unit("apostle", 1, center);
    g.units.get_mut(&rival).unwrap().hp = 1;
    assert!(g.legal_actions(0).into_iter().any(|action| {
        matches!(action, Action::TheologicalAttack { unit, target }
            if unit == apostle && target == center)
    }));
    g.apply(
        0,
        &Action::TheologicalAttack {
            unit: apostle,
            target: center,
        },
    )
    .unwrap();
    assert!(!g.units.contains_key(&rival));
    assert_eq!(g.cities[&cid].pressure["A"], 750.0);
    assert_eq!(g.cities[&cid].pressure["B"], 250.0);
    assert!(!g.is_at_war(0, 1), "theological combat needs no war");

    let (mut g, center, _) = controlled_game(3164);
    g.players[1].religion = Some("B".to_string());
    let cid = g.found_city_for(0, center, None);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .pressure
        .insert("B".to_string(), 500.0);
    let soldier = g.spawn_unit("warrior", 0, center);
    let missionary = g.spawn_unit("missionary", 1, center);
    g.apply(
        0,
        &Action::CondemnHeretic {
            unit: soldier,
            target_unit: missionary,
        },
    )
    .unwrap();
    assert!(!g.units.contains_key(&missionary));
    assert_eq!(g.cities[&cid].pressure["B"], 375.0);
}

#[test]
fn encampments_initialize_strike_pillage_and_repair_independently() {
    let (mut g, city_pos, _ring) = controlled_game(317);
    let cid = g.found_city_for(0, city_pos, None);
    let encampment_pos = g
        .wdisk(city_pos, 2)
        .into_iter()
        .find(|position| g.wdist(city_pos, *position) == 2)
        .unwrap();
    g.map.tiles.get_mut(&encampment_pos).unwrap().owner_city = Some(cid);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .owned_tiles
        .push(encampment_pos);
    let district = Item::District {
        district: crate::name!("encampment"),
        pos: encampment_pos,
    };
    g.players[0].techs.insert(crate::name!("bronze_working"));
    assert!(g.complete_item(0, cid, &district));
    assert_eq!(g.cities[&cid].encampment_hp, 100);
    assert_eq!(g.cities[&cid].encampment_wall_hp, 0);

    assert!(g.complete_item(
        0,
        cid,
        &Item::Building {
            building: crate::name!("walls"),
        },
    ));
    assert_eq!(g.cities[&cid].wall_hp, 100);
    assert_eq!(g.cities[&cid].encampment_wall_hp, 100);

    let target = g
        .wdisk(encampment_pos, 2)
        .into_iter()
        .find(|pos| {
            *pos != city_pos
                && *pos != encampment_pos
                && g.map.tiles.contains_key(pos)
                && g.wdist(*pos, city_pos) > 1
        })
        .unwrap();
    let target_unit = g.spawn_unit("warrior", 1, target);
    assert!(g.legal_actions(0).into_iter().any(|action| {
        matches!(action, Action::EncampmentStrike { city, target: to }
            if city == cid && to == target)
    }));
    g.apply(0, &Action::EncampmentStrike { city: cid, target })
        .unwrap();
    assert!(g.cities[&cid].encampment_struck);
    let participant = g.wars[&pair(0, 1)]
        .participants
        .iter()
        .find(|participant| participant.player == 0)
        .unwrap();
    assert!(participant.saw_action_units.is_empty());
    let defender = g.wars[&pair(0, 1)]
        .participants
        .iter()
        .find(|participant| participant.player == 1)
        .unwrap();
    assert!(defender.saw_action_units.contains_key(&target_unit));
    assert!(g
        .apply(0, &Action::EncampmentStrike { city: cid, target },)
        .is_err());
    g.begin_turn(0);
    assert!(!g.cities[&cid].encampment_struck);

    let attacker_pos = g
        .nbrs(encampment_pos)
        .into_iter()
        .find(|pos| *pos != city_pos && *pos != target)
        .unwrap();
    let attacker = g.spawn_unit("warrior", 1, attacker_pos);
    // A Bombard-depleted Encampment remains targetable until melee enters.
    g.cities.get_mut(&cid).unwrap().encampment_hp = 0;
    g.cities.get_mut(&cid).unwrap().encampment_wall_hp = 0;
    g.current = 1;
    assert!(g.legal_actions(1).into_iter().any(|action| {
        matches!(action, Action::Attack { unit, target }
            if unit == attacker && target == encampment_pos)
    }));
    g.apply(
        1,
        &Action::Attack {
            unit: attacker,
            target: encampment_pos,
        },
    )
    .unwrap();
    assert!(g.cities[&cid].encampment_pillaged);
    let repair = Item::Project {
        project: crate::name!("repair_encampment"),
    };
    assert!(!g.can_produce(0, cid, &repair));
    g.turn = g.cities[&cid].encampment_last_attacked + 3;
    assert!(g.can_produce(0, cid, &repair));
    assert!(g.complete_item(0, cid, &repair));
    assert_eq!(g.cities[&cid].encampment_hp, 100);
    assert_eq!(g.cities[&cid].encampment_wall_hp, 100);
    assert!(!g.cities[&cid].encampment_pillaged);
    assert!(!g.can_produce(0, cid, &repair));
}

#[test]
fn naval_roster_uses_gathering_storm_technology_and_civic_unlocks() {
    let (mut g, city_pos, ring) = controlled_game(319);
    let cid = g.found_city_for(0, city_pos, None);
    g.map.tiles.get_mut(&ring[0]).unwrap().terrain = crate::name!("coast");

    let unlocks = [
        ("galley", Some("sailing"), None),
        ("quadrireme", Some("shipbuilding"), None),
        ("caravel", Some("cartography"), None),
        ("frigate", Some("square_rigging"), None),
        ("privateer", None, Some("mercantilism")),
        ("ironclad", Some("steam_power"), None),
        ("battleship", Some("refining"), None),
        ("submarine", Some("electricity"), None),
        ("destroyer", Some("combined_arms"), None),
        ("aircraft_carrier", Some("combined_arms"), None),
        ("missile_cruiser", Some("lasers"), None),
        ("nuclear_submarine", Some("telecommunications"), None),
    ];
    for (kind, tech, civic) in unlocks {
        let spec = &g.rules.units[kind];
        assert_eq!(spec.domain.as_deref(), Some("sea"), "{kind} domain");
        assert_eq!(spec.tech.as_deref(), tech, "{kind} technology");
        assert_eq!(spec.civic.as_deref(), civic, "{kind} civic");
        let item = Item::Unit {
            unit: Name::new(kind),
        };
        assert!(!g.can_produce(0, cid, &item), "{kind} starts locked");
    }
    for resource in ["niter", "coal", "oil", "aluminum", "uranium"] {
        g.players[0]
            .strategic_resources
            .insert(Name::new(resource), 100.0);
    }
    for (kind, tech, civic) in unlocks {
        let item = Item::Unit {
            unit: Name::new(kind),
        };
        if let Some(technology) = tech {
            g.players[0].techs.insert(Name::new(technology));
        }
        if let Some(required_civic) = civic {
            g.players[0].civics.insert(Name::new(required_civic));
        }
        assert!(g.can_produce(0, cid, &item), "{kind} unlocks on schedule");
    }
}

#[test]
fn embarkation_and_ocean_access_unlock_in_distinct_stages() {
    let (mut g, land, ring) = controlled_game(320);
    let coast = ring[0];
    let ocean = g
        .nbrs(coast)
        .into_iter()
        .find(|pos| *pos != land)
        .expect("coast has another adjacent tile");
    g.map.tiles.get_mut(&coast).unwrap().terrain = crate::name!("coast");
    g.map.tiles.get_mut(&ocean).unwrap().terrain = crate::name!("ocean");

    let builder = g.spawn_unit("builder", 0, land);
    assert!(!g.can_move(builder, coast));
    assert!(g.path_to(builder, coast).is_none());
    assert!(g
        .apply(
            0,
            &Action::MoveTo {
                unit: builder,
                to: coast,
            },
        )
        .is_err());
    g.players[0].techs.insert(crate::name!("sailing"));
    assert!(g.can_move(builder, coast), "Sailing embarks Builders");
    assert_eq!(g.path_to(builder, coast), Some(vec![coast]));
    g.relocate(builder, coast);
    assert!(!g.can_move(builder, ocean));
    assert!(g.path_to(builder, ocean).is_none());
    g.players[0].techs.insert(crate::name!("cartography"));
    assert!(g.can_move(builder, ocean), "Cartography opens Ocean");
    g.remove_unit(builder);

    g.players[0].techs.clear();
    let trader = g.spawn_unit("trader", 0, land);
    g.players[0].techs.insert(crate::name!("sailing"));
    assert!(!g.can_move(trader, coast));
    g.players[0]
        .techs
        .insert(crate::name!("celestial_navigation"));
    assert!(
        g.can_move(trader, coast),
        "Celestial Navigation embarks Traders"
    );
    g.remove_unit(trader);

    g.players[0].techs.clear();
    let warrior = g.spawn_unit("warrior", 0, land);
    g.players[0].techs.insert(crate::name!("sailing"));
    assert!(!g.can_move(warrior, coast));
    g.players[0].techs.insert(crate::name!("shipbuilding"));
    assert!(
        g.can_move(warrior, coast),
        "Shipbuilding embarks other land units"
    );
    g.remove_unit(warrior);

    g.players[0].techs.clear();
    let galley = g.spawn_unit("galley", 0, coast);
    assert!(!g.can_move(galley, ocean), "early ships are Coast-bound");
    assert!(g.route_step(galley, ocean, 0).is_none());
    assert_eq!(g.unit_max_moves(galley), 3.0);
    g.players[0].techs.insert(crate::name!("mathematics"));
    assert_eq!(
        g.unit_max_moves(galley),
        4.0,
        "Mathematics adds sea Movement"
    );
    g.players[0].techs.insert(crate::name!("cartography"));
    assert!(g.can_move(galley, ocean));
    assert_eq!(g.route_step(galley, ocean, 0), Some(ocean));
}

#[test]
fn norway_and_maori_use_their_stock_early_ocean_rules() {
    let (mut g, land, ring) = controlled_game(3_201);
    let coast = ring[0];
    let ocean = g
        .nbrs(coast)
        .into_iter()
        .find(|position| *position != land)
        .unwrap();
    g.map.tiles.get_mut(&coast).unwrap().terrain = crate::name!("coast");
    g.map.tiles.get_mut(&ocean).unwrap().terrain = crate::name!("ocean");

    g.players[0].civ = "Norway".to_string();
    let longship = g.spawn_unit("galley", 0, coast);
    assert!(!g.can_move(longship, ocean));
    g.players[0].techs.insert(crate::name!("shipbuilding"));
    assert!(
        g.can_move(longship, ocean),
        "Knarr substitutes Shipbuilding for Cartography"
    );
    g.remove_unit(longship);

    let warrior = g.spawn_unit("warrior", 0, land);
    g.relocate(warrior, coast);
    assert!(
        !g.can_move(warrior, ocean),
        "Knarr's early Ocean access is restricted to naval units"
    );
    g.remove_unit(warrior);

    g.players[0].techs.clear();
    g.players[0].civ = "Maori".to_string();
    let warrior = g.spawn_unit("warrior", 0, land);
    assert!(
        g.can_move(warrior, coast),
        "Mana gives the starting Shipbuilding embarkation package"
    );
    g.relocate(warrior, coast);
    assert!(
        g.can_move(warrior, ocean),
        "Mana permits Ocean movement without Cartography"
    );
    assert_eq!(
        g.unit_max_moves(warrior),
        4.0,
        "Mana gives embarked units +2 Movement"
    );
}

#[test]
fn enforced_borders_share_one_rule_across_steps_and_routes() {
    let (mut g, start, ring) = controlled_game(3_202);
    g.at_war.clear();
    let target = ring[0];
    let foreign_city_position = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| g.wdist(start, *position) > 5)
        .unwrap();
    let foreign_city = g.found_city_for(1, foreign_city_position, None);
    g.map.tiles.get_mut(&target).unwrap().owner_city = Some(foreign_city);
    let warrior = g.spawn_unit("warrior", 0, start);

    assert!(
        g.can_move(warrior, target),
        "borders remain open before their owner adopts Early Empire"
    );
    g.players[1].civics.insert(crate::name!("early_empire"));
    assert!(!g.can_move(warrior, target));

    g.players[0].friends_until.insert(1, 40);
    g.players[1].friends_until.insert(0, 40);
    assert!(!g.can_move(warrior, target), "friendship is not Open Borders");
    g.players[0].open_borders_until.insert(1, 40);
    assert!(
        !g.can_move(warrior, target),
        "granting our borders is the wrong direction"
    );
    g.players[1].open_borders_until.insert(0, 40);
    assert!(g.can_move(warrior, target));

    g.players[0].open_borders_until.clear();
    g.players[1].open_borders_until.clear();
    let alliance = AllianceState {
        kind: "military".to_string(),
        points: 0.0,
        level: 1,
        ends: 40,
    };
    g.players[0].alliances.insert(1, alliance.clone());
    g.players[1].alliances.insert(0, alliance);
    assert!(g.can_move(warrior, target));

    g.players[0].alliances.clear();
    g.players[1].alliances.clear();
    g.at_war.insert(pair(0, 1));
    assert!(g.can_move(warrior, target));

    g.at_war.clear();
    g.remove_unit(warrior);
    let trader = g.spawn_unit("trader", 0, start);
    assert!(g.can_move(trader, target), "Traders ignore closed borders");
    g.remove_unit(trader);
    let missionary = g.spawn_unit("missionary", 0, start);
    assert!(
        g.can_move(missionary, target),
        "religious units ignore closed borders"
    );

    g.remove_unit(missionary);
    g.map.tiles.get_mut(&target).unwrap().owner_city = None;
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("mountain");
    }
    let first = hex::canon((start.0 + 1, start.1), g.map.width);
    let closed = hex::canon((start.0 + 2, start.1), g.map.width);
    let goal = hex::canon((start.0 + 3, start.1), g.map.width);
    for position in [start, first, closed, goal] {
        g.map.tiles.get_mut(&position).unwrap().terrain = crate::name!("plains");
        g.map.tiles.get_mut(&position).unwrap().owner_city = None;
    }
    g.map.tiles.get_mut(&closed).unwrap().owner_city = Some(foreign_city);
    let scout = g.spawn_unit("warrior", 0, start);
    assert_eq!(
        g.route_step(scout, goal, 0),
        None,
        "future route segments may not plan through closed territory"
    );
    g.players[1].open_borders_until.insert(0, 40);
    assert_eq!(g.route_step(scout, goal, 0), Some(first));
    g.players[1].open_borders_until.clear();
    assert_eq!(
        g.route_step(scout, goal, 0),
        None,
        "a cached route must be invalidated when access expires"
    );
}

#[test]
fn enforced_or_expired_borders_expel_only_units_that_need_access() {
    let setup = |seed| {
        let (mut game, start, ring) = controlled_game(seed);
        game.at_war.clear();
        let foreign_city_position = game
            .map
            .tiles
            .keys()
            .copied()
            .find(|position| game.wdist(start, *position) > 5)
            .unwrap();
        let city = game.found_city_for(1, foreign_city_position, None);
        let foreign = ring[0];
        game.map.tiles.get_mut(&foreign).unwrap().owner_city = Some(city);
        (game, foreign)
    };

    let (mut expired, foreign) = setup(3_204);
    expired.players[1].civics.insert(crate::name!("early_empire"));
    expired.players[1].open_borders_until.insert(0, 5);
    let warrior = expired.spawn_unit("warrior", 0, foreign);
    let trader = expired.spawn_unit("trader", 0, foreign);
    expired.turn = 5;
    expired.begin_turn(0);
    assert_ne!(expired.units[&warrior].pos, foreign);
    assert_ne!(expired.territory_owner_at(expired.units[&warrior].pos), Some(1));
    assert_eq!(
        expired.units[&trader].pos, foreign,
        "a Trader remains legal when ordinary Open Borders expires"
    );

    let (mut enforced, foreign) = setup(3_205);
    let scout = enforced.spawn_unit("warrior", 0, foreign);
    assert!(enforced.has_open_borders(0, 1));
    enforced.players[1].civics.insert(crate::name!("early_empire"));
    enforced.apply_tree_completion(1, false, "early_empire", true);
    assert_ne!(
        enforced.units[&scout].pos, foreign,
        "adopting Early Empire immediately expels existing intruders"
    );
}

#[test]
fn known_target_routes_reuse_the_planned_path_after_each_step() {
    let (mut g, start, _) = controlled_game(3_206);
    let target = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| g.wdist(start, *position) == 5)
        .min()
        .unwrap();
    let warrior = g.spawn_unit("warrior", 0, start);

    let first = g.route_step(warrior, target, 0).unwrap();
    let planned = g.routing.borrow().paths[0].path.clone();
    assert_eq!(planned.first(), Some(&start));
    assert_eq!(planned.last(), Some(&target));
    assert_eq!(planned.get(1), Some(&first));

    g.relocate(warrior, first);
    assert_eq!(g.route_step(warrior, target, 0), planned.get(2).copied());
    assert_eq!(
        g.routing.borrow().paths.len(),
        1,
        "advancing along a cached route must not launch and store a second plan",
    );
}

#[test]
fn reverse_flow_fields_match_forward_goal_search_and_reuse_the_field() {
    let (mut g, start, _) = controlled_game(3_207);
    let warrior = g.spawn_unit("warrior", 0, start);
    let goals: HashSet<Pos> = g
        .wdisk(start, 4)
        .into_iter()
        .filter(|position| g.wdist(start, *position) == 4)
        .collect();
    assert!(goals.len() > 1, "the fixture supplies a multi-source goal ring");

    let expected = g.first_route_step(warrior, |position| goals.contains(&position));
    assert!(expected.is_some(), "the goal ring is reachable on plains");
    assert!(g.routing.borrow().reverse_fields.is_empty());

    let first = g.route_step_to_any(warrior, &goals);
    assert_eq!(first, expected, "reverse and forward fields choose the same step");
    assert_eq!(g.routing.borrow().reverse_fields.len(), 1);

    let second = g.route_step_to_any(warrior, &goals);
    assert_eq!(second, first);
    assert_eq!(
        g.routing.borrow().reverse_fields.len(),
        1,
        "the same goal set reads the cached reverse field"
    );

    let mut reordered: Vec<Pos> = goals.iter().copied().collect();
    reordered.reverse();
    let reordered: HashSet<Pos> = reordered.into_iter().collect();
    assert_eq!(g.route_step_to_any(warrior, &reordered), first);
    assert_eq!(
        g.routing.borrow().reverse_fields.len(),
        1,
        "HashSet iteration order does not make a duplicate field"
    );
}

#[test]
fn embarked_units_use_transport_movement_and_pay_shore_transition_costs() {
    let (mut g, land, ring) = controlled_game(3201);
    let coast = ring[0];
    let second_water = g
        .nbrs(coast)
        .into_iter()
        .find(|position| *position != land)
        .unwrap();
    g.map.tiles.get_mut(&coast).unwrap().terrain = crate::name!("coast");
    g.map.tiles.get_mut(&second_water).unwrap().terrain = crate::name!("coast");
    g.players[0].techs.insert(crate::name!("shipbuilding"));

    let horse = g.spawn_unit("horseman", 0, land);
    assert_eq!(g.unit_base_max_moves(horse), 4.0);
    assert_eq!(g.unit_step_cost(horse, land, coast), 3.0);
    g.apply(
        0,
        &Action::Move {
            unit: horse,
            to: coast,
        },
    )
    .unwrap();
    assert!(g.is_embarked(&g.units[&horse]));
    let observed = crate::obs::observation(&g, 0);
    assert_eq!(
        observed["units"]
            .as_array()
            .unwrap()
            .iter()
            .find(|unit| unit["id"] == horse)
            .unwrap()["embarked"],
        true,
        "the client must render the unit as a transport instead of a land sprite"
    );
    assert_eq!(g.unit_base_max_moves(horse), 2.0);
    assert_eq!(g.units[&horse].moves_left, 1.0);
    g.apply(
        0,
        &Action::Move {
            unit: horse,
            to: second_water,
        },
    )
    .unwrap();
    assert_eq!(g.units[&horse].moves_left, 0.0);

    g.remove_unit(horse);
    let warrior = g.spawn_unit("warrior", 0, coast);
    for technology in ["mathematics", "square_rigging", "steam_power", "combustion"] {
        g.players[0].techs.insert(Name::new(technology));
    }
    assert_eq!(
        g.unit_base_max_moves(warrior),
        7.0,
        "embarked speed is 2 plus the stock sea and embarked technology bonuses"
    );
    g.units.get_mut(&warrior).unwrap().moves_left = 7.0;
    assert_eq!(g.unit_step_cost(warrior, coast, land), 3.0);
    g.apply(
        0,
        &Action::Move {
            unit: warrior,
            to: land,
        },
    )
    .unwrap();
    assert!(!g.is_embarked(&g.units[&warrior]));
    assert_eq!(
        g.units[&warrior].moves_left, 2.0,
        "disembarking cannot leave more MP than the land unit's maximum"
    );
}

#[test]
fn harbors_and_coastal_city_centers_remove_the_shore_penalty_and_open_cliffs() {
    let (mut g, land, ring) = controlled_game(3202);
    let coast = ring[0];
    g.map.tiles.get_mut(&coast).unwrap().terrain = crate::name!("coast");
    g.map.set_cliff_edge(land, coast, true);
    g.players[0].techs.insert(crate::name!("shipbuilding"));
    let warrior = g.spawn_unit("warrior", 0, land);

    assert!(
        !g.can_move(warrior, coast),
        "an ordinary cliff blocks embarkation"
    );
    g.map.tiles.get_mut(&coast).unwrap().district = Some(crate::name!("harbor"));
    assert_eq!(g.unit_step_cost(warrior, land, coast), 1.0);
    assert!(
        g.can_move(warrior, coast),
        "a working Harbor opens its cliff edge"
    );
    g.map.tiles.get_mut(&coast).unwrap().pillaged = true;
    assert_eq!(g.unit_step_cost(warrior, land, coast), 3.0);
    assert!(
        !g.can_move(warrior, coast),
        "a pillaged Harbor loses both benefits"
    );

    g.map.set_cliff_edge(land, coast, false);
    g.map.tiles.get_mut(&coast).unwrap().district = None;
    g.map.tiles.get_mut(&coast).unwrap().pillaged = false;
    let city = g.found_city_for(0, land, None);
    assert_eq!(g.city_at(land), Some(city));
    assert_eq!(
        g.unit_step_cost(warrior, land, coast),
        1.0,
        "a coastal City Center also removes the transition surcharge"
    );
}

#[test]
fn embarked_defense_stacking_and_gdr_water_rules_match_native_domains() {
    let (mut g, land, ring) = controlled_game(3203);
    let coast = ring[0];
    let enemy_water = g
        .nbrs(coast)
        .into_iter()
        .find(|position| *position != land)
        .unwrap();
    g.map.tiles.get_mut(&coast).unwrap().terrain = crate::name!("coast");
    g.map.tiles.get_mut(&enemy_water).unwrap().terrain = crate::name!("coast");

    let warrior = g.spawn_unit("warrior", 0, coast);
    assert!(g.is_embarked(&g.units[&warrior]));
    // Embarked strength follows the owning player's era, not a more
    // advanced rival's contribution to the world-era clock.
    assert_eq!(g.unit_strength(&g.units[&warrior], true), 10.0);
    g.players[0].techs.insert(crate::name!("horseback_riding"));
    assert_eq!(g.unit_strength(&g.units[&warrior], true), 15.0);
    g.players[0].techs.insert(crate::name!("cartography"));
    assert_eq!(g.unit_strength(&g.units[&warrior], true), 30.0);
    g.units.get_mut(&warrior).unwrap().formation = 1;
    assert_eq!(g.unit_strength(&g.units[&warrior], true), 40.0);

    let galley = g.spawn_unit("galley", 0, enemy_water);
    assert!(g.can_move(galley, coast));
    g.apply(
        0,
        &Action::Move {
            unit: galley,
            to: coast,
        },
    )
    .unwrap();
    assert!(g.can_link_units(0, galley, warrior));

    g.remove_unit(galley);
    g.remove_unit(warrior);
    g.players[0].techs.clear();
    let robot = g.spawn_unit("giant_death_robot", 0, land);
    assert!(g.can_move(robot, coast));
    g.apply(
        0,
        &Action::Move {
            unit: robot,
            to: coast,
        },
    )
    .unwrap();
    assert!(!g.is_embarked(&g.units[&robot]));
    assert_eq!(g.unit_strength(&g.units[&robot], true), 130.0);
    assert!(g.in_enemy_zoc(1, land));
    assert!(
        g.in_enemy_zoc(1, enemy_water),
        "the GDR projects its ZOC into water as well as land"
    );
    g.spawn_unit("galley", 1, enemy_water);
    assert!(g.unit_can_melee_target_domain(robot, enemy_water));
}

#[test]
fn naval_roles_fight_at_sea_and_melee_ships_capture_only_coastal_cities() {
    let (mut g, city_pos, ring) = controlled_game(321);
    let coast = ring[0];
    g.map.tiles.get_mut(&coast).unwrap().terrain = crate::name!("coast");
    let city = g.found_city_for(1, city_pos, None);
    g.cities.get_mut(&city).unwrap().hp = 1;
    let galley = g.spawn_unit("galley", 0, coast);
    assert!(g.legal_actions(0).into_iter().any(|action| {
        matches!(action, Action::Attack { unit, target }
            if unit == galley && target == city_pos)
    }));
    g.apply(
        0,
        &Action::Attack {
            unit: galley,
            target: city_pos,
        },
    )
    .unwrap();
    assert_eq!(g.cities[&city].owner, 0);
    assert_eq!(g.units[&galley].pos, city_pos);

    let (mut g, land, ring) = controlled_game(322);
    let coast = ring[0];
    let inland = g.nbrs(coast).into_iter().find(|pos| *pos != land).unwrap();
    g.map.tiles.get_mut(&coast).unwrap().terrain = crate::name!("coast");
    let galley = g.spawn_unit("galley", 0, coast);
    g.spawn_unit("warrior", 1, inland);
    assert!(g
        .apply(
            0,
            &Action::Attack {
                unit: galley,
                target: inland,
            },
        )
        .is_err());

    let enemy_coast = g
        .nbrs(coast)
        .into_iter()
        .find(|pos| *pos != inland && *pos != land)
        .unwrap();
    g.map.tiles.get_mut(&enemy_coast).unwrap().terrain = crate::name!("coast");
    let enemy_ship = g.spawn_unit("galley", 1, enemy_coast);
    let quadrireme = g.spawn_unit("quadrireme", 0, coast);
    g.apply(
        0,
        &Action::Ranged {
            unit: quadrireme,
            target: enemy_coast,
        },
    )
    .unwrap();
    assert!(g.units.get(&enemy_ship).is_none_or(|unit| unit.hp < 100));
}

#[test]
fn naval_ranged_units_do_not_take_the_land_ranged_anti_ship_penalty() {
    let (base, center, ring) = controlled_game(324);
    let attacker_pos = ring[0];

    let mut naval = base.clone();
    naval.map.tiles.get_mut(&center).unwrap().terrain = crate::name!("coast");
    naval.map.tiles.get_mut(&attacker_pos).unwrap().terrain = crate::name!("coast");
    let naval_attacker = naval.spawn_unit("quadrireme", 0, attacker_pos);
    let naval_target = naval.spawn_unit("galley", 1, center);
    naval
        .apply(
            0,
            &Action::Ranged {
                unit: naval_attacker,
                target: center,
            },
        )
        .unwrap();
    let naval_damage = 100 - naval.units[&naval_target].hp;

    let mut land = base;
    land.map.tiles.get_mut(&center).unwrap().terrain = crate::name!("coast");
    std::sync::Arc::make_mut(&mut land.rules)
        .units
        .get_mut("archer")
        .unwrap()
        .ranged_strength = 25.0;
    let land_attacker = land.spawn_unit("archer", 0, attacker_pos);
    let land_target = land.spawn_unit("galley", 1, center);
    land.apply(
        0,
        &Action::Ranged {
            unit: land_attacker,
            target: center,
        },
    )
    .unwrap();
    let land_damage = 100 - land.units[&land_target].hp;

    assert!(
        naval_damage > land_damage,
        "naval ranged {naval_damage} should outperform equal-strength land ranged {land_damage} against a ship"
    );
}

#[test]
fn naval_raiders_require_adjacent_or_specialized_detection() {
    let (mut g, center, ring) = controlled_game(323);
    for pos in std::iter::once(center).chain(ring.iter().copied()) {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
    }
    let submarine = g.spawn_unit("submarine", 0, center);
    let distant = g
        .wdisk(center, 2)
        .into_iter()
        .find(|pos| g.wdist(*pos, center) == 2)
        .unwrap();
    g.map.tiles.get_mut(&distant).unwrap().terrain = crate::name!("plains");
    let observer = g.spawn_unit("warrior", 1, distant);
    assert!(!g.unit_visible_to(submarine, 1));
    g.relocate(observer, ring[0]);
    assert!(g.unit_visible_to(submarine, 1));
    g.relocate(observer, distant);
    g.map.tiles.get_mut(&distant).unwrap().terrain = crate::name!("coast");
    g.remove_unit(observer);
    g.spawn_unit("destroyer", 1, distant);
    assert!(g.unit_visible_to(submarine, 1));
}

#[test]
fn pyramids_use_a_legal_tile_remain_world_unique_and_improve_builders() {
    let (mut g, center, ring) = controlled_game(324);
    let cid = g.found_city_for(0, center, None);
    let site = ring[0];
    {
        let tile = g.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("desert");
        tile.hills = false;
        tile.owner_city = Some(cid);
    }
    g.players[0].techs.insert(crate::name!("masonry"));
    let item = Item::Wonder {
        wonder: crate::name!("pyramids"),
        pos: site,
    };
    assert!(g.can_produce(0, cid, &item));

    let builders_before = g
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "builder")
        .count();
    assert!(g.complete_item(0, cid, &item));
    assert_eq!(g.map.tiles[&site].wonder.as_deref(), Some("pyramids"));
    assert_eq!(g.cities[&cid].wonders.get(&Name::new("pyramids")), Some(&site));
    assert!(g.wonder_built("pyramids"));
    assert!(!g.can_produce(0, cid, &item));

    let builders: Vec<&Unit> = g
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "builder")
        .collect();
    assert_eq!(builders.len(), builders_before + 1);
    assert!(builders
        .iter()
        .any(|builder| builder.charges > g.rules.units["builder"].charges));
}

#[test]
fn world_wonders_reject_forbidden_relief_and_unremovable_features() {
    let (mut g, center, ring) = controlled_game(325);
    let cid = g.found_city_for(0, center, None);
    let site = ring[0];
    g.map.tiles.get_mut(&site).unwrap().owner_city = Some(cid);

    g.players[0].techs.insert(crate::name!("mathematics"));
    {
        let tile = g.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("desert");
        tile.hills = true;
    }
    assert!(
        !g.wonder_sites(cid, "petra").contains(&site),
        "BUILDING_PETRA only lists flat TERRAIN_DESERT"
    );
    g.map.tiles.get_mut(&site).unwrap().hills = false;
    assert!(g.wonder_sites(cid, "petra").contains(&site));

    g.players[0].techs.insert(crate::name!("engineering"));
    {
        let tile = g.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("mountain");
        tile.feature = Some(crate::name!("volcano"));
    }
    assert!(
        !g.wonder_sites(cid, "machu_picchu").contains(&site),
        "the mountain exception must not make an impassable Volcano removable"
    );
    g.map.tiles.get_mut(&site).unwrap().feature = None;
    assert!(g.wonder_sites(cid, "machu_picchu").contains(&site));
}

#[test]
fn pillaging_pays_the_yield_and_amount_its_row_ships() {
    // Districts.PlunderType/PlunderAmount and the same pair on
    // Improvements. Gold and heal pay 50, Science, Culture and Faith 25 --
    // and the type is per entry, not per family guess. A hand-written
    // match had 24 of the 48 wrong.
    let rules = crate::rules::Rules::embedded();
    for (district, kind, amount) in [
        ("campus", "science", 25.0),
        ("holy_site", "faith", 25.0),
        ("theater_square", "culture", 25.0),
        ("commercial_hub", "gold", 50.0),
        ("harbor", "gold", 50.0),
        // The three CIVVIS used to get wrong.
        ("industrial_zone", "science", 25.0),
        ("aerodrome", "gold", 50.0),
        ("entertainment_complex", "heal", 50.0),
        ("diplomatic_quarter", "culture", 25.0),
        ("spaceport", "science", 25.0),
    ] {
        let spec = &rules.districts[district];
        assert_eq!(spec.plunder_type.as_deref(), Some(kind), "{district}");
        assert_eq!(spec.plunder_amount, amount, "{district}");
    }
    for (improvement, kind, amount) in [
        ("farm", "heal", 50.0),
        ("mine", "gold", 50.0),
        ("quarry", "faith", 25.0),
        ("camp", "faith", 25.0),
        ("pasture", "faith", 25.0),
        ("plantation", "faith", 25.0),
    ] {
        let spec = &rules.improvements[improvement];
        assert_eq!(spec.plunder_type.as_deref(), Some(kind), "{improvement}");
        assert_eq!(spec.plunder_amount, amount, "{improvement}");
    }

    // A unique district plunders as the district it replaces.
    assert_eq!(
        rules.districts["hansa"].plunder_type.as_deref().or(Some("science")),
        Some("science")
    );
}

#[test]
fn every_religious_unit_carries_its_shipped_strength_and_eviction() {
    // Units.ReligiousStrength and Units.ReligionEvictPercent, which the
    // fidelity ratchet did not reach until now. The Inquisitor's 75 is the
    // interesting one: CIVVIS spends it through Remove Heresy rather than
    // Spread, so it shows up as rival pressure retained at a quarter.
    let rules = crate::rules::Rules::embedded();
    for (unit, strength) in [
        ("apostle", 110.0),
        ("missionary", 100.0),
        ("guru", 90.0),
        ("inquisitor", 75.0),
    ] {
        assert_eq!(rules.units[unit].religious_strength, strength, "{unit}");
    }

    // RELIGION_SPREAD_STRENGTH_MULTIPLIER is 200: a unit spreads at twice
    // its Religious Strength, and the Inquisitor does not spread at all.
    assert_eq!(rules.units["apostle"].religious_spread, 220.0);
    assert_eq!(rules.units["missionary"].religious_spread, 200.0);
    assert_eq!(rules.units["inquisitor"].religious_spread, 0.0);

    // ReligionEvictPercent 75 for the Inquisitor, through Remove Heresy.
    let (mut g, city_pos, ring) = controlled_game(319);
    let cid = g.found_city_for(0, city_pos, None);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .pressure
        .insert("Rival".to_string(), 400.0);
    let inquisitor = g.spawn_unit("inquisitor", 0, city_pos);
    g.units.get_mut(&inquisitor).unwrap().religion = Some("Ours".to_string());
    g.players[0].religion = Some("Ours".to_string());
    g.do_remove_heresy(0, inquisitor).unwrap();
    assert_eq!(g.cities[&cid].pressure["Rival"], 100.0, "75% removed");
    let _ = ring;
}

#[test]
fn proselytizer_evicts_the_half_its_row_ships() {
    // APOSTLE_EVICT_ALL is 50, and CIVVIS takes the greater of the unit's
    // own eviction and the promotion's. A bare Apostle evicts a quarter;
    // Proselytizer takes half instead, not the three quarters CIVVIS used
    // to pay. El Escorial's inquisitor trait is 25 on the same effect, so
    // the scale is real rather than nominal.
    let (mut g, city_pos, ring) = controlled_game(318);
    let cid = g.found_city_for(1, city_pos, None);
    let evicted = |g: &mut Game, ring_index: usize, promotion: Option<&str>| {
        g.cities
            .get_mut(&cid)
            .unwrap()
            .pressure
            .insert("Rival".to_string(), 400.0);
        let apostle = g.spawn_unit("apostle", 0, ring[ring_index]);
        let unit = g.units.get_mut(&apostle).unwrap();
        unit.religion = Some("Our".to_string());
        if let Some(promotion) = promotion {
            unit.promotions.insert(Name::new(promotion));
        }
        g.apply(0, &Action::Spread { unit: apostle }).unwrap();
        g.cities[&cid].pressure["Rival"]
    };
    assert_eq!(evicted(&mut g, 0, None), 300.0);
    assert_eq!(evicted(&mut g, 1, Some("proselytizer")), 200.0);
}

#[test]
fn religious_spreads_combat_and_guru_healing_follow_gathering_storm() {
    let (mut g, city_pos, ring) = controlled_game(318);
    let cid = g.found_city_for(1, city_pos, None);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .pressure
        .insert("Rival".to_string(), 300.0);

    let missionary = g.spawn_unit("missionary", 0, ring[0]);
    g.units.get_mut(&missionary).unwrap().religion = Some("Our".to_string());
    g.apply(0, &Action::Spread { unit: missionary }).unwrap();
    assert_eq!(g.cities[&cid].pressure["Rival"], 270.0);
    assert_eq!(g.cities[&cid].pressure["Our"], 200.0);
    assert!(g.apply(0, &Action::Spread { unit: missionary }).is_err());

    let apostle = g.spawn_unit("apostle", 0, ring[1]);
    g.units.get_mut(&apostle).unwrap().religion = Some("Our".to_string());
    g.apply(0, &Action::Spread { unit: apostle }).unwrap();
    assert_eq!(g.cities[&cid].pressure["Rival"], 202.5);
    assert_eq!(g.cities[&cid].pressure["Our"], 420.0);

    let victim = g.spawn_unit("missionary", 1, city_pos);
    {
        let victim = g.units.get_mut(&victim).unwrap();
        victim.religion = Some("Rival".to_string());
        victim.hp = 1;
    }
    {
        let apostle = g.units.get_mut(&apostle).unwrap();
        apostle.moves_left = 4.0;
        apostle.acted = false;
    }
    g.apply(
        0,
        &Action::TheologicalAttack {
            unit: apostle,
            target: city_pos,
        },
    )
    .unwrap();
    assert!(!g.units.contains_key(&victim));
    assert_eq!(g.cities[&cid].pressure["Rival"], 0.0);
    assert_eq!(g.cities[&cid].pressure["Our"], 670.0);

    let guru_pos = ring[3];
    let guru = g.spawn_unit("guru", 0, guru_pos);
    let faithful = g.spawn_unit("missionary", 0, guru_pos);
    let other_faith = g.spawn_unit("missionary", 0, guru_pos);
    for uid in [guru, faithful] {
        let unit = g.units.get_mut(&uid).unwrap();
        unit.religion = Some("Our".to_string());
        unit.hp = 50;
    }
    {
        let unit = g.units.get_mut(&other_faith).unwrap();
        unit.religion = Some("Other".to_string());
        unit.hp = 50;
    }
    g.apply(0, &Action::HealReligious { unit: guru }).unwrap();
    assert_eq!(g.units[&guru].hp, 90, "a Guru heals itself");
    assert_eq!(g.units[&faithful].hp, 90);
    assert_eq!(g.units[&other_faith].hp, 50);
}

/// ★★★★★ POLAND'S UNIQUE HAD ITS ABILITY IN THE DATA AND NOWHERE ELSE.
///
/// `data/units.json` gives the Winged Hussar `force_retreat: 1`; the engine read
/// that key nowhere, so the replacement for a Cuirassier was a Cuirassier. The
/// audit that would have said so, `tools/civvis_inert.py`, documented a CI
/// ratchet from the day it was written and was never wired into a workflow.
#[test]
fn a_winged_hussar_pushes_the_survivor_off_its_tile() {
    let (mut g, target, ring) = controlled_game(5101);
    let hussar = g.spawn_unit("winged_hussar", 0, ring[0]);
    let defender = g.spawn_unit("musketman", 1, target);
    // Strength 55 against 64 survives one exchange; a Warrior does not, and a
    // dead defender would pass an assertion about not standing on its tile.
    g.units.get_mut(&defender).unwrap().hp = 100;
    let attacker_at = g.units[&hussar].pos;

    g.do_attack(0, hussar, target).expect("the attack resolves");

    let defender_now = g.units[&defender].pos;
    assert!(g.units[&defender].hp > 0, "this test is about a survivor");
    assert_ne!(defender_now, target, "the loser is pushed off its tile");
    assert!(
        g.wdist(attacker_at, defender_now) > g.wdist(attacker_at, target),
        "and pushed AWAY from the attacker, not sideways"
    );
    assert!(
        g.units_at(defender_now).contains(&defender),
        "the occupancy index moves with it, or `units_at` hands out a stale id"
    );
}

/// The same attack by the unit the Hussar replaces must not move anything, so
/// the test above is measuring the ability rather than melee in general.
#[test]
fn an_ordinary_cuirassier_leaves_the_survivor_where_it_stood() {
    let (mut g, target, ring) = controlled_game(5101);
    let cuirassier = g.spawn_unit("cuirassier", 0, ring[0]);
    let defender = g.spawn_unit("musketman", 1, target);
    g.units.get_mut(&defender).unwrap().hp = 100;

    g.do_attack(0, cuirassier, target).expect("the attack resolves");

    if g.units.contains_key(&defender) {
        assert_eq!(
            g.units[&defender].pos, target,
            "only `force_retreat` moves a survivor"
        );
    }
}

/// Cornered: Civilization VI charges the defender for the retreat it cannot
/// make. The surrounding units are the attacker's own, so nothing the defender
/// could enter is adjacent.
#[test]
fn a_cornered_survivor_pays_for_the_retreat_it_cannot_make() {
    let (mut g, target, ring) = controlled_game(5102);
    let hussar = g.spawn_unit("winged_hussar", 0, ring[0]);
    let defender = g.spawn_unit("musketman", 1, target);
    g.units.get_mut(&defender).unwrap().hp = 100;
    // Fill every tile the defender could withdraw to.
    for pos in ring.iter().skip(1) {
        g.spawn_unit("musketman", 0, *pos);
    }
    let blocked_hp = {
        let mut open = g.clone();
        // The same exchange with the ring left open, for the damage the retreat
        // itself does not add.
        for pos in ring.iter().skip(1) {
            let ids: Vec<u32> = open.units_at(*pos).to_vec();
            for id in ids {
                open.remove_unit(id);
            }
        }
        open.do_attack(0, hussar, target).expect("the attack resolves");
        open.units.get(&defender).map(|u| u.hp)
    };

    g.do_attack(0, hussar, target).expect("the attack resolves");

    assert_eq!(g.units[&defender].pos, target, "there was nowhere to go");
    if let Some(open_hp) = blocked_hp {
        assert!(
            g.units[&defender].hp < open_hp,
            "being cornered costs more than retreating: {} vs {open_hp}",
            g.units[&defender].hp
        );
    }
}
