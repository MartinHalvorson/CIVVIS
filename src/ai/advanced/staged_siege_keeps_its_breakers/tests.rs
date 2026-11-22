use super::*;

fn one_city() -> (Game, u32) {
    let mut g = Game::new_full(1, 18, 10, 9_633, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.turn = 90;
    (g, g.player_city_ids(0)[0])
}

fn plan(threatened: Option<u32>) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: threatened,
        desired_cities: 6,
        assessed_turn: 90,
        rush: false,
    }
}

fn unit(kind: &str) -> Item {
    Item::Unit {
        unit: crate::name::Name::new(kind),
    }
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    if gene {
        ai.enable_staged_siege_keeps_its_breakers();
    } else {
        ai.disable_staged_siege_keeps_its_breakers();
    }
    ai
}

#[test]
fn the_gene_is_an_off_by_default_opt_in() {
    assert!(!AdvancedAi::new().staged_siege_keeps_its_breakers);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    assert!(!live.staged_siege_keeps_its_breakers);
    let enable = crate::ai::advanced::genes::GENES
        .iter()
        .find(|gene| gene.tag == "staged-siege-keeps-its-breakers")
        .expect("an opt-in row")
        .enable;
    enable(&mut live);
    assert!(live.staged_siege_keeps_its_breakers);
}

/// The Khmer case: an unstarted Siege Tower at the head while the train reads
/// "nothing to open the walls". On, the queue is held; off, it is not (the
/// old guard holds only invested siege-class guns).
#[test]
fn an_unstarted_tower_keeps_its_queue_while_the_siege_waits() {
    for gene in [true, false] {
        let (g, city) = one_city();
        let mut ai = seat(gene);
        ai.note_siege_wants_a_breaker(&g);
        assert_eq!(ai.siege_breaker_wanted_turn.is_some(), gene);
        for breaker in ["siege_tower", "battering_ram", "catapult"] {
            assert_eq!(
                ai.breaker_commitment_holds(&g, city, &unit(breaker), &plan(None)),
                gene,
                "{breaker} with the gene {gene}"
            );
        }
    }
}

#[test]
fn a_stale_reading_another_unit_or_a_threatened_city_releases_the_queue() {
    let (mut g, city) = one_city();
    let mut ai = seat(true);
    ai.note_siege_wants_a_breaker(&g);
    // Not a breaker.
    assert!(!ai.breaker_commitment_holds(&g, city, &unit("warrior"), &plan(None)));
    // The plan names the city threatened.
    assert!(!ai.breaker_commitment_holds(&g, city, &unit("siege_tower"), &plan(Some(city))));
    // Attacked two turns ago.
    g.cities.get_mut(&city).unwrap().last_attacked = g.turn - 2;
    assert!(!ai.breaker_commitment_holds(&g, city, &unit("siege_tower"), &plan(None)));
    g.cities.get_mut(&city).unwrap().last_attacked = 0;
    // The next turn still holds; two turns on the reading is stale.
    g.turn += 1;
    assert!(ai.breaker_commitment_holds(&g, city, &unit("siege_tower"), &plan(None)));
    g.turn += 1;
    assert!(!ai.breaker_commitment_holds(&g, city, &unit("siege_tower"), &plan(None)));
}
