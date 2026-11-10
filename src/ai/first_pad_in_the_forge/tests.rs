use super::*;
use crate::game::Action;

/// Three cities of ours with Rocketry known: the capital at population 12,
/// the other two at population 1, every queue empty.
fn board() -> (Game, u32, Vec<u32>) {
    let mut g = Game::new_full(
        2,
        24,
        16,
        crate::rng::fixture_seed("FIRSTPADFORGE", 47_120),
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
    let mut others = Vec::new();
    for (dq, dr) in [(5, 0), (-5, 0), (0, 5), (0, -5), (5, -5), (-5, 5)] {
        if others.len() == 2 {
            break;
        }
        let pos = (center.0 + dq, center.1 + dr);
        let fits = g.map.tiles.get(&pos).is_some_and(|tile| {
            tile.owner_city.is_none() && !g.rules.is_water(tile) && g.rules.is_passable(tile)
        });
        if fits {
            others.push(g.place_city(0, pos, None));
        }
    }
    assert_eq!(others.len(), 2, "the fixture needs two more cities");
    g.players[0].techs.insert(crate::name!("rocketry"));
    for cid in g.player_city_ids(0) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = if cid == capital { 12 } else { 1 };
        city.queue.clear();
    }
    (g, capital, others)
}

fn armed() -> BasicAi {
    let mut ai = BasicAi::new();
    ai.enable_first_pad_in_the_forge();
    ai
}

fn placing(g: &Game) -> Vec<u32> {
    g.player_city_ids(0)
        .into_iter()
        .filter(|cid| BasicAi::city_can_place_a_pad(g, 0, *cid))
        .collect()
}

/// With no pad committed, a city may start the first one exactly when its
/// Production is at least the share of the best placing producer's; the gene
/// off allows every city.
#[test]
fn only_a_forge_may_start_the_first_pad() {
    let (g, _, _) = board();
    let cities = placing(&g);
    assert!(cities.len() >= 2, "fixture: several cities can place a pad");
    let best = cities
        .iter()
        .map(|cid| g.city_yields(*cid).production)
        .fold(f64::MIN, f64::max);
    let ai = armed();
    let stock = BasicAi::new();
    let mut vetoed = 0;
    for cid in &cities {
        let production = g.city_yields(*cid).production;
        let expected = production + 1e-9 >= FIRST_PAD_FORGE_SHARE * best;
        assert_eq!(ai.first_pad_city_allowed(&g, 0, *cid), expected);
        assert!(stock.first_pad_city_allowed(&g, 0, *cid), "off: every city");
        if !expected {
            vetoed += 1;
        }
    }
    assert!(
        vetoed > 0,
        "fixture: a weak city is vetoed (best {best:.1})"
    );
}

/// Once any city has a pad queued, the gene leaves every city alone.
#[test]
fn a_committed_pad_lifts_the_veto() {
    let (mut g, _, _) = board();
    let ai = armed();
    let cities = placing(&g);
    let forge = *cities
        .iter()
        .max_by(|a, b| {
            g.city_yields(**a)
                .production
                .total_cmp(&g.city_yields(**b).production)
        })
        .unwrap();
    let pad = crate::name!("spaceport");
    let pos = g
        .district_sites(forge, pad)
        .into_iter()
        .find(|pos| {
            g.can_produce(
                0,
                forge,
                &Item::District {
                    district: pad,
                    pos: *pos,
                },
            )
        })
        .expect("fixture: the forge has a pad site");
    g.cities
        .get_mut(&forge)
        .unwrap()
        .queue
        .push(Item::District { district: pad, pos });
    for cid in g.player_city_ids(0) {
        assert!(ai.first_pad_city_allowed(&g, 0, cid));
    }
}

/// Items other than a Spaceport are never touched.
#[test]
fn other_items_are_allowed() {
    let (g, _, others) = board();
    let ai = armed();
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    for cid in others {
        assert!(ai.first_pad_item_allowed(&g, 0, cid, &monument));
    }
}

/// `BasicAi::pick_item`'s first-pad step: the stock picker hands the first
/// pad to the weak city it is filling; under the gene that city is vetoed.
#[test]
fn the_basic_picker_leaves_the_first_pad_to_the_forge() {
    let (mut g, capital, others) = board();
    for cid in &others {
        g.cities.get_mut(cid).unwrap().pop = 6;
    }
    let ai = armed();
    let weak = *others
        .iter()
        .find(|cid| {
            BasicAi::city_can_place_a_pad(&g, 0, **cid) && !ai.first_pad_city_allowed(&g, 0, **cid)
        })
        .expect("fixture: a weak city that can place a pad and is vetoed");
    assert!(ai.first_pad_city_allowed(&g, 0, capital));
    let is_pad = |item: &Option<Item>| matches!(item, Some(Item::District { district, .. }) if g.district_family(*district) == "spaceport");
    let stock = BasicAi::new().pick_item(&g, 0, weak, 2, 2, 2, 2, 1, 10, 5, 5);
    assert!(
        is_pad(&stock),
        "fixture: the stock step gives the weak city the pad: {stock:?}"
    );
    let gene = ai.pick_item(&g, 0, weak, 2, 2, 2, 2, 1, 10, 5, 5);
    assert!(
        !is_pad(&gene),
        "the weak city leaves the first pad alone: {gene:?}"
    );
}
