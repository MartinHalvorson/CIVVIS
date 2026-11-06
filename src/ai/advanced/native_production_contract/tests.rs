use super::*;

fn board() -> Game {
    let mut g = Game::new_full(
        2,
        24,
        16,
        crate::rng::fixture_seed("FIRSTPADFORGE", 47_113),
        250,
        0,
        false,
    );
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = g.player_city_ids(0)[0];
    let center = g.cities[&capital].pos;
    for (dq, dr) in [(5, 0), (-5, 0), (0, 5), (0, -5), (5, -5), (-5, 5)] {
        if g.player_city_ids(0).len() == 3 {
            break;
        }
        let pos = (center.0 + dq, center.1 + dr);
        if g.map.tiles.get(&pos).is_some_and(|tile| {
            tile.owner_city.is_none() && !g.rules.is_water(tile) && g.rules.is_passable(tile)
        }) {
            g.place_city(0, pos, None);
        }
    }
    assert_eq!(g.player_city_ids(0).len(), 3);
    g.players[0].techs.insert(crate::name!("rocketry"));
    for cid in g.player_city_ids(0) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = if cid == capital { 12 } else { 1 };
        city.queue.clear();
    }
    g
}

#[test]
fn untreated_native_bridge_has_no_extra_production_contract() {
    assert!(AdvancedAi::new()
        .native_first_pad_allowed_cities(&board(), 0)
        .is_none());
}

#[test]
fn a_discarded_planned_pad_cannot_relax_the_observed_producer_floor() {
    let observed = board();
    let mut ai = AdvancedAi::new();
    ai.enable_first_pad_in_the_forge();
    let allowed = ai.native_first_pad_allowed_cities(&observed, 0).unwrap();
    let placing: Vec<_> = observed
        .player_city_ids(0)
        .into_iter()
        .filter(|cid| {
            observed
                .district_sites(*cid, crate::name!("spaceport"))
                .into_iter()
                .any(|pos| {
                    observed.can_produce(
                        0,
                        *cid,
                        &Item::District {
                            district: crate::name!("spaceport"),
                            pos,
                        },
                    )
                })
        })
        .collect();
    let weak = *placing.iter().find(|cid| !allowed.contains(cid)).unwrap();
    let forge = *placing
        .iter()
        .filter(|cid| allowed.contains(cid))
        .max_by(|a, b| {
            observed
                .city_yields(**a)
                .production
                .total_cmp(&observed.city_yields(**b).production)
        })
        .unwrap();
    let pos = observed
        .district_sites(forge, crate::name!("spaceport"))
        .into_iter()
        .find(|pos| {
            observed.can_produce(
                0,
                forge,
                &Item::District {
                    district: crate::name!("spaceport"),
                    pos: *pos,
                },
            )
        })
        .unwrap();
    let mut planned = observed.clone();
    planned
        .apply(
            0,
            &Action::Produce {
                city: forge,
                item: Item::District {
                    district: crate::name!("spaceport"),
                    pos,
                },
            },
        )
        .unwrap();
    planned
        .apply(
            0,
            &Action::Produce {
                city: forge,
                item: Item::Unit {
                    unit: crate::name!("builder"),
                },
            },
        )
        .unwrap();
    assert!(
        ai.native_first_pad_allowed_cities(&planned, 0)
            .unwrap()
            .contains(&weak),
        "the speculative foundation explains the old planner's relaxed floor"
    );
    assert!(!allowed.contains(&weak));
    assert_eq!(
        ai.native_first_pad_allowed_cities(&observed, 0).unwrap(),
        allowed,
        "native eligibility must still come from the unmutated observation"
    );
}
