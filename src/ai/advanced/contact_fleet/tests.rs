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
/// launch a ship into uncharted water, knows Cartography (so a Caravel
/// outclasses a Galley), and has not met player 1, who holds no city. Every
/// starting unit is gone and the turn is past the opening.
fn coastal_seat() -> (Game, u32) {
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
            continue;
        };
        let city = game.place_city(0, site, None);
        game.turn = 80;
        if BasicAi::city_has_naval_recon_launch(&game, 0, city)
            && game.can_produce(0, city, &unit("galley"))
            && game.can_produce(0, city, &unit("caravel"))
            && !game.has_met(0, 1)
        {
            return (game, city);
        }
    }
    panic!("fixture: no seed gave a coastal city with uncharted water");
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
/// less than anything else on the menu (a Galley beside a Caravel is vetoed
/// outright), so the governor's rescoring replaced it the same turn. Under
/// the gene it holds while the contact gap is open — and only then: a met,
/// located rival with no city-state left unmet, or a charting ship of ours
/// afloat, hands the queue back to the rescoring.
#[test]
fn the_queued_sea_scout_holds_only_while_the_contact_gap_is_open() {
    let (mut game, city) = coastal_seat();
    let galley = unit("galley");
    queue(&mut game, city, &galley);
    let plan = plan(game.turn);
    assert!(
        BasicAi::contact_fleet_gap(&game, 0),
        "fixture: player 1 is unmet"
    );
    assert!(
        diplomat(true).base.naval_recon_on(),
        "fixture: the naval eye is live"
    );

    let mut off = game.clone();
    diplomat(false).advanced_production(&mut off, 0, &plan, false);
    assert_ne!(
        off.cities[&city].queue.first(),
        Some(&galley),
        "fixture: without the gene the rescoring replaces the hull"
    );

    let mut on = game.clone();
    let mut armed = diplomat(true);
    assert!(armed.naval_eye_holds(&on, 0, city, &plan, &galley));
    armed.advanced_production(&mut on, 0, &plan, false);
    assert_eq!(
        on.cities[&city].queue.first(),
        Some(&galley),
        "the gene keeps the sea scout against a higher-scored item"
    );

    // The gap closes: player 1 met and one of its cities seen.
    let mut closed = game.clone();
    close_the_gap(&mut closed, city);
    assert!(!BasicAi::contact_fleet_gap(&closed, 0));
    let mut armed = diplomat(true);
    assert!(!armed.naval_eye_holds(&closed, 0, city, &plan, &galley));
    armed.advanced_production(&mut closed, 0, &plan, false);
    assert_ne!(
        closed.cities[&city].queue.first(),
        Some(&galley),
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
        assert!(!diplomat(true).naval_eye_holds(&afloat, 0, city, &plan, &galley));
    }

    // A threatened city keeps no hold.
    let mut threatened = plan.clone();
    threatened.threatened_city = Some(city);
    assert!(!diplomat(true).naval_eye_holds(&game, 0, city, &threatened, &galley));
    // Nor does anything but a sea unit.
    assert!(!diplomat(true).naval_eye_holds(&game, 0, city, &plan, &unit("warrior")));
}

/// The faster hull: among hulls costing up to twice the cheapest, the most
/// moves, when the city finishes it inside the window; otherwise, and with
/// the gap closed or the gene off, the stock cheapest hull. The live menu of
/// 003455Z at t75 offered no Galley after Cartography, so the cheapest hull
/// was the Quadrireme and the Caravel cost exactly twice it; the native
/// ruleset keeps the Galley to Steam Power, so the fixture withdraws it.
#[test]
fn the_faster_hull_is_chosen_only_inside_its_window() {
    let (mut game, city) = coastal_seat();
    std::sync::Arc::make_mut(&mut game.rules)
        .units
        .get_mut("galley")
        .expect("the Galley row")
        .buildable = false;
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
