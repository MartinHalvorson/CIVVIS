use super::*;
use crate::ai::advanced::near_rival_deterrence::{
    DETERRENCE_NEAR_TILES, DETERRENCE_START_STANDARD,
};

/// Three majors; we and the first rival have met at peace and hold one city
/// each within the deterrence radius, our capital idle, every unit removed,
/// inside the window. The third major holds nothing and is unmet. Returns the
/// game and our city.
fn neighbours() -> Option<(Game, u32)> {
    let mut game = Game::new_full(3, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")?;
    game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
    for pid in [0, 1, 2] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let ours = game.player_city_ids(0)[0];
    let site = *land_within(&game, ours, 5, DETERRENCE_NEAR_TILES).first()?;
    game.place_city(1, site, None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    game.players[0].techs.insert(crate::name!("archery"));
    game.players[0].techs.insert(crate::name!("bronze_working"));
    let city = game.cities.get_mut(&ours).unwrap();
    city.pop = 4;
    city.queue.clear();
    game.turn = game.standard_duration(DETERRENCE_START_STANDARD) + 5;
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 200.0);
    Some((game, ours))
}

/// Land tiles from `near` to `far` tiles of `cid`, in a fixed order.
fn land_within(game: &Game, cid: u32, near: i32, far: i32) -> Vec<Pos> {
    let home = game.cities[&cid].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (near..=far).contains(&game.wdist(**pos, home))
                && !game.rules.is_water(tile)
                && game.city_at(**pos).is_none()
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    sites
}

fn power(game: &mut Game, pid: usize, value: f64) {
    std::sync::Arc::make_mut(&mut game.observed_military_power).insert(pid, value);
}

fn treasury(game: &mut Game, gold: f64, income: f64) {
    game.players[0].gold = gold;
    game.players[0].gold_per_turn = income;
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn deterrence(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.victory_target = Some(VictoryTarget::Science);
    ai.enable_near_rival_deterrence();
    if gene {
        ai.enable_deterrence_waits_for_the_income();
    }
    ai
}

/// The deterrent claim's pick on a copy of `game`: our city's queue head.
fn claim(ai: &AdvancedAi, game: &Game) -> Option<Item> {
    let mut game = game.clone();
    let plan = ai.assess(&game, 0);
    ai.claim_deterrence_unit(&mut game, 0, &plan);
    let cid = game.player_city_ids(0)[0];
    game.cities[&cid].queue.first().cloned()
}

fn waits(ai: &AdvancedAi, game: &Game, cid: u32, item: &Item) -> Option<DeterrentShortfall> {
    let plan = ai.assess(game, 0);
    ai.deterrent_income_shortfall(game, 0, cid, item, &plan)
}

/// The blocked case, the live t68 frame: income below zero on a thin
/// treasury. Off, the claim trains its unit; on, it trains none. Income that
/// pays, or a treasury at Recovery's reserve, trains it again.
#[test]
fn a_deterrent_the_income_cannot_pay_waits() {
    let Some((mut game, _ours)) = neighbours() else {
        panic!("fixture: two capitals within the neighbour radius");
    };
    treasury(&mut game, 66.0, -1.0);
    assert!(
        claim(&deterrence(false), &game).is_some(),
        "off: a deterrent at any income"
    );
    assert_eq!(
        claim(&deterrence(true), &game),
        None,
        "on: none while income is negative"
    );

    treasury(&mut game, 66.0, 2.0);
    assert!(
        claim(&deterrence(true), &game).is_some(),
        "income that pays for it trains it"
    );

    treasury(
        &mut game,
        DETERRENT_RESERVE_BASE + DETERRENT_RESERVE_PER_CITY,
        -1.0,
    );
    assert!(
        claim(&deterrence(true), &game).is_some(),
        "a treasury at Recovery's reserve carries it"
    );
}

/// The wait looks one unit ahead and counts what the queues add: an Archer at
/// +0.4 income waits and a free Warrior does not; once income is under the
/// line a free unit waits as well; an Archer already in another city's queue
/// makes the Warrior wait too, and so does the Library the deterrent would
/// displace, which keeps its progress and comes back.
#[test]
fn the_wait_counts_the_unit_and_every_queue_head() {
    let Some((mut game, ours)) = neighbours() else {
        panic!("fixture: two capitals within the neighbour radius");
    };
    let ai = deterrence(true);
    let (archer, warrior) = (unit("archer"), unit("warrior"));
    assert_eq!(game.rules.units["archer"].maintenance, 1.0);
    assert_eq!(game.rules.units["warrior"].maintenance, 0.0);

    treasury(&mut game, 66.0, 0.4);
    let short = waits(&ai, &game, ours, &archer).expect("0.4 - 1 is under the line");
    assert_eq!(
        (short.upkeep, short.committed, short.reserve),
        (1.0, 0.0, 125.0)
    );
    assert_eq!(
        waits(&ai, &game, ours, &warrior),
        None,
        "a free unit at +0.4"
    );
    assert_eq!(
        waits(&deterrence(false), &game, ours, &archer),
        None,
        "off: nothing waits"
    );

    treasury(&mut game, 66.0, 0.6);
    assert_eq!(
        waits(&ai, &game, ours, &archer),
        None,
        "0.6 - 1 is inside the line"
    );

    treasury(&mut game, 66.0, -1.0);
    assert!(
        waits(&ai, &game, ours, &warrior).is_some(),
        "under the line already, a free unit waits too"
    );

    let site = land_within(&game, ours, 4, 6)[0];
    let second = game.place_city(0, site, None);
    game.cities.get_mut(&second).unwrap().queue = vec![archer.clone()];
    treasury(&mut game, 66.0, 0.4);
    let short = waits(&ai, &game, ours, &warrior).expect("the other queue's Archer counts");
    assert_eq!(
        (short.upkeep, short.committed, short.reserve),
        (0.0, 1.0, 150.0)
    );

    let library = Item::Building {
        building: crate::name!("library"),
    };
    assert_eq!(AdvancedAi::item_added_upkeep(&game, &library), 1.0);
    game.cities.get_mut(&second).unwrap().queue.clear();
    assert_eq!(waits(&ai, &game, ours, &warrior), None);
    game.cities.get_mut(&ours).unwrap().queue = vec![library];
    let short = waits(&ai, &game, ours, &warrior).expect("the displaced head counts");
    assert_eq!((short.upkeep, short.committed), (0.0, 1.0));
}

/// The line is Recovery's own: on a board with nothing committed, a free unit
/// waits exactly when `live_war_economy_requires_recovery` reads.
#[test]
fn the_line_is_recoverys_own() {
    let Some((mut game, ours)) = neighbours() else {
        panic!("fixture: two capitals within the neighbour radius");
    };
    let ai = deterrence(true);
    let warrior = unit("warrior");
    let reserve = DETERRENT_RESERVE_BASE + DETERRENT_RESERVE_PER_CITY;
    let mut read = Vec::new();
    for (gold, income) in [
        (reserve - 1.0, -0.6),
        (reserve, -0.6),
        (reserve - 1.0, -0.4),
        (0.0, -0.5),
        (0.0, -0.51),
        (0.0, 3.0),
    ] {
        treasury(&mut game, gold, income);
        let counts = ai.counts(&game, 0);
        let recovery = ai.live_war_economy_requires_recovery(&game, 0, &counts);
        assert_eq!(
            waits(&ai, &game, ours, &warrior).is_some(),
            recovery,
            "{gold} Gold at {income} a turn"
        );
        read.push(recovery);
    }
    assert_eq!(read, [true, false, false, false, true, false]);
}

/// The slotted per-unit discount is counted: under Conscription an Archer
/// costs nothing and does not wait at an income of zero.
#[test]
fn a_discounted_unit_is_priced_at_what_the_host_charges() {
    let Some((mut game, ours)) = neighbours() else {
        panic!("fixture: two capitals within the neighbour radius");
    };
    let ai = deterrence(true);
    let archer = unit("archer");
    treasury(&mut game, 66.0, 0.0);
    assert!(waits(&ai, &game, ours, &archer).is_some());
    game.players[0]
        .policies
        .insert(crate::name!("conscription"));
    game.board_written();
    assert_eq!(game.policy_effect(0, "unit_maintenance_discount"), 1.0);
    assert_eq!(waits(&ai, &game, ours, &archer), None);
}

/// War, a threatened city, a Recovery plan and a fresh attack on the city
/// each lift the wait: the gate is a peacetime rule.
#[test]
fn war_a_threat_and_a_fresh_attack_lift_the_wait() {
    let Some((mut game, ours)) = neighbours() else {
        panic!("fixture: two capitals within the neighbour radius");
    };
    let ai = deterrence(true);
    let archer = unit("archer");
    treasury(&mut game, 66.0, -3.0);
    let plan = ai.assess(&game, 0);
    assert_eq!(plan.threatened_city, None, "fixture: nobody threatens us");
    assert_ne!(plan.strategy, GrandStrategy::Recovery);
    assert!(ai
        .deterrent_income_shortfall(&game, 0, ours, &archer, &plan)
        .is_some());

    // At war with a third major, the neighbour still at peace: the claim
    // trains its deterrent as it does with the gene off.
    let mut war = game.clone();
    war.players[0].met.insert(2);
    war.players[2].met.insert(0);
    war.at_war.insert((0, 2));
    assert!(war.is_at_war(0, 2), "fixture: the war reads");
    assert_eq!(
        ai.deterrent_income_shortfall(&war, 0, ours, &archer, &plan),
        None,
        "at war nothing waits"
    );
    assert_eq!(
        claim(&ai, &war),
        claim(&deterrence(false), &war),
        "at war the claim is the ungated one"
    );
    assert!(claim(&ai, &war).is_some());

    let mut city_state = game.clone();
    city_state.players[2].is_minor = true;
    city_state.at_war.insert((0, 2));
    assert_eq!(
        ai.deterrent_income_shortfall(&city_state, 0, ours, &archer, &plan),
        None,
        "a city-state war is a war"
    );

    let threatened = StrategicPlan {
        threatened_city: Some(ours),
        ..plan.clone()
    };
    assert_eq!(
        ai.deterrent_income_shortfall(&game, 0, ours, &archer, &threatened),
        None,
        "a threatened city lifts it"
    );
    let recovery = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        ..plan.clone()
    };
    assert_eq!(
        ai.deterrent_income_shortfall(&game, 0, ours, &archer, &recovery),
        None,
        "a Recovery plan lifts it"
    );

    let mut struck = game.clone();
    struck.cities.get_mut(&ours).unwrap().last_attacked = struck.turn - 2;
    assert_eq!(
        ai.deterrent_income_shortfall(&struck, 0, ours, &archer, &plan),
        None,
        "a city attacked two turns ago trains what it wants"
    );
    struck.cities.get_mut(&ours).unwrap().last_attacked = struck.turn - 9;
    assert!(
        ai.deterrent_income_shortfall(&struck, 0, ours, &archer, &plan)
            .is_some(),
        "nine turns on, the wait is back"
    );
}
