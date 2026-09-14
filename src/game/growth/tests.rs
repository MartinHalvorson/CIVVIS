use super::*;
use crate::game::Action;

fn board() -> (Game, u32) {
    let mut g = Game::new_full(1, 24, 16, 914_371_001, 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    g.cities.get_mut(&cid).unwrap().pop = 4;
    (g, cid)
}

#[test]
fn growth_query_preserves_housing_amenity_loyalty_and_starvation_bands() {
    let (mut g, cid) = board();
    g.cities.get_mut(&cid).unwrap().loyalty = 100.0;
    assert_eq!(g.city_growth_surplus(0, cid, 18.0, 4.0, 0), 2.5);
    assert_eq!(g.city_growth_surplus(0, cid, 18.0, 5.0, 0), 5.0);
    assert_eq!(g.city_growth_surplus(0, cid, 18.0, 6.0, 0), 10.0);
    assert_eq!(g.city_growth_surplus(0, cid, 18.0, 0.0, 0), 0.0);
    assert_eq!(g.city_growth_surplus(0, cid, 18.0, 6.0, -5), 0.0);
    assert_eq!(g.city_growth_surplus(0, cid, 18.0, 6.0, 5), 12.0);
    g.cities.get_mut(&cid).unwrap().loyalty = 25.0;
    assert_eq!(g.city_growth_surplus(0, cid, 18.0, 6.0, 5), 0.0);
    assert_eq!(g.city_growth_surplus(0, cid, 7.0, 0.0, -5), -1.0);
}

#[test]
fn growth_query_agrees_with_food_actually_banked_by_city_processing() {
    for loyalty in [100.0, 60.0, 30.0, 20.0] {
        let (mut g, cid) = board();
        g.cities.get_mut(&cid).unwrap().loyalty = loyalty;
        g.cities.get_mut(&cid).unwrap().food = 5.0;
        g.players[0].pantheon = Some("fertility_rites".to_string());
        let food = g.city_yields(cid).food;
        let housing = g.city_housing(&g.cities[&cid]);
        let amenities = g.city_amenity_surplus(&g.cities[&cid]);
        let surplus = g.city_growth_surplus(0, cid, food, housing, amenities);
        let need = g.growth_cost(g.cities[&cid].pop);
        let bank = 5.0 + surplus;
        let expected = if bank >= need {
            bank - need
        } else {
            bank.max(0.0)
        };
        g.process_city(0, cid);
        assert_eq!(g.cities[&cid].food, expected, "loyalty {loyalty}");
    }
}
