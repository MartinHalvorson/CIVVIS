use super::*;
use crate::game::install_test_district;
use crate::world::DistrictFoundation;

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

fn board() -> (Game, u32, Item, Item) {
    let mut g = Game::new(2, 32, 24, 5_414, 250, 0);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for pid in 0..g.players.len() {
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    let cid = g.player_city_ids(0)[0];
    g.players[0].techs = g.rules.techs.keys().cloned().collect();
    g.cities.get_mut(&cid).unwrap().pop = 14;
    g.turn = 100;
    let industrial = install_test_district(&mut g, cid, "industrial_zone");
    g.map.tiles.get_mut(&industrial).unwrap().pillaged = true;
    let site = g.district_sites(cid, crate::name!("spaceport"))[0];
    let pad = Item::District {
        district: crate::name!("spaceport"),
        pos: site,
    };
    g.map.tiles.get_mut(&site).unwrap().district_foundation = Some(DistrictFoundation {
        district: crate::name!("spaceport"),
        cost: 900.0,
    });
    let repair = Item::Repair {
        repair: crate::name!("district"),
        pos: industrial,
    };
    assert!(g.can_produce(0, cid, &repair));
    g.cities.get_mut(&cid).unwrap().queue = vec![repair.clone()];
    assert_eq!(AdvancedAi::unfinished_launch_pad_site(&g, 0), Some(cid));
    (g, cid, pad, repair)
}

fn seat() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_launch_pad_holds_its_queue();
    ai
}

#[test]
fn the_repair_exception_is_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "launch-pad-spares-the-repair",
        |ai| ai.launch_pad_spares_the_repair,
    );
}

#[test]
fn the_original_hold_replaces_a_repair_but_the_new_bit_spares_it() {
    let (g, cid, pad, repair) = board();
    let mut old = g.clone();
    seat().claim_launch_pad_holds_its_queue(&mut old, 0, &plan());
    assert_eq!(old.cities[&cid].queue.first(), Some(&pad));
    let mut ai = seat();
    ai.enable_launch_pad_spares_the_repair();
    let mut on = g.clone();
    ai.claim_launch_pad_holds_its_queue(&mut on, 0, &plan());
    assert_eq!(on.cities[&cid].queue.first(), Some(&repair));
    ai.disable_launch_pad_spares_the_repair();
    ai.claim_launch_pad_holds_its_queue(&mut on, 0, &plan());
    assert_eq!(on.cities[&cid].queue.first(), Some(&pad));
}

#[test]
fn a_legal_building_repair_keeps_its_turn_too() {
    let (mut g, cid, _, repair) = board();
    let Item::Repair { pos, .. } = repair else {
        unreachable!()
    };
    g.map.tiles.get_mut(&pos).unwrap().pillaged = false;
    let c = g.cities.get_mut(&cid).unwrap();
    c.buildings.push(crate::name!("workshop"));
    c.pillaged_buildings.insert(crate::name!("workshop"));
    let repair = Item::Repair {
        repair: crate::name!("workshop"),
        pos,
    };
    assert!(g.can_produce(0, cid, &repair));
    g.cities.get_mut(&cid).unwrap().queue = vec![repair.clone()];
    let mut ai = seat();
    ai.enable_launch_pad_spares_the_repair();
    ai.claim_launch_pad_holds_its_queue(&mut g, 0, &plan());
    assert_eq!(g.cities[&cid].queue.first(), Some(&repair));
}

#[test]
fn a_repaired_site_no_longer_blocks_the_pad() {
    let (mut g, cid, pad, repair) = board();
    let Item::Repair { pos, .. } = repair else {
        unreachable!()
    };
    g.map.tiles.get_mut(&pos).unwrap().pillaged = false;
    let mut ai = seat();
    ai.enable_launch_pad_spares_the_repair();
    ai.claim_launch_pad_holds_its_queue(&mut g, 0, &plan());
    assert_eq!(g.cities[&cid].queue.first(), Some(&pad));
}

#[test]
fn a_foreign_site_does_not_hold_our_queue() {
    let (mut g, cid, pad, repair) = board();
    let Item::Repair { pos, .. } = repair else {
        unreachable!()
    };
    g.map.tiles.get_mut(&pos).unwrap().owner_city = None;
    let mut ai = seat();
    ai.enable_launch_pad_spares_the_repair();
    ai.claim_launch_pad_holds_its_queue(&mut g, 0, &plan());
    assert_eq!(g.cities[&cid].queue.first(), Some(&pad));
}

#[test]
fn an_ordinary_unit_is_still_displaced() {
    let (mut g, cid, pad, _) = board();
    g.cities.get_mut(&cid).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("builder"),
    }];
    let mut ai = seat();
    ai.enable_launch_pad_spares_the_repair();
    ai.claim_launch_pad_holds_its_queue(&mut g, 0, &plan());
    assert_eq!(g.cities[&cid].queue.first(), Some(&pad));
}

#[test]
fn the_new_bit_does_not_enable_the_original_hold() {
    let (mut g, cid, _, _) = board();
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    g.cities.get_mut(&cid).unwrap().queue = vec![builder.clone()];
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_launch_pad_spares_the_repair();
    ai.claim_launch_pad_holds_its_queue(&mut g, 0, &plan());
    assert_eq!(g.cities[&cid].queue.first(), Some(&builder));
}
