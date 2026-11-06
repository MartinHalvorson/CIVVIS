//! Tests of `advanced/opportunistic_war.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

#[test]
fn competitive_religious_opening_produces_multiple_founders() {
    let mut game = Game::new_full(
        4,
        24,
        16,
        crate::rng::fixture_seed("PROPHET", 76_106),
        110,
        0,
        false,
    );
    let mut ais = AdvancedAi::fleet(&game);
    run_game(&mut game, &mut ais);
    assert!(
        game.religions_founded() >= 2,
        "a stock Prophet race should not end with one uncontested founder: turn {}, {:?}",
        game.turn,
        game.players
            .iter()
            .take(4)
            .map(|player| (
                &player.civ,
                &player.religion,
                player.prophet_pending,
                player.gpp.get("prophet"),
                player.techs.contains(&crate::name!("astrology")),
                game.player_city_ids(player.id)
                    .iter()
                    .filter(|city| game.cities[city]
                        .districts
                        .contains_key(crate::name!("holy_site")))
                    .count(),
            ))
            .collect::<Vec<_>>()
    );
}

#[test]
fn advanced_selfplay_completes() {
    let mut g = Game::new(2, 20, 14, 73, 65, 1);
    let mut ais = AdvancedAi::fleet(&g);
    run_game(&mut g, &mut ais);
    assert!(g.winner.is_some());
    assert!(g
        .players
        .iter()
        .filter(|p| !p.is_minor && p.alive)
        .all(|p| p.techs.len() > 1));
    // Settlers lost to Barbarians or captured legitimately break any
    // lifetime produced-vs-founded accounting. Guard the behavior this
    // test actually cares about instead: the production gate only queues a
    // Settler while the player holds none, so a player must never end the
    // game sitting on a backlog of idle Settlers.
    for player in g.players.iter().filter(|p| !p.is_minor && p.alive) {
        let idle = g
            .units
            .values()
            .filter(|u| u.owner == player.id && u.kind == "settler")
            .count();
        assert!(
            idle <= 1,
            "advanced AI accumulated idle Settlers: player {} holds {idle}",
            player.id
        );
    }
}

/// Fires-check for the religion genes, run by hand: four 6p games with the
/// deployment genome plus the gene on every seat, counting defensive
/// Apostles, founders and Inquisitions.
/// `cargo test --profile ci --lib -- --ignored --nocapture religion_genes_fires_check`
#[test]
#[ignore]
fn religion_genes_fires_check() {
    for seed in 0..4u64 {
        let mut game = Game::new(6, 60, 38, 611 + seed, 200, 6);
        let mut ais = AdvancedAi::fleet(&game);
        for ai in ais.iter_mut() {
            ai.enable_engine_repairs();
            ai.enable_founder_temple();
        }
        crate::ai::run_game(&mut game, &mut ais);
        let mut census = StrategyCensus::default();
        for ai in ais.iter().take(6) {
            census.absorb(&ai.strategy_census());
        }
        let founders = game
            .players
            .iter()
            .take(6)
            .filter(|p| p.religion.is_some())
            .count();
        let inquisitions = game
            .players
            .iter()
            .take(6)
            .filter(|p| p.counters.get("inquisition").copied().unwrap_or(0) > 0)
            .count();
        eprintln!(
            "FIRES seed={seed} t{} victory={:?} founders={founders} inquisitions={inquisitions}",
            game.turn, game.victory_type
        );
    }
}

/// The airfield is claimed before anything else in the package: no Bomber can
/// be trained anywhere else, so a wing ordered first is a wing ordered nowhere.
#[test]
fn the_airfield_is_claimed_before_the_wing() {
    let (mut game, _, _) = air_surge_fixture(941_105);
    air_surge_arm(&mut game);
    let mut ai = air_surge_ai();
    ai.maintain_air_surge(&game, 0);
    assert!(ai.air_surge_active());
    for cid in game.player_city_ids(0) {
        game.cities.get_mut(&cid).expect("own city").queue.clear();
    }

    assert!(
        ai.air_surge_production(&mut game, 0),
        "an idle empire with no airfield claims one"
    );
    let claimed = game
        .player_city_ids(0)
        .into_iter()
        .filter_map(|cid| game.cities[&cid].queue.first().cloned())
        .next()
        .expect("the claim writes a queue");
    assert!(
        matches!(claimed, Item::District { district, .. }
                 if game.district_family(district) == crate::name!("aerodrome")),
        "claimed {claimed:?} instead of the airfield"
    );

    // A second call does not start a duplicate: the queue already counts.
    let before: Vec<_> = game
        .player_city_ids(0)
        .into_iter()
        .map(|cid| game.cities[&cid].queue.clone())
        .collect();
    ai.air_surge_production(&mut game, 0);
    let after: Vec<_> = game
        .player_city_ids(0)
        .into_iter()
        .map(|cid| game.cities[&cid].queue.clone())
        .collect();
    assert!(
        after
            .iter()
            .flatten()
            .filter(|item| matches!(item, Item::District { district, .. }
                                    if game.district_family(*district)
                                        == crate::name!("aerodrome")))
            .count()
            <= 1,
        "before {before:?} after {after:?}"
    );
}
