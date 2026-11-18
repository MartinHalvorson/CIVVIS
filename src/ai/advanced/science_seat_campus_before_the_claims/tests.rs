use super::*;
use crate::ai::advanced::campus_and_library_before_the_claims::tests::{
    campus_board_with, granary, head, hub_head, journalled, library, library_board, quote, unit,
    university_board,
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
    claimed.cities.get_mut(&cid).unwrap().queue = vec![campus.clone()];
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
fn a_later_first_campus_waits_for_the_settler_below_the_city_count() {
    // Another city already has its Campus queued, so this is not the
    // empire's first: the count decides.
    let floor = SCIENCE_SEAT_CAMPUS_SETTLER_MIN_CITIES;
    let settler = AdvancedAi::early_settler_item();

    let (mut below, ai, cid, plan, _) = settler_board(floor - 1, 0.0, true);
    ai.claim_free_slot_campus(&mut below, 0, &plan);
    assert_eq!(head(&below, cid), Some(settler.clone()), "below the count");

    let (mut at, mut ai, cid, plan, campus) = settler_board(floor, 0.0, true);
    let details = journalled(&mut ai, &mut at, |ai, g| {
        ai.claim_free_slot_campus(g, 0, &plan)
    });
    assert_eq!(head(&at, cid), Some(campus), "at the count");
    assert!(
        details.iter().any(|detail| detail.starts_with(TAG)
            && detail.contains("first Campus goes ahead of the Settler (0 of")),
        "{details:?}"
    );

    // More than half paid: the Settler finishes.
    let (mut paid, ai, cid, plan, _) = settler_board(floor, 60.0, true);
    ai.claim_free_slot_campus(&mut paid, 0, &plan);
    assert_eq!(head(&paid, cid), Some(settler), "more than half paid");
}

#[test]
fn every_library_goes_ahead_of_a_commercial_hub_on_a_science_seat() {
    let (g, ranked, _, plan) = library_board();
    let mut ai = science_seat();
    ai.enable_campus_building_claims_the_queue();

    let mut off = ai.clone();
    off.disable_science_seat_campus_before_the_claims();
    let mut control = g.clone();
    off.claim_campus_building(&mut control, 0, &plan);
    assert!(
        ranked.iter().all(|cid| hub_head(&control, *cid)),
        "off: the Hub with progress keeps every queue"
    );

    let mut on = g.clone();
    let details = journalled(&mut ai, &mut on, |ai, g| {
        ai.claim_campus_building(g, 0, &plan)
    });
    for cid in &ranked {
        assert_eq!(head(&on, *cid), Some(library()));
        assert!(ai.campus_and_library_holds(&on, 0, *cid, &plan, &library()));
    }
    let passed = details
        .iter()
        .filter(|detail| {
            detail.starts_with(TAG) && detail.contains("Library goes ahead of the Commercial Hub")
        })
        .count();
    assert_eq!(passed, ranked.len(), "{details:?}");

    // The gene opens the Library claim with the building gene off too.
    let mut alone = science_seat();
    let mut board = g.clone();
    alone.claim_campus_building(&mut board, 0, &plan);
    assert!(ranked
        .iter()
        .all(|cid| head(&board, *cid) == Some(library())));
    alone.disable_science_seat_campus_before_the_claims();
    assert!(alone
        .campus_building_claim_item(&g, 0, ranked[0], &plan)
        .is_none());
}

#[test]
fn the_university_keeps_the_building_claims_own_rules() {
    let (g, ranked, domination, plan) = university_board();
    let cid = ranked[0];
    let turns = g.item_cost_for_city(
        0,
        cid,
        &crate::ai::advanced::campus_and_library_before_the_claims::tests::university(),
    ) / 15.0;
    assert!(domination.campus_and_library_puts_first(&g, 1, turns));
    let ai = science_seat();
    assert!(ai.campus_and_library_puts_first(&g, 0, 3.0), "the Library");
    assert!(
        !ai.campus_and_library_puts_first(&g, 1, turns),
        "a quick University is not this gene's"
    );
    // With the building gene off, this gene alone claims no University.
    assert!(ai.campus_building_claim_item(&g, 0, cid, &plan).is_none());
}
