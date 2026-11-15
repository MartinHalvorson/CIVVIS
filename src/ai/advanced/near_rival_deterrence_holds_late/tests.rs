use super::*;

/// Two majors who have met, each with one founded city within the neighbour
/// radius, our capital idle at population 4, every unit removed, five turns
/// past the opening window. Returns the game, our capital and theirs.
fn neighbours() -> Option<(Game, u32, u32)> {
    let mut game = Game::new_full(2, 32, 20, 936_221, 400, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")?;
    game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let ours = game.player_city_ids(0)[0];
    let home = game.cities[&ours].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (5..=DETERRENCE_NEAR_TILES).contains(&game.wdist(**pos, home))
                && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    let site = *sites.first()?;
    let theirs = game.place_city(1, site, None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    let city = game.cities.get_mut(&ours).unwrap();
    city.pop = 4;
    city.queue.clear();
    game.players[0].techs.insert(crate::name!("bronze_working"));
    game.turn = game.standard_duration(DETERRENCE_END_STANDARD) + 5;
    Some((game, ours, theirs))
}

fn fixture() -> (Game, u32, u32) {
    neighbours().expect("fixture: two capitals within the neighbour radius")
}

fn power(game: &mut Game, pid: usize, value: f64) {
    std::sync::Arc::make_mut(&mut game.observed_military_power).insert(pid, value);
}

fn late(mut ai: AdvancedAi) -> AdvancedAi {
    ai.enable_near_rival_deterrence_holds_late();
    ai
}

fn land_units_queued(game: &Game) -> usize {
    game.player_city_ids(0)
        .into_iter()
        .filter(|cid| {
            matches!(
                game.cities[cid].queue.first(),
                Some(Item::Unit { unit }) if AdvancedAi::deterrence_land_unit(game, unit)
            )
        })
        .count()
}

fn claim(ai: &AdvancedAi, game: &Game) -> Game {
    let mut after = game.clone();
    let plan = ai.assess(&after, 0);
    ai.claim_deterrence_unit(&mut after, 0, &plan);
    after
}

/// Off, past the window nothing reads and nothing is queued, the opening gene
/// on or off; on, a neighbour with twice our power gets a land unit, held
/// while we stand short of 0.8 of its power.
#[test]
fn off_is_a_no_op_and_on_trains_a_deterrent_after_the_window() {
    let (mut game, ours, _) = fixture();
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 200.0);

    for off in [AdvancedAi::new(), {
        let mut opening = AdvancedAi::new();
        opening.enable_near_rival_deterrence();
        opening
    }] {
        assert_eq!(off.deterrence_phase(&game), None);
        assert!(
            off.deterrence_threat(&game, 0).is_none(),
            "off: past the window"
        );
        assert!(off.late_deterrence_shortfall(&game, 0).is_none());
        let after = claim(&off, &game);
        assert_eq!(
            after.cities[&ours].queue, game.cities[&ours].queue,
            "off: no claim"
        );
    }

    let ai = late(AdvancedAi::new());
    assert_eq!(ai.deterrence_phase(&game), Some(DeterrencePhase::Late));
    assert_eq!(ai.deterrence_ratio(&game), LATE_DETERRENCE_RATIO);
    let after = claim(&ai, &game);
    assert_eq!(
        land_units_queued(&after),
        1,
        "one land unit against the neighbour"
    );
    let head = after.cities[&ours].queue.first().cloned().unwrap();
    assert!(
        ai.deterrence_unit_holds(&after, 0, &head),
        "the deterrent is held while short of the target"
    );
}

/// The late gene leaves the opening window to the opening gene: alone, it
/// does nothing before the window's end.
#[test]
fn the_late_gene_waits_for_the_window_to_close() {
    let (mut game, _, _) = fixture();
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 200.0);
    game.turn = game.standard_duration(DETERRENCE_END_STANDARD) - 1;
    let ai = late(AdvancedAi::new());
    assert_eq!(ai.deterrence_phase(&game), None);
    assert_eq!(land_units_queued(&claim(&ai, &game)), 0);
    // With the opening gene on too, the opening gene answers at parity.
    let mut both = late(AdvancedAi::new());
    both.enable_near_rival_deterrence();
    assert_eq!(both.deterrence_phase(&game), Some(DeterrencePhase::Opening));
    assert_eq!(both.deterrence_ratio(&game), DETERRENCE_RATIO);
}

/// The target: at 0.85 of the neighbour's power nothing is claimed or held;
/// at 0.75 the claim fires (the opening gene would build to parity).
#[test]
fn the_claim_stops_at_the_target() {
    let (mut game, ours, _) = fixture();
    let ai = late(AdvancedAi::new());
    power(&mut game, 1, 200.0);

    let mut above = game.clone();
    power(&mut above, 0, 170.0);
    assert!(ai.late_deterrence_shortfall(&above, 0).is_none());
    assert_eq!(
        land_units_queued(&claim(&ai, &above)),
        0,
        "past 0.8: no claim"
    );
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    assert!(
        !ai.deterrence_unit_holds(&above, 0, &warrior),
        "past 0.8: no hold"
    );

    let mut below = game.clone();
    power(&mut below, 0, 150.0);
    assert!(ai.late_deterrence_shortfall(&below, 0).is_some());
    let after = claim(&ai, &below);
    assert_eq!(land_units_queued(&after), 1, "below 0.8: the claim fires");
    assert!(matches!(
        after.cities[&ours].queue.first(),
        Some(Item::Unit { .. })
    ));

    let mut war = below.clone();
    war.at_war.insert((0, 1));
    assert!(
        ai.late_deterrence_shortfall(&war, 0).is_none(),
        "a rival at war is the war's"
    );
}

/// A Domination seat's army has its own plan: nothing reads, nothing moves.
/// Another target with the same board does fire.
#[test]
fn a_domination_seat_is_untouched() {
    let (mut game, ours, _) = fixture();
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 200.0);
    let domination = late(AdvancedAi::targeting(VictoryTarget::Domination));
    assert_eq!(domination.deterrence_phase(&game), None);
    let after = claim(&domination, &game);
    assert_eq!(after.cities[&ours].queue, game.cities[&ours].queue);
    assert_eq!(domination.late_deterrence_upgrades(&mut game.clone(), 0), 0);

    let science = late(AdvancedAi::targeting(VictoryTarget::Science));
    assert_eq!(land_units_queued(&claim(&science, &game)), 1);
}

/// On a Culture seat a Theater Square building keeps its queue past the
/// window; on a Science seat the same building gives way.
#[test]
fn a_culture_seat_keeps_its_theater_queue() {
    let (mut game, ours, _) = fixture();
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 200.0);
    let amphitheater = Item::Building {
        building: crate::name!("amphitheater"),
    };
    game.cities.get_mut(&ours).unwrap().queue = vec![amphitheater.clone()];

    let culture = late(AdvancedAi::targeting(VictoryTarget::Culture));
    assert!(culture.late_deterrent_spares_the_theater(&game, ours));
    let kept = claim(&culture, &game);
    assert_eq!(kept.cities[&ours].queue.first(), Some(&amphitheater));

    let science = late(AdvancedAi::targeting(VictoryTarget::Science));
    assert!(!science.late_deterrent_spares_the_theater(&game, ours));
    let displaced = claim(&science, &game);
    assert_eq!(
        land_units_queued(&displaced),
        1,
        "elsewhere the building gives way"
    );
}

/// Upgrades go down to the wartime reserve while short of the target and
/// the income is positive. Off, the slinger waits for the peacetime floor.
#[test]
fn upgrades_spend_to_the_wartime_reserve_while_short() {
    let (mut game, ours, _) = fixture();
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 200.0);
    game.players[0].techs.insert(crate::name!("archery"));
    let home = game.cities[&ours].pos;
    let slinger = game.spawn_test_unit("slinger", 0, home);
    let (_, quote, _) = game
        .unit_gold_upgrade_offer(0, slinger)
        .expect("fixture: the slinger can upgrade at home");
    game.players[0].gold = WARTIME_UPGRADE_FLOOR + quote + 1.0;
    assert!(
        game.players[0].gold < PEACETIME_UPGRADE_FLOOR + quote,
        "fixture: below the peacetime reserve plus one quote"
    );
    game.players[0].gold_per_turn = 10.0;

    let mut off = game.clone();
    assert_eq!(AdvancedAi::new().late_deterrence_upgrades(&mut off, 0), 0);
    assert_eq!(off.units[&slinger].kind, "slinger");

    let ai = late(AdvancedAi::new());
    let mut broke = game.clone();
    broke.players[0].gold_per_turn = -1.0;
    assert_eq!(
        ai.late_deterrence_upgrades(&mut broke, 0),
        0,
        "no upgrade on a losing income"
    );

    let mut level = game.clone();
    power(&mut level, 0, 170.0);
    assert_eq!(
        ai.late_deterrence_upgrades(&mut level, 0),
        0,
        "at target: the reserve holds"
    );

    let mut on = game.clone();
    assert_eq!(ai.late_deterrence_upgrades(&mut on, 0), 1);
    let upgraded = on
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| on.units[uid].kind == "archer");
    assert!(upgraded.is_some(), "the slinger became an archer");
    assert!(on.players[0].gold >= WARTIME_UPGRADE_FLOOR);
}

/// Walls: with Castles known and Ancient Walls standing, the next tier is
/// claimed in the city facing the neighbour and held; off, nothing moves; one
/// at a time.
#[test]
fn the_next_wall_tier_is_claimed_facing_the_neighbour() {
    let (mut game, ours, _) = fixture();
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 200.0);
    for tech in ["masonry", "construction", "engineering", "castles"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    game.cities
        .get_mut(&ours)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    let medieval = Item::Building {
        building: crate::name!("medieval_walls"),
    };
    assert!(
        game.can_produce(0, ours, &medieval),
        "fixture: Medieval Walls are open"
    );

    let mut off = game.clone();
    let plan = AdvancedAi::new().assess(&off, 0);
    AdvancedAi::new().claim_late_deterrence_walls(&mut off, 0, &plan);
    assert!(off.cities[&ours].queue.is_empty(), "off: no walls");

    let ai = late(AdvancedAi::new());
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_late_deterrence_walls(&mut on, 0, &plan);
    assert_eq!(on.cities[&ours].queue.first(), Some(&medieval));
    assert!(ai.late_deterrence_walls_hold(&on, 0, ours, &medieval));
    assert!(!AdvancedAi::new().late_deterrence_walls_hold(&on, 0, ours, &medieval));

    // Walls in training already: no second claim, and the head is kept.
    let mut again = on.clone();
    let plan = ai.assess(&again, 0);
    ai.claim_late_deterrence_walls(&mut again, 0, &plan);
    assert_eq!(again.cities[&ours].queue, on.cities[&ours].queue);
}
