use super::*;
use crate::ai::advanced::campus_and_library_before_the_claims::tests::{
    campus_board_with, granary, head, hub_head, journalled, library, library_board, quote, unit,
    university, university_board,
};
use crate::ai::advanced::campus_and_library_before_the_claims::{
    ScienceFirstItem, ScienceFirstReach,
};
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

const TAG: &str = "science-seat-campus-before-the-claims:";

/// A Science seat with this gene alone: neither Domination gene is on, and
/// `near-done-work-finishes` is off so only this gene decides.
fn science_seat() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_campus_claims_the_free_slot_2();
    ai.disable_near_done_work_finishes();
    ai.enable_science_seat_campus_before_the_claims();
    assert!(!ai.campus_and_library_before_the_claims);
    assert!(!ai.first_campus_before_the_settler);
    ai
}

/// The Domination fixture's board (`campus_board_with`: the capital at five
/// citizens and 11 Production with a five-turn first Campus quoted and a
/// Granary at its head, the other cities at one citizen behind Walls) played
/// by a Science seat.
fn science_board(cities: usize) -> (Game, AdvancedAi, u32, StrategicPlan, Item) {
    let (g, _, cid, plan, campus) = campus_board_with(cities);
    let ai = science_seat();
    assert!(ai
        .free_slot_campus_item(&g, 0, cid, &plan)
        .is_some_and(|(item, turns)| item == campus && turns == 5.0));
    (g, ai, cid, plan, campus)
}

/// [`science_board`] with the capital training the early floor's Settler,
/// `paid` of 85 Production in, `no-city-limit` armed as on the live seat.
/// With `campus_elsewhere` the second city has its own first Campus queued,
/// so the empire's first Campus is no longer this one.
fn settler_board(
    cities: usize,
    paid: f64,
    campus_elsewhere: bool,
) -> (Game, AdvancedAi, u32, StrategicPlan, Item) {
    let (mut g, mut ai, cid, plan, campus) = science_board(cities);
    let settler = AdvancedAi::early_settler_item();
    quote(&mut g, cid, &settler, 8.0, 85.0);
    let capital = g.cities.get_mut(&cid).unwrap();
    capital.queue = vec![settler];
    capital.production = paid;
    if campus_elsewhere {
        let other = g
            .player_city_ids(0)
            .into_iter()
            .find(|other| *other != cid)
            .unwrap();
        let there = Item::District {
            district: crate::name!("campus"),
            pos: g.cities[&other].pos,
        };
        g.cities.get_mut(&other).unwrap().queue.push(there);
        assert!(AdvancedAi::empire_has_a_campus(&g, 0));
    }
    ai.enable_early_settler_floor();
    ai.enable_no_city_limit();
    assert!(ai.early_settler_floor_holds(&g, 0, cid, &plan));
    (g, ai, cid, plan, campus)
}

#[test]
fn ships_off() {
    opt_in_off_in_both_controllers("science-seat-campus-before-the-claims", |ai| {
        ai.science_seat_campus_before_the_claims
    });
}

#[test]
fn a_science_seats_first_campus_goes_ahead_of_the_granary_and_the_builder() {
    for (label, item, passed) in [
        (
            "granary",
            granary(),
            "first Campus goes ahead of the Granary",
        ),
        (
            "builder",
            unit("builder"),
            "first Campus goes ahead of the Builder",
        ),
    ] {
        let (mut g, mut ai, cid, plan, campus) = science_board(3);
        let capital = g.cities.get_mut(&cid).unwrap();
        capital.queue = vec![item.clone()];
        capital.production = 5.0;

        let mut off = ai.clone();
        off.disable_science_seat_campus_before_the_claims();
        let mut control = g.clone();
        off.claim_free_slot_campus(&mut control, 0, &plan);
        assert_eq!(head(&control, cid), Some(item.clone()), "off: {label}");

        let details = journalled(&mut ai, &mut g, |ai, g| {
            ai.claim_free_slot_campus(g, 0, &plan)
        });
        assert_eq!(head(&g, cid), Some(campus.clone()), "{label}");
        assert_eq!(g.item_invested_production(cid, &item), 5.0, "{label}");
        assert!(
            details
                .iter()
                .any(|detail| detail.starts_with(TAG) && detail.contains(passed)),
            "{label}: {details:?}"
        );
        // The Granary and Builder writers leave the claimed Campus alone.
        assert!(ai.campus_and_library_holds(&g, 0, cid, &plan, &campus));
        assert!(!off.campus_and_library_holds(&g, 0, cid, &plan, &campus));
    }
}

#[test]
fn another_lane_keeps_its_granary_with_the_gene_on() {
    let (g, armed, cid, plan, _) = science_board(3);
    for target in [
        VictoryTarget::Diplomacy,
        VictoryTarget::Culture,
        VictoryTarget::Religion,
        VictoryTarget::Domination,
        VictoryTarget::Score,
    ] {
        let mut board = g.clone();
        let mut ai = armed.clone();
        ai.victory_target = Some(target);
        assert_eq!(ai.campus_and_library_reach(&board, 0, cid, &plan), None);
        ai.claim_free_slot_campus(&mut board, 0, &plan);
        assert_eq!(head(&board, cid), Some(granary()), "{target:?}");
    }
}

#[test]
fn no_soldier_is_passed_and_the_deterrent_does_not_spare_the_city() {
    let (g, ai, cid, plan, campus) = science_board(3);
    // At peace with nothing pressing the city, the Domination gene reaches a
    // soldier and this one does not.
    assert_eq!(
        ai.campus_and_library_reach(&g, 0, cid, &plan),
        Some(ScienceFirstReach::Economy)
    );
    let (_, domination, ..) = campus_board_with(3);
    assert_eq!(
        domination.campus_and_library_reach(&g, 0, cid, &plan),
        Some(ScienceFirstReach::EconomyAndArmy)
    );
    // Both genes on a Science seat: still no soldier.
    let mut both = ai.clone();
    both.enable_campus_and_library_before_the_claims();
    both.enable_first_campus_before_the_settler();
    assert_eq!(
        both.campus_and_library_reach(&g, 0, cid, &plan),
        Some(ScienceFirstReach::Economy)
    );
    for (soldier, what) in [
        ("warrior", ScienceFirstItem::FirstCampus),
        ("archer", ScienceFirstItem::FirstCampus),
        ("catapult", ScienceFirstItem::Library),
    ] {
        assert_eq!(
            ai.campus_and_library_passes(
                &g,
                0,
                cid,
                &plan,
                &unit(soldier),
                ScienceFirstReach::Economy,
                what
            ),
            None,
            "{soldier}"
        );
    }
    // The deterrent's rule is the one it had: a first Campus at the head
    // spares the city on a Domination seat with its gene, never here.
    let mut claimed = g.clone();
    let capital = claimed.cities.get_mut(&cid).unwrap();
    capital.queue = vec![campus.clone()];
    capital.production = 0.0;
    let mut off = ai.clone();
    off.disable_science_seat_campus_before_the_claims();
    assert!(ai.campus_and_library_holds(&claimed, 0, cid, &plan, &campus));
    assert_eq!(
        ai.deterrence_spares_the_city(&claimed, 0, cid, &plan),
        off.deterrence_spares_the_city(&claimed, 0, cid, &plan)
    );
    assert!(!ai.deterrence_spares_the_city(&claimed, 0, cid, &plan));
    assert!(domination.deterrence_spares_the_city(&claimed, 0, cid, &plan));
}

#[test]
fn walls_a_holy_site_a_threat_and_a_fresh_attack_keep_the_queue() {
    let (base, ai, cid, base_plan, campus) = science_board(3);
    let center = base.cities[&cid].pos;
    let campus_site = match &campus {
        Item::District { pos, .. } => *pos,
        _ => unreachable!(),
    };
    let site = base.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .filter(|pos| *pos != center && *pos != campus_site)
        .min()
        .expect("a free tile");
    for case in ["walls", "holy site", "threat", "recent attack"] {
        let mut g = base.clone();
        let mut plan = base_plan.clone();
        let kept = match case {
            "walls" => {
                let walls = Item::Building {
                    building: crate::name!("walls"),
                };
                g.cities.get_mut(&cid).unwrap().queue = vec![walls.clone()];
                walls
            }
            // The faith defence's Holy Site with nothing paid: the Domination
            // gene passes it, this one does not.
            "holy site" => {
                let holy_site = Item::District {
                    district: crate::name!("holy_site"),
                    pos: site,
                };
                let capital = g.cities.get_mut(&cid).unwrap();
                capital.queue = vec![holy_site.clone()];
                capital.production = 0.0;
                holy_site
            }
            "threat" => {
                plan.threatened_city = Some(cid);
                granary()
            }
            _ => {
                g.cities.get_mut(&cid).unwrap().last_attacked = g.turn - 1;
                granary()
            }
        };
        ai.claim_free_slot_campus(&mut g, 0, &plan);
        assert_eq!(head(&g, cid), Some(kept), "{case}");
    }
}

#[test]
fn the_empires_first_campus_goes_ahead_of_the_floor_settler_from_two_cities() {
    let (g, mut ai, cid, plan, campus) = settler_board(3, 20.0, false);
    let settler = AdvancedAi::early_settler_item();

    let mut off = ai.clone();
    off.disable_science_seat_campus_before_the_claims();
    let mut control = g.clone();
    off.claim_free_slot_campus(&mut control, 0, &plan);
    assert_eq!(head(&control, cid), Some(settler.clone()), "off");

    let mut on = g.clone();
    let details = journalled(&mut ai, &mut on, |ai, g| {
        ai.claim_free_slot_campus(g, 0, &plan)
    });
    assert_eq!(head(&on, cid), Some(campus));
    assert_eq!(on.item_invested_production(cid, &settler), 20.0);
    assert!(
        details
            .iter()
            .any(|detail| detail.starts_with(TAG) && detail.contains("3 cities standing")),
        "{details:?}"
    );
    // Both Settler claims leave the committed Campus alone.
    assert!(ai.science_campus_holds_repeat_settler(&on, 0, cid, &plan));
    assert!(!off.science_campus_holds_repeat_settler(&on, 0, cid, &plan));

    // A war with a major keeps the Settler first.
    let mut war = g.clone();
    war.at_war.insert((0, 1));
    ai.claim_free_slot_campus(&mut war, 0, &plan);
    assert_eq!(head(&war, cid), Some(settler), "at war");
}

#[test]
fn a_later_first_campus_never_passes_a_settler() {
    // Another city already has its Campus queued, so this is not the
    // empire's first: the Settler keeps its queue at any city count, where
    // the Domination gene passes it from six.
    let settler = AdvancedAi::early_settler_item();
    for cities in [4, 6] {
        let (mut g, ai, cid, plan, _) = settler_board(cities, 0.0, true);
        ai.claim_free_slot_campus(&mut g, 0, &plan);
        assert_eq!(head(&g, cid), Some(settler.clone()), "{cities} cities");
    }
}

#[test]
fn a_slow_first_campus_waits_for_the_granary() {
    // Seven Online turns pass the Granary; eight do not, and the Granary
    // and Builder writers are not held off a slow Campus either.
    for (turns, passes) in [(7.0, true), (8.0, false)] {
        let (mut g, ai, cid, plan, campus) = science_board(3);
        quote(&mut g, cid, &campus, turns, 56.0);
        assert_eq!(
            AdvancedAi::science_seat_campus_is_quick(&g, 0, cid, &campus),
            passes
        );
        let mut claimed = g.clone();
        let capital = claimed.cities.get_mut(&cid).unwrap();
        capital.queue = vec![campus.clone()];
        capital.production = 0.0;
        assert_eq!(
            ai.campus_and_library_holds(&claimed, 0, cid, &plan, &campus),
            passes,
            "{turns} turns, nothing paid"
        );
        ai.claim_free_slot_campus(&mut g, 0, &plan);
        let expected = if passes { campus } else { granary() };
        assert_eq!(head(&g, cid), Some(expected), "{turns} turns");
    }
}

#[test]
fn the_library_and_the_university_keep_the_building_claims_own_rules() {
    // The Domination gene puts every Library ahead of a Commercial Hub with
    // progress; on a Science seat this gene changes nothing there.
    let (g, ranked, _, plan) = library_board();
    let mut ai = science_seat();
    ai.enable_campus_building_claims_the_queue();
    let mut off = ai.clone();
    off.disable_science_seat_campus_before_the_claims();
    let mut control = g.clone();
    off.claim_campus_building(&mut control, 0, &plan);
    let mut on = g.clone();
    ai.claim_campus_building(&mut on, 0, &plan);
    for cid in &ranked {
        assert!(hub_head(&control, *cid), "off: the Hub keeps its queue");
        assert!(hub_head(&on, *cid), "on: so it does");
        assert!(!ai.campus_and_library_holds(&on, 0, *cid, &plan, &library()));
    }
    // With the building gene off, this gene opens no building claim.
    let alone = science_seat();
    assert!(alone
        .campus_building_claim_item(&g, 0, ranked[0], &plan)
        .is_none());

    let (g, ranked, domination, plan) = university_board();
    let turns = g.item_cost_for_city(0, ranked[0], &university()) / 15.0;
    assert!(domination.campus_and_library_puts_first(&g, 0, 3.0));
    assert!(domination.campus_and_library_puts_first(&g, 1, turns));
    assert!(!ai.campus_and_library_puts_first(&g, 0, 3.0));
    assert!(!ai.campus_and_library_puts_first(&g, 1, turns));
    assert!(alone
        .campus_building_claim_item(&g, 0, ranked[0], &plan)
        .is_none());
}

fn plaza_head(g: &Game, cid: u32) -> bool {
    matches!(head(g, cid), Some(Item::District { district, .. })
        if g.district_family(district) == "government_plaza")
}

#[test]
fn the_plaza_leaves_a_city_its_last_slot_until_its_campus() {
    let (mut g, mut ai, cid, plan, campus) = science_board(3);
    g.players[0].civics.insert(crate::name!("state_workforce"));
    g.cities.get_mut(&cid).unwrap().is_capital = true;
    let others: Vec<u32> = g
        .player_city_ids(0)
        .into_iter()
        .filter(|other| *other != cid)
        .collect();
    for other in &others {
        let city = g.cities.get_mut(other).unwrap();
        city.is_capital = false;
        city.queue = vec![unit("builder")];
    }
    ai.enable_plaza_serves_the_scientist_3();
    let mut off = ai.clone();
    off.disable_science_seat_campus_before_the_claims();

    // The capital's five citizens leave a slot beside the Plaza; a city of
    // one citizen has only the one.
    assert!(!ai.plaza_would_take_the_campus_slot(&g, 0, cid));
    assert!(ai.plaza_would_take_the_campus_slot(&g, 0, others[0]));
    assert!(!off.plaza_would_take_the_campus_slot(&g, 0, others[0]));
    let mut diplomat = ai.clone();
    diplomat.victory_target = Some(VictoryTarget::Diplomacy);
    assert!(!diplomat.plaza_would_take_the_campus_slot(&g, 0, others[0]));
    let mut queued = g.clone();
    let there = Item::District {
        district: crate::name!("campus"),
        pos: queued.cities[&others[0]].pos,
    };
    queued.cities.get_mut(&others[0]).unwrap().queue.push(there);
    assert!(
        !ai.plaza_would_take_the_campus_slot(&queued, 0, others[0]),
        "its Campus is queued"
    );

    // `plaza-serves-the-scientist-3`: the capital has no Campus yet, so the
    // Plaza claim turns to the other cities.
    let mut control = g.clone();
    off.claim_queue_for_the_plaza_district(&mut control, 0, &plan);
    assert!(
        others.iter().any(|other| plaza_head(&control, *other)),
        "off: a one-citizen city's only slot goes to the Plaza"
    );
    let mut on = g.clone();
    ai.claim_queue_for_the_plaza_district(&mut on, 0, &plan);
    assert!(
        others
            .iter()
            .all(|other| head(&on, *other) == Some(unit("builder"))),
        "on: every one-slot city keeps it"
    );
    assert!(!plaza_head(&on, cid), "the capital's Campus comes first");

    // A city with a second slot may hold the Plaza.
    let mut grown = g.clone();
    grown.cities.get_mut(&others[0]).unwrap().pop = 4;
    assert!(!ai.plaza_would_take_the_campus_slot(&grown, 0, others[0]));
    ai.claim_queue_for_the_plaza_district(&mut grown, 0, &plan);
    assert!(plaza_head(&grown, others[0]));

    // `plaza-in-the-district-list`: while the capital builds its Campus the
    // Plaza goes to the idle city that produces most, unless that is its
    // last slot.
    let mut generic = science_seat();
    generic.enable_plaza_in_the_district_list();
    generic.enable_capital_campus_before_the_plaza();
    generic.base.w.city_target = 1.0;
    let mut board = g.clone();
    board.cities.get_mut(&cid).unwrap().queue = vec![campus];
    for other in &others {
        board.cities.get_mut(other).unwrap().queue.clear();
    }
    let mut generic_off = generic.clone();
    generic_off.disable_science_seat_campus_before_the_claims();
    let mut control = board.clone();
    generic_off.reserve_government_plaza(&mut control, 0, &plan);
    assert!(
        others.iter().any(|other| plaza_head(&control, *other)),
        "off: the idle one-citizen city takes the Plaza"
    );
    let mut on = board.clone();
    generic.reserve_government_plaza(&mut on, 0, &plan);
    assert!(
        others.iter().all(|other| head(&on, *other).is_none()),
        "on: the idle one-slot cities keep their slot"
    );
}
