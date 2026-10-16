use super::*;

fn campaign_gun() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(3, 30, 16, 407_000, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.clear_mirror_cities();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.owner_city = None;
    }
    g.found_city_for(0, (2, 6), None);
    let cid = g.found_city_for(1, (10, 6), Some("Siege objective".into()));
    g.found_city_for(2, (20, 6), None);
    g.cities.get_mut(&cid).unwrap().wall_hp = 400;
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    let memory = g.remember_city(&g.cities[&cid]);
    g.players[0].remembered_cities.insert(cid, memory);
    let gun = g.spawn_test_unit("bombard", 0, (8, 6));
    g.units.get_mut(&gun).unwrap().xp = 15;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_city_campaign();
    ai.campaign = Some(city_campaign::CampaignPlan {
        target: 1,
        cities: vec![cid],
        requirement: 100.0,
        bodies: 3,
        planned: g.turn,
        declared: Some(g.turn),
        taken: 0,
    });
    (g, ai, cid, gun)
}

#[test]
fn a_walled_campaign_promotes_the_first_gun_toward_shells() {
    let (mut g, mut on, _, gun) = campaign_gun();
    let mut off = g.clone();
    on.advanced_promotions(&mut off, 0, GrandStrategy::Conquest);
    assert!(
        off.units[&gun].promotions.contains("grape_shot"),
        "actual old choice"
    );
    on.enable_siege_promotions_open_the_walls();
    on.enable_promote_when_wounded();
    on.advanced_promotions(&mut g, 0, GrandStrategy::Conquest);
    assert!(g.units[&gun].promotions.contains("crew_weapons"));
    assert_eq!(
        g.players[0].counters.get("siege-promotions:walls"),
        Some(&1)
    );
    assert!(!g.promotion_pending(gun));
    assert_eq!(g.units[&gun].hp, 100);
}

#[test]
fn both_promotion_paths_are_compared_on_an_executed_city_shot() {
    let (initial, mut ai, cid, gun) = campaign_gun();
    let mut off = initial.clone();
    let mut on = initial;
    ai.advanced_promotions(&mut off, 0, GrandStrategy::Conquest);
    ai.enable_siege_promotions_open_the_walls();
    ai.advanced_promotions(&mut on, 0, GrandStrategy::Conquest);
    for (game, enabled) in [(&mut off, false), (&mut on, true)] {
        let unit = game.units.get_mut(&gun).unwrap();
        unit.xp = 45;
        unit.moves_left = 3.0;
        if enabled {
            ai.enable_siege_promotions_open_the_walls();
        } else {
            ai.disable_siege_promotions_open_the_walls();
        }
        ai.advanced_promotions(game, 0, GrandStrategy::Conquest);
        let unit = game.units.get_mut(&gun).unwrap();
        unit.moves_left = 3.0;
        unit.attacks_left = 1;
        let target = game.cities[&cid].pos;
        game.apply(0, &Action::Ranged { unit: gun, target })
            .unwrap();
    }
    assert!(off.units[&gun].promotions.contains("shrapnel"));
    assert!(on.units[&gun].promotions.contains("shells"));
    assert!(
        on.cities[&cid].wall_hp < off.cities[&cid].wall_hp,
        "the legal Shells path must deal more actual wall damage"
    );
}

#[test]
fn a_pending_promotion_still_heals_a_wounded_gun() {
    let (mut g, mut ai, _, gun) = campaign_gun();
    g.units.get_mut(&gun).unwrap().hp = 40;
    ai.enable_siege_promotions_open_the_walls();
    ai.enable_promote_when_wounded();
    ai.advanced_promotions(&mut g, 0, GrandStrategy::Conquest);
    assert_eq!(g.units[&gun].hp, 90);
    assert!(!g.promotion_pending(gun));
}

#[test]
fn peace_unwalled_unknown_and_stale_campaigns_keep_the_old_choice() {
    for control in [
        "peace",
        "unwalled",
        "unknown",
        "stale_owner",
        "no_campaign",
        "not_conquest",
    ] {
        let (mut g, mut ai, cid, gun) = campaign_gun();
        let mut strategy = GrandStrategy::Conquest;
        match control {
            "peace" => g.at_war.clear(),
            "unwalled" => {
                g.cities.get_mut(&cid).unwrap().wall_hp = 0;
                g.players[0]
                    .remembered_cities
                    .get_mut(&cid)
                    .unwrap()
                    .wall_hp = 0;
            }
            "unknown" => {
                g.relocate(gun, (2, 6));
                g.players[0].remembered_cities.clear();
            }
            "stale_owner" => g.cities.get_mut(&cid).unwrap().owner = 2,
            "no_campaign" => ai.campaign = None,
            "not_conquest" => strategy = GrandStrategy::Recovery,
            _ => unreachable!(),
        }
        let mut off = g.clone();
        ai.advanced_promotions(&mut off, 0, strategy);
        ai.enable_siege_promotions_open_the_walls();
        ai.advanced_promotions(&mut g, 0, strategy);
        assert_eq!(
            g.units[&gun].promotions, off.units[&gun].promotions,
            "{control}"
        );
        assert_eq!(
            g.players[0].counters.get("siege-promotions:walls"),
            None,
            "{control}"
        );
    }
}

#[test]
fn fogged_campaign_reads_the_last_seen_wall_pool() {
    let (mut g, mut ai, cid, gun) = campaign_gun();
    g.relocate(gun, (2, 6));
    assert!(!g.player_visibility(0).contains(&g.cities[&cid].pos));
    ai.enable_siege_promotions_open_the_walls();
    let available = g.available_promotions(gun);
    assert_eq!(
        ai.wall_campaign_promotion(&g, 0, gun, GrandStrategy::Conquest, &available),
        Some(crate::name!("crew_weapons"))
    );
    g.players[0]
        .remembered_cities
        .get_mut(&cid)
        .unwrap()
        .wall_hp = 0;
    assert_eq!(
        ai.wall_campaign_promotion(&g, 0, gun, GrandStrategy::Conquest, &available),
        None
    );
}

#[test]
fn other_classes_and_unearned_promotions_remain_unchanged() {
    let (mut g, mut ai, _, gun) = campaign_gun();
    ai.enable_siege_promotions_open_the_walls();
    let archer = g.spawn_test_unit("archer", 0, (8, 7));
    g.units.get_mut(&archer).unwrap().xp = 15;
    let available = g.available_promotions(archer);
    assert_eq!(
        ai.wall_campaign_promotion(&g, 0, archer, GrandStrategy::Conquest, &available),
        None
    );
    g.units.get_mut(&gun).unwrap().xp = 14;
    ai.advanced_promotions(&mut g, 0, GrandStrategy::Conquest);
    assert!(g.units[&gun].promotions.is_empty());
}

#[test]
fn a_legal_expert_crew_keeps_priority_and_does_not_count_as_a_changed_choice() {
    let (mut g, mut ai, _, gun) = campaign_gun();
    let unit = g.units.get_mut(&gun).unwrap();
    unit.promotions
        .extend([crate::name!("crew_weapons"), crate::name!("shells")]);
    unit.level = 3;
    unit.xp = 90;
    ai.enable_siege_promotions_open_the_walls();
    ai.advanced_promotions(&mut g, 0, GrandStrategy::Conquest);
    assert!(g.units[&gun].promotions.contains("expert_crew"));
    assert_eq!(g.players[0].counters.get("siege-promotions:walls"), None);
}
