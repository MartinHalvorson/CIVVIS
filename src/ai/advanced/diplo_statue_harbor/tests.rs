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
