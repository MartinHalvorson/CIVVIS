use super::*;

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn plan(turn: u32) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: turn,
        rush: false,
    }
}

/// A Diplomatic seat whose controller rescores committed queues, the way the
/// live seat does.
fn diplomat(contact_fleet: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    if contact_fleet {
        ai.enable_contact_fleet();
    }
    ai
}

/// Two majors and no city-states; player 0 holds one coastal city that can
/// launch a ship into uncharted water and has not met player 1, who holds no
/// city. It knows Cartography (Ocean; the Galley retires behind the Caravel,
/// so the cheapest hull is the Quadrireme) and Mercantilism (a Privateer
/// outguns a Quadrireme, so the governor vetoes the Quadrireme at -2000, as
/// the Barbary Corsair did to 003455Z's t84 Quadrireme). Every starting unit
/// is gone and the turn is past the opening.
fn coastal_seat() -> (Game, u32) {
    let mut refusals = Vec::new();
    for seed in 0..60u64 {
        let mut game = Game::new_full(2, 40, 24, 431_700 + seed, 250, 0, false);
        for uid in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(uid);
        }
        game.current = 0;
        for tech in [
            "sailing",
            "shipbuilding",
            "celestial_navigation",
            "cartography",
        ] {
            game.players[0].techs.insert(Name::new(tech));
        }
        game.players[0].civics.insert(Name::new("mercantilism"));
        game.players[0].met.remove(&1);
        game.players[1].met.remove(&0);
        let site = game
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && tile.owner_city.is_none()
                    && game.nbrs(**pos).into_iter().any(|next| {
                        game.map
                            .get(next)
                            .is_some_and(|tile| game.rules.is_water(tile))
                    })
            })
            .map(|(pos, _)| *pos)
            .min();
        let Some(site) = site else {
            refusals.push(format!("{seed}: no coastal site"));
            continue;
        };
        let city = game.place_city(0, site, None);
        game.turn = 80;
        let checks = [
            (
                "launch",
                BasicAi::city_has_naval_recon_launch(&game, 0, city),
            ),
            ("quadrireme", game.can_produce(0, city, &unit("quadrireme"))),
            ("caravel", game.can_produce(0, city, &unit("caravel"))),
            ("privateer", game.can_produce(0, city, &unit("privateer"))),
            ("unmet", !game.has_met(0, 1)),
            ("alive", game.players[1].alive),
        ];
        if checks.iter().all(|(_, ok)| *ok) {
            return (game, city);
        }
        refusals.push(format!(
            "{seed}: {:?}",
            checks
                .iter()
                .filter(|(_, ok)| !ok)
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
        ));
    }
    panic!("fixture: no seed gave a coastal city with uncharted water: {refusals:?}");
}

/// Meet player 1 and see one of its cities: with no city-states on the map
/// the contact gap closes.
fn close_the_gap(game: &mut Game, home: u32) {
    let home = game.cities[&home].pos;
    let site = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && tile.owner_city.is_none()
                && game.wdist(**pos, home) >= 8
        })
        .map(|(pos, _)| *pos)
        .min()
        .expect("fixture: room for a rival city");
    game.place_city(1, site, None);
    game.players[0].met.insert(1);
    game.players[0].explored.insert(site);
}

fn queue(game: &mut Game, city: u32, item: &Item) {
    game.apply(
        0,
        &Action::Produce {
            city,
            item: item.clone(),
        },
    )
    .expect("fixture: the city can build it");
}

/// ★ The 003455Z shape: the sea scout the reservation laid down is worth
/// less than anything else on the menu (here, as at t84, a Quadrireme beside
/// a better-gunned raider is vetoed outright), so the governor's rescoring
/// replaced it the same turn. Under the gene it holds while the contact gap
/// is open — and only then: a met, located rival with no city-state left
/// unmet, or a charting ship of ours afloat, hands the queue back.
#[test]
fn the_queued_sea_scout_holds_only_while_the_contact_gap_is_open() {
    let (mut game, city) = coastal_seat();
    let hull = unit("quadrireme");
    queue(&mut game, city, &hull);
    let plan = plan(game.turn);
    assert!(
        BasicAi::contact_fleet_gap(&game, 0),
        "fixture: player 1 is unmet"
    );
    assert_eq!(BasicAi::contact_fleet_missing_majors(&game, 0), (1, 0));
    assert!(
        diplomat(true).base.naval_recon_on(),
        "fixture: the naval eye is live"
    );

    let mut off = game.clone();
    diplomat(false).advanced_production(&mut off, 0, &plan, false);
    assert_ne!(
        off.cities[&city].queue.first(),
        Some(&hull),
        "fixture: without the gene the rescoring replaces the hull"
    );

    let mut on = game.clone();
    let mut armed = diplomat(true);
    assert!(armed.naval_eye_holds(&on, 0, city, &plan, &hull));
    armed.advanced_production(&mut on, 0, &plan, false);
    assert_eq!(
        on.cities[&city].queue.first(),
        Some(&hull),
        "the gene keeps the sea scout against a higher-scored item"
    );

    // The gap closes: player 1 met and one of its cities seen.
    let mut closed = game.clone();
    close_the_gap(&mut closed, city);
    assert!(!BasicAi::contact_fleet_gap(&closed, 0));
    let mut armed = diplomat(true);
    assert!(!armed.naval_eye_holds(&closed, 0, city, &plan, &hull));
    armed.advanced_production(&mut closed, 0, &plan, false);
    assert_ne!(
        closed.cities[&city].queue.first(),
        Some(&hull),
        "with the gap closed the rescoring has the queue back"
    );

    // Met but not located keeps the gap open.
    let mut unlocated = game.clone();
    unlocated.players[0].met.insert(1);
    let rival_home = {
        let mut located = game.clone();
        close_the_gap(&mut located, city);
        located
            .cities
            .values()
            .find(|other| other.owner == 1)
            .map(|other| other.pos)
            .expect("fixture: a rival city")
    };
    unlocated.place_city(1, rival_home, None);
    assert!(!unlocated.players[0].explored.contains(&rival_home));
    assert_eq!(BasicAi::contact_fleet_missing_majors(&unlocated, 0), (0, 1));
    assert!(BasicAi::contact_fleet_gap(&unlocated, 0));

    // A charting ship of ours afloat is the eye: the queue is free again.
    let shore = game
        .nbrs(game.cities[&city].pos)
        .into_iter()
        .find(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| game.rules.is_water(tile))
        })
        .expect("fixture: the city touches water");
    let mut afloat = game.clone();
    let ship = afloat.spawn_test_unit("galley", 0, shore);
    if BasicAi::naval_recon_ship_can_chart(&afloat, 0, ship) {
        assert!(!diplomat(true).naval_eye_holds(&afloat, 0, city, &plan, &hull));
    }

    // A threatened city keeps no hold.
    let mut threatened = plan.clone();
    threatened.threatened_city = Some(city);
    assert!(!diplomat(true).naval_eye_holds(&game, 0, city, &threatened, &hull));
    // Nor does anything but a sea unit.
    assert!(!diplomat(true).naval_eye_holds(&game, 0, city, &plan, &unit("warrior")));
}

/// The faster hull: among hulls costing up to twice the cheapest, the most
/// moves, when the city finishes it inside the window; otherwise, and with
/// the gap closed or the gene off, the stock cheapest hull. The live menu of
/// 003455Z at t75 offered no Galley after Cartography, so the cheapest hull
/// was the Quadrireme and the Caravel cost exactly twice it; the native
/// fixture's Cartography retires the Galley behind the Caravel the same way.
#[test]
fn the_faster_hull_is_chosen_only_inside_its_window() {
    let (mut game, city) = coastal_seat();
    let stock = diplomat(false);
    let armed = diplomat(true);
    let cheapest = stock
        .base
        .best_naval_recon(&game, 0, city)
        .expect("fixture: a hull");
    assert_eq!(
        cheapest, "quadrireme",
        "fixture: the live t75 menu's cheapest hull"
    );
    // A young city cannot finish a Caravel inside the window.
    assert_eq!(armed.base.contact_fleet_fast_hull(&game, 0, city), None);
    assert_eq!(
        armed.base.best_naval_recon(&game, 0, city),
        Some(cheapest.clone())
    );

    // With the Caravel all but paid, it is the faster hull in reach.
    let caravel = unit("caravel");
    let cost = game.item_cost_for_city(0, city, &caravel);
    game.cities
        .get_mut(&city)
        .unwrap()
        .production_progress
        .insert(Game::item_progress_key(&caravel), (cost - 1.0).max(0.0));
    let fast = armed
        .base
        .contact_fleet_fast_hull(&game, 0, city)
        .expect("a paid-up faster hull is in reach");
    let moves = |name: &str| game.rules.units[name].moves;
    let cost_of = |name: &str| game.rules.units[name].cost;
    let floor = cost_of(&cheapest);
    for (name, spec) in &game.rules.units {
        if spec.class == "military"
            && spec.domain.as_deref() == Some("sea")
            && spec.cost <= 2.0 * floor
            && game.can_produce(0, city, &Item::Unit { unit: *name })
        {
            assert!(
                moves(&fast) >= spec.moves,
                "{fast} must have the most moves of the affordable hulls, not fewer than {name}"
            );
        }
    }
    assert!(
        moves(&fast) > moves(&cheapest),
        "{fast} is faster than {cheapest}"
    );
    assert_eq!(armed.base.best_naval_recon(&game, 0, city), Some(fast));
    assert_eq!(
        stock.base.best_naval_recon(&game, 0, city),
        Some(cheapest.clone()),
        "off: the stock cheapest hull"
    );

    // The gap closed: back to the cheapest.
    close_the_gap(&mut game, city);
    assert_eq!(armed.base.contact_fleet_fast_hull(&game, 0, city), None);
    assert_eq!(armed.base.best_naval_recon(&game, 0, city), Some(cheapest));
}

/// The gene is registered, opt-in, off in every shipped constructor, and its
/// toggles reach the `BasicAi` flag.
#[test]
fn contact_fleet_is_opt_in_and_off_by_default() {
    let gene = crate::ai::advanced::gene("contact-fleet").expect("registered");
    assert_eq!(gene.field, "contact_fleet");
    assert!(matches!(gene.kind, crate::ai::advanced::genes::Kind::OptIn));
    assert!(!AdvancedAi::new().base.contact_fleet);
    assert!(!AdvancedAi::legacy().base.contact_fleet);
    assert!(
        !AdvancedAi::targeting(VictoryTarget::Diplomacy)
            .base
            .contact_fleet
    );
    let mut ai = AdvancedAi::new();
    ai.enable_contact_fleet();
    assert!(ai.base.contact_fleet);
    ai.disable_contact_fleet();
    assert!(!ai.base.contact_fleet);
}

/// A seat whose controller has both `contact-fleet` and the claim on.
fn claimant() -> AdvancedAi {
    let mut ai = diplomat(true);
    ai.enable_sea_scout_claims_the_queue();
    ai
}

/// The fixture's city with `head` queued and the Quadrireme all but paid, so
/// that it can finish the hull inside the claim's window.
fn city_working_on(head: Item) -> (Game, u32) {
    let (mut game, city) = coastal_seat();
    let hull = unit("quadrireme");
    let cost = game.item_cost_for_city(0, city, &hull);
    let held = game.cities.get_mut(&city).unwrap();
    held.queue = vec![head];
    held.production_progress
        .insert(Game::item_progress_key(&hull), (cost - 1.0).max(0.0));
    (game, city)
}

/// The fixture's city with a Library at the head of its queue.
fn working_city() -> (Game, u32) {
    city_working_on(Item::Building {
        building: Name::new("library"),
    })
}

fn head_is_a_ship(game: &Game, city: u32) -> bool {
    matches!(game.cities[&city].queue.first(), Some(Item::Unit { unit })
        if game.rules.units[unit.as_str()].domain.as_deref() == Some("sea"))
}

#[test]
fn the_sea_scout_claims_a_working_queue_only_with_the_gene() {
    let (mut game, city) = working_city();
    diplomat(true).claim_queue_for_the_sea_scout(&mut game, 0, &plan(80));
    assert!(!head_is_a_ship(&game, city), "off: the queue is untouched");
    assert!(!game.players[0].counters.contains_key("sea_scout_claims"));

    claimant().claim_queue_for_the_sea_scout(&mut game, 0, &plan(80));
    assert!(head_is_a_ship(&game, city), "on: the hull takes the head");
    assert_eq!(game.players[0].counters["sea_scout_claims"], 1);
}

#[test]
fn the_claim_stands_alone_once_a_hull_is_queued() {
    let (mut game, city) = working_city();
    let mut ai = claimant();
    ai.claim_queue_for_the_sea_scout(&mut game, 0, &plan(80));
    let queued = game.cities[&city].queue.clone();
    ai.claim_queue_for_the_sea_scout(&mut game, 0, &plan(80));
    assert_eq!(game.cities[&city].queue, queued);
    assert_eq!(game.players[0].counters["sea_scout_claims"], 1);
}

#[test]
fn no_claim_before_a_hull_can_cross_ocean() {
    let (mut game, city) = working_city();
    game.players[0].techs.remove(&Name::new("cartography"));
    claimant().claim_queue_for_the_sea_scout(&mut game, 0, &plan(80));
    assert!(!head_is_a_ship(&game, city));
}

#[test]
fn no_claim_once_the_contact_gap_closes() {
    let (mut game, city) = working_city();
    close_the_gap(&mut game, city);
    claimant().claim_queue_for_the_sea_scout(&mut game, 0, &plan(80));
    assert!(!head_is_a_ship(&game, city));
}

#[test]
fn a_threatened_city_keeps_its_queue() {
    let (mut game, city) = working_city();
    let mut threatened = plan(80);
    threatened.threatened_city = Some(city);
    claimant().claim_queue_for_the_sea_scout(&mut game, 0, &threatened);
    assert!(!head_is_a_ship(&game, city));
}

#[test]
fn a_wonder_or_a_soldier_keeps_the_head() {
    for wonder in [true, false] {
        let (mut game, city) = working_city();
        let head = if wonder {
            Item::Wonder {
                wonder: Name::new("great_library"),
                pos: game.cities[&city].pos,
            }
        } else {
            unit("warrior")
        };
        game.cities.get_mut(&city).unwrap().queue = vec![head.clone()];
        claimant().claim_queue_for_the_sea_scout(&mut game, 0, &plan(80));
        assert_eq!(game.cities[&city].queue.first(), Some(&head));
    }
}

#[test]
fn a_city_too_slow_to_finish_the_hull_keeps_its_queue() {
    let (mut game, city) = coastal_seat();
    game.cities.get_mut(&city).unwrap().queue = vec![Item::Building {
        building: Name::new("library"),
    }];
    claimant().claim_queue_for_the_sea_scout(&mut game, 0, &plan(80));
    assert!(!head_is_a_ship(&game, city));
    assert!(!game.players[0].counters.contains_key("sea_scout_claims"));
}

/// A seat with `contact-fleet` and version 2 of the claim on.
fn careful_claimant() -> AdvancedAi {
    let mut ai = diplomat(true);
    ai.enable_sea_scout_claims_the_queue_2();
    ai
}

#[test]
fn version_two_leaves_other_claims_work_alone() {
    let library = Item::Building {
        building: Name::new("library"),
    };
    let agency = Item::Building {
        building: Name::new("intelligence_agency"),
    };
    for head in [library, agency.clone(), unit("builder")] {
        let (mut game, city) = city_working_on(head.clone());
        careful_claimant().claim_queue_for_the_sea_scout(&mut game, 0, &plan(80));
        assert_eq!(game.cities[&city].queue.first(), Some(&head), "{head:?}");
    }
    // Version 1 took the Agency, as it did in 015754Z.
    let (mut game, city) = city_working_on(agency);
    claimant().claim_queue_for_the_sea_scout(&mut game, 0, &plan(80));
    assert!(head_is_a_ship(&game, city));
}

#[test]
fn version_two_still_takes_routine_work() {
    let (mut game, city) = city_working_on(Item::Building {
        building: Name::new("monument"),
    });
    careful_claimant().claim_queue_for_the_sea_scout(&mut game, 0, &plan(80));
    assert!(head_is_a_ship(&game, city));
}

#[test]
fn version_two_stands_off_a_city_that_took_the_hull_back() {
    let routine = Item::Building {
        building: Name::new("monument"),
    };
    let (mut game, city) = city_working_on(routine.clone());
    let mut careful = careful_claimant();
    careful.claim_queue_for_the_sea_scout(&mut game, 0, &plan(80));
    assert!(head_is_a_ship(&game, city));

    // Another claim retakes the head; the hull leaves the queue, as in 015754Z.
    // Starting it moved its banked progress, so pay it again: the stand-off must
    // be why it is not reclaimed, not a hull too slow to start.
    let hull = unit("quadrireme");
    let cost = game.item_cost_for_city(0, city, &hull);
    let held = game.cities.get_mut(&city).unwrap();
    held.queue = vec![routine.clone()];
    held.production_progress
        .insert(Game::item_progress_key(&hull), (cost - 1.0).max(0.0));
    careful.claim_queue_for_the_sea_scout(&mut game, 0, &plan(80));
    game.turn += 1;
    careful.claim_queue_for_the_sea_scout(&mut game, 0, &plan(81));
    assert_eq!(game.cities[&city].queue.first(), Some(&routine));
    assert_eq!(game.players[0].counters["sea_scout_claims"], 1);

    // Version 1, on the same board, fights for it every frame.
    claimant().claim_queue_for_the_sea_scout(&mut game, 0, &plan(81));
    assert!(head_is_a_ship(&game, city));
}

/// ★ The 100022Z shape: the hull the sea scout's claim laid down lost the
/// head to other queue claims 15 times and floated 18 turns after
/// Cartography. A Religion founder under `religion-seat-charts-the-rivals`
/// keeps the committed hull at the head; any other seat still yields it.
#[test]
fn a_religion_founders_hull_keeps_the_head_from_other_claims() {
    let (mut game, city) = coastal_seat();
    let hull = unit("quadrireme");
    let granary = Item::Building {
        building: Name::new("granary"),
    };
    game.players[0].techs.insert(Name::new("pottery"));
    game.players[0].religion = Some("Buddhism".to_string());
    let housing = game.city_housing(&game.cities[&city]);
    {
        let held = game.cities.get_mut(&city).unwrap();
        held.pop = (housing.ceil() as i32).max(2);
        held.production_progress
            .insert(Game::item_progress_key(&granary), 50.0);
    }
    queue(&mut game, city, &hull);
    let plan = plan(game.turn);
    let founder = |charts: bool| {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
        ai.enable_contact_fleet();
        ai.enable_granary_claims_the_bound_queue();
        if charts {
            ai.enable_religion_seat_charts_the_rivals();
        }
        ai
    };
    assert!(founder(true).charting_hull_holds(&game, 0, city, &plan, &hull));
    assert!(!founder(false).charting_hull_holds(&game, 0, city, &plan, &hull));
    assert!(
        !founder(true).charting_hull_holds(&game, 0, city, &plan, &granary),
        "only the hull holds"
    );
    let mut other = diplomat(true);
    other.enable_religion_seat_charts_the_rivals();
    assert!(
        !other.charting_hull_holds(&game, 0, city, &plan, &hull),
        "another aim keeps the old order"
    );

    let mut yielded = game.clone();
    founder(false).claim_bound_granaries(&mut yielded, 0, &plan);
    assert_eq!(
        yielded.cities[&city].queue.first(),
        Some(&granary),
        "fixture: without the gene the Granary claim takes the head"
    );

    let mut kept = game.clone();
    founder(true).claim_bound_granaries(&mut kept, 0, &plan);
    assert_eq!(kept.cities[&city].queue.first(), Some(&hull));
}

/// ★ The 123943Z shape: the one ship afloat spent t77-133 in sea task forces
/// and never left home waters while two rivals stood unmet. A Religion
/// founder's first charting ship is left off the objective board; a second
/// ship, another seat's ship, and a ship with the gap closed are not.
#[test]
fn a_religion_founders_charting_ship_is_left_off_the_board() {
    let (mut game, city) = coastal_seat();
    game.players[0].religion = Some("Buddhism".to_string());
    let mut shores = game
        .nbrs(game.cities[&city].pos)
        .into_iter()
        .filter(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| game.rules.is_water(tile))
        })
        .collect::<Vec<_>>();
    shores.sort();
    let first = game.spawn_test_unit("galley", 0, shores[0]);
    let founder = |charts: bool| {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
        ai.enable_contact_fleet();
        if charts {
            ai.enable_religion_seat_charts_the_rivals();
        }
        ai
    };
    if !BasicAi::naval_recon_ship_can_chart(&game, 0, first) {
        // A fixture sea with nothing to chart has no charting ship.
        assert_eq!(founder(true).religion_seat_charting_ship(&game, 0), None);
        return;
    }
    assert_eq!(
        founder(true).religion_seat_charting_ship(&game, 0),
        Some(first)
    );
    assert!(!founder(true).board_pool(&game, 0).contains(&first));
    assert!(founder(false).board_pool(&game, 0).contains(&first));
    assert_eq!(founder(false).religion_seat_charting_ship(&game, 0), None);
    let mut other = diplomat(true);
    other.enable_religion_seat_charts_the_rivals();
    assert!(other.board_pool(&game, 0).contains(&first));

    // One eye: a second hull is a soldier.
    if let Some(berth) = shores.get(1).copied() {
        let second = game.spawn_test_unit("galley", 0, berth);
        let pool = founder(true).board_pool(&game, 0);
        assert!(!pool.contains(&first));
        assert!(pool.contains(&second));
    }

    // With every rival met and charted the ship is a soldier again.
    let mut closed = game.clone();
    close_the_gap(&mut closed, city);
    if !founder(true).base.contact_gap_open(&closed, 0) {
        assert_eq!(founder(true).religion_seat_charting_ship(&closed, 0), None);
        assert!(founder(true).board_pool(&closed, 0).contains(&first));
    }
}
