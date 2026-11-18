use super::*;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

/// Up to four cities of four citizens with Celestial Navigation in hand,
/// production 40 down to 25, idle queues, on the first fixture seed where
/// some city can build its first Harbor. Returns the game and our cities
/// ranked by production.
fn coastal_empire() -> (Game, Vec<u32>) {
    for seed in 1..60 {
        let mut game = Game::new_full(
            1,
            44,
            26,
            crate::rng::fixture_seed("STATUEHARBOR", seed),
            250,
            0,
            false,
        );
        game.players[0].civ = "Gran Colombia".to_string();
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        while game.player_city_ids(0).len() < 4 {
            let Some(pos) = game
                .map
                .tiles
                .values()
                .filter(|tile| {
                    game.rules.is_passable(tile)
                        && !game.rules.is_water(tile)
                        && tile.owner_city.is_none()
                        && tile.district.is_none()
                        && tile.wonder.is_none()
                        && game
                            .cities
                            .values()
                            .all(|city| game.wdist(city.pos, tile.pos) >= 5)
                        && game
                            .cities
                            .values()
                            .any(|city| game.wdist(city.pos, tile.pos) <= 8)
                })
                .map(|tile| tile.pos)
                .min()
            else {
                break;
            };
            game.found_city_for(0, pos, None);
        }
        for unit in game.player_unit_ids(0) {
            game.remove_unit(unit);
        }
        game.game_speed = GameSpeed::Online;
        game.turn = 110;
        for tech in [
            "pottery",
            "sailing",
            "astrology",
            "celestial_navigation",
            "mining",
            "bronze_working",
            "irrigation",
        ] {
            game.players[0].techs.insert(Name::new(tech));
        }
        let cities = game.player_city_ids(0);
        for (rank, cid) in cities.iter().enumerate() {
            let city = game.cities.get_mut(cid).unwrap();
            city.pop = 4;
            city.queue.clear();
            Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
                *cid,
                Yields {
                    production: 40.0 - 5.0 * rank as f64,
                    ..Default::default()
                },
            );
        }
        let mut ranked = cities;
        ranked.sort_by(|a, b| {
            game.city_yields(*b)
                .production
                .total_cmp(&game.city_yields(*a).production)
                .then(a.cmp(b))
        });
        if ranked
            .iter()
            .any(|cid| AdvancedAi::statue_harbor_site(&game, 0, *cid).is_some())
        {
            return (game, ranked);
        }
    }
    panic!("fixture: no seed offers a Harbor site with room for the Statue");
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_diplo_seat_statue_harbor();
    }
    ai
}

fn is_harbor(game: &Game, item: Option<&Item>) -> bool {
    matches!(item, Some(Item::District { district, .. })
        if game.district_family(*district) == "harbor")
}

fn claim(ai: &AdvancedAi, game: &mut Game) {
    let plan = ai.assess(game, 0);
    ai.claim_statue_harbor(game, 0, &plan);
}

/// The strongest city that can build a Harbor claims it, once, and the
/// claim holds; off, or for a Culture seat, nothing moves.
#[test]
fn the_strongest_coastal_city_claims_the_statues_harbor() {
    let (game, ranked) = coastal_empire();
    let harbor_city = *ranked
        .iter()
        .find(|cid| AdvancedAi::statue_harbor_site(&game, 0, **cid).is_some())
        .unwrap();

    for (ai, why) in [
        (seat(VictoryTarget::Diplomacy, false), "off"),
        (seat(VictoryTarget::Culture, true), "a Culture seat"),
    ] {
        let mut kept = game.clone();
        claim(&ai, &mut kept);
        for cid in &ranked {
            assert!(kept.cities[cid].queue.is_empty(), "{why}: no claim");
        }
    }

    let ai = seat(VictoryTarget::Diplomacy, true);
    let mut on = game.clone();
    claim(&ai, &mut on);
    let head = on.cities[&harbor_city].queue.first().cloned();
    assert!(is_harbor(&on, head.as_ref()), "the Harbor goes first");
    let plan = ai.assess(&on, 0);
    assert!(ai.statue_harbor_holds(&on, 0, harbor_city, &plan, head.as_ref().unwrap()));
    for cid in ranked.iter().filter(|cid| **cid != harbor_city) {
        assert!(on.cities[cid].queue.is_empty(), "one Harbor is claimed");
    }
    assert!(
        ai.statue_harbor_item(&on, 0, &plan).is_none(),
        "a queued Harbor in the strongest city serves the Statue"
    );
}

/// `science-seat-builds-the-statue`: a Science seat claims the Harbor from
/// t120, not before, and only under the tag.
#[test]
fn a_science_seat_waits_for_the_statues_harbor() {
    let (mut game, ranked) = coastal_empire();
    let harbor_city = *ranked
        .iter()
        .find(|cid| AdvancedAi::statue_harbor_site(&game, 0, **cid).is_some())
        .unwrap();
    let mut on = seat(VictoryTarget::Science, true);
    on.enable_science_seat_builds_the_statue();
    let off = seat(VictoryTarget::Science, true);

    game.turn = SCIENCE_STATUE_HARBOR_TURN - 1;
    let mut early = game.clone();
    claim(&on, &mut early);
    assert!(
        early.cities[&harbor_city].queue.is_empty(),
        "before the turn the first Campus keeps the city"
    );

    game.turn = SCIENCE_STATUE_HARBOR_TURN;
    let mut untagged = game.clone();
    claim(&off, &mut untagged);
    assert!(
        untagged.cities[&harbor_city].queue.is_empty(),
        "without the tag a Science seat claims nothing"
    );
    let mut tagged = game.clone();
    claim(&on, &mut tagged);
    let head = tagged.cities[&harbor_city].queue.first().cloned();
    assert!(is_harbor(&tagged, head.as_ref()), "the Harbor goes first");
}

/// The tag reaches only a Science seat: a Culture seat claims nothing.
#[test]
fn the_tag_leaves_a_culture_seat_alone() {
    let (mut game, ranked) = coastal_empire();
    game.turn = SCIENCE_STATUE_HARBOR_TURN;
    let mut ai = seat(VictoryTarget::Culture, true);
    ai.enable_science_seat_builds_the_statue();
    claim(&ai, &mut game);
    for cid in &ranked {
        assert!(game.cities[cid].queue.is_empty());
    }
}

/// Once the Statue stands anywhere, no Harbor is claimed for it.
#[test]
fn a_built_statue_claims_nothing() {
    let (mut game, ranked) = coastal_empire();
    let pos = game.cities[&ranked[0]].pos;
    game.cities
        .get_mut(&ranked[0])
        .unwrap()
        .wonders
        .insert(crate::name!("statue_of_liberty"), pos);
    let ai = seat(VictoryTarget::Diplomacy, true);
    let plan = ai.assess(&game, 0);
    assert!(ai.statue_harbor_item(&game, 0, &plan).is_none());
}

/// `diplo-seat-statue-civic-first`: a Diplomatic seat that holds a tier-two
/// government civic and whose Statue Harbor is queued names Civil Engineering
/// as its civic goal until it is adopted; off, without the civic, without
/// the Harbor, with the Statue gone, on a lost lane or another target it
/// names nothing.
#[test]
fn a_diplomatic_seat_with_the_statues_harbor_goes_for_civil_engineering() {
    let (mut game, _) = coastal_empire();
    let mut ai = seat(VictoryTarget::Diplomacy, true);
    ai.enable_diplo_seat_statue_civic_first();
    game.players[0].civics.insert(crate::name!("divine_right"));
    assert_eq!(
        ai.diplo_statue_civic_goal(&game, 0),
        None,
        "no Harbor for the Statue stands or is queued yet"
    );
    claim(&ai, &mut game);
    assert_eq!(
        ai.diplo_statue_civic_goal(&game, 0),
        Some("civil_engineering"),
        "the Harbor is queued and Divine Right is in hand"
    );

    let off = seat(VictoryTarget::Diplomacy, true);
    assert_eq!(off.diplo_statue_civic_goal(&game, 0), None, "off");

    // The ladder's first rung comes first: any tier-two government civic.
    let mut early = game.clone();
    early.players[0]
        .civics
        .remove(&crate::name!("divine_right"));
    assert_eq!(ai.diplo_statue_civic_goal(&early, 0), None);
    for civic in ["exploration", "reformed_church"] {
        let mut other = early.clone();
        other.players[0].civics.insert(Name::new(civic));
        assert_eq!(
            ai.diplo_statue_civic_goal(&other, 0),
            Some("civil_engineering"),
            "{civic}"
        );
    }

    // Adopted, the goal is done.
    let mut adopted = game.clone();
    adopted.players[0]
        .civics
        .insert(crate::name!("civil_engineering"));
    assert_eq!(ai.diplo_statue_civic_goal(&adopted, 0), None);

    // The Statue standing anywhere, or ruled out by the host, ends it.
    let mut built = game.clone();
    let city = built.player_city_ids(0)[0];
    let pos = built.cities[&city].pos;
    built
        .cities
        .get_mut(&city)
        .unwrap()
        .wonders
        .insert(crate::name!("statue_of_liberty"), pos);
    assert_eq!(ai.diplo_statue_civic_goal(&built, 0), None);

    // A lost lane and another target never.
    let mut lost = ai.clone();
    lost.lane_lost = true;
    assert_eq!(lost.diplo_statue_civic_goal(&game, 0), None);
    let mut science = seat(VictoryTarget::Science, true);
    science.enable_diplo_seat_statue_civic_first();
    assert_eq!(science.diplo_statue_civic_goal(&game, 0), None);
}

/// A Harbor whose coast neighbours are all taken leaves the Statue no site:
/// the claim looks for one that does.
#[test]
fn a_harbor_site_without_room_for_the_statue_is_passed_over() {
    let (mut game, ranked) = coastal_empire();
    let cid = *ranked
        .iter()
        .find(|cid| AdvancedAi::statue_harbor_site(&game, 0, **cid).is_some())
        .unwrap();
    let (item, _) = AdvancedAi::statue_harbor_site(&game, 0, cid).unwrap();
    let Item::District { pos, .. } = item else {
        unreachable!()
    };
    assert!(AdvancedAi::statue_room(&game, &game.cities[&cid], pos));
    let blocker = game
        .cities
        .values()
        .next()
        .and_then(|city| city.districts.iter().next().map(|(name, _)| *name))
        .unwrap_or(crate::name!("city_center"));
    for next in game.nbrs(pos) {
        if let Some(tile) = game.map.tiles.get_mut(&next) {
            if tile.terrain == "coast" {
                tile.district = Some(blocker);
            }
        }
    }
    assert!(!AdvancedAi::statue_room(&game, &game.cities[&cid], pos));
}
