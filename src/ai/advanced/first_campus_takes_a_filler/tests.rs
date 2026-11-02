use super::*;
use crate::ai::advanced::GrandStrategy;

/// Three cities of ours at population 7 with Writing known, every queue
/// holding only `head`, and a peaceful Expansion plan.
fn board(head: Option<Item>) -> (Game, StrategicPlan) {
    let mut g = Game::new_full(
        2,
        24,
        16,
        crate::rng::fixture_seed("FIRSTCAMPUSFILLER", 91_771),
        250,
        0,
        false,
    );
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = g.player_city_ids(0)[0];
    let center = g.cities[&capital].pos;
    let mut placed = 0;
    for (dq, dr) in [(5, 0), (-5, 0), (0, 5), (0, -5), (5, -5), (-5, 5)] {
        if placed == 2 {
            break;
        }
        let pos = (center.0 + dq, center.1 + dr);
        let fits = g.map.tiles.get(&pos).is_some_and(|tile| {
            tile.owner_city.is_none() && !g.rules.is_water(tile) && g.rules.is_passable(tile)
        });
        if fits {
            g.place_city(0, pos, None);
            placed += 1;
        }
    }
    assert_eq!(placed, 2, "the fixture needs two more cities");
    g.players[0].techs.insert(crate::name!("writing"));
    for cid in g.player_city_ids(0) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = 7;
        city.queue.clear();
        if let Some(item) = head.clone() {
            city.queue.push(item);
        }
    }
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, plan)
}

fn monument() -> Item {
    Item::Building {
        building: crate::name!("monument"),
    }
}

fn leads_with_campus(g: &Game, cid: u32) -> bool {
    matches!(g.cities[&cid].queue.first(),
        Some(Item::District { district, .. }) if g.district_family(*district) == "campus")
}

fn campus_heads(g: &Game) -> Vec<u32> {
    g.player_city_ids(0)
        .into_iter()
        .filter(|cid| leads_with_campus(g, *cid))
        .collect()
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_first_campus_takes_a_filler();
    ai
}

/// A Monument at every head: off, nothing moves; on, one city's first Campus
/// takes the head. The next turn the gene is inert, because a Campus is
/// committed.
#[test]
fn the_first_campus_displaces_a_monument() {
    let (game, plan) = board(Some(monument()));

    let mut off = game.clone();
    AdvancedAi::new().claim_first_campus_takes_a_filler(&mut off, 0, &plan);
    assert!(campus_heads(&off).is_empty(), "off: no Campus");

    let ai = armed();
    let mut on = game.clone();
    assert!(ai.first_campus_filler_claim(&on, 0, &plan).is_some());
    ai.claim_first_campus_takes_a_filler(&mut on, 0, &plan);
    let heads = campus_heads(&on);
    assert_eq!(
        heads.len(),
        1,
        "on: exactly one city starts the first Campus"
    );
    // `Action::Produce` switches the head and banks the Monument's progress;
    // the ordinary writers pick the Monument again after the Campus.
    assert_ne!(
        on.cities[&heads[0]].queue.first(),
        Some(&monument()),
        "the Campus, not the Monument, heads the claimed city"
    );
    assert!(AdvancedAi::empire_has_campus_commitment(&on, 0));
    assert!(ai.first_campus_filler_claim(&on, 0, &plan).is_none());
}

/// A Settler, a Builder or Walls at every head is never displaced.
#[test]
fn settlers_builders_and_walls_keep_their_heads() {
    let ai = armed();
    for head in [
        Item::Unit {
            unit: crate::name!("settler"),
        },
        Item::Unit {
            unit: crate::name!("builder"),
        },
        Item::Building {
            building: crate::name!("walls"),
        },
    ] {
        let (mut g, plan) = board(Some(head.clone()));
        ai.claim_first_campus_takes_a_filler(&mut g, 0, &plan);
        assert!(campus_heads(&g).is_empty(), "{head:?} keeps its head");
    }
}

/// Without Writing, or in a threatened city, the claim stands down.
#[test]
fn the_claim_needs_writing_and_a_safe_city() {
    let ai = armed();
    let (mut g, plan) = board(Some(monument()));
    g.players[0].techs.remove(&crate::name!("writing"));
    assert!(ai.first_campus_filler_claim(&g, 0, &plan).is_none());

    let (g, mut plan) = board(Some(monument()));
    let Some((cid, _)) = ai.first_campus_filler_claim(&g, 0, &plan) else {
        panic!("fixture: a claim");
    };
    plan.threatened_city = Some(cid);
    assert_ne!(
        ai.first_campus_filler_claim(&g, 0, &plan)
            .map(|(city, _)| city),
        Some(cid),
        "a threatened city is not the claim's city"
    );
}

/// Soldiers are fillers only at peace with every major.
#[test]
fn soldiers_are_fillers_only_at_peace() {
    let archer = Item::Unit {
        unit: crate::name!("archer"),
    };
    assert!(AdvancedAi::first_campus_filler(&archer, false));
    assert!(!AdvancedAi::first_campus_filler(&archer, true));
    assert!(AdvancedAi::first_campus_filler(&monument(), true));
    assert!(!AdvancedAi::first_campus_filler(
        &Item::Unit {
            unit: crate::name!("trader")
        },
        false
    ));
}

#[test]
fn first_campus_takes_a_filler_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "first-campus-takes-a-filler",
        |ai| ai.first_campus_takes_a_filler,
    );
}
