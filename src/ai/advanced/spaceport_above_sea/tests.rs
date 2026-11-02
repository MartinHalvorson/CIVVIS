use super::*;

fn board() -> (Game, AdvancedAi, u32, Pos, Pos) {
    let mut g = Game::new_full(1, 24, 16, 91_989, 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let city = g.found_city_for(0, g.units[&settler].pos, None);
    g.remove_unit(settler);
    g.cities.get_mut(&city).unwrap().pop = 9;
    g.players[0].techs = g.rules.techs.keys().copied().collect();
    let sites: Vec<_> = g
        .producible_items(0, city)
        .into_iter()
        .filter_map(|item| match item {
            Item::District { district, pos } if district == "spaceport" => Some(pos),
            _ => None,
        })
        .collect();
    assert!(sites.len() >= 2, "fixture needs two legal Spaceport sites");
    for pos in &sites {
        g.map.tiles.get_mut(pos).unwrap().coastal_lowland = 1;
    }
    let (low, safe) = (sites[0], sites[1]);
    g.map.tiles.get_mut(&safe).unwrap().coastal_lowland = 0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_spaceport_above_the_sea();
    (g, ai, city, low, safe)
}

#[test]
fn spaceport_above_sea_is_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers("spaceport-above-the-sea", |ai| {
        ai.spaceport_above_the_sea
    });
}

#[test]
fn a_new_lowland_pad_uses_another_legal_site_in_the_same_city() {
    let (g, ai, cid, low, safe) = board();
    assert_eq!(
        ai.native_spaceport_above_sea_position(&g, 0, cid, low),
        Some(safe)
    );
    assert!(g.can_produce(
        0,
        cid,
        &Item::District {
            district: crate::name!("spaceport"),
            pos: safe,
        }
    ));
}

#[test]
fn off_or_an_already_safe_proposal_stays_unchanged() {
    let (g, mut ai, cid, low, safe) = board();
    assert_eq!(
        ai.native_spaceport_above_sea_position(&g, 0, cid, safe),
        None
    );
    ai.disable_spaceport_above_the_sea();
    assert_eq!(
        ai.native_spaceport_above_sea_position(&g, 0, cid, low),
        None
    );
}

#[test]
fn no_safe_legal_site_releases_the_override() {
    let (mut g, ai, cid, low, safe) = board();
    g.map.tiles.get_mut(&safe).unwrap().coastal_lowland = 3;
    assert_eq!(
        ai.native_spaceport_above_sea_position(&g, 0, cid, low),
        None
    );
}

#[test]
fn a_completed_active_flood_barrier_preserves_the_original_site() {
    let (mut g, ai, cid, low, safe) = board();
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("flood_barrier"));
    assert_eq!(
        ai.native_spaceport_above_sea_position(&g, 0, cid, low),
        None
    );
    g.cities
        .get_mut(&cid)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("flood_barrier"));
    assert_eq!(
        ai.native_spaceport_above_sea_position(&g, 0, cid, low),
        Some(safe)
    );
}

#[test]
fn existing_and_queued_pads_are_never_relocated() {
    let (mut g, ai, cid, low, _) = board();
    g.cities.get_mut(&cid).unwrap().queue.push(Item::District {
        district: crate::name!("spaceport"),
        pos: low,
    });
    assert_eq!(
        ai.native_spaceport_above_sea_position(&g, 0, cid, low),
        None
    );
    g.cities.get_mut(&cid).unwrap().queue.clear();
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), low);
    assert_eq!(
        ai.native_spaceport_above_sea_position(&g, 0, cid, low),
        None
    );
}

#[test]
fn a_foreign_city_or_illegal_original_is_not_rewritten() {
    let (mut g, ai, cid, low, _) = board();
    assert_eq!(
        ai.native_spaceport_above_sea_position(&g, 1, cid, low),
        None
    );
    let center = g.cities[&cid].pos;
    g.map.tiles.get_mut(&center).unwrap().coastal_lowland = 1;
    assert_eq!(
        ai.native_spaceport_above_sea_position(&g, 0, cid, center),
        None
    );
}
