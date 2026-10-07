use super::*;
use crate::ai::advanced::{GrandStrategy, VictoryTarget};

fn board() -> (Game, u32, StrategicPlan) {
    let mut g = Game::new_full(
        2,
        24,
        16,
        crate::rng::fixture_seed("PROPHETSITE", 62_068),
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
    let cid = g.player_city_ids(0)[0];
    g.players[0].techs.insert(crate::name!("astrology"));
    g.players[0].gold = 175.0;
    let city = g.cities.get_mut(&cid).unwrap();
    city.pop = 5;
    city.queue.clear();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, cid, plan)
}

/// A second city of ours, on dry passable land five or more tiles out.
fn second_city(g: &mut Game, first: u32) -> u32 {
    let origin = g.cities[&first].pos;
    let pos = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| {
            let tile = &g.map.tiles[pos];
            let distance = g.wdist(origin, *pos);
            (5..=8).contains(&distance)
                && !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
                && tile.owner_city.is_none()
        })
        .min()
        .unwrap();
    let cid = g.found_city_for(0, pos, None);
    g.cities.get_mut(&cid).unwrap().queue.clear();
    cid
}

fn racer() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_enter_the_prophet_race_2();
    ai.enable_prophet_race_earns_its_points();
    ai
}

fn holy_site_first(g: &Game, cid: u32) -> bool {
    g.cities[&cid]
        .queue
        .first()
        .is_some_and(|item| AdvancedAi::holy_site_item(g, item))
}

/// Live Emperor G348: Revelation earned a Prophet at turn 62 with no Holy
/// Site anywhere, and it stood unused until the Khmer won at 68. A held
/// Prophet now puts the Holy Site in front of the city's Scout, keeps the
/// Scout's progress, and holds it through rescoring; the gene off leaves the
/// queue alone.
#[test]
fn a_held_prophet_puts_its_holy_site_first() {
    let (mut g, cid, plan) = board();
    let scout = Item::Unit {
        unit: crate::name!("scout"),
    };
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: scout.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&cid).unwrap().production = 3.0;
    g.players[0].prophet_pending = true;
    // The host lists the class exhausted once the last Prophet is ours.
    g.players[0].live_great_person_exhausted =
        Some(std::iter::once("prophet".to_string()).collect());
    let mut ai = racer();

    let mut off = g.clone();
    ai.reserve_prophet_site(&mut off, 0, &plan);
    assert_eq!(
        off.cities[&cid].queue.first(),
        Some(&scout),
        "off by default"
    );

    ai.enable_prophet_builds_its_site();
    assert!(
        ai.prophet_site_due(&g, 0).is_some(),
        "a held Prophet asks for its site"
    );
    ai.reserve_prophet_site(&mut g, 0, &plan);
    assert!(
        holy_site_first(&g, cid),
        "queue: {:?}",
        g.cities[&cid].queue
    );
    assert_eq!(
        g.item_invested_production(cid, &scout),
        3.0,
        "the Scout keeps its progress"
    );
    let head = g.cities[&cid].queue.first().cloned().unwrap();
    assert!(
        ai.prophet_site_committed(&g, 0, cid, &head),
        "the governor keeps the site"
    );
    // Idempotent on the next frame.
    ai.reserve_prophet_site(&mut g, 0, &plan);
    assert!(holy_site_first(&g, cid));

    // A founded religion, or a finished Holy Site, needs nothing more.
    let mut founded = g.clone();
    founded.players[0].religion = Some("RELIGION_BUDDHISM".to_string());
    assert!(
        ai.prophet_site_due(&founded, 0).is_none(),
        "the religion is founded"
    );
}

/// The host's exhausted Prophet class closes the race for this seat: no
/// Revelation, no race Shrine, no race Holy Site. A Prophet of ours still
/// pending keeps it open, and the gene off keeps the shipped slot count.
#[test]
fn an_exhausted_prophet_class_closes_the_race() {
    let (mut g, cid, plan) = board();
    second_city(&mut g, cid);
    g.players[0].live_great_person_exhausted =
        Some(std::iter::once("prophet".to_string()).collect());
    let mut ai = racer();
    assert!(
        ai.prophet_race_open_for(&g, 0),
        "off: the shipped slot count"
    );
    assert!(ai.race_points_wanted(&g, 0));

    ai.enable_prophet_builds_its_site();
    assert!(
        !ai.prophet_race_open_for(&g, 0),
        "the host has no Prophet left"
    );
    assert!(!ai.race_points_wanted(&g, 0));
    assert!(!ai.race_wants_revelation(&g, 0));
    assert!(ai.prophet_site_due(&g, 0).is_none());
    let mut reserved = g.clone();
    ai.reserve_prophet_site(&mut reserved, 0, &plan);
    assert!(
        !holy_site_first(&reserved, cid),
        "no site for a closed race"
    );

    let mut pending = g.clone();
    pending.players[0].prophet_pending = true;
    assert!(
        ai.prophet_race_open_for(&pending, 0),
        "our own Prophet keeps it open"
    );
    assert!(ai.prophet_site_due(&pending, 0).is_some());
}

/// Revelation waits on a Holy Site built or queued; the race that wants its
/// points queues the site first, after which the card is wanted. The gene
/// off slots the card with no site.
#[test]
fn revelation_waits_for_its_holy_site_and_the_race_queues_it() {
    let (mut g, cid, plan) = board();
    let second = second_city(&mut g, cid);
    let mut ai = racer();
    assert!(ai.race_wants_revelation(&g, 0), "off: Holy Site or not");

    ai.enable_prophet_builds_its_site();
    assert!(!ai.race_wants_revelation(&g, 0), "no site, no Revelation");
    let why = ai.prophet_site_due(&g, 0).expect("the race wants the site");
    assert!(why.contains("Revelation"), "{why}");
    ai.reserve_prophet_site(&mut g, 0, &plan);
    assert!(
        holy_site_first(&g, cid) || holy_site_first(&g, second),
        "queues: {:?} / {:?}",
        g.cities[&cid].queue,
        g.cities[&second].queue
    );
    assert!(
        ai.race_wants_revelation(&g, 0),
        "a queued site admits Revelation"
    );

    // One city is not yet a race.
    let (single, _, _) = board();
    assert!(ai.prophet_site_due(&single, 0).is_none());
}

/// A city holding a wonder or a paid district, or under threat, keeps its
/// queue; the site goes to another city of ours.
#[test]
fn the_site_skips_a_held_or_threatened_city() {
    let (mut g, cid, plan) = board();
    let second = second_city(&mut g, cid);
    g.players[0].prophet_pending = true;
    let mut ai = racer();
    ai.enable_prophet_builds_its_site();

    let mut held = plan.clone();
    held.threatened_city = Some(cid);
    ai.reserve_prophet_site(&mut g, 0, &held);
    assert!(!holy_site_first(&g, cid), "the threatened city defends");
    assert!(
        holy_site_first(&g, second),
        "queue: {:?}",
        g.cities[&second].queue
    );
}
