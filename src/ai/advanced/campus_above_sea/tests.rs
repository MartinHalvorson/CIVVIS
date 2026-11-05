use super::*;

fn board() -> (Game, AdvancedAi, u32, Pos, Pos) {
    let mut g = Game::new_full(1, 24, 16, 91_989, 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let cid = g.found_city_for(0, g.units[&settler].pos, None);
    g.remove_unit(settler);
    g.cities.get_mut(&cid).unwrap().pop = 9;
    g.players[0].techs = g.rules.techs.keys().copied().collect();
    let sites: Vec<_> = g
        .producible_items(0, cid)
        .into_iter()
        .filter_map(|item| match item {
            Item::District { district, pos } if district == "campus" => Some(pos),
            _ => None,
        })
        .collect();
    assert!(sites.len() >= 2);
    for pos in &sites {
        g.map.tiles.get_mut(pos).unwrap().coastal_lowland = 1;
    }
    let (low, safe) = (sites[0], sites[1]);
    g.map.tiles.get_mut(&safe).unwrap().coastal_lowland = 0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_campus_above_the_sea();
    (g, ai, cid, low, safe)
}

#[test]
fn a_fresh_science_campus_uses_a_safe_legal_site_in_the_same_city() {
    let (g, ai, cid, low, safe) = board();
    assert_eq!(
        ai.native_campus_above_sea_position(&g, 0, cid, low),
        Some(safe)
    );
    assert!(g.can_produce(
        0,
        cid,
        &Item::District {
            district: crate::name!("campus"),
            pos: safe
        }
    ));
    assert_eq!(ai.native_campus_above_sea_position(&g, 1, cid, low), None);
}

#[test]
fn no_safe_site_or_an_already_safe_proposal_keeps_the_existing_choice() {
    let (mut g, ai, cid, low, safe) = board();
    assert_eq!(ai.native_campus_above_sea_position(&g, 0, cid, safe), None);
    g.map.tiles.get_mut(&safe).unwrap().coastal_lowland = 3;
    assert_eq!(ai.native_campus_above_sea_position(&g, 0, cid, low), None);
}

#[test]
fn an_active_barrier_keeps_the_original_site_but_a_pillaged_one_does_not() {
    let (mut g, ai, cid, low, safe) = board();
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("flood_barrier"));
    assert_eq!(ai.native_campus_above_sea_position(&g, 0, cid, low), None);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("flood_barrier"));
    assert_eq!(
        ai.native_campus_above_sea_position(&g, 0, cid, low),
        Some(safe)
    );
}

#[test]
fn an_existing_queue_or_paused_foundation_is_never_relocated() {
    let (mut g, ai, cid, low, _) = board();
    g.cities.get_mut(&cid).unwrap().queue.push(Item::District {
        district: crate::name!("campus"),
        pos: low,
    });
    assert_eq!(ai.native_campus_above_sea_position(&g, 0, cid, low), None);
    g.cities.get_mut(&cid).unwrap().queue.clear();
    g.map.tiles.get_mut(&low).unwrap().district_foundation =
        Some(crate::world::DistrictFoundation {
            district: crate::name!("campus"),
            cost: 56.0,
        });
    assert_eq!(ai.native_campus_above_sea_position(&g, 0, cid, low), None);
}

#[test]
fn the_gene_is_off_in_both_controllers_and_other_lanes_keep_their_sites() {
    super::super::test_support::opt_in_off_in_both_controllers("campus-above-the-sea", |ai| {
        ai.campus_above_the_sea
    });
    let (g, mut ai, cid, low, _) = board();
    ai.disable_campus_above_the_sea();
    assert_eq!(ai.native_campus_above_sea_position(&g, 0, cid, low), None);
    let mut domination = AdvancedAi::targeting(VictoryTarget::Domination);
    domination.enable_campus_above_the_sea();
    assert_eq!(
        domination.native_campus_above_sea_position(&g, 0, cid, low),
        None
    );
}
