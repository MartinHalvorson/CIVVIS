use super::super::{GrandStrategy, StrategicPlan};
use super::*;
use crate::name::Name;

/// Our Settler on a three-hex islet beside our own city, so no plot of the
/// islet is a legal site, with two rings of Coast around it and plain open
/// land beyond. Sailing known, Shipbuilding not, the research slot empty,
/// every other unit removed. `water` false keeps the rings dry: the same
/// board with a walk to open land.
fn islet(water: bool) -> (Game, u32) {
    let mut g = Game::new_full(1, 24, 16, 936_331, 300, 0, false);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .expect("a starting Settler");
    for uid in g.player_unit_ids(0) {
        if uid != settler {
            g.remove_unit(uid);
        }
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    let here = g.units[&settler].pos;
    if water {
        for pos in g.wdisk(here, 3) {
            if g.wdist(here, pos) >= 2 {
                g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
            }
        }
    }
    let home = g
        .nbrs(here)
        .into_iter()
        .find(|pos| g.map.get(*pos).is_some())
        .expect("a neighbour for our city");
    g.place_city(0, home, None);
    g.players[0].techs.insert(crate::name!("sailing"));
    g.players[0].research = None;
    g.turn = 80;
    (g, settler)
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_stranded_settler_studies_shipbuilding();
    ai
}

fn plan(g: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 8,
        assessed_turn: g.turn,
        rush: false,
    }
}

/// The gene ships off in the stock controller; the toggles are twins.
#[test]
fn the_gene_is_off_by_default_and_the_toggles_are_twins() {
    assert!(!AdvancedAi::new().stranded_settler_studies_shipbuilding);
    assert!(!AdvancedAi::legacy().stranded_settler_studies_shipbuilding);
    let mut ai = AdvancedAi::new();
    ai.enable_stranded_settler_studies_shipbuilding();
    assert!(ai.stranded_settler_studies_shipbuilding);
    ai.disable_stranded_settler_studies_shipbuilding();
    assert!(!ai.stranded_settler_studies_shipbuilding);
}

#[test]
fn a_settler_on_a_shore_with_nowhere_to_walk_names_shipbuilding() {
    let (g, settler) = islet(true);
    let here = g.units[&settler].pos;
    assert!(
        !armed().base.valid_settle_site(&g, 0, here),
        "fixture: the islet is too close to our city to found on"
    );
    assert_eq!(
        armed().stranded_settler_sea_goal(&g, 0),
        Some("shipbuilding")
    );
}

#[test]
fn the_gene_off_leaves_the_slot_alone() {
    let (g, _) = islet(true);
    assert_eq!(AdvancedAi::new().stranded_settler_sea_goal(&g, 0), None);
}

#[test]
fn a_settler_that_can_walk_to_a_site_needs_no_ship() {
    let (g, _) = islet(false);
    assert_eq!(armed().stranded_settler_sea_goal(&g, 0), None);
}

#[test]
fn a_known_shipbuilding_ends_the_rule() {
    let (mut g, _) = islet(true);
    g.players[0].techs.insert(crate::name!("shipbuilding"));
    assert_eq!(armed().stranded_settler_sea_goal(&g, 0), None);
}

#[test]
fn no_settler_no_goal() {
    let (mut g, settler) = islet(true);
    g.remove_unit(settler);
    assert_eq!(armed().stranded_settler_sea_goal(&g, 0), None);
}

#[test]
fn a_settler_standing_on_a_legal_site_founds_instead() {
    let (mut g, settler) = islet(true);
    let home = g.player_city_ids(0)[0];
    g.cities.remove(&home);
    for tile in g.map.tiles.values_mut() {
        tile.owner_city = None;
    }
    let here = g.units[&settler].pos;
    assert!(armed().base.valid_settle_site(&g, 0, here));
    assert_eq!(armed().stranded_settler_sea_goal(&g, 0), None);
}

#[test]
fn the_research_slot_takes_shipbuilding_and_off_it_does_not() {
    let (mut g, _) = islet(true);
    assert!(g
        .available_techs(0)
        .contains(&Name::new(STRANDED_SETTLER_SEA_TECH)));
    let mut off = g.clone();
    let plan = plan(&g);
    armed().advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("shipbuilding"));
    AdvancedAi::new().advanced_research(&mut off, 0, &plan);
    assert_ne!(off.players[0].research.as_deref(), Some("shipbuilding"));
}
