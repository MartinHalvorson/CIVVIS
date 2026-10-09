use super::*;
use crate::ai::advanced::near_rival_deterrence::DETERRENCE_START_STANDARD;

/// `players` majors (met when two), our capital founded at turn 40 with every
/// unit removed, Pottery known, the population at its housing, and 50
/// Production already banked on the Granary so it finishes inside the cap.
/// With two players the rival's city stands 5-10 tiles away. Returns the
/// game, our capital and the Granary item.
fn bound_capital(players: usize) -> Option<(Game, u32, Item)> {
    let mut game = Game::new_full(players, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")?;
    game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
    for pid in 0..players {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let ours = game.player_city_ids(0)[0];
    if players > 1 {
        let home = game.cities[&ours].pos;
        let mut sites: Vec<Pos> = game
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                (5..=10).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
            })
            .map(|(pos, _)| *pos)
            .collect();
        sites.sort();
        let site = *sites.first()?;
        game.place_city(1, site, None);
        game.players[0].met.insert(1);
        game.players[1].met.insert(0);
    }
    game.players[0].techs.insert(crate::name!("pottery"));
    game.players[0].techs.insert(crate::name!("bronze_working"));
    game.turn = game.standard_duration(DETERRENCE_START_STANDARD) + 5;
    let housing = game.city_housing(&game.cities[&ours]);
    let granary = Item::Building {
        building: crate::name!("granary"),
    };
    let city = game.cities.get_mut(&ours).unwrap();
    city.pop = (housing.ceil() as i32).max(2);
    city.queue.clear();
    city.production_progress
        .insert(Game::item_progress_key(&granary), 50.0);
    if !game.can_produce(0, ours, &granary) {
        return None;
    }
    Some((game, ours, granary))
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_granary_claims_the_bound_queue();
    ai
}

fn head(game: &Game, cid: u32) -> Option<Item> {
    game.cities[&cid].queue.first().cloned()
}

fn queue(game: &mut Game, cid: u32, item: Item) {
    game.cities.get_mut(&cid).unwrap().queue = vec![item];
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

/// The blocked case: a housing-bound capital training a routine Warrior, and
/// an idle one with no Settler due, put the Granary first; off, nothing
/// moves; with the Granary built, nothing moves either.
#[test]
fn a_bound_city_claims_its_granary_over_a_routine_unit() {
    let Some((game, capital, granary)) = bound_capital(1) else {
        panic!("fixture: a capital that can build its Granary");
    };
    let mut busy = game.clone();
    queue(&mut busy, capital, unit("warrior"));

    let stock = AdvancedAi::new();
    let mut off = busy.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_bound_granaries(&mut off, 0, &plan);
    assert_eq!(head(&off, capital), Some(unit("warrior")), "off: no claim");

    let ai = armed();
    let mut on = busy.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_bound_granaries(&mut on, 0, &plan);
    assert_eq!(head(&on, capital), Some(granary.clone()), "the Granary goes first");

    // An idle queue with a Settler due is left to the Settler step; with a
    // Settler already walking, the idle queue takes the Granary.
    let mut due = game.clone();
    let plan = ai.assess(&due, 0);
    let settler_due = ai.base.settler_due(&due, 0, capital, 1, 0);
    ai.claim_bound_granaries(&mut due, 0, &plan);
    if settler_due {
        assert_eq!(head(&due, capital), None, "a due Settler keeps the idle queue");
    }
    let mut idle = game.clone();
    let home = idle.cities[&capital].pos;
    idle.spawn_unit("settler", 0, home);
    assert!(!ai.base.settler_due(&idle, 0, capital, 1, 1), "fixture: a Settler walking");
    let plan = ai.assess(&idle, 0);
    ai.claim_bound_granaries(&mut idle, 0, &plan);
    assert_eq!(head(&idle, capital), Some(granary.clone()), "an idle queue takes it");

    let mut built = busy.clone();
    built
        .cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("granary"));
    let plan = ai.assess(&built, 0);
    ai.claim_bound_granaries(&mut built, 0, &plan);
    assert_eq!(head(&built, capital), Some(unit("warrior")), "a built Granary claims nothing");
}

/// A Settler (which relieves the housing), the faith defence's Shrine, a
/// freshly attacked city and a city with room to grow keep their queues.
#[test]
fn a_settler_an_attacked_city_and_an_unbound_city_keep_their_queues() {
    let Some((game, capital, _granary)) = bound_capital(1) else {
        panic!("fixture: a capital that can build its Granary");
    };
    let ai = armed();

    let mut settling = game.clone();
    queue(&mut settling, capital, unit("settler"));
    let plan = ai.assess(&settling, 0);
    ai.claim_bound_granaries(&mut settling, 0, &plan);
    assert_eq!(head(&settling, capital), Some(unit("settler")), "the Settler stays");

    let mut shrine = game.clone();
    let faith = Item::Building {
        building: crate::name!("shrine"),
    };
    queue(&mut shrine, capital, faith.clone());
    let plan = ai.assess(&shrine, 0);
    ai.claim_bound_granaries(&mut shrine, 0, &plan);
    assert_eq!(head(&shrine, capital), Some(faith), "the faith defence's Shrine stays");

    let mut attacked = game.clone();
    queue(&mut attacked, capital, unit("warrior"));
    let turn = attacked.turn;
    attacked.cities.get_mut(&capital).unwrap().last_attacked = turn;
    let plan = ai.assess(&attacked, 0);
    ai.claim_bound_granaries(&mut attacked, 0, &plan);
    assert_eq!(head(&attacked, capital), Some(unit("warrior")), "an attacked city keeps its unit");

    let mut roomy = game.clone();
    queue(&mut roomy, capital, unit("warrior"));
    let housing = roomy.city_housing(&roomy.cities[&capital]);
    if housing >= 3.0 {
        roomy.cities.get_mut(&capital).unwrap().pop = (housing.floor() as i32 - 2).max(1);
        let plan = ai.assess(&roomy, 0);
        ai.claim_bound_granaries(&mut roomy, 0, &plan);
        assert_eq!(head(&roomy, capital), Some(unit("warrior")), "room to grow: no claim");
    }
}

/// At war with a major the military unit keeps its queue; a Monument still
/// waits for the Granary.
#[test]
fn at_war_a_unit_keeps_its_queue_but_a_monument_waits() {
    let Some((mut game, capital, granary)) = bound_capital(2) else {
        panic!("fixture: two majors and a capital that can build its Granary");
    };
    game.at_war.insert((0, 1));
    let ai = armed();

    let mut unit_head = game.clone();
    queue(&mut unit_head, capital, unit("warrior"));
    let plan = ai.assess(&unit_head, 0);
    ai.claim_bound_granaries(&mut unit_head, 0, &plan);
    assert_eq!(head(&unit_head, capital), Some(unit("warrior")), "a war unit stays");

    let mut monument = game.clone();
    let item = Item::Building {
        building: crate::name!("monument"),
    };
    queue(&mut monument, capital, item);
    let plan = ai.assess(&monument, 0);
    if plan.threatened_city != Some(capital) {
        ai.claim_bound_granaries(&mut monument, 0, &plan);
        assert_eq!(head(&monument, capital), Some(granary), "the Monument waits");
    }
}

/// The deterrent leaves the claimed Granary alone: with
/// `near-rival-deterrence` armed and a neighbour at peace out-gunning us, the
/// only city keeps its Granary under the gene and trains the deterrent
/// without it.
#[test]
fn the_deterrent_leaves_a_claimed_granary_alone() {
    let Some((mut game, capital, granary)) = bound_capital(2) else {
        panic!("fixture: two majors and a capital that can build its Granary");
    };
    std::sync::Arc::make_mut(&mut game.observed_military_power).insert(0, 100.0);
    std::sync::Arc::make_mut(&mut game.observed_military_power).insert(1, 200.0);
    // The fixture's 50 banked leave the Granary more than a turn of
    // Production short, so the deterrent may displace it without the gene.
    queue(&mut game, capital, granary.clone());

    let mut deterrence_only = AdvancedAi::new();
    deterrence_only.enable_near_rival_deterrence();
    let mut off = game.clone();
    let plan = deterrence_only.assess(&off, 0);
    deterrence_only.claim_deterrence_unit(&mut off, 0, &plan);
    assert_ne!(head(&off, capital), Some(granary.clone()), "off: the deterrent displaces it");

    let mut both = armed();
    both.enable_near_rival_deterrence();
    let mut on = game.clone();
    let plan = both.assess(&on, 0);
    assert!(both.bound_granary_holds(&on, 0, capital, &plan, &granary), "the claim holds");
    both.claim_deterrence_unit(&mut on, 0, &plan);
    assert_eq!(head(&on, capital), Some(granary), "on: the Granary stays");
}

/// Registered as a version-less opt-in, off in the stock controller.
#[test]
fn the_gene_is_an_opt_in_off_by_default() {
    let gene = crate::ai::advanced::genes::GENES
        .iter()
        .find(|gene| gene.tag == "granary-claims-the-bound-queue")
        .expect("registered");
    assert!(matches!(gene.kind, crate::ai::advanced::genes::Kind::OptIn));
    assert!(!AdvancedAi::new().granary_claims_the_bound_queue);
}
